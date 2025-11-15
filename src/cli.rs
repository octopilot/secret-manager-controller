//! # MSMCTL CLI
//!
//! Command-line interface for the Microscaler Secret Manager Controller.
//!
//! Similar to `fluxctl`, this CLI tool allows users to trigger reconciliations
//! and interact with the Secret Manager Controller running in Kubernetes.
//!
//! ## Usage
//!
//! ```bash
//! # Trigger reconciliation for a specific SecretManagerConfig
//! msmctl reconcile secretmanagerconfig my-secrets
//!
//! # List all SecretManagerConfig resources
//! msmctl list secretmanagerconfig
//!
//! # Show status of a SecretManagerConfig
//! msmctl status secretmanagerconfig my-secrets
//!
//! # Suspend reconciliation
//! msmctl suspend secretmanagerconfig my-secrets
//!
//! # Resume reconciliation
//! msmctl resume secretmanagerconfig my-secrets
//!
//! # Suspend Git pulls
//! msmctl suspend-git-pulls secretmanagerconfig my-secrets
//!
//! # Resume Git pulls
//! msmctl resume-git-pulls secretmanagerconfig my-secrets
//! ```

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use kube::{
    api::{Api, Patch, PatchParams},
    Client,
};
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

// Use types from the main library to ensure consistency with CRD
use secret_manager_controller::SecretManagerConfig;

/// Microscaler Secret Manager Controller CLI
#[derive(Parser)]
#[command(name = "msmctl")]
#[command(
    about = "Microscaler Secret Manager Controller CLI",
    long_about = None,
    after_help = "\
Available resource types:
  secretmanagerconfig (or 'smc') - SecretManagerConfig resource

Examples:
  msmctl list secretmanagerconfig
  msmctl reconcile smc my-secrets
  msmctl status secretmanagerconfig my-secrets --namespace default
"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Kubernetes namespace (defaults to current context namespace)
    #[arg(short, long, global = true)]
    namespace: Option<String>,

    /// Kubernetes context to use
    #[arg(short, long, global = true)]
    context: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Trigger reconciliation for a SecretManagerConfig resource
    Reconcile {
        /// Resource type
        /// Available types: secretmanagerconfig (or 'smc' for short)
        #[arg(
            value_enum,
            value_name = "RESOURCE_TYPE",
            help = "Resource type\nAvailable types:\n  secretmanagerconfig (or 'smc') - SecretManagerConfig resource"
        )]
        resource_type: ResourceType,

        /// Name of the SecretManagerConfig resource
        #[arg(value_name = "NAME")]
        name: String,

        /// Force reconciliation by deleting and waiting for GitOps to recreate
        /// Useful when resources get stuck. Deletes the resource, waits for Flux/GitOps
        /// to recreate it, then triggers reconciliation.
        #[arg(long)]
        force: bool,
    },
    /// List all SecretManagerConfig resources
    List {
        /// Resource type
        /// Available types: secretmanagerconfig (or 'smc' for short)
        #[arg(
            value_enum,
            value_name = "RESOURCE_TYPE",
            help = "Resource type\nAvailable types:\n  secretmanagerconfig (or 'smc') - SecretManagerConfig resource"
        )]
        resource_type: Option<ResourceType>,
    },
    /// Show status of a SecretManagerConfig resource
    Status {
        /// Resource type
        /// Available types: secretmanagerconfig (or 'smc' for short)
        #[arg(
            value_enum,
            value_name = "RESOURCE_TYPE",
            help = "Resource type\nAvailable types:\n  secretmanagerconfig (or 'smc') - SecretManagerConfig resource"
        )]
        resource_type: ResourceType,

        /// Name of the SecretManagerConfig resource
        #[arg(value_name = "NAME")]
        name: String,
    },
    /// Suspend reconciliation for a SecretManagerConfig resource
    Suspend {
        /// Resource type
        /// Available types: secretmanagerconfig (or 'smc' for short)
        #[arg(
            value_enum,
            value_name = "RESOURCE_TYPE",
            help = "Resource type\nAvailable types:\n  secretmanagerconfig (or 'smc') - SecretManagerConfig resource"
        )]
        resource_type: ResourceType,

        /// Name of the SecretManagerConfig resource
        #[arg(value_name = "NAME")]
        name: String,
    },
    /// Resume reconciliation for a SecretManagerConfig resource
    Resume {
        /// Resource type
        /// Available types: secretmanagerconfig (or 'smc' for short)
        #[arg(
            value_enum,
            value_name = "RESOURCE_TYPE",
            help = "Resource type\nAvailable types:\n  secretmanagerconfig (or 'smc') - SecretManagerConfig resource"
        )]
        resource_type: ResourceType,

        /// Name of the SecretManagerConfig resource
        #[arg(value_name = "NAME")]
        name: String,
    },
    /// Suspend Git pulls for a SecretManagerConfig resource
    /// Suspends GitRepository pulls but continues reconciliation with the last pulled commit
    #[command(name = "suspend-git-pulls")]
    SuspendGitPulls {
        /// Resource type
        /// Available types: secretmanagerconfig (or 'smc' for short)
        #[arg(
            value_enum,
            value_name = "RESOURCE_TYPE",
            help = "Resource type\nAvailable types:\n  secretmanagerconfig (or 'smc') - SecretManagerConfig resource"
        )]
        resource_type: ResourceType,

        /// Name of the SecretManagerConfig resource
        #[arg(value_name = "NAME")]
        name: String,
    },
    /// Resume Git pulls for a SecretManagerConfig resource
    #[command(name = "resume-git-pulls")]
    ResumeGitPulls {
        /// Resource type
        /// Available types: secretmanagerconfig (or 'smc' for short)
        #[arg(
            value_enum,
            value_name = "RESOURCE_TYPE",
            help = "Resource type\nAvailable types:\n  secretmanagerconfig (or 'smc') - SecretManagerConfig resource"
        )]
        resource_type: ResourceType,

        /// Name of the SecretManagerConfig resource
        #[arg(value_name = "NAME")]
        name: String,
    },
}

