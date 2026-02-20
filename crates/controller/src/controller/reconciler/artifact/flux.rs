//! # FluxCD Artifact Handling
//!
//! Handles FluxCD GitRepository artifacts.
//! Downloads and extracts tar.gz artifacts from FluxCD source-controller.

use crate::controller::reconciler::types::Reconciler;
use crate::controller::reconciler::utils::{SMC_BASE_PATH, sanitize_path_component};
use crate::crd::SourceRef;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::Instant;
use tracing::{Instrument, info, info_span, warn};

use super::download::{
    cleanup_old_revisions, download_artifact, extract_artifact, verify_checksum,
    verify_tar_gz_format,
};

/// Get FluxCD GitRepository resource
#[allow(
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    reason = "Markdown formatting is intentional and error docs are in comments"
)]
pub async fn get_flux_git_repository(
    reconciler: &Reconciler,
    source_ref: &SourceRef,
) -> Result<serde_json::Value> {
    // Use Kubernetes API to get GitRepository
    // GitRepository is a CRD from source.toolkit.fluxcd.io/v1beta2
    use kube::api::ApiResource;

    let span = info_span!(
        "gitrepository.get_artifact",
        gitrepository.name = source_ref.name,
        namespace = source_ref.namespace
    );
    let span_clone = span.clone();
    let start = Instant::now();

    async move {
        let ar = ApiResource::from_gvk(&kube::core::GroupVersionKind {
            group: "source.toolkit.fluxcd.io".to_string(),
            version: "v1beta2".to_string(),
            kind: "GitRepository".to_string(),
        });

        let api: kube::Api<kube::core::DynamicObject> =
            kube::Api::namespaced_with(reconciler.client.clone(), &source_ref.namespace, &ar);

        let git_repo = api.get(&source_ref.name).await.context(format!(
            "Failed to get FluxCD GitRepository: {}/{}",
            source_ref.namespace, source_ref.name
        ))?;

        span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
        span_clone.record("operation.success", true);
        Ok(serde_json::to_value(git_repo)?)
    }
    .instrument(span)
    .await
}

