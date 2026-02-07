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
//!
//! # Install the controller (similar to flux install)
//! msmctl install
//!
//! # Install to custom namespace
//! msmctl install --namespace my-namespace
//!
//! # Export manifests
//! msmctl install --export
//!
//! # Check installation (similar to flux check)
//! msmctl check
//!
//! # Check prerequisites only
//! msmctl check --pre
//! ```

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use kube::Client;

// Import from the library

mod check;
mod git_pulls;
mod install;
mod list;
mod reconcile;
mod status;
mod suspend;

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
    /// Install the Secret Manager Controller to the cluster
    /// Similar to `flux install`, this command installs CRDs, RBAC, and deployment manifests
    Install {
        /// Kubernetes namespace to install into (default: octopilot-system)
        #[arg(short, long)]
        namespace: Option<String>,

        /// Export manifests to stdout instead of applying them
        #[arg(long)]
        export: bool,

        /// Dry-run: show what would be installed without applying
        #[arg(long)]
        dry_run: bool,
    },
    /// Check the Secret Manager Controller installation
    /// Similar to `flux check`, this command verifies that the controller is properly installed and healthy
    Check {
        /// Kubernetes namespace to check (default: octopilot-system)
        #[arg(short, long)]
        namespace: Option<String>,

        /// Only run pre-installation checks (prerequisites)
        #[arg(long)]
        pre: bool,
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
        .unwrap_or_else(|_| panic!("Failed to install rustls crypto provider"));

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
            reconcile::reconcile_command(client, name, cli.namespace, force).await
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
            list::list_command(client, cli.namespace).await
        }
        Commands::Status {
            resource_type,
            name,
        } => {
            validate_resource_type(&resource_type)?;
            status::status_command(client, name, cli.namespace).await
        }
        Commands::Suspend {
            resource_type,
            name,
        } => {
            validate_resource_type(&resource_type)?;
            suspend::suspend_command(client, name, cli.namespace).await
        }
        Commands::Resume {
            resource_type,
            name,
        } => {
            validate_resource_type(&resource_type)?;
            suspend::resume_command(client, name, cli.namespace).await
        }
        Commands::SuspendGitPulls {
            resource_type,
            name,
        } => {
            validate_resource_type(&resource_type)?;
            git_pulls::suspend_git_pulls_command(client, name, cli.namespace).await
        }
        Commands::ResumeGitPulls {
            resource_type,
            name,
        } => {
            validate_resource_type(&resource_type)?;
            git_pulls::resume_git_pulls_command(client, name, cli.namespace).await
        }
        Commands::Install {
            namespace,
            export,
            dry_run,
        } => install::install_command(client, namespace, export, dry_run).await,
        Commands::Check { namespace, pre } => check::check_command(client, namespace, pre).await,
    }
}

/// Validate that the resource type is supported
fn validate_resource_type(resource_type: &ResourceType) -> Result<()> {
    match resource_type {
        ResourceType::SecretManagerConfig => Ok(()),
    }
}