/// Resource types supported by msmctl
#[derive(Clone, ValueEnum)]
enum ResourceType {
    /// SecretManagerConfig resource (full name)
    /// Short form: 'smc'
    #[value(name = "secretmanagerconfig", alias = "smc")]
    SecretManagerConfig,
}



#[tokio::main]
async fn main() -> Result<()> {
    // Configure rustls crypto provider FIRST, before any other operations
    // Required for rustls 0.23+ when no default provider is set via features
    // This must be called synchronously before any async operations that use rustls
    // We use ring as the crypto provider (matches main controller)
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "msmctl=info".into()),
        )
        .init();

    let cli = Cli::parse();

    // Create Kubernetes client
    let client = Client::try_default()
        .await
        .context("Failed to create Kubernetes client. Ensure kubeconfig is configured.")?;

    match cli.command {
        Commands::Reconcile {
            resource_type,
            name,
            force,
        } => {
            validate_resource_type(&resource_type)?;
            reconcile_command(client, name, cli.namespace, force).await
        }
        Commands::List { resource_type } => {
            let rt = resource_type.ok_or_else(|| {
                anyhow::anyhow!(
                    "Resource type is required.\n\n\
                    Available resource types:\n\
                      secretmanagerconfig (or 'smc') - SecretManagerConfig resource\n\n\
                    Example: msmctl list secretmanagerconfig\n\
                    Example: msmctl list smc"
                )
            })?;
            validate_resource_type(&rt)?;
            list_command(client, cli.namespace).await
        }
        Commands::Status { resource_type, name } => {
            validate_resource_type(&resource_type)?;
            status_command(client, name, cli.namespace).await
        }
        Commands::Suspend { resource_type, name } => {
            validate_resource_type(&resource_type)?;
            suspend_command(client, name, cli.namespace).await
        }
        Commands::Resume { resource_type, name } => {
            validate_resource_type(&resource_type)?;
            resume_command(client, name, cli.namespace).await
        }
        Commands::SuspendGitPulls { resource_type, name } => {
            validate_resource_type(&resource_type)?;
            suspend_git_pulls_command(client, name, cli.namespace).await
        }
        Commands::ResumeGitPulls { resource_type, name } => {
            validate_resource_type(&resource_type)?;
            resume_git_pulls_command(client, name, cli.namespace).await
        }
    }
}

