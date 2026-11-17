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
use tracing::{Instrument, debug, info, info_span, warn};

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
    // GitRepository is a CRD from source.toolkit.fluxcd.io/v1 (v1beta2 was removed in Flux 2.8)
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
            version: "v1".to_string(),
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

    // Every SecretManagerConfig that points at this source shares this directory, and a
    // new revision triggers them all at once. One of them fetches the artifact while the
    // others wait, then find it cached.
    let lock = artifact_lock(&cache_path);
    let _fetching = lock.lock().await;

    if is_complete_cache(&cache_path) {
        debug!(
            "Using cached FluxCD artifact at {} (revision: {}, dir: {})",
            cache_path.display(),
            revision,
            revision_dir
        );
        return Ok(cache_path);
    }

    info!(
        "Downloading FluxCD artifact from {} (revision: {}, dir: {})",
        artifact_url, revision, revision_dir
    );

    // Download and unpack beside the cache directory and move the result into place in
    // one step, so the cache directory only ever exists complete.
    let staging_dir = cache_path.with_file_name(format!(".{revision_dir}.staging"));
    let temp_tar = cache_path.with_file_name(format!(".{revision_dir}.tar.gz"));
    let _ = tokio::fs::remove_dir_all(&staging_dir).await;
    let _ = tokio::fs::remove_file(&temp_tar).await;

    let fetched = async {
        download_artifact(&artifact_url, &temp_tar).await?;

        // Verify checksum if provided by FluxCD
        // FluxCD provides digest in artifact status (e.g., "sha256:...")
        if let Some(digest_str) = status.get("digest").and_then(|d| d.as_str()) {
            verify_checksum(&temp_tar, digest_str)?;
        }

        // Verify file is a valid tar.gz by checking magic bytes
        verify_tar_gz_format(&temp_tar)?;

        extract_artifact(&temp_tar, &staging_dir).await?;

        // A directory left by an interrupted fetch of an earlier version is replaced.
        let _ = tokio::fs::remove_dir_all(&cache_path).await;
        tokio::fs::rename(&staging_dir, &cache_path)
            .await
            .context(format!(
                "Failed to move extracted artifact into {}",
                cache_path.display()
            ))
    }
    .await;

    let _ = tokio::fs::remove_file(&temp_tar).await;
    if let Err(e) = fetched {
        let _ = tokio::fs::remove_dir_all(&staging_dir).await;
        return Err(e);
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

/// One lock per cache directory, so concurrent reconciles of configs sharing a source
/// do not download and unpack the same artifact on top of each other.
fn artifact_lock(cache_path: &std::path::Path) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};

    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    let mut locks = LOCKS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    locks.entry(cache_path.to_path_buf()).or_default().clone()
}

/// A cache directory is complete when it holds extracted files and no download in
/// progress: versions before 0.1.6 downloaded `artifact.tar.gz` into the directory itself.
fn is_complete_cache(cache_path: &std::path::Path) -> bool {
    if cache_path.join("artifact.tar.gz").exists() {
        return false;
    }
    std::fs::read_dir(cache_path).is_ok_and(|mut entries| entries.next().is_some())
}

#[cfg(test)]
mod cache_tests {
    use super::{artifact_lock, is_complete_cache};

    #[test]
    fn a_directory_with_a_download_in_it_is_not_a_cache() {
        let dir = std::env::temp_dir().join(format!("smc-cache-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!is_complete_cache(&dir), "missing directory");
        std::fs::create_dir_all(&dir).expect("create");
        assert!(!is_complete_cache(&dir), "empty directory");
        std::fs::write(dir.join("artifact.tar.gz"), b"partial").expect("write");
        assert!(!is_complete_cache(&dir), "download in progress");
        std::fs::remove_file(dir.join("artifact.tar.gz")).expect("remove");
        std::fs::write(dir.join("application.secrets.env"), b"A=1").expect("write");
        assert!(is_complete_cache(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn fetches_of_one_source_take_turns() {
        let path = std::path::Path::new("/tmp/smc/flux-artifact/ns/repo/main-sha-abc1234");
        let first = artifact_lock(path);
        let held = first.lock().await;
        let second = artifact_lock(path);
        assert!(second.try_lock().is_err(), "same directory, same lock");
        let other = artifact_lock(std::path::Path::new(
            "/tmp/smc/flux-artifact/ns/other/main-sha-abc1234",
        ));
        assert!(other.try_lock().is_ok(), "another source is not held up");
        drop(held);
        assert!(second.try_lock().is_ok());
    }
}
