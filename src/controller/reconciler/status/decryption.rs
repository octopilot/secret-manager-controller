//! # Decryption Status Updates
//!
//! Handles updating SOPS decryption status.

use crate::controller::reconciler::types::Reconciler;
use crate::crd::{SecretManagerConfig, SecretManagerConfigStatus};
use anyhow::Result;
use kube::api::PatchParams;
use kube::Api;
use tracing::debug;

/// Update SOPS decryption status
/// Called when SOPS decryption succeeds or fails to track decryption state
pub async fn update_decryption_status(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
    status: &str, // "Success", "TransientFailure", "PermanentFailure", "NotApplicable"
    error_message: Option<&str>,
) -> Result<()> {
    let api: kube::Api<SecretManagerConfig> = kube::Api::namespaced(
        reconciler.client.clone(),
        config.metadata.namespace.as_deref().unwrap_or("default"),
    );

    // Get existing status to preserve other fields
    let existing_status = config.status.as_ref();
    let mut new_status = existing_status.cloned().unwrap_or_default();

    // Update decryption fields
    new_status.decryption_status = Some(status.to_string());
    new_status.last_decryption_attempt = Some(chrono::Utc::now().to_rfc3339());
    new_status.last_decryption_error = error_message.map(|s| s.to_string());

    // Preserve other fields
    if new_status.phase.is_none() {
        new_status.phase = existing_status.and_then(|s| s.phase.clone());
    }
    if new_status.description.is_none() {
        new_status.description = existing_status.and_then(|s| s.description.clone());
    }
    if new_status.conditions.is_empty() {
        new_status.conditions = existing_status
            .map(|s| s.conditions.clone())
            .unwrap_or_default();
    }
    if new_status.observed_generation.is_none() {
        new_status.observed_generation = config.metadata.generation;
    }
    if new_status.last_reconcile_time.is_none() {
        new_status.last_reconcile_time = Some(chrono::Utc::now().to_rfc3339());
    }
    if new_status.next_reconcile_time.is_none() {
        new_status.next_reconcile_time =
            existing_status.and_then(|s| s.next_reconcile_time.clone());
    }
    if new_status.secrets_synced.is_none() {
        new_status.secrets_synced = existing_status.and_then(|s| s.secrets_synced);
    }
    if new_status.sops_key_available.is_none() {
        new_status.sops_key_available = existing_status.and_then(|s| s.sops_key_available);
    }
    if new_status.sops_key_secret_name.is_none() {
        new_status.sops_key_secret_name =
            existing_status.and_then(|s| s.sops_key_secret_name.clone());
    }
    if new_status.sops_key_namespace.is_none() {
        new_status.sops_key_namespace = existing_status.and_then(|s| s.sops_key_namespace.clone());
    }
    if new_status.sops_key_last_checked.is_none() {
        new_status.sops_key_last_checked =
            existing_status.and_then(|s| s.sops_key_last_checked.clone());
    }

    let patch = serde_json::json!({
        "status": new_status
    });

    api.patch_status(
        config.metadata.name.as_deref().unwrap_or("unknown"),
        &PatchParams::apply("secret-manager-controller"),
        &kube::api::Patch::Merge(patch),
    )
    .await?;

    debug!(
        "Updated decryption status for SecretManagerConfig {}/{}: {}",
        config.metadata.namespace.as_deref().unwrap_or("default"),
        config.metadata.name.as_deref().unwrap_or("unknown"),
        status
    );

    Ok(())
}