/// Validate that the resource type is supported
fn validate_resource_type(resource_type: &ResourceType) -> Result<()> {
    match resource_type {
        ResourceType::SecretManagerConfig => Ok(()),
    }
}

/// Trigger reconciliation by adding/updating an annotation
/// This is the Kubernetes-native approach - the controller watches for annotation changes
async fn reconcile_command(
    client: Client,
    name: String,
    namespace: Option<String>,
    force: bool,
) -> Result<()> {
    let ns = namespace.as_deref().unwrap_or("default");

    // Create API for SecretManagerConfig
    let api: Api<SecretManagerConfig> = Api::namespaced(client.clone(), ns);

    if force {
        println!("🔄 Force reconciliation mode enabled");
        println!("   Resource: {ns}/{name}");
        println!();

        // Step 1: Get the resource spec before deletion (for verification)
        let existing = api.get(&name).await;
        let resource_exists = existing.is_ok();

        if !resource_exists {
            return Err(anyhow::anyhow!(
                "Resource '{ns}/{name}' does not exist. Cannot force reconcile."
            ));
        }

        // Step 2: Delete the resource
        println!("🗑️  Deleting SecretManagerConfig '{ns}/{name}'...");
        match api.delete(&name, &kube::api::DeleteParams::default()).await {
            Ok(_) => {
                println!("   ✅ Resource deleted");
            }
            Err(kube::Error::Api(api_err)) if api_err.code == 404 => {
                println!("   ⚠️  Resource already deleted (may have been removed by GitOps)");
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to delete resource '{ns}/{name}': {e}"
                ));
            }
        }

        // Step 3: Wait for resource recreation
        println!();
        println!("⏳ Waiting for GitOps to recreate resource...");
        println!("   (This may take a few moments depending on GitOps sync interval)");

        let timeout = Duration::from_secs(300); // 5 minute timeout
        let start = SystemTime::now();
        let mut recreated = false;
        let mut last_log = SystemTime::now();

        // Poll for resource recreation
        // After deletion, wait a moment for deletion to complete
        sleep(Duration::from_secs(1)).await;

        while !recreated {
            // Check timeout
            if start.elapsed().unwrap_or(Duration::MAX) > timeout {
                return Err(anyhow::anyhow!(
                    "Timeout waiting for resource '{ns}/{name}' to be recreated by GitOps. \
                     Resource may not exist in Git, or GitOps sync interval is too long."
                ));
            }

            // Log progress every 10 seconds
            if last_log.elapsed().unwrap_or(Duration::MAX) > Duration::from_secs(10) {
                let elapsed = start.elapsed().unwrap_or(Duration::ZERO).as_secs();
                println!("   ⏳ Still waiting... ({elapsed}s elapsed)");
                last_log = SystemTime::now();
            }

            // Check if resource exists
            match api.get(&name).await {
                Ok(_) => {
                    recreated = true;
                    println!("   ✅ Resource recreated by GitOps");
                }
                Err(kube::Error::Api(api_err)) if api_err.code == 404 => {
                    // Resource doesn't exist yet, continue waiting
                    sleep(Duration::from_secs(2)).await;
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Error checking for resource '{ns}/{name}': {e}"
                    ));
                }
            }
        }

        // Step 4: Wait a moment for resource to be fully ready
        println!("   ⏳ Waiting for resource to be ready...");
        sleep(Duration::from_secs(2)).await;
    }

    println!("🔄 Triggering reconciliation for SecretManagerConfig '{ns}/{name}'...");

    // Get current resource to check if it exists
    let resource = api
        .get(&name)
        .await
        .with_context(|| format!("Failed to get SecretManagerConfig '{ns}/{name}'"))?;

    // Check if resource is suspended
    if resource.spec.suspend {
        println!("   ⚠️  Warning: Resource is suspended. Reconciliation will be skipped.");
        println!("   Use 'msmctl resume secretmanagerconfig {name}' to resume reconciliation.");
    }

    // Add or update the reconciliation annotation
    // The controller watches for this annotation and triggers reconciliation
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let patch = json!({
        "metadata": {
            "annotations": {
                "secret-management.microscaler.io/reconcile": timestamp.to_string()
            }
        }
    });

    let patch_params = PatchParams::apply("msmctl").force();

    api.patch(&name, &patch_params, &Patch::Merge(patch))
        .await
        .with_context(|| format!("Failed to trigger reconciliation for SecretManagerConfig '{ns}/{name}'"))?;

    println!("✅ Reconciliation triggered successfully");
    println!("   Resource: {ns}/{name}");
    println!("   Annotation: secret-management.microscaler.io/reconcile={timestamp}");
    println!("\nThe controller will reconcile this resource shortly.");

    Ok(())
}

