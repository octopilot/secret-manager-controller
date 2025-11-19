//! # Status Phase Updates
//!
//! Handles updating status phase and description.

use crate::controller::reconciler::types::Reconciler;
use crate::controller::reconciler::validation::parse_kubernetes_duration;
use crate::crd::{Condition, SecretManagerConfig, SecretManagerConfigStatus};
use anyhow::Result;
use kube::api::PatchParams;
use kube::Api;
use tracing::debug;

/// Update status phase and description
/// CRITICAL: Checks if status actually changed before updating to prevent unnecessary watch events
pub async fn update_status_phase(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
    phase: &str,
    message: Option<&str>,
) -> Result<()> {
    // CRITICAL: Check if status actually changed before updating
    // This prevents unnecessary status updates that trigger watch events
    let current_phase = config.status.as_ref().and_then(|s| s.phase.as_deref());
    let current_description = config
        .status
        .as_ref()
        .and_then(|s| s.description.as_deref());

    // Only update if phase or description actually changed
    if current_phase == Some(phase) && current_description == message.as_deref() {
        debug!(
            "Skipping status update - phase and description unchanged: phase={:?}, description={:?}",
            phase, message
        );
        return Ok(());
    }

    let api: kube::Api<SecretManagerConfig> = kube::Api::namespaced(
        reconciler.client.clone(),
        config.metadata.namespace.as_deref().unwrap_or("default"),
    );

    let mut conditions = vec![];
    let ready_status = if phase == "Ready" { "True" } else { "False" };
    let ready_reason = if phase == "Ready" {
        "ReconciliationSucceeded"
    } else if phase == "Failed" {
        "ReconciliationFailed"
    } else {
        "ReconciliationInProgress"
    };

    conditions.push(Condition {
        r#type: "Ready".to_string(),
        status: ready_status.to_string(),
        last_transition_time: Some(chrono::Utc::now().to_rfc3339()),
        reason: Some(ready_reason.to_string()),
        message: message.map(|s| s.to_string()),
    });

    // Calculate next reconcile time based on reconcile interval
    let next_reconcile_time = parse_kubernetes_duration(&config.spec.reconcile_interval)
        .ok()
        .map(|duration| {
            chrono::Utc::now()
                .checked_add_signed(
                    chrono::Duration::from_std(duration).unwrap_or(chrono::Duration::zero()),
                )
                .map(|dt| dt.to_rfc3339())
        })
        .flatten();

    // Preserve existing decryption status fields if they exist
    let existing_status = config.status.as_ref();
    let status = SecretManagerConfigStatus {
        phase: Some(phase.to_string()),
        description: message.map(|s| s.to_string()),
        conditions,
        observed_generation: config.metadata.generation,
        last_reconcile_time: Some(chrono::Utc::now().to_rfc3339()),
        next_reconcile_time,
        secrets_synced: None,
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
