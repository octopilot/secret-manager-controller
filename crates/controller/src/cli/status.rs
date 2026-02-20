//! # Status Command
//!
//! Command to show detailed status of a SecretManagerConfig resource.

use anyhow::{Context, Result};
use controller::crd::SecretManagerConfig;
use kube::{Client, api::Api};

/// Show detailed status of a SecretManagerConfig resource
pub async fn status_command(client: Client, name: String, namespace: Option<String>) -> Result<()> {
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
    println!(
        "  Name: {}",
        config.metadata.name.as_deref().unwrap_or("<unknown>")
    );
    println!(
        "  Namespace: {}",
        config.metadata.namespace.as_deref().unwrap_or("<unknown>")
    );
    if let Some(uid) = &config.metadata.uid {
        println!("  UID: {}", uid);
    }

    // Spec info
    println!();
    println!("Spec:");
    println!("  Suspend: {}", config.spec.suspend);
    println!("  Suspend Git Pulls: {}", config.spec.suspend_git_pulls);
    println!("  Reconcile Interval: {}", config.spec.reconcile_interval);
    println!(
        "  Git Repository Pull Interval: {}",
        config.spec.git_repository_pull_interval
    );
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
        controller::crd::ProviderConfig::Gcp(gcp) => {
            println!("  Type: GCP");
            println!("  Project ID: {}", gcp.project_id);
        }
        controller::crd::ProviderConfig::Aws(aws) => {
            println!("  Type: AWS");
            println!("  Region: {}", aws.region);
        }
        controller::crd::ProviderConfig::Azure(azure) => {
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
