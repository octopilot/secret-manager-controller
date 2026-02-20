//! # Reconcile Command
//!
//! Command to trigger reconciliation for SecretManagerConfig resources.

use anyhow::{Context, Result};
use controller::crd::SecretManagerConfig;
use kube::{Client, api::Api, api::Patch};
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

/// Trigger reconciliation by adding/updating an annotation
/// This is the Kubernetes-native approach - the controller watches for annotation changes
pub async fn reconcile_command(
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
        .context("System time is before UNIX epoch - this should never happen")?
        .as_secs();

    let patch = json!({
        "metadata": {
            "annotations": {
                "secret-management.octopilot.io/reconcile": timestamp.to_string()
            }
        }
    });

    // Use Patch::Merge for annotations - simpler and more reliable
    // Patch::Apply with force() requires full object structure which is complex
    let patch_params = kube::api::PatchParams::default();

    api.patch(&name, &patch_params, &Patch::Merge(patch))
        .await
        .with_context(|| {
            format!("Failed to trigger reconciliation for SecretManagerConfig '{ns}/{name}'")
        })?;

    println!("✅ Reconciliation triggered successfully");
    println!("   Resource: {ns}/{name}");
    println!("   Annotation: secret-management.octopilot.io/reconcile={timestamp}");
    println!("\nThe controller will reconcile this resource shortly.");

    Ok(())
}
