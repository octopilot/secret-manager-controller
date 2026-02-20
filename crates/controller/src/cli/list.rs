//! # List Command
//!
//! Command to list all SecretManagerConfig resources.

use anyhow::{Context, Result};
use controller::crd::SecretManagerConfig;
use kube::{Client, api::Api};

/// List all SecretManagerConfig resources
pub async fn list_command(client: Client, namespace: Option<String>) -> Result<()> {
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
                s.conditions
                    .iter()
                    .find(|c| c.r#type == "Ready")
                    .map(|c| if c.status == "True" { "True" } else { "False" })
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