/// List all SecretManagerConfig resources
async fn list_command(client: Client, namespace: Option<String>) -> Result<()> {
    let api: Api<SecretManagerConfig> = if let Some(ns) = &namespace {
        println!("Listing SecretManagerConfig resources in namespace '{ns}'...");
        Api::namespaced(client, ns)
    } else {
        println!("Listing SecretManagerConfig resources in all namespaces...");
        Api::all(client)
    };

    let configs = api
        .list(&kube::api::ListParams::default())
        .await
        .context("Failed to list SecretManagerConfig resources")?;

    if configs.items.is_empty() {
        println!("No SecretManagerConfig resources found.");
        return Ok(());
    }

    println!(
        "\n{:<30} {:<20} {:<12} {:<15} {:<15}",
        "NAME", "NAMESPACE", "SUSPEND", "READY", "SECRETS SYNCED"
    );
    println!("{}", "-".repeat(92));

    for config in configs.items {
        let name = config.metadata.name.as_deref().unwrap_or("<unknown>");
        let ns = config.metadata.namespace.as_deref().unwrap_or("<unknown>");

        // Get suspend status
        let suspend = if config.spec.suspend { "Yes" } else { "No" };

        // Get status
        let ready = config
            .status
            .as_ref()
            .and_then(|s| {
                s.conditions.iter().find(|c| c.r#type == "Ready").map(|c| {
                    if c.status == "True" {
                        "True"
                    } else {
                        "False"
                    }
                })
            })
            .unwrap_or("Unknown");

        let secrets_synced = config
            .status
            .as_ref()
            .and_then(|s| s.secrets_synced)
            .map(|n| n.to_string())
            .unwrap_or_else(|| "-".to_string());

        println!("{name:<30} {ns:<20} {suspend:<12} {ready:<15} {secrets_synced:<15}");
    }

    Ok(())
}

