//! # Status Management
//!
//! Updates SecretManagerConfig status with reconciliation results.

use crate::controller::reconciler::types::Reconciler;
use crate::controller::reconciler::validation::parse_kubernetes_duration;
use crate::{Condition, SecretManagerConfig, SecretManagerConfigStatus};
use anyhow::{Context, Result};
use kube::api::PatchParams;
use kube::Api;
use tracing::{debug, warn};

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

/// Calculate progressive backoff duration based on error count using Fibonacci sequence
/// Fibonacci backoff: 1m -> 1m -> 2m -> 3m -> 5m -> 8m -> 13m -> 21m -> 34m -> 55m -> 60m (1 hour max)
/// This prevents controller overload when parsing errors occur
/// Each resource maintains its own error count independently
pub fn calculate_progressive_backoff(error_count: u32) -> std::time::Duration {
    // Fibonacci sequence for backoff (in minutes): 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, then cap at 60
    let fib_sequence = [
        1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377, 610, 987, 1597, 2584, 4181, 6765,
    ]; // in minutes
    let index = std::cmp::min(error_count as usize, fib_sequence.len() - 1);
    let minutes = fib_sequence[index];
    let duration = std::time::Duration::from_secs(minutes * 60); // Convert minutes to seconds

    // Cap at 60 minutes (3600 seconds)
    std::cmp::min(duration, std::time::Duration::from_secs(3600))
}

/// Clear the manual trigger annotation from a SecretManagerConfig resource
/// This prevents repeated manual reconciliations after a successful run
pub async fn clear_manual_trigger_annotation(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
) -> Result<()> {
    let name = config.metadata.name.as_deref().unwrap_or("unknown");
    let namespace = config.metadata.namespace.as_deref().unwrap_or("default");

    let api: kube::Api<SecretManagerConfig> =
        kube::Api::namespaced(reconciler.client.clone(), namespace);

    // Create a JSON patch to remove the annotation
    let patch = serde_json::json!({
        "metadata": {
            "annotations": {
                "secret-management.microscaler.io/reconcile": serde_json::Value::Null
            }
        }
    });

    let patch_params = PatchParams::apply("secret-manager-controller").force();

    api.patch(name, &patch_params, &kube::api::Patch::Merge(patch))
        .await
        .context(format!(
            "Failed to clear manual trigger annotation for SecretManagerConfig {}/{}",
            namespace, name
        ))?;

    debug!(
        "Cleared manual trigger annotation for SecretManagerConfig {}/{}",
        namespace, name
    );
    Ok(())
}

/// Clear the parsing error count annotation from a SecretManagerConfig resource
/// This resets the backoff for duration parsing errors after a successful parse
pub async fn clear_parsing_error_count(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
) -> Result<()> {
    let name = config.metadata.name.as_deref().unwrap_or("unknown");
    let namespace = config.metadata.namespace.as_deref().unwrap_or("default");

    let api: kube::Api<SecretManagerConfig> =
        kube::Api::namespaced(reconciler.client.clone(), namespace);

    // Create a JSON patch to remove the annotation
    let patch = serde_json::json!({
        "metadata": {
            "annotations": {
                "secret-management.microscaler.io/parsing-error-count": serde_json::Value::Null
            }
        }
    });

    let patch_params = PatchParams::apply("secret-manager-controller").force();

    api.patch(name, &patch_params, &kube::api::Patch::Merge(patch))
        .await
        .context(format!(
            "Failed to clear parsing error count annotation for SecretManagerConfig {}/{}",
            namespace, name
        ))?;

    debug!(
        "Cleared parsing error count annotation for SecretManagerConfig {}/{}",
        namespace, name
    );
    Ok(())
}

/// Increment the parsing error count annotation for a SecretManagerConfig resource
/// This persists the error count across controller restarts for progressive backoff
pub async fn increment_parsing_error_count(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
    current_count: u32,
) -> Result<()> {
    let name = config.metadata.name.as_deref().unwrap_or("unknown");
    let namespace = config.metadata.namespace.as_deref().unwrap_or("default");

    let api: kube::Api<SecretManagerConfig> =
        kube::Api::namespaced(reconciler.client.clone(), namespace);

    let new_count = current_count + 1;
    let patch = serde_json::json!({
        "metadata": {
            "annotations": {
                "secret-management.microscaler.io/parsing-error-count": new_count.to_string()
            }
        }
    });

    let patch_params = PatchParams::apply("secret-manager-controller").force();

    api.patch(name, &patch_params, &kube::api::Patch::Merge(patch))
        .await
        .context(format!(
            "Failed to increment parsing error count annotation for SecretManagerConfig {}/{}",
            namespace, name
        ))?;

    debug!(
        "Incremented parsing error count for SecretManagerConfig {}/{} to {}",
        namespace, name, new_count
    );
    Ok(())
}

/// Get the current parsing error count from a SecretManagerConfig resource's annotations
/// Returns 0 if annotation is not found or cannot be parsed
pub fn get_parsing_error_count(config: &SecretManagerConfig) -> u32 {
    config
        .metadata
        .annotations
        .as_ref()
        .and_then(|ann| ann.get("secret-management.microscaler.io/parsing-error-count"))
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0)
}

