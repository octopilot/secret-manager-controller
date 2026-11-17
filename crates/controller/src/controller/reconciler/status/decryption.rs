//! # Decryption Status Updates
//!
//! Handles updating SOPS decryption status.

use crate::controller::reconciler::types::Reconciler;
use crate::crd::SecretManagerConfig;
use anyhow::Result;
use kube::api::PatchParams;
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

    // Nothing to write when the outcome is the one already recorded: a write per
    // reconcile only to move a timestamp wakes the controller for nothing.
    let existing_status = config.status.as_ref();
    let unchanged = existing_status.is_some_and(|existing| {
        existing.decryption_status.as_deref() == Some(status)
            && existing.last_decryption_error.as_deref() == error_message
    });
    if unchanged {
        return Ok(());
    }

    // Only the decryption fields. `config` is the snapshot this reconcile started from:
    // writing the whole status back from it would undo what was written since, the next
    // reconcile time included, and with that keep the resource permanently due.
    let patch = decryption_status_patch(status, error_message, &chrono::Utc::now().to_rfc3339());

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
        Ok(_) => {}
        Err(kube::Error::Api(api_err)) if api_err.code == 404 => {
            // Resource was deleted during reconciliation - this is expected and not an error
            debug!(
                "SecretManagerConfig {}/{} was deleted during reconciliation, skipping decryption status update",
                resource_namespace, resource_name
            );
            return Ok(());
        }
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Failed to update decryption status for SecretManagerConfig {}/{}: {}",
                resource_namespace,
                resource_name,
                e
            ));
        }
    }

    debug!(
        "Updated decryption status for SecretManagerConfig {}/{}: {}",
        config.metadata.namespace.as_deref().unwrap_or("default"),
        config.metadata.name.as_deref().unwrap_or("unknown"),
        status
    );

    Ok(())
}

/// Merge patch that sets the decryption fields of the status and nothing else.
fn decryption_status_patch(
    status: &str,
    error_message: Option<&str>,
    attempted_at: &str,
) -> serde_json::Value {
    serde_json::json!({
        "status": {
            "decryptionStatus": status,
            "lastDecryptionAttempt": attempted_at,
            "lastDecryptionError": error_message,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::decryption_status_patch;

    /// The regression this guards: the patch used to carry the whole status of a stale
    /// snapshot, which put an already-passed nextReconcileTime back and made the
    /// resource reconcile in a loop.
    #[test]
    fn the_patch_touches_only_the_decryption_fields() {
        let patch = decryption_status_patch("Success", None, "2026-11-17T20:00:00Z");
        let status = patch["status"].as_object().expect("status object");
        let mut fields: Vec<&str> = status.keys().map(String::as_str).collect();
        fields.sort_unstable();
        assert_eq!(
            fields,
            vec![
                "decryptionStatus",
                "lastDecryptionAttempt",
                "lastDecryptionError"
            ]
        );
        assert!(
            status["lastDecryptionError"].is_null(),
            "a past error is cleared"
        );
    }

    #[test]
    fn a_failure_records_its_error() {
        let patch = decryption_status_patch("PermanentFailure", Some("wrong key"), "t");
        assert_eq!(patch["status"]["lastDecryptionError"], "wrong key");
    }
}
