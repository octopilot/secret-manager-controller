//! # Source Management
//!
//! Handles GitRepository and ArgoCD Application source management.

use crate::controller::reconciler::types::Reconciler;
use crate::crd::SourceRef;
use anyhow::{Context, Result};
use tracing::{debug, info};

/// Log how source changes reach the controller.
///
/// Flux: the controller watches `GitRepository` objects and reconciles the
/// SecretManagerConfigs that reference one as soon as its artifact revision changes
/// (`runtime::watch_loop`). ArgoCD `Application` sources have no watch yet and are
/// picked up on `reconcileInterval`.
pub fn start_source_watch(
    _reconciler: std::sync::Arc<Reconciler>,
    _configs_api: kube::Api<crate::crd::SecretManagerConfig>,
) {
    info!(
        "Source changes: Flux GitRepository artifacts are watched; ArgoCD Applications are polled on reconcileInterval"
    );
}

/// Suspend or resume GitRepository pulls
/// Patches the FluxCD GitRepository resource to control Git pulls independently from reconciliation
/// When suspended, FluxCD stops fetching new commits but the last artifact remains available
pub async fn suspend_git_repository(
    reconciler: &Reconciler,
    source_ref: &SourceRef,
    suspend: bool,
) -> Result<()> {
    use kube::api::{ApiResource, Patch, PatchParams};
    use kube::core::DynamicObject;

    let ar = ApiResource::from_gvk(&kube::core::GroupVersionKind {
        group: "source.toolkit.fluxcd.io".to_string(),
        version: "v1".to_string(),
        kind: "GitRepository".to_string(),
    });

    let api: kube::Api<DynamicObject> =
        kube::Api::namespaced_with(reconciler.client.clone(), &source_ref.namespace, &ar);

    // Check current suspend status
    let git_repo = api.get(&source_ref.name).await.context(format!(
        "Failed to get GitRepository: {}/{}",
        source_ref.namespace, source_ref.name
    ))?;

    let current_suspend = git_repo
        .data
        .get("spec")
        .and_then(|s| s.get("suspend"))
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    // Only patch if status needs to change
    if current_suspend == suspend {
        debug!(
            "GitRepository {}/{} already {}",
            source_ref.namespace,
            source_ref.name,
            if suspend { "suspended" } else { "active" }
        );
        return Ok(());
    }

    // Patch GitRepository to set suspend status
    let patch = serde_json::json!({
        "spec": {
            "suspend": suspend
        }
    });

    let patch_params = PatchParams::apply("secret-manager-controller").force();

    api.patch(&source_ref.name, &patch_params, &Patch::Merge(patch))
        .await
        .context(format!(
            "Failed to {} GitRepository: {}/{}",
            if suspend { "suspend" } else { "resume" },
            source_ref.namespace,
            source_ref.name
        ))?;

    info!(
        "✅ GitRepository {}/{} {}",
        source_ref.namespace,
        source_ref.name,
        if suspend {
            "suspended (pulls paused, using last commit)"
        } else {
            "resumed (pulls enabled)"
        }
    );

    Ok(())
}
