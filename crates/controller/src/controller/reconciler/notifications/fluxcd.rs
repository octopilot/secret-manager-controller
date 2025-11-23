//! # FluxCD Notification Support
//!
//! Creates and manages FluxCD Alert CRDs that watch SecretManagerConfig resources
//! and send notifications via FluxCD Providers when drift is detected.

use crate::controller::reconciler::types::Reconciler;
use crate::crd::{ProviderRef, SecretManagerConfig};
use anyhow::{Context, Result};
use kube::{
    api::{Api, ApiResource, Patch, PatchParams, PostParams},
    core::{DynamicObject, GroupVersionKind},
};
use tracing::{debug, info, warn};

/// Ensure FluxCD Alert CRD exists for the SecretManagerConfig
/// Creates or updates the Alert to watch the SecretManagerConfig and send notifications
pub async fn ensure_fluxcd_alert(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
    provider_ref: &ProviderRef,
) -> Result<()> {
    let name = config.metadata.name.as_deref().unwrap_or("unknown");
    let namespace = config.metadata.namespace.as_deref().unwrap_or("default");
    let provider_namespace = provider_ref.namespace.as_deref().unwrap_or(namespace);

    // Get Alert API
    let gvk = GroupVersionKind {
        group: "notification.toolkit.fluxcd.io".to_string(),
        version: "v1beta2".to_string(),
        kind: "Alert".to_string(),
    };

    let ar = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::namespaced_with(reconciler.client.clone(), namespace, &ar);

    let alert_name = format!("secret-drift-alert-{}", name);

    // Check if Alert already exists
    let existing_alert = api.get(&alert_name).await.ok();

    // Build Alert spec
    let alert_spec = serde_json::json!({
        "providerRef": {
            "name": provider_ref.name,
            "namespace": provider_namespace
        },
        "eventSources": [
            {
                "kind": "SecretManagerConfig",
                "name": name,
                "namespace": namespace
            }
        ],
        "exclusionList": [
            ".*Ready.*",
            ".*ReconciliationSucceeded.*",
            ".*ReconciliationInProgress.*",
            ".*Started.*"
        ]
    });

    // Add owner reference if config has UID
    let mut alert_metadata = serde_json::json!({
        "name": alert_name,
        "namespace": namespace
    });

    if let Some(uid) = &config.metadata.uid {
        alert_metadata["ownerReferences"] = serde_json::json!([{
            "apiVersion": "secret-management.microscaler.io/v1beta1",
            "kind": "SecretManagerConfig",
            "name": name,
            "uid": uid,
            "controller": true,
            "blockOwnerDeletion": true
        }]);
    }

    let alert = serde_json::json!({
        "apiVersion": "notification.toolkit.fluxcd.io/v1beta2",
        "kind": "Alert",
        "metadata": alert_metadata,
        "spec": alert_spec
    });

    let obj: DynamicObject =
        serde_json::from_value(alert).context("Failed to deserialize FluxCD Alert")?;

    if existing_alert.is_some() {
        // Update existing Alert
        let patch_params = PatchParams::apply("secret-manager-controller").force();
        api.patch(
            &alert_name,
            &patch_params,
            &Patch::Merge(serde_json::json!({
                "spec": alert_spec
            })),
        )
        .await
        .context(format!(
            "Failed to update FluxCD Alert {}/{}",
            namespace, alert_name
        ))?;

        debug!(
            "Updated FluxCD Alert {}/{} for SecretManagerConfig {}/{}",
            namespace, alert_name, namespace, name
        );
    } else {
        // Create new Alert
        api.create(&PostParams::default(), &obj)
            .await
            .context(format!(
                "Failed to create FluxCD Alert {}/{}",
                namespace, alert_name
            ))?;

        info!(
            "Created FluxCD Alert {}/{} for SecretManagerConfig {}/{}",
            namespace, alert_name, namespace, name
        );
    }

    Ok(())
}

/// Remove FluxCD Alert CRD when notifications are disabled
pub async fn remove_fluxcd_alert(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
) -> Result<()> {
    let name = config.metadata.name.as_deref().unwrap_or("unknown");
    let namespace = config.metadata.namespace.as_deref().unwrap_or("default");

    let alert_name = format!("secret-drift-alert-{}", name);

    // Get Alert API
    let gvk = GroupVersionKind {
        group: "notification.toolkit.fluxcd.io".to_string(),
        version: "v1beta2".to_string(),
        kind: "Alert".to_string(),
    };

    let ar = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::namespaced_with(reconciler.client.clone(), namespace, &ar);

    // Try to delete Alert (ignore if not found)
    match api
        .delete(&alert_name, &kube::api::DeleteParams::default())
        .await
    {
        Ok(_) => {
            info!(
                "Deleted FluxCD Alert {}/{} for SecretManagerConfig {}/{}",
                namespace, alert_name, namespace, name
            );
        }
        Err(kube::Error::Api(kube::error::ErrorResponse { code: 404, .. })) => {
            // Alert doesn't exist - that's fine
            debug!(
                "FluxCD Alert {}/{} does not exist (already deleted)",
                namespace, alert_name
            );
        }
        Err(e) => {
            warn!(
                "Failed to delete FluxCD Alert {}/{}: {}",
                namespace, alert_name, e
            );
            // Don't fail - Alert might not exist or might be managed elsewhere
        }
    }

    Ok(())
}
