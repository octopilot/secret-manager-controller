//! # Annotation Management
//!
//! Handles annotation-based state management (manual triggers, parsing errors).

use crate::controller::reconciler::types::Reconciler;
use crate::crd::SecretManagerConfig;
use anyhow::{Context, Result};
use kube::api::PatchParams;
use tracing::debug;

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
