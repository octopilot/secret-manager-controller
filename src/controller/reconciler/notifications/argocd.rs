//! # ArgoCD Notification Support
//!
//! Adds annotations to ArgoCD Application resources to trigger notifications
//! when drift is detected.

use crate::controller::reconciler::types::Reconciler;
use crate::crd::{NotificationSubscription, SourceRef};
use anyhow::{Context, Result};
use kube::{
    api::{Api, ApiResource, Patch, PatchParams},
    core::{DynamicObject, GroupVersionKind},
};
use tracing::{debug, info};

/// Send ArgoCD notification by adding annotation to Application resource
/// This triggers ArgoCD notification-controller to send notifications
pub async fn send_argocd_notification(
    reconciler: &Reconciler,
    source_ref: &SourceRef,
    subscriptions: &[NotificationSubscription],
) -> Result<()> {
    // Only proceed if source is an ArgoCD Application
    if source_ref.kind != "Application" {
        debug!(
            "Skipping ArgoCD notification - source is not an Application: {}",
            source_ref.kind
        );
        return Ok(());
    }

    // Get Application API
    let gvk = GroupVersionKind {
        group: "argoproj.io".to_string(),
        version: "v1alpha1".to_string(),
        kind: "Application".to_string(),
    };

    let ar = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> =
        Api::namespaced_with(reconciler.client.clone(), &source_ref.namespace, &ar);

    // Get current Application
    let application = api.get(&source_ref.name).await.context(format!(
        "Failed to get ArgoCD Application: {}/{}",
        source_ref.namespace, source_ref.name
    ))?;

    // Build annotations map
    let mut annotations = serde_json::Map::new();

    // Add notification annotations
    // Format: notifications.argoproj.io/subscribe.<trigger>.<service>: <channel>
    for subscription in subscriptions {
        let annotation_key = format!(
            "notifications.argoproj.io/subscribe.{}.{}",
            subscription.trigger, subscription.service
        );
        annotations.insert(
            annotation_key,
            serde_json::Value::String(subscription.channel.clone()),
        );
    }

    // Merge with existing annotations
    let existing_annotations = application
        .data
        .get("metadata")
        .and_then(|m| m.get("annotations"))
        .and_then(|a| a.as_object())
        .cloned()
        .unwrap_or_default();

    for (key, value) in existing_annotations {
        // Only merge if not already set by us (don't overwrite our notifications)
        if !key.starts_with("notifications.argoproj.io/subscribe.drift-detected.") {
            annotations.insert(key, value);
        }
    }

    // Patch Application with annotations
    let patch = serde_json::json!({
        "metadata": {
            "annotations": annotations
        }
    });

    let patch_params = PatchParams::apply("secret-manager-controller").force();

    api.patch(&source_ref.name, &patch_params, &Patch::Merge(patch))
        .await
        .context(format!(
            "Failed to patch ArgoCD Application {}/{} with notification annotations",
            source_ref.namespace, source_ref.name
        ))?;

    info!(
        "Added notification annotations to ArgoCD Application {}/{}",
        source_ref.namespace, source_ref.name
    );

    Ok(())
}

/// Remove ArgoCD notification annotations when notifications are disabled
pub async fn remove_argocd_notifications(
    reconciler: &Reconciler,
    source_ref: &SourceRef,
) -> Result<()> {
    // Only proceed if source is an ArgoCD Application
    if source_ref.kind != "Application" {
        return Ok(());
    }

    // Get Application API
    let gvk = GroupVersionKind {
        group: "argoproj.io".to_string(),
        version: "v1alpha1".to_string(),
        kind: "Application".to_string(),
    };

    let ar = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> =
        Api::namespaced_with(reconciler.client.clone(), &source_ref.namespace, &ar);

    // Get current Application
    let application = api.get(&source_ref.name).await.context(format!(
        "Failed to get ArgoCD Application: {}/{}",
        source_ref.namespace, source_ref.name
    ))?;

    // Get existing annotations
    let existing_annotations = application
        .data
        .get("metadata")
        .and_then(|m| m.get("annotations"))
        .and_then(|a| a.as_object())
        .cloned()
        .unwrap_or_default();

    // Remove drift-detected notification annotations
    let mut annotations = serde_json::Map::new();
    for (key, value) in existing_annotations {
        if !key.starts_with("notifications.argoproj.io/subscribe.drift-detected.") {
            annotations.insert(key, value);
        }
    }

    // Patch Application to remove annotations
    let patch = serde_json::json!({
        "metadata": {
            "annotations": annotations
        }
    });

    let patch_params = PatchParams::apply("secret-manager-controller").force();

    api.patch(&source_ref.name, &patch_params, &Patch::Merge(patch))
        .await
        .context(format!(
            "Failed to remove notification annotations from ArgoCD Application {}/{}",
            source_ref.namespace, source_ref.name
        ))?;

    info!(
        "Removed notification annotations from ArgoCD Application {}/{}",
        source_ref.namespace, source_ref.name
    );

    Ok(())
}
