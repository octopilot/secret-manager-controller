//! # GitRepository and Application Test Utilities
//!
//! Utilities for creating and managing FluxCD GitRepository and ArgoCD Application resources
//! in integration tests.

use anyhow::{Context, Result};
use kube::{
    api::{Api, PostParams},
    core::DynamicObject,
    Client,
};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use tokio::time::sleep;
use tracing::{info, warn};

/// Create a FluxCD GitRepository resource in Kubernetes
///
/// This creates a GitRepository resource with the specified configuration.
/// For tests, we'll set up the artifact path in the status to point to a test directory.
pub async fn create_flux_git_repository(
    client: &Client,
    name: &str,
    namespace: &str,
    repo_url: &str,
    branch: &str,
    _path: &str,
) -> Result<DynamicObject> {
    use kube::api::ApiResource;
    use kube::core::GroupVersionKind;

    let gvk = GroupVersionKind {
        group: "source.toolkit.fluxcd.io".to_string(),
        version: "v1beta2".to_string(),
        kind: "GitRepository".to_string(),
    };

    let ar = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), namespace, &ar);

    // Check if resource already exists
    if let Ok(existing) = api.get(name).await {
        info!(
            "GitRepository {}/{} already exists, returning existing resource",
            namespace, name
        );
        return Ok(existing);
    }

    // Create GitRepository resource
    let git_repo = json!({
        "apiVersion": "source.toolkit.fluxcd.io/v1beta2",
        "kind": "GitRepository",
        "metadata": {
            "name": name,
            "namespace": namespace,
        },
        "spec": {
            "url": repo_url,
            "ref": {
                "branch": branch
            },
            "interval": "1m"
        }
    });

    let obj: DynamicObject =
        serde_json::from_value(git_repo).context("Failed to deserialize GitRepository")?;

    info!(
        "Creating FluxCD GitRepository: {}/{} pointing to {} (branch: {})",
        namespace, name, repo_url, branch
    );

    let created = api
        .create(&PostParams::default(), &obj)
        .await
        .context(format!(
            "Failed to create GitRepository {}/{}",
            namespace, name
        ))?;

    Ok(created)
}

/// Create an ArgoCD Application resource in Kubernetes
///
/// This creates an Application resource with the specified configuration.
/// The controller will clone the repository using the git binary (not libgit2).
pub async fn create_argocd_application(
    client: &Client,
    name: &str,
    namespace: &str,
    repo_url: &str,
    target_revision: &str,
    path: &str,
) -> Result<DynamicObject> {
    use kube::api::ApiResource;
    use kube::core::GroupVersionKind;

    let gvk = GroupVersionKind {
        group: "argoproj.io".to_string(),
        version: "v1alpha1".to_string(),
        kind: "Application".to_string(),
    };

    let ar = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), namespace, &ar);

    // Check if resource already exists
    if let Ok(existing) = api.get(name).await {
        info!(
            "ArgoCD Application {}/{} already exists, returning existing resource",
            namespace, name
        );
        return Ok(existing);
    }

    // Create ArgoCD Application resource
    let application = json!({
        "apiVersion": "argoproj.io/v1alpha1",
        "kind": "Application",
        "metadata": {
            "name": name,
            "namespace": namespace,
        },
        "spec": {
            "source": {
                "repoURL": repo_url,
                "targetRevision": target_revision,
                "path": path
            },
            "project": "default"
        }
    });

    let obj: DynamicObject =
        serde_json::from_value(application).context("Failed to deserialize ArgoCD Application")?;

    info!(
        "Creating ArgoCD Application: {}/{} pointing to {} (revision: {})",
        namespace, name, repo_url, target_revision
    );

    let created = api
        .create(&PostParams::default(), &obj)
        .await
        .context(format!(
            "Failed to create ArgoCD Application {}/{}",
            namespace, name
        ))?;

    Ok(created)
}