/// Get artifact path from FluxCD GitRepository status
/// Downloads and extracts the tar.gz artifact from FluxCD source-controller HTTP service
/// Returns the path to the extracted directory
#[allow(
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    reason = "Markdown formatting is intentional, error docs in comments"
)]
pub async fn get_flux_artifact_path(
    _reconciler: &Reconciler,
    git_repo: &serde_json::Value,
) -> Result<PathBuf> {
    // Extract artifact information from GitRepository status
    // FluxCD stores artifacts as tar.gz files accessible via HTTP from source-controller
    let status = git_repo
        .get("status")
        .and_then(|s| s.get("artifact"))
        .context("FluxCD GitRepository has no artifact in status")?;

    // Get artifact URL - this is the HTTP endpoint to download the tar.gz
    // FluxCD sometimes includes a dot before the path (e.g., cluster.local./path)
    // which causes HTTP requests to fail, so we normalize it
    let artifact_url_raw = status
        .get("url")
        .and_then(|u| u.as_str())
        .context("FluxCD GitRepository artifact has no URL")?;

    // Normalize URL: remove dots before path separators (e.g., cluster.local./path -> cluster.local/path)
    // This handles cases where Kubernetes DNS FQDNs include trailing dots before paths
    let artifact_url = artifact_url_raw
        .replace("./", "/")
        .trim_end_matches('.')
        .to_string();

    // Get revision for caching - use revision to determine if we need to re-download
    let revision = status
        .get("revision")
        .and_then(|r| r.as_str())
        .unwrap_or("unknown");

    // Extract branch name and short SHA from revision
    // FluxCD revision format: "main@sha1:7680da431ea59ae7d3f4fdbb903a0f4509da9078"
    // We need both branch and SHA to avoid conflicts when same SHA exists on different branches
    let (branch_name, short_sha) = if let Some(at_pos) = revision.find('@') {
        // Extract branch name (before @)
        let branch = &revision[..at_pos];
        let sanitized_branch = sanitize_path_component(branch);

        // Extract SHA (after @sha1: or @sha256:)
        let sha = if let Some(sha_start) = revision.find("sha1:") {
            &revision[sha_start + 5..]
        } else if let Some(sha_start) = revision.find("sha256:") {
            &revision[sha_start + 7..]
        } else {
            // No SHA found, use full revision after @
            &revision[at_pos + 1..]
        };

        let short_sha = if sha.len() >= 7 { &sha[..7] } else { sha };

        (sanitized_branch, short_sha.to_string())
    } else {
        // No @ separator found, treat entire revision as branch
        (sanitize_path_component(revision), "unknown".to_string())
    };

    // Create revision directory name: {branch}-sha-{short_sha}
    // Example: "main-sha-7680da4" or "old-branch-sha-7680da4"
    let revision_dir = format!("{}-sha-{}", branch_name, short_sha);

    // Get metadata for constructing cache path
    let metadata = git_repo
        .get("metadata")
        .context("FluxCD GitRepository has no metadata")?;

    let name = metadata
        .get("name")
        .and_then(|n| n.as_str())
        .context("FluxCD GitRepository has no name")?;

    let namespace = metadata
        .get("namespace")
        .and_then(|n| n.as_str())
        .context("FluxCD GitRepository has no namespace")?;

    // Create hierarchical cache directory path: /tmp/smc/flux-artifact/{namespace}/{name}/{branch}-sha-{short_sha}/
    // This structure:
    // 1. Avoids performance issues with many files in a single directory
    // 2. Allows cluster owners to mount a PVC at /tmp/smc for persistent storage
    // 3. Provides clear organization by namespace, name, branch, and SHA
    // 4. Uses branch name + short SHA (7 chars) to avoid conflicts when same SHA exists on different branches
    // 5. Cleanup uses mtime (filesystem modification time) to determine oldest revisions per branch
    let sanitized_namespace = sanitize_path_component(namespace);
    let sanitized_name = sanitize_path_component(name);

    let cache_path = PathBuf::from(SMC_BASE_PATH)
        .join("flux-artifact")
        .join(&sanitized_namespace)
        .join(&sanitized_name)
        .join(&revision_dir);

    // Check if artifact is already cached (directory exists and is not empty)
    if cache_path.exists() && cache_path.is_dir() {
        // Verify cache is valid by checking if it contains files
        if let Ok(mut entries) = std::fs::read_dir(&cache_path) {
            if entries.next().is_some() {
                info!(
                    "Using cached FluxCD artifact at {} (revision: {}, dir: {})",
                    cache_path.display(),
                    revision,
                    revision_dir
                );
                return Ok(cache_path);
            }
        }
    }

    info!(
        "Downloading FluxCD artifact from {} (revision: {}, dir: {})",
        artifact_url, revision, revision_dir
    );

    // Download tar.gz file to temporary location
    let temp_tar = cache_path.join("artifact.tar.gz");
    let (_temp_file, _downloaded_size) = download_artifact(&artifact_url, &temp_tar).await?;

    // Verify checksum if provided by FluxCD
    // FluxCD provides digest in artifact status (e.g., "sha256:...")
    if let Some(digest_str) = status.get("digest").and_then(|d| d.as_str()) {
        verify_checksum(&temp_tar, digest_str)?;
    }

    // Verify file is a valid tar.gz by checking magic bytes
    verify_tar_gz_format(&temp_tar)?;

    // Extract artifact
    extract_artifact(&temp_tar, &cache_path).await?;

    // Clean up temporary tar file after successful extraction
    if let Err(e) = tokio::fs::remove_file(&temp_tar).await {
        warn!(
            "Failed to remove temporary tar file {}: {}",
            temp_tar.display(),
            e
        );
        // Don't fail reconciliation if cleanup fails
    }

    // Clean up old revisions - keep only the 3 newest revisions per namespace/name
    // This prevents disk space from growing unbounded
    if let Some(parent_dir) = cache_path.parent() {
        if let Err(e) = cleanup_old_revisions(parent_dir).await {
            warn!("Failed to cleanup old revisions: {}", e);
            // Don't fail reconciliation if cleanup fails
        }
    } else {
        warn!(
            "Cannot cleanup old revisions: cache path {} has no parent directory",
            cache_path.display()
        );
    }

    info!(
        "Successfully downloaded and extracted FluxCD artifact to {} (revision: {}, dir: {})",
        cache_path.display(),
        revision,
        revision_dir
    );

    Ok(cache_path)
}
