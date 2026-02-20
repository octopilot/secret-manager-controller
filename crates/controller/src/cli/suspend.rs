//! # Suspend/Resume Commands
//!
//! Commands to suspend and resume reconciliation for SecretManagerConfig resources.

use anyhow::{Context, Result};
use controller::crd::SecretManagerConfig;
use kube::{Client, api::Api, api::Patch, api::PatchParams};
use serde_json::json;

/// Suspend reconciliation for a SecretManagerConfig resource
pub async fn suspend_command(
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

    // Use Patch::Merge for spec changes - simpler and more reliable
    let patch_params = PatchParams::default();

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
pub async fn resume_command(client: Client, name: String, namespace: Option<String>) -> Result<()> {
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

    // Use Patch::Merge for spec changes - simpler and more reliable
    let patch_params = PatchParams::default();

    api.patch(&name, &patch_params, &Patch::Merge(patch))
        .await
        .with_context(|| format!("Failed to resume SecretManagerConfig '{ns}/{name}'"))?;

    println!("✅ Reconciliation resumed successfully");
    println!("   Resource: {ns}/{name}");
    println!("   Status: Active (reconciliation will proceed)");
    println!("\nThe controller will reconcile this resource shortly.");

    Ok(())
}
