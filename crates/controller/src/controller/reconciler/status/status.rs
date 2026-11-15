//! # Status Updates
//!
//! Handles updating status with secrets synced count.

use crate::controller::reconciler::types::Reconciler;
use crate::controller::reconciler::validation::parse_kubernetes_duration;
use crate::crd::{
    Condition, ResourceSyncState, SecretManagerConfig, SecretManagerConfigStatus, SyncStatus,
};
use anyhow::Result;
use kube::api::PatchParams;
use tracing::debug;

/// Update status with secrets synced count and push state tracking
/// CRITICAL: Checks if status actually changed before updating to prevent unnecessary watch events
/// synced_secrets tracks which secrets have been successfully pushed and their update counts
/// synced_properties tracks which properties have been successfully pushed and their update counts
pub async fn update_status(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
    secrets_synced: i32,
    synced_secrets: &std::collections::HashMap<String, ResourceSyncState>,
    synced_properties: &std::collections::HashMap<String, ResourceSyncState>,
) -> Result<()> {
    // Always written: it carries last_reconcile_time and next_reconcile_time, which the
    // watch loop uses to schedule the next periodic run.

    let api: kube::Api<SecretManagerConfig> = kube::Api::namespaced(
        reconciler.client.clone(),
        config.metadata.namespace.as_deref().unwrap_or("default"),
    );

    let description = format!("Successfully synced {} secrets", secrets_synced);

    // Preserve existing decryption status fields if they exist
    let existing_status = config.status.as_ref();
    let status = SecretManagerConfigStatus {
        phase: Some("Ready".to_string()),
        description: Some(description.clone()),
        conditions: vec![Condition {
            r#type: "Ready".to_string(),
            status: "True".to_string(),
            last_transition_time: Some(chrono::Utc::now().to_rfc3339()),
            reason: Some("ReconciliationSucceeded".to_string()),
            message: Some(description),
        }],
        observed_generation: config.metadata.generation,
        last_reconcile_time: Some(chrono::Utc::now().to_rfc3339()),
        next_reconcile_time: parse_kubernetes_duration(&config.spec.reconcile_interval)
            .ok()
            .map(|duration| {
                chrono::Utc::now()
                    .checked_add_signed(
                        chrono::Duration::from_std(duration).unwrap_or(chrono::Duration::zero()),
                    )
                    .map(|dt| dt.to_rfc3339())
            })
            .flatten(),
        secrets_synced: Some(secrets_synced),
        sync: Some(SyncStatus {
            secrets: Some(synced_secrets.clone()),
            properties: Some(synced_properties.clone()),
        }),
        decryption_status: existing_status.and_then(|s| s.decryption_status.clone()),
        last_decryption_attempt: existing_status.and_then(|s| s.last_decryption_attempt.clone()),
        last_decryption_error: existing_status.and_then(|s| s.last_decryption_error.clone()),
        sops_key_available: existing_status.and_then(|s| s.sops_key_available),
        sops_key_secret_name: existing_status.and_then(|s| s.sops_key_secret_name.clone()),
        sops_key_namespace: existing_status.and_then(|s| s.sops_key_namespace.clone()),
        sops_key_last_checked: existing_status.and_then(|s| s.sops_key_last_checked.clone()),
    };

    let patch = serde_json::json!({
        "status": status
    });

    let resource_name = config.metadata.name.as_deref().unwrap_or("unknown");
    let resource_namespace = config.metadata.namespace.as_deref().unwrap_or("default");

    match api
        .patch_status(
            resource_name,
            &PatchParams::apply("secret-manager-controller"),
            &kube::api::Patch::Merge(patch),
        )
        .await
    {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(api_err)) if api_err.code == 404 => {
            // Resource was deleted during reconciliation - this is expected and not an error
            debug!(
                "SecretManagerConfig {}/{} was deleted during reconciliation, skipping status update",
                resource_namespace, resource_name
            );
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!(
            "Failed to update status for SecretManagerConfig {}/{}: {}",
            resource_namespace,
            resource_name,
            e
        )),
    }
}