/// Check SOPS key availability in a namespace
/// Returns (key_available, secret_name) tuple
pub async fn check_sops_key_availability(
    reconciler: &Reconciler,
    namespace: &str,
) -> Result<(bool, Option<String>)> {
    use k8s_openapi::api::core::v1::Secret;

    let secrets: Api<Secret> = Api::namespaced(reconciler.client.clone(), namespace);
    let secret_names = vec!["sops-private-key", "sops-gpg-key", "gpg-key"];

    for secret_name in secret_names {
        match secrets.get(secret_name).await {
            Ok(secret) => {
                if let Some(ref data_map) = secret.data {
                    if data_map
                        .get("private-key")
                        .or_else(|| data_map.get("key"))
                        .or_else(|| data_map.get("gpg-key"))
                        .is_some()
                    {
                        return Ok((true, Some(secret_name.to_string())));
                    }
                }
            }
            Err(kube::Error::Api(api_err)) if api_err.code == 404 => {
                continue;
            }
            Err(e) => {
                debug!(
                    "Failed to check secret '{}/{}': {}",
                    namespace, secret_name, e
                );
            }
        }
    }

    Ok((false, None))
}

/// Update SOPS key status in SecretManagerConfig resource
pub async fn update_sops_key_status(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
    key_available: bool,
    secret_name: Option<String>,
) -> Result<()> {
    let api: Api<SecretManagerConfig> = Api::namespaced(
        reconciler.client.clone(),
        config.metadata.namespace.as_deref().unwrap_or("default"),
    );

    // Get existing status to preserve other fields
    let existing_status = config.status.as_ref();
    let mut new_status = existing_status.cloned().unwrap_or_default();

    // Update SOPS key fields
    new_status.sops_key_available = Some(key_available);
    new_status.sops_key_secret_name = secret_name;
    new_status.sops_key_namespace = config.metadata.namespace.clone();
    new_status.sops_key_last_checked = Some(chrono::Utc::now().to_rfc3339());

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
        new_status.last_reconcile_time =
            existing_status.and_then(|s| s.last_reconcile_time.clone());
    }
    if new_status.next_reconcile_time.is_none() {
        new_status.next_reconcile_time =
            existing_status.and_then(|s| s.next_reconcile_time.clone());
    }
    if new_status.secrets_synced.is_none() {
        new_status.secrets_synced = existing_status.and_then(|s| s.secrets_synced);
    }
    if new_status.decryption_status.is_none() {
        new_status.decryption_status = existing_status.and_then(|s| s.decryption_status.clone());
    }
    if new_status.last_decryption_attempt.is_none() {
        new_status.last_decryption_attempt =
            existing_status.and_then(|s| s.last_decryption_attempt.clone());
    }
    if new_status.last_decryption_error.is_none() {
        new_status.last_decryption_error =
            existing_status.and_then(|s| s.last_decryption_error.clone());
    }

    let patch = serde_json::json!({
        "status": new_status
    });

    api.patch_status(
        config.metadata.name.as_deref().unwrap_or("unknown"),
        &PatchParams::apply("secret-manager-controller"),
        &kube::api::Patch::Merge(patch),
    )
    .await
    .context(format!(
        "Failed to patch SOPS key status for SecretManagerConfig {}/{}",
        config.metadata.namespace.as_deref().unwrap_or("default"),
        config.metadata.name.as_deref().unwrap_or("unknown")
    ))?;

    debug!(
        "Updated SOPS key status for SecretManagerConfig {}/{}: available={}",
        config.metadata.namespace.as_deref().unwrap_or("default"),
        config.metadata.name.as_deref().unwrap_or("unknown"),
        key_available
    );

    Ok(())
}

/// Update SOPS key status for all SecretManagerConfig resources in a namespace
/// Called by watch when SOPS key secret is created/deleted
pub async fn update_all_resources_in_namespace(
    reconciler: &Reconciler,
    namespace: &str,
    key_available: bool,
    secret_name: Option<String>,
) -> Result<()> {
    let api: Api<SecretManagerConfig> = Api::namespaced(reconciler.client.clone(), namespace);

    // List all SecretManagerConfig resources in this namespace
    let resources = api.list(&kube::api::ListParams::default()).await?;

    let mut updated_count = 0;
    let mut failed_count = 0;

    for resource in resources {
        match update_sops_key_status(reconciler, &resource, key_available, secret_name.clone())
            .await
        {
            Ok(_) => {
                updated_count += 1;
            }
            Err(e) => {
                failed_count += 1;
                debug!(
                    "Failed to update SOPS key status for {}/{}: {}",
                    namespace,
                    resource.metadata.name.as_deref().unwrap_or("unknown"),
                    e
                );
            }
        }
    }

    if updated_count > 0 {
        debug!(
            "Updated SOPS key status for {} resource(s) in namespace '{}' (available={})",
            updated_count, namespace, key_available
        );
    }
    if failed_count > 0 {
        warn!(
            "Failed to update SOPS key status for {} resource(s) in namespace '{}'",
            failed_count, namespace
        );
    }

    Ok(())
}
