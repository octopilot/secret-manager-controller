//! # Artifact Path Resolution
//!
//! Handles resolving artifact paths from FluxCD GitRepository or ArgoCD Application sources.

use crate::controller::reconciler::artifact::{
    get_argocd_artifact_path, get_flux_artifact_path, get_flux_git_repository,
};
use crate::controller::reconciler::source::suspend_git_repository;
use crate::controller::reconciler::status::update_status_phase;
use crate::controller::reconciler::types::{Reconciler, ReconcilerError};
use crate::crd::SecretManagerConfig;
use crate::observability;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Result type for artifact path resolution
/// Can return a path, indicate waiting for resource, or an error
#[derive(Debug)]
pub enum ArtifactPathResult {
    /// Successfully resolved artifact path
    Path(PathBuf),
    /// Need to wait for resource (GitRepository not found or still reconciling)
    AwaitChange,
    /// Error occurred
    Error(ReconcilerError),
}

/// Resolve artifact path from source (FluxCD GitRepository or ArgoCD Application)
pub async fn resolve_artifact_path(
    config: &Arc<SecretManagerConfig>,
    ctx: &Arc<Reconciler>,
) -> Result<ArtifactPathResult, ReconcilerError> {
    info!(
        "🔍 Checking source: {} '{}' in namespace '{}'",
        config.spec.source_ref.kind, config.spec.source_ref.name, config.spec.source_ref.namespace
    );

    // Determine artifact path based on source type (GitRepository vs Application)
    // This path points to the cloned/checked-out repository directory containing secrets
    match config.spec.source_ref.kind.as_str() {
        "GitRepository" => {
            // FluxCD GitRepository: Extract artifact path from GitRepository status
            // The GitRepository controller clones the repo and exposes the path in status.artifact.path

            // Check if Git pulls should be suspended
            // If suspendGitPulls is true, we need to ensure the GitRepository is suspended
            // This allows reconciliation to continue with the last pulled commit
            if config.spec.suspend_git_pulls {
                info!(
                    "⏸️  Git pulls suspended - ensuring GitRepository {}/{} is suspended",
                    config.spec.source_ref.namespace, config.spec.source_ref.name
                );
                if let Err(e) = suspend_git_repository(ctx, &config.spec.source_ref, true).await {
                    warn!("Failed to suspend GitRepository: {}", e);
                    // Continue anyway - GitRepository might already be suspended
                }
            } else {
                // Ensure GitRepository is not suspended if suspendGitPulls is false
                if let Err(e) = suspend_git_repository(ctx, &config.spec.source_ref, false).await {
                    warn!("Failed to resume GitRepository: {}", e);
                    // Continue anyway - GitRepository might already be active
                }
            }

            // Update status to Cloning - indicates we're fetching the GitRepository
            if let Err(e) = update_status_phase(
                ctx,
                config,
                "Cloning",
                Some("Fetching GitRepository artifact"),
            )
            .await
            {
                warn!("Failed to update status to Cloning: {}", e);
            }

            // Fetch GitRepository resource from Kubernetes API
            // This gives us access to the cloned repository path
            info!(
                "📦 Fetching FluxCD GitRepository: {}/{}",
                config.spec.source_ref.namespace, config.spec.source_ref.name
            );

            let git_repo = match get_flux_git_repository(ctx, &config.spec.source_ref).await {
                Ok(repo) => {
                    info!(
                        "✅ Successfully retrieved GitRepository: {}/{}",
                        config.spec.source_ref.namespace, config.spec.source_ref.name
                    );
                    repo
                }
                Err(e) => {
                    // Check if this is a 404 (resource not found) - this is expected and we should wait
                    // The error is wrapped in anyhow::Error, so we need to check the root cause
                    let is_404 = e.chain().any(|err| {
                        if let Some(kube::Error::Api(api_err)) = err.downcast_ref::<kube::Error>() {
                            return api_err.code == 404;
                        }
                        false
                    });

                    if is_404 {
                        warn!(
                            "⏳ GitRepository {}/{} not found yet, waiting for watch event",
                            config.spec.source_ref.namespace, config.spec.source_ref.name
                        );
                        info!(
                            "👀 Waiting for GitRepository creation (trigger source: watch-event)",
                        );
                        // Update status to Pending (waiting for GitRepository)
                        let _ = update_status_phase(
                            ctx,
                            config,
                            "Pending",
                            Some("GitRepository not found, waiting for creation"),
                        )
                        .await;
                        // The caller requeues; the GitRepository watch also triggers a
                        // reconcile as soon as the GitRepository appears with an artifact.
                        return Ok(ArtifactPathResult::AwaitChange);
                    }

                    // For other errors, log and fail
                    error!(
                        "❌ Failed to get FluxCD GitRepository: {}/{} - {}",
                        config.spec.source_ref.namespace, config.spec.source_ref.name, e
                    );
                    observability::metrics::increment_reconciliation_errors();
                    // Update status to Failed
                    let _ = update_status_phase(
                        ctx,
                        config,
                        "Failed",
                        Some(&format!("Clone failed, repo unavailable: {e}")),
                    )
                    .await;
                    return Ok(ArtifactPathResult::Error(
                        ReconcilerError::ReconciliationFailed(e),
                    ));
                }
            };

            // Extract artifact path from GitRepository status
            // Downloads and extracts tar.gz artifact from FluxCD source-controller
            // Returns path to extracted directory
            match get_flux_artifact_path(ctx, &git_repo).await {
                Ok(path) => {
                    info!(
                        "Found FluxCD artifact path: {} for GitRepository: {}",
                        path.display(),
                        config.spec.source_ref.name
                    );
                    return Ok(ArtifactPathResult::Path(path));
                }
                Err(e) => {
                    // Check if GitRepository is ready - if not, wait for it to become ready
                    let status = git_repo.get("status");
                    let is_ready = status
                        .and_then(|s| s.get("conditions"))
                        .and_then(|c| c.as_array())
                        .and_then(|conditions| {
                            conditions.iter().find(|c| {
                                c.get("type")
                                    .and_then(|t| t.as_str())
                                    .map(|t| t == "Ready")
                                    .unwrap_or(false)
                            })
                        })
                        .and_then(|c| c.get("status"))
                        .and_then(|s| s.as_str())
                        .map(|s| s == "True")
                        .unwrap_or(false);

                    if !is_ready {
                        // GitRepository exists but is not ready yet (still cloning or failed)
                        // Check if it's a transient error (still reconciling) vs permanent (failed)
                        let is_reconciling = status
                            .and_then(|s| s.get("conditions"))
                            .and_then(|c| c.as_array())
                            .and_then(|conditions| {
                                conditions.iter().find(|c| {
                                    c.get("type")
                                        .and_then(|t| t.as_str())
                                        .map(|t| t == "Reconciling")
                                        .unwrap_or(false)
                                })
                            })
                            .and_then(|c| c.get("status"))
                            .and_then(|s| s.as_str())
                            .map(|s| s == "True")
                            .unwrap_or(false);

                        if is_reconciling {
                            // Still reconciling - wait for it to complete
                            warn!(
                                "⏳ GitRepository {}/{} is still reconciling, waiting for artifact",
                                config.spec.source_ref.namespace, config.spec.source_ref.name
                            );
                            info!(
                                "👀 Waiting for GitRepository to become ready (trigger source: watch-event)",
                            );
                            // Update status to Pending (waiting for GitRepository to be ready)
                            let _ = update_status_phase(
                                ctx,
                                config,
                                "Pending",
                                Some("GitRepository is reconciling, waiting for artifact"),
                            )
                            .await;
                            // Wait for watch event - GitRepository status updates will trigger reconciliation
                            return Ok(ArtifactPathResult::AwaitChange);
                        } else {
                            // Not reconciling and not ready - likely a permanent failure
                            let reason = status
                                .and_then(|s| s.get("conditions"))
                                .and_then(|c| c.as_array())
                                .and_then(|conditions| {
                                    conditions.iter().find(|c| {
                                        c.get("type")
                                            .and_then(|t| t.as_str())
                                            .map(|t| t == "Ready")
                                            .unwrap_or(false)
                                    })
                                })
                                .and_then(|c| c.get("reason"))
                                .and_then(|r| r.as_str())
                                .unwrap_or("Unknown");

                            error!(
                                "❌ GitRepository {}/{} is not ready (reason: {}), cannot proceed",
                                config.spec.source_ref.namespace,
                                config.spec.source_ref.name,
                                reason
                            );
                            observability::metrics::increment_reconciliation_errors();
                            // Update status to Failed
                            let _ = update_status_phase(
                                ctx,
                                config,
                                "Failed",
                                Some(&format!("GitRepository not ready: {}", reason)),
                            )
                            .await;
                            return Ok(ArtifactPathResult::Error(
                                ReconcilerError::ReconciliationFailed(anyhow::anyhow!(
                                    "GitRepository not ready: {}",
                                    reason
                                )),
                            ));
                        }
                    }

                    // GitRepository is ready but artifact path extraction failed
                    // This is unexpected - log error and fail
                    error!("Failed to get FluxCD artifact path: {}", e);
                    observability::metrics::increment_reconciliation_errors();
                    // Update status to Failed
                    let _ = update_status_phase(
                        ctx,
                        config,
                        "Failed",
                        Some(&format!("Failed to get artifact path: {e}")),
                    )
                    .await;
                    return Ok(ArtifactPathResult::Error(
                        ReconcilerError::ReconciliationFailed(e),
                    ));
                }
            }
        }
        "Application" => {
            // ArgoCD Application: Clone repository directly
            // Unlike FluxCD, ArgoCD doesn't expose artifact paths, so we clone ourselves
            // This supports both GitRepository and Helm sources
            match get_argocd_artifact_path(ctx, &config.spec.source_ref).await {
                Ok(path) => {
                    info!(
                        "Found ArgoCD artifact path: {} for Application: {}",
                        path.display(),
                        config.spec.source_ref.name
                    );
                    return Ok(ArtifactPathResult::Path(path));
                }
                Err(e) => {
                    error!("Failed to get ArgoCD artifact path: {}", e);
                    observability::metrics::increment_reconciliation_errors();
                    return Ok(ArtifactPathResult::Error(
                        ReconcilerError::ReconciliationFailed(e),
                    ));
                }
            }
        }
        _ => {
            error!("Unsupported source kind: {}", config.spec.source_ref.kind);
            observability::metrics::increment_reconciliation_errors();
            return Ok(ArtifactPathResult::Error(
                ReconcilerError::ReconciliationFailed(anyhow::anyhow!(
                    "Unsupported source kind: {}",
                    config.spec.source_ref.kind
                )),
            ));
        }
    }
    // All match branches return early, so this point is unreachable
    // The unreachable!() macro is removed to avoid compiler warnings
}
