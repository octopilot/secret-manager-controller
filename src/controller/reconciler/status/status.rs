//! # Status Updates
//!
//! Handles updating status with secrets synced count.

use crate::controller::reconciler::types::Reconciler;
use crate::controller::reconciler::validation::parse_kubernetes_duration;
use crate::crd::{Condition, SecretManagerConfig, SecretManagerConfigStatus};
use anyhow::Result;
use kube::api::PatchParams;
use tracing::debug;

/// Update status with secrets synced count
/// CRITICAL: Checks if status actually changed before updating to prevent unnecessary watch events
pub async fn update_status(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
    secrets_synced: i32,
) -> Result<()> {
    // CRITICAL: Check if status actually changed before updating
    let current_secrets_synced = config
        .status
        .as_ref()
        .and_then(|s| s.secrets_synced)
        .unwrap_or(0);

    if current_secrets_synced == secrets_synced
        && config.status.as_ref().and_then(|s| s.phase.as_deref()) == Some("Ready")
    {
        debug!(
            "Skipping status update - secrets_synced and phase unchanged: secrets_synced={}",
            secrets_synced
        );
        return Ok(());
    }

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

    api.patch_status(
        config.metadata.name.as_deref().unwrap_or("unknown"),
        &PatchParams::apply("secret-manager-controller"),
        &kube::api::Patch::Merge(patch),
    )
    .await?;

    Ok(())
}
