//! # Source Management
//!
//! Handles GitRepository and ArgoCD Application source management.

use crate::controller::reconciler::types::Reconciler;
use crate::crd::SourceRef;
use anyhow::{Context, Result};
use tracing::{debug, info};

/// Start watching for GitRepository and ArgoCD Application changes
/// Note: The main controller watch already handles SecretManagerConfig changes.
/// During reconciliation, the controller fetches the latest GitRepository/Application,
/// so source changes are automatically picked up on the next reconciliation cycle.
/// This function is a placeholder for future enhancement to directly watch source resources.
pub fn start_source_watch(
    _reconciler: std::sync::Arc<Reconciler>,
    _configs_api: kube::Api<crate::crd::SecretManagerConfig>,
) {
    // Currently, the main controller watch handles SecretManagerConfig changes,
    // and during reconciliation, it fetches the latest GitRepository/Application.
    // This ensures source changes are picked up without restarting the controller.
    // Future enhancement: Directly watch GitRepository and Application resources
    // and trigger reconciliation of referencing SecretManagerConfig resources.
    info!(
        "Source watch: SecretManagerConfig resources are watched by main controller, source changes are picked up during reconciliation"
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
        version: "v1beta2".to_string(),
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