/// Wait for a FluxCD GitRepository to be ready
///
/// Polls the GitRepository status until it has a Ready condition with status "True",
/// or until the timeout is reached.
pub async fn wait_for_git_repository_ready(
    client: &Client,
    name: &str,
    namespace: &str,
    timeout: Duration,
) -> Result<()> {
    use kube::api::ApiResource;
    use kube::core::GroupVersionKind;

    let gvk = GroupVersionKind {
        group: "source.toolkit.fluxcd.io".to_string(),
        version: "v1beta2".to_string(),
        kind: "GitRepository".to_string(),
    };

    let ar = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), namespace, &ar);

    let start = std::time::Instant::now();
    let poll_interval = Duration::from_secs(1);

    while start.elapsed() < timeout {
        match api.get(name).await {
            Ok(git_repo) => {
                if let Some(status) = git_repo.data.get("status") {
                    if let Some(conditions) = status.get("conditions").and_then(|c| c.as_array()) {
                        for condition in conditions {
                            if let Some(cond_type) = condition.get("type").and_then(|t| t.as_str())
                            {
                                if cond_type == "Ready" {
                                    if let Some(cond_status) =
                                        condition.get("status").and_then(|s| s.as_str())
                                    {
                                        if cond_status == "True" {
                                            info!("GitRepository {}/{} is ready", namespace, name);
                                            return Ok(());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Error checking GitRepository {}/{} status: {}",
                    namespace, name, e
                );
            }
        }

        sleep(poll_interval).await;
    }

    Err(anyhow::anyhow!(
        "GitRepository {}/{} did not become ready within {:?}",
        namespace,
        name,
        timeout
    ))
}

/// Wait for an ArgoCD Application to be ready
///
/// For ArgoCD Applications, we check if the resource exists and is accessible.
/// Since we're using minimal ArgoCD (CRD only), we don't wait for ArgoCD controllers.
/// The controller will clone the repository itself.
pub async fn wait_for_argocd_application_ready(
    client: &Client,
    name: &str,
    namespace: &str,
    timeout: Duration,
) -> Result<()> {
    use kube::api::ApiResource;
    use kube::core::GroupVersionKind;

    let gvk = GroupVersionKind {
        group: "argoproj.io".to_string(),
        version: "v1alpha1".to_string(),
        kind: "Application".to_string(),
    };

    let ar = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), namespace, &ar);

    let start = std::time::Instant::now();
    let poll_interval = Duration::from_millis(500);

    while start.elapsed() < timeout {
        match api.get(name).await {
            Ok(_) => {
                info!(
                    "ArgoCD Application {}/{} exists and is accessible",
                    namespace, name
                );
                return Ok(());
            }
            Err(e) => {
                if start.elapsed().as_secs() < 2 {
                    // Allow a brief moment for the resource to be created
                    sleep(poll_interval).await;
                    continue;
                }
                warn!(
                    "Error checking ArgoCD Application {}/{} status: {}",
                    namespace, name, e
                );
            }
        }

        sleep(poll_interval).await;
    }

    Err(anyhow::anyhow!(
        "ArgoCD Application {}/{} did not become accessible within {:?}",
        namespace,
        name,
        timeout
    ))
}

/// Set up FluxCD artifact path structure
///
/// Creates a temporary directory structure matching FluxCD's artifact layout
/// and copies test files from `deployment-configuration/profiles/{profile}/` to it.
pub async fn setup_flux_artifact_path(
    namespace: &str,
    name: &str,
    profile: &str,
) -> Result<PathBuf> {
    // Create artifact path matching FluxCD structure: /tmp/smc/flux-source-{namespace}-{name}/
    let base_path = Path::new("/tmp/smc");
    let artifact_dir = base_path.join(format!("flux-source-{}-{}", namespace, name));

    // Create directory structure
    fs::create_dir_all(&artifact_dir).await.context(format!(
        "Failed to create artifact directory: {:?}",
        artifact_dir
    ))?;

    // Copy files from deployment-configuration/profiles/{profile}/
    let source_dir = Path::new("deployment-configuration/profiles").join(profile);

    if source_dir.exists() {
        info!(
            "Copying test files from {:?} to {:?}",
            source_dir, artifact_dir
        );

        // Copy all files from source directory
        copy_directory(&source_dir, &artifact_dir).await?;
    } else {
        warn!(
            "Source directory {:?} does not exist, creating empty artifact directory",
            source_dir
        );
    }

    Ok(artifact_dir)
}

/// Set up ArgoCD repository clone path structure
///
/// Creates a temporary directory structure matching ArgoCD's repository clone layout
/// and copies test files from `deployment-configuration/profiles/{profile}/` to it.
/// The controller will use the git binary to clone (not libgit2, avoids OpenSSL issues).
pub async fn setup_argocd_repo_path(namespace: &str, name: &str, profile: &str) -> Result<PathBuf> {
    // Create repository path matching ArgoCD structure: /tmp/smc/argocd-repo/{namespace}/{name}/{hash}/
    // For tests, we'll use a simple hash based on profile name
    let base_path = Path::new("/tmp/smc");
    let hash = format!(
        "{:x}",
        md5::compute(format!("{}-{}-{}", namespace, name, profile))
    );
    let repo_dir = base_path
        .join("argocd-repo")
        .join(namespace)
        .join(name)
        .join(&hash);

    // Create directory structure
    fs::create_dir_all(&repo_dir).await.context(format!(
        "Failed to create repository directory: {:?}",
        repo_dir
    ))?;

    // Copy files from deployment-configuration/profiles/{profile}/
    let source_dir = Path::new("deployment-configuration/profiles").join(profile);

    if source_dir.exists() {
        info!("Copying test files from {:?} to {:?}", source_dir, repo_dir);

        // Copy all files from source directory
        copy_directory(&source_dir, &repo_dir).await?;
    } else {
        warn!(
            "Source directory {:?} does not exist, creating empty repository directory",
            source_dir
        );
    }

    Ok(repo_dir)
}

/// Copy test files to artifact path
///
/// Helper function to copy files from source to destination directory.
/// Uses a non-recursive approach to avoid async recursion issues.
async fn copy_directory(source: &PathBuf, dest: &PathBuf) -> Result<()> {
    // Use a stack-based approach to handle directories recursively
    let mut stack = vec![(source.clone(), dest.clone())];

    while let Some((src, dst)) = stack.pop() {
        let mut entries = fs::read_dir(&src)
            .await
            .context(format!("Failed to read source directory: {:?}", src))?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .context("Invalid file name")?;

            let dest_path = dst.join(file_name);

            if path.is_file() {
                fs::copy(&path, &dest_path)
                    .await
                    .context(format!("Failed to copy file {:?} to {:?}", path, dest_path))?;
                info!("Copied file: {:?} -> {:?}", path, dest_path);
            } else if path.is_dir() {
                // Add directory to stack for processing
                fs::create_dir_all(&dest_path)
                    .await
                    .context(format!("Failed to create directory: {:?}", dest_path))?;
                stack.push((path, dest_path));
            }
        }
    }

    Ok(())
}

/// Update FluxCD GitRepository status with artifact path
///
/// Updates the GitRepository status to point to the artifact path.
/// This simulates what FluxCD source-controller does.
pub async fn update_git_repository_artifact_path(
    client: &Client,
    name: &str,
    namespace: &str,
    artifact_path: &PathBuf,
    revision: &str,
) -> Result<()> {
    use kube::api::ApiResource;
    use kube::api::{Patch, PatchParams};
    use kube::core::GroupVersionKind;

    let gvk = GroupVersionKind {
        group: "source.toolkit.fluxcd.io".to_string(),
        version: "v1beta2".to_string(),
        kind: "GitRepository".to_string(),
    };

    let ar = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), namespace, &ar);

    // Create status patch
    let status_patch = json!({
        "status": {
            "artifact": {
                "url": format!("file://{}", artifact_path.display()),
                "path": artifact_path.to_string_lossy(),
                "revision": revision,
                "checksum": "test-checksum",
                "lastUpdateTime": chrono::Utc::now().to_rfc3339(),
            },
            "conditions": [{
                "type": "Ready",
                "status": "True",
                "reason": "Succeeded",
                "message": format!("Fetched revision: {}", revision),
                "lastTransitionTime": chrono::Utc::now().to_rfc3339(),
            }]
        }
    });

    let patch = Patch::Apply(status_patch);
    let params = PatchParams::apply("integration-test").force();

    api.patch_status(name, &params, &patch)
        .await
        .context(format!(
            "Failed to update GitRepository {}/{} status",
            namespace, name
        ))?;

    info!(
        "Updated GitRepository {}/{} status with artifact path: {:?}",
        namespace, name, artifact_path
    );

    Ok(())
}