/// Show detailed status of a SecretManagerConfig resource
async fn status_command(client: Client, name: String, namespace: Option<String>) -> Result<()> {
    let ns = namespace.as_deref().unwrap_or("default");

    println!("📊 Status for SecretManagerConfig '{ns}/{name}'");
    println!();

    let api: Api<SecretManagerConfig> = Api::namespaced(client, ns);

    let config = api
        .get(&name)
        .await
        .with_context(|| format!("Failed to get SecretManagerConfig '{ns}/{name}'"))?;

    // Basic info
    println!("Resource Information:");
    println!("  Name: {}", config.metadata.name.as_deref().unwrap_or("<unknown>"));
    println!("  Namespace: {}", config.metadata.namespace.as_deref().unwrap_or("<unknown>"));
    if let Some(uid) = &config.metadata.uid {
        println!("  UID: {}", uid);
    }

    // Spec info
    println!();
    println!("Spec:");
    println!("  Suspend: {}", config.spec.suspend);
    println!("  Suspend Git Pulls: {}", config.spec.suspend_git_pulls);
    println!("  Reconcile Interval: {}", config.spec.reconcile_interval);
    println!("  Git Repository Pull Interval: {}", config.spec.git_repository_pull_interval);
    println!("  Environment: {}", config.spec.secrets.environment);
    if let Some(prefix) = &config.spec.secrets.prefix {
        println!("  Prefix: {}", prefix);
    }
    if let Some(base_path) = &config.spec.secrets.base_path {
        println!("  Base Path: {}", base_path);
    }

    // Provider info
    println!();
    println!("Provider:");
    match &config.spec.provider {
        secret_manager_controller::ProviderConfig::Gcp(gcp) => {
            println!("  Type: GCP");
            println!("  Project ID: {}", gcp.project_id);
        }
        secret_manager_controller::ProviderConfig::Aws(aws) => {
            println!("  Type: AWS");
            println!("  Region: {}", aws.region);
        }
        secret_manager_controller::ProviderConfig::Azure(azure) => {
            println!("  Type: Azure");
            println!("  Vault Name: {}", azure.vault_name);
        }
    }

    // Source ref
    println!();
    println!("Source:");
    println!("  Kind: {}", config.spec.source_ref.kind);
    println!("  Name: {}", config.spec.source_ref.name);
    println!("  Namespace: {}", config.spec.source_ref.namespace);

    // Status info
    if let Some(status) = &config.status {
        println!();
        println!("Status:");
        if let Some(phase) = &status.phase {
            println!("  Phase: {}", phase);
        }
        if let Some(description) = &status.description {
            println!("  Description: {}", description);
        }
        if let Some(secrets_synced) = status.secrets_synced {
            println!("  Secrets Synced: {}", secrets_synced);
        }
        if let Some(observed_generation) = status.observed_generation {
            println!("  Observed Generation: {}", observed_generation);
        }
        if let Some(last_reconcile_time) = &status.last_reconcile_time {
            println!("  Last Reconcile Time: {}", last_reconcile_time);
        }

        // Conditions
        if !status.conditions.is_empty() {
            println!();
            println!("Conditions:");
            for condition in &status.conditions {
                println!("  {}: {}", condition.r#type, condition.status);
                if let Some(reason) = &condition.reason {
                    println!("    Reason: {}", reason);
                }
                if let Some(message) = &condition.message {
                    println!("    Message: {}", message);
                }
                if let Some(last_transition_time) = &condition.last_transition_time {
                    println!("    Last Transition: {}", last_transition_time);
                }
            }
        }
    } else {
        println!();
        println!("Status: No status available (resource may not have been reconciled yet)");
    }

    Ok(())
}

/// Suspend reconciliation for a SecretManagerConfig resource
async fn suspend_command(
    client: Client,
    name: String,
    namespace: Option<String>,
) -> Result<()> {
    let ns = namespace.as_deref().unwrap_or("default");

    println!("⏸️  Suspending reconciliation for SecretManagerConfig '{ns}/{name}'...");

    let api: Api<SecretManagerConfig> = Api::namespaced(client, ns);

    // Check if resource exists
    let resource = api
        .get(&name)
        .await
        .with_context(|| format!("Failed to get SecretManagerConfig '{ns}/{name}'"))?;

    // Check if already suspended
    if resource.spec.suspend {
        println!("   ℹ️  Resource is already suspended");
        return Ok(());
    }

    // Patch the resource to set suspend: true
    let patch = json!({
        "spec": {
            "suspend": true
        }
    });

    let patch_params = PatchParams::apply("msmctl").force();

    api.patch(&name, &patch_params, &Patch::Merge(patch))
        .await
        .with_context(|| format!("Failed to suspend SecretManagerConfig '{ns}/{name}'"))?;

    println!("✅ Reconciliation suspended successfully");
    println!("   Resource: {ns}/{name}");
    println!("   Status: Suspended (reconciliation paused)");
    println!("\nTo resume reconciliation, run:");
    println!("   msmctl resume secretmanagerconfig {name} --namespace {ns}");

    Ok(())
}

/// Resume reconciliation for a SecretManagerConfig resource
async fn resume_command(
    client: Client,
    name: String,
    namespace: Option<String>,
) -> Result<()> {
    let ns = namespace.as_deref().unwrap_or("default");

    println!("▶️  Resuming reconciliation for SecretManagerConfig '{ns}/{name}'...");

    let api: Api<SecretManagerConfig> = Api::namespaced(client, ns);

    // Check if resource exists
    let resource = api
        .get(&name)
        .await
        .with_context(|| format!("Failed to get SecretManagerConfig '{ns}/{name}'"))?;

    // Check if already resumed
    if !resource.spec.suspend {
        println!("   ℹ️  Resource is already active (not suspended)");
        return Ok(());
    }

    // Patch the resource to set suspend: false
    let patch = json!({
        "spec": {
            "suspend": false
        }
    });

    let patch_params = PatchParams::apply("msmctl").force();

    api.patch(&name, &patch_params, &Patch::Merge(patch))
        .await
        .with_context(|| format!("Failed to resume SecretManagerConfig '{ns}/{name}'"))?;

    println!("✅ Reconciliation resumed successfully");
    println!("   Resource: {ns}/{name}");
    println!("   Status: Active (reconciliation will proceed)");
    println!("\nThe controller will reconcile this resource shortly.");

    Ok(())
}

/// Suspend Git pulls for a SecretManagerConfig resource
/// Sets suspendGitPulls: true in the spec, which suspends GitRepository pulls but continues reconciliation
async fn suspend_git_pulls_command(
    client: Client,
    name: String,
    namespace: Option<String>,
) -> Result<()> {
    let ns = namespace.as_deref().unwrap_or("default");

    println!("⏸️  Suspending Git pulls for SecretManagerConfig '{ns}/{name}'...");

    let api: Api<SecretManagerConfig> = Api::namespaced(client, ns);

    // Check if resource exists
    let resource = api
        .get(&name)
        .await
        .with_context(|| format!("Failed to get SecretManagerConfig '{ns}/{name}'"))?;

    // Check if already suspended
    if resource.spec.suspend_git_pulls {
        println!("   ℹ️  Git pulls are already suspended");
        return Ok(());
    }

    // Patch the resource to set suspendGitPulls: true
    let patch = json!({
        "spec": {
            "suspendGitPulls": true
        }
    });

    let patch_params = PatchParams::apply("msmctl").force();

    api.patch(&name, &patch_params, &Patch::Merge(patch))
        .await
        .with_context(|| format!("Failed to suspend Git pulls for SecretManagerConfig '{ns}/{name}'"))?;

    println!("✅ Git pulls suspended successfully");
    println!("   Resource: {ns}/{name}");
    println!("   Status: Git pulls paused (reconciliation continues with last commit)");
    println!("\nTo resume Git pulls, run:");
    println!("   msmctl resume-git-pulls secretmanagerconfig {name} --namespace {ns}");

    Ok(())
}

/// Resume Git pulls for a SecretManagerConfig resource
/// Sets suspendGitPulls: false in the spec, which resumes GitRepository pulls
async fn resume_git_pulls_command(
    client: Client,
    name: String,
    namespace: Option<String>,
) -> Result<()> {
    let ns = namespace.as_deref().unwrap_or("default");

    println!("▶️  Resuming Git pulls for SecretManagerConfig '{ns}/{name}'...");

    let api: Api<SecretManagerConfig> = Api::namespaced(client, ns);

    // Check if resource exists
    let resource = api
        .get(&name)
        .await
        .with_context(|| format!("Failed to get SecretManagerConfig '{ns}/{name}'"))?;

    // Check if already resumed
    if !resource.spec.suspend_git_pulls {
        println!("   ℹ️  Git pulls are already active");
        return Ok(());
    }

    // Patch the resource to set suspendGitPulls: false
    let patch = json!({
        "spec": {
            "suspendGitPulls": false
        }
    });

    let patch_params = PatchParams::apply("msmctl").force();

    api.patch(&name, &patch_params, &Patch::Merge(patch))
        .await
        .with_context(|| format!("Failed to resume Git pulls for SecretManagerConfig '{ns}/{name}'"))?;

    println!("✅ Git pulls resumed successfully");
    println!("   Resource: {ns}/{name}");
    println!("   Status: Git pulls enabled (will fetch new commits)");
    println!("\nThe controller will resume pulling from Git shortly.");

    Ok(())
}
