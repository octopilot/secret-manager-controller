//! # ArgoCD Artifact Handling
//!
//! Handles ArgoCD Application artifacts.
//! Clones Git repositories directly from ArgoCD Application specs.

use crate::controller::reconciler::types::Reconciler;
use crate::controller::reconciler::utils::{sanitize_path_component, SMC_BASE_PATH};
use crate::crd::SourceRef;
use anyhow::{Context, Result};
use k8s_openapi::api::core::v1::Secret;
use kube::Api;
use std::path::PathBuf;
use std::time::Instant;
use tracing::{info, info_span, warn, Instrument};

use super::download::cleanup_old_revisions;

/// Git credentials for repository authentication
#[derive(Debug, Clone)]
enum GitCredentials {
    /// HTTPS authentication (username/password)
    Https { username: String, password: String },
    /// GitHub token authentication (token used as both username and password)
    GitHubToken { token: String },
    /// SSH authentication (private key)
    Ssh { private_key: String },
}

/// Load git credentials from Kubernetes secret
async fn load_git_credentials(
    reconciler: &Reconciler,
    git_credentials_ref: &crate::crd::GitCredentialsRef,
    default_namespace: &str,
) -> Result<Option<GitCredentials>> {
    let namespace = git_credentials_ref
        .namespace
        .as_deref()
        .unwrap_or(default_namespace);

    let secrets: Api<Secret> = Api::namespaced(reconciler.client.clone(), namespace);

    let secret = match secrets.get(&git_credentials_ref.name).await {
        Ok(secret) => secret,
        Err(e) => {
            warn!(
                "Failed to load git credentials secret {}/{}: {}",
                namespace, git_credentials_ref.name, e
            );
            return Ok(None);
        }
    };

    let data = secret.data.as_ref().context("Secret has no data")?;

    // Check for SSH private key first (identity key)
    if let Some(identity_data) = data.get("identity") {
        let private_key = String::from_utf8(identity_data.0.clone())
            .context("SSH private key is not valid UTF-8")?;
        info!(
            "Loaded SSH git credentials from secret {}/{}",
            namespace, git_credentials_ref.name
        );
        return Ok(Some(GitCredentials::Ssh { private_key }));
    }

    // Check for GitHub token first (special case - token can be used directly)
    if let Some(token_data) = data.get("token").or_else(|| data.get("githubToken")) {
        if let Ok(token) = String::from_utf8(token_data.0.clone()) {
            // Check if it's a GitHub token (starts with ghp_ or github_pat_)
            if token.starts_with("ghp_")
                || token.starts_with("github_pat_")
                || token.starts_with("gho_")
            {
                info!(
                    "Loaded GitHub token from secret {}/{}",
                    namespace, git_credentials_ref.name
                );
                return Ok(Some(GitCredentials::GitHubToken { token }));
            }
        }
    }

    // Check for HTTPS credentials (username and password/token)
    let username = data
        .get("username")
        .and_then(|u| String::from_utf8(u.0.clone()).ok());

    let password = data
        .get("password")
        .or_else(|| data.get("token"))
        .and_then(|p| String::from_utf8(p.0.clone()).ok());

    if let (Some(username), Some(password)) = (username.clone(), password.clone()) {
        info!(
            "Loaded HTTPS git credentials from secret {}/{}",
            namespace, git_credentials_ref.name
        );
        return Ok(Some(GitCredentials::Https { username, password }));
    }

    // If we have a token but no username, treat it as GitHub token
    if let Some(password) = password {
        info!(
            "Loaded token as GitHub token from secret {}/{}",
            namespace, git_credentials_ref.name
        );
        return Ok(Some(GitCredentials::GitHubToken { token: password }));
    }

    warn!(
        "Git credentials secret {}/{} does not contain valid credentials (expected 'identity' for SSH or 'username'/'password' for HTTPS)",
        namespace, git_credentials_ref.name
    );
    Ok(None)
}

/// Prepare git URL with credentials for HTTPS authentication
fn prepare_https_url(repo_url: &str, username: &str, password: &str) -> String {
    // Parse the URL and inject credentials
    if let Some(at_pos) = repo_url.find('@') {
        // URL already has credentials, replace them
        if let Some(scheme_end) = repo_url.find("://") {
            let scheme = &repo_url[..scheme_end + 3];
            let rest = &repo_url[at_pos + 1..];
            format!("{}{}:{}@{}", scheme, username, password, rest)
        } else {
            // No scheme, assume https://
            format!(
                "https://{}:{}@{}",
                username,
                password,
                &repo_url[at_pos + 1..]
            )
        }
    } else if let Some(scheme_end) = repo_url.find("://") {
        // No credentials, inject them
        let scheme = &repo_url[..scheme_end + 3];
        let rest = &repo_url[scheme_end + 3..];
        format!("{}{}:{}@{}", scheme, username, password, rest)
    } else {
        // No scheme, assume https://
        format!("https://{}:{}@{}", username, password, repo_url)
    }
}

/// Prepare git URL with GitHub token authentication
/// GitHub tokens can be used as username with token as password, or just as token
fn prepare_github_token_url(repo_url: &str, token: &str) -> String {
    // For GitHub, use token as both username and password
    // GitHub accepts this pattern: https://token@github.com/owner/repo.git
    // Or: https://username:token@github.com/owner/repo.git
    prepare_https_url(repo_url, token, token)
}

/// Setup SSH key for git operations
/// Returns the path to the SSH key file and GIT_SSH_COMMAND environment variable
async fn setup_ssh_key(private_key: &str, clone_path: &str) -> Result<(PathBuf, String)> {
    use std::os::unix::fs::PermissionsExt;

    // Create .ssh directory in the clone path's parent
    let ssh_dir = PathBuf::from(clone_path)
        .parent()
        .context("Cannot determine parent directory")?
        .join(".ssh");
    tokio::fs::create_dir_all(&ssh_dir)
        .await
        .context("Failed to create .ssh directory")?;

    // Write SSH private key to file
    let ssh_key_path = ssh_dir.join("id_rsa");
    tokio::fs::write(&ssh_key_path, private_key)
        .await
        .context("Failed to write SSH private key")?;

    // Set permissions to 600 (read/write for owner only)
    let mut perms = tokio::fs::metadata(&ssh_key_path)
        .await
        .context("Failed to get SSH key metadata")?
        .permissions();
    perms.set_mode(0o600);
    tokio::fs::set_permissions(&ssh_key_path, perms)
        .await
        .context("Failed to set SSH key permissions")?;

    // Create GIT_SSH_COMMAND to use the key
    let git_ssh_command = format!(
        "ssh -i {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null",
        ssh_key_path.display()
    );

    Ok((ssh_key_path, git_ssh_command))
}

/// Get artifact path from ArgoCD Application
/// Clones the Git repository directly from the Application spec
#[allow(
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::too_many_lines,
    reason = "Markdown formatting is intentional, error docs in comments, complex logic"
)]
pub async fn get_argocd_artifact_path(
    reconciler: &Reconciler,
    source_ref: &SourceRef,
) -> Result<PathBuf> {
    use kube::api::ApiResource;
    use kube::core::DynamicObject;

    // Get ArgoCD Application CRD
    // Application is from argoproj.io/v1alpha1
    let ar = ApiResource::from_gvk(&kube::core::GroupVersionKind {
        group: "argoproj.io".to_string(),
        version: "v1alpha1".to_string(),
        kind: "Application".to_string(),
    });

    let api: kube::Api<DynamicObject> =
        kube::Api::namespaced_with(reconciler.client.clone(), &source_ref.namespace, &ar);

    let application = api.get(&source_ref.name).await.context(format!(
        "Failed to get ArgoCD Application: {}/{}",
        source_ref.namespace, source_ref.name
    ))?;

    // Extract Git source from Application spec
    let spec = application
        .data
        .get("spec")
        .context("ArgoCD Application has no spec")?;

    let source = spec
        .get("source")
        .context("ArgoCD Application has no source in spec")?;

    let repo_url = source
        .get("repoURL")
        .and_then(|u| u.as_str())
        .context("ArgoCD Application source has no repoURL")?;

    let target_revision = source
        .get("targetRevision")
        .and_then(|r| r.as_str())
        .unwrap_or("HEAD");

    info!(
        "ArgoCD Application source: repo={}, revision={}",
        repo_url, target_revision
    );

    // Load git credentials if specified
    let git_credentials = if let Some(ref git_creds_ref) = source_ref.git_credentials {
        load_git_credentials(reconciler, git_creds_ref, &source_ref.namespace).await?
    } else {
        None
    };

    // Clone repository to hierarchical cache directory: /tmp/smc/argocd-repo/{namespace}/{name}/{hash}/
    // This structure:
    // 1. Avoids performance issues with many files in a single directory
    // 2. Allows cluster owners to mount a PVC at /tmp/smc for persistent storage
    // 3. Uses hash for revision to handle long/branch names safely
    let sanitized_namespace = sanitize_path_component(&source_ref.namespace);
    let sanitized_name = sanitize_path_component(&source_ref.name);
    let repo_hash = format!(
        "{:x}",
        md5::compute(format!(
            "{}-{}-{}",
            source_ref.namespace, source_ref.name, target_revision
        ))
    );

    let path_buf = PathBuf::from(SMC_BASE_PATH)
        .join("argocd-repo")
        .join(&sanitized_namespace)
        .join(&sanitized_name)
        .join(&repo_hash);

    let clone_path = path_buf.to_string_lossy().to_string();

    // Setup SSH key if needed for fetch operations (used when updating existing repos)
    let git_env_for_fetch = if let Some(GitCredentials::Ssh { private_key }) = &git_credentials {
        let (key_path, ssh_cmd) = setup_ssh_key(private_key, &clone_path).await?;
        let mut env = std::collections::HashMap::new();
        env.insert("GIT_SSH_COMMAND".to_string(), ssh_cmd);
        env.insert(
            "GIT_SSH_KEY_PATH".to_string(),
            key_path.to_string_lossy().to_string(),
        );
        Some(env)
    } else {
        None
    };

    // Acquire singleton lock for this resource to serialize git operations
    // This ensures only one git operation (clone/fetch) per resource at a time
    // The lock is automatically released when the guard is dropped
    let git_lock = reconciler.get_git_operation_lock(&source_ref.namespace, &source_ref.name);
    info!(
        "Acquiring git operation lock for resource: {}/{}",
        source_ref.namespace, source_ref.name
    );
    let _lock_guard = git_lock.lock().await;

    // Check if repository already exists and try to update it if it's a valid git repo
    if path_buf.exists() {
        let git_dir = path_buf.join(".git");
        let is_git_repo = git_dir.exists() || path_buf.join("HEAD").exists();

        if is_git_repo {
            // Try to update existing repository instead of removing it
            info!(
                "Existing git repository found at {}, attempting to update to revision {}",
                clone_path, target_revision
            );

            // Fetch latest changes
            let mut fetch_cmd = tokio::process::Command::new("git");
            fetch_cmd
                .arg("-C")
                .arg(&path_buf)
                .arg("fetch")
                .arg("origin")
                .arg(target_revision);

            if let Some(ref env_vars) = git_env_for_fetch {
                for (key, value) in env_vars {
                    fetch_cmd.env(key, value);
                }
            }

            let fetch_output = fetch_cmd.output().await;

            if let Ok(fetch_output) = fetch_output {
                if fetch_output.status.success() {
                    // Reset hard to the target revision
                    let reset_output = tokio::process::Command::new("git")
                        .arg("-C")
                        .arg(&path_buf)
                        .arg("reset")
                        .arg("--hard")
                        .arg(target_revision)
                        .output()
                        .await;

                    if let Ok(reset_output) = reset_output {
                        if reset_output.status.success() {
                            // Verify we're at the correct revision
                            let mut verify_cmd = tokio::process::Command::new("git");
                            verify_cmd
                                .arg("-C")
                                .arg(&path_buf)
                                .arg("rev-parse")
                                .arg("HEAD");

                            if let Some(ref env_vars) = git_env_for_fetch {
                                for (key, value) in env_vars {
                                    verify_cmd.env(key, value);
                                }
                            }

                            let verify_output = verify_cmd.output().await;

                            if let Ok(verify_output) = verify_output {
                                if verify_output.status.success() {
                                    let current_rev_str =
                                        String::from_utf8_lossy(&verify_output.stdout);
                                    let current_rev = current_rev_str.trim();
                                    info!(
                                        "Successfully updated ArgoCD repository at {} to revision {} (current: {})",
                                        clone_path, target_revision, current_rev
                                    );
                                    return Ok(path_buf);
                                }
                            }
                        } else {
                            let error_msg = String::from_utf8_lossy(&reset_output.stderr);
                            warn!(
                                "Failed to reset repository at {} to revision {}: {}. Will remove and re-clone.",
                                clone_path, target_revision, error_msg
                            );
                        }
                    } else {
                        warn!(
                            "Failed to execute git reset for repository at {}. Will remove and re-clone.",
                            clone_path
                        );
                    }
                } else {
                    let error_msg = String::from_utf8_lossy(&fetch_output.stderr);
                    warn!(
                        "Failed to fetch updates for repository at {}: {}. Will remove and re-clone.",
                        clone_path, error_msg
                    );
                }
            } else {
                warn!(
                    "Failed to execute git fetch for repository at {}. Will remove and re-clone.",
                    clone_path
                );
            }
        }

        // If update failed or it's not a valid git repo, remove and re-clone
        // Retry removal a few times in case of transient filesystem issues
        let mut removal_attempts = 0;
        const MAX_REMOVAL_ATTEMPTS: u32 = 3;
        let mut removal_succeeded = false;

        while removal_attempts < MAX_REMOVAL_ATTEMPTS {
            match tokio::fs::remove_dir_all(&path_buf).await {
                Ok(_) => {
                    removal_succeeded = true;
                    break;
                }
                Err(e) => {
                    removal_attempts += 1;
                    if removal_attempts >= MAX_REMOVAL_ATTEMPTS {
                        return Err(anyhow::anyhow!(
                            "Failed to remove stale repository at {} after {} attempts: {}. \
                            This may indicate a filesystem issue or the directory is in use. \
                            The controller will retry on the next reconciliation.",
                            clone_path,
                            MAX_REMOVAL_ATTEMPTS,
                            e
                        ));
                    }
                    // Wait a bit before retrying (exponential backoff)
                    let delay_ms = 100u64 * removal_attempts as u64;
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    warn!(
                        "Failed to remove stale repository at {} (attempt {}/{}): {}. Retrying...",
                        clone_path, removal_attempts, MAX_REMOVAL_ATTEMPTS, e
                    );
                }
            }
        }

        if !removal_succeeded {
            return Err(anyhow::anyhow!(
                "Failed to remove stale repository at {} after {} attempts",
                clone_path,
                MAX_REMOVAL_ATTEMPTS
            ));
        }
    }

    // Clone the repository using git command
    let clone_path_for_match = clone_path.clone();
    let path_buf_for_match = path_buf.clone();
    let span = info_span!(
        "git.clone",
        repository.url = repo_url,
        clone.path = clone_path,
        revision = target_revision
    );
    let span_clone_for_match = span.clone();
    let span_clone = span.clone();
    let start = Instant::now();

    // Prepare credentials for git operations
    let (authenticated_repo_url, ssh_key_path): (Option<String>, Option<String>) =
        match &git_credentials {
            Some(GitCredentials::Https { username, password }) => {
                let url = prepare_https_url(repo_url, username, password);
                (Some(url), None)
            }
            Some(GitCredentials::GitHubToken { token }) => {
                let url = prepare_github_token_url(repo_url, token);
                (Some(url), None)
            }
            Some(GitCredentials::Ssh { private_key }) => {
                // Setup SSH key - this will be done inside the async block
                (None, Some(private_key.clone()))
            }
            None => (None, None),
        };

    let clone_result = async move {
        // Setup SSH key if needed
        let (final_repo_url, git_env) = if let Some(private_key) = ssh_key_path {
            let (key_path, ssh_cmd) = setup_ssh_key(&private_key, &clone_path).await?;
            let mut env = std::collections::HashMap::new();
            env.insert("GIT_SSH_COMMAND".to_string(), ssh_cmd);
            env.insert("GIT_SSH_KEY_PATH".to_string(), key_path.to_string_lossy().to_string());
            (repo_url.to_string(), Some(env))
        } else if let Some(url) = authenticated_repo_url {
            (url, None)
        } else {
            (repo_url.to_string(), None)
        };

        info!(
            "Cloning ArgoCD repository: {} (revision: {})",
            final_repo_url, target_revision
        );

        // Create parent directory
        let parent_dir = path_buf.parent().ok_or_else(|| {
            anyhow::anyhow!("Cannot determine parent directory for path: {clone_path}")
        })?;
        tokio::fs::create_dir_all(parent_dir)
            .await
            .context(format!(
                "Failed to create parent directory for {clone_path}"
            ))?;

        // Ensure the clone path doesn't exist before cloning, or try to update if it's a git repo
        // This handles race conditions where the directory might have been recreated
        if path_buf.exists() {
            let git_dir = path_buf.join(".git");
            let is_git_repo = git_dir.exists() || path_buf.join("HEAD").exists();

            if is_git_repo {
                // Try to update existing repository
                info!(
                    "Clone path {} exists and is a git repository, attempting to update to revision {}",
                    clone_path, target_revision
                );

                // Fetch and reset to target revision
                let mut fetch_cmd = tokio::process::Command::new("git");
                fetch_cmd
                    .arg("-C")
                    .arg(&path_buf)
                    .arg("fetch")
                    .arg("origin")
                    .arg(target_revision);

                // Set environment variables for SSH if needed
                if let Some(ref env_vars) = git_env {
                    for (key, value) in env_vars {
                        fetch_cmd.env(key, value);
                    }
                }

                let fetch_output = fetch_cmd.output().await;

                if let Ok(fetch_output) = fetch_output {
                    if fetch_output.status.success() {
                        let mut reset_cmd = tokio::process::Command::new("git");
                        reset_cmd
                            .arg("-C")
                            .arg(&path_buf)
                            .arg("reset")
                            .arg("--hard")
                            .arg(target_revision);

                        if let Some(ref env_vars) = git_env_for_fetch {
                            for (key, value) in env_vars {
                                reset_cmd.env(key, value);
                            }
                        }

                        let reset_output = reset_cmd.output().await;

                        if let Ok(reset_output) = reset_output {
                            if reset_output.status.success() {
                                info!(
                                    "Successfully updated existing repository at {} to revision {}",
                                    clone_path, target_revision
                                );
                                // Return early - we've successfully updated the repo
                                return Ok(());
                            }
                        }
                    }
                }
                // Update failed, fall through to remove and re-clone
                warn!(
                    "Failed to update existing repository at {}, will remove and re-clone",
                    clone_path
                );
            }

            // Not a git repo or update failed, remove it
            if let Err(e) = tokio::fs::remove_dir_all(&path_buf).await {
                return Err(anyhow::anyhow!(
                    "Clone path {} exists and cannot be removed: {}. \
                    This may indicate a concurrent operation or filesystem issue.",
                    clone_path, e
                ));
            }
        }

        // Clone repository (shallow clone for efficiency)
        // First try shallow clone with branch (works for branch/tag names)
        let mut clone_cmd = tokio::process::Command::new("git");
        clone_cmd
            .arg("clone")
            .arg("--depth")
            .arg("1")
            .arg("--branch")
            .arg(target_revision)
            .arg(&final_repo_url)
            .arg(&clone_path);

        // Set environment variables for SSH if needed
        if let Some(ref env_vars) = git_env {
            for (key, value) in env_vars {
                clone_cmd.env(key, value);
            }
        }

        let clone_output = clone_cmd
            .output()
            .await
            .context(format!("Failed to execute git clone for {}", final_repo_url))?;

        if !clone_output.status.success() {
            // Check if the error is about destination already existing
            let error_msg = String::from_utf8_lossy(&clone_output.stderr);
            if error_msg.contains("already exists") && error_msg.contains("not an empty directory") {
                // Directory was created between our check and clone - try to update it if it's a git repo
                warn!(
                    "Destination path {} was created during clone attempt, checking if it's a git repository",
                    clone_path
                );

                let git_dir = path_buf.join(".git");
                let is_git_repo = git_dir.exists() || path_buf.join("HEAD").exists();

                if is_git_repo {
                    // Try to update existing repository
                    info!(
                        "Existing git repository found at {}, attempting to update to revision {}",
                        clone_path, target_revision
                    );

                    let fetch_output = tokio::process::Command::new("git")
                        .arg("-C")
                        .arg(&path_buf)
                        .arg("fetch")
                        .arg("origin")
                        .arg(target_revision)
                        .output()
                        .await;

                    if let Ok(fetch_output) = fetch_output {
                        if fetch_output.status.success() {
                            let reset_output = tokio::process::Command::new("git")
                                .arg("-C")
                                .arg(&path_buf)
                                .arg("reset")
                                .arg("--hard")
                                .arg(target_revision)
                                .output()
                                .await;

                            if let Ok(reset_output) = reset_output {
                                if reset_output.status.success() {
                                    info!(
                                        "Successfully updated existing repository at {} to revision {}",
                                        clone_path, target_revision
                                    );
                                    // Success on update, skip to end
                                    return Ok(());
                                }
                            }
                        }
                    }
                    // Update failed, fall through to remove and retry clone
                    warn!(
                        "Failed to update existing repository at {}, will remove and retry clone",
                        clone_path
                    );
                }

                // Not a git repo or update failed, remove it and retry clone
                if let Err(e) = tokio::fs::remove_dir_all(&path_buf).await {
                    return Err(anyhow::anyhow!(
                        "Failed to remove directory {} that was created during clone: {}. \
                        This may indicate a race condition or filesystem issue.",
                        clone_path, e
                    ));
                }

                // Retry the clone once
                let mut retry_cmd = tokio::process::Command::new("git");
                retry_cmd
                    .arg("clone")
                    .arg("--depth")
                    .arg("1")
                    .arg("--branch")
                    .arg(target_revision)
                    .arg(&final_repo_url)
                    .arg(&clone_path);

                if let Some(ref env_vars) = git_env {
                    for (key, value) in env_vars {
                        retry_cmd.env(key, value);
                    }
                }

                let retry_output = retry_cmd
                    .output()
                    .await
                    .context(format!("Failed to execute git clone retry for {}", final_repo_url))?;

                if retry_output.status.success() {
                    // Success on retry, skip to end
                    return Ok(());
                }
                // Fall through to the deeper clone attempt
            }
            // If branch clone fails, clone default branch and checkout specific revision
            // This handles commit SHAs and other revision types
            let mut fallback_clone_cmd = tokio::process::Command::new("git");
            fallback_clone_cmd
                .arg("clone")
                .arg("--depth")
                .arg("50") // Deeper clone to ensure revision is available
                .arg(&final_repo_url)
                .arg(&clone_path);

            if let Some(ref env_vars) = git_env {
                for (key, value) in env_vars {
                    fallback_clone_cmd.env(key, value);
                }
            }

            let clone_output = fallback_clone_cmd
                .output()
                .await
                .context(format!("Failed to execute git clone for {}", final_repo_url))?;

            if !clone_output.status.success() {
                let error_msg = String::from_utf8_lossy(&clone_output.stderr);

                // Check if the error is about destination already existing
                if error_msg.contains("already exists") && error_msg.contains("not an empty directory") {
                    // Directory was created between our check and clone - try to update it if it's a git repo
                    warn!(
                        "Destination path {} was created during fallback clone attempt, checking if it's a git repository",
                        clone_path
                    );

                    let git_dir = path_buf.join(".git");
                    let is_git_repo = git_dir.exists() || path_buf.join("HEAD").exists();

                    if is_git_repo {
                        // Try to update existing repository
                        info!(
                            "Existing git repository found at {}, attempting to update to revision {}",
                            clone_path, target_revision
                        );

                        let fetch_output = tokio::process::Command::new("git")
                            .arg("-C")
                            .arg(&path_buf)
                            .arg("fetch")
                            .arg("origin")
                            .arg(target_revision)
                            .output()
                            .await;

                        if let Ok(fetch_output) = fetch_output {
                            if fetch_output.status.success() {
                                let reset_output = tokio::process::Command::new("git")
                                    .arg("-C")
                                    .arg(&path_buf)
                                    .arg("reset")
                                    .arg("--hard")
                                    .arg(target_revision)
                                    .output()
                                    .await;

                                if let Ok(reset_output) = reset_output {
                                    if reset_output.status.success() {
                                        info!(
                                            "Successfully updated existing repository at {} to revision {}",
                                            clone_path, target_revision
                                        );
                                        // Success on update, continue with checkout
                                        // Skip the fetch/checkout below since we're already at the right revision
                                        // But we still need to verify we're at the right place
                                        let verify_output = tokio::process::Command::new("git")
                                            .arg("-C")
                                            .arg(&path_buf)
                                            .arg("rev-parse")
                                            .arg("HEAD")
                                            .output()
                                            .await;

                                        if let Ok(verify_output) = verify_output {
                                            if verify_output.status.success() {
                                                // We're done - return success
                                                return Ok(());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // Update failed, fall through to remove and retry clone
                        warn!(
                            "Failed to update existing repository at {}, will remove and retry clone",
                            clone_path
                        );
                    }

                    // Not a git repo or update failed, remove it and retry clone
                    if let Err(e) = tokio::fs::remove_dir_all(&path_buf).await {
                        span_clone.record("operation.success", false);
                        span_clone.record("error.message", format!("Failed to remove existing directory: {}", e));
                        crate::observability::metrics::increment_git_clone_errors_total();
                        return Err(anyhow::anyhow!(
                            "Failed to remove directory {} that was created during fallback clone: {}. \
                            This may indicate a race condition or filesystem issue.",
                            clone_path, e
                        ));
                    }

                    // Retry the fallback clone once
                    let mut fallback_retry_cmd = tokio::process::Command::new("git");
                    fallback_retry_cmd
                        .arg("clone")
                        .arg("--depth")
                        .arg("50")
                        .arg(&final_repo_url)
                        .arg(&clone_path);

                    if let Some(ref env_vars) = git_env {
                        for (key, value) in env_vars {
                            fallback_retry_cmd.env(key, value);
                        }
                    }

                    let retry_output = fallback_retry_cmd
                        .output()
                        .await
                        .context(format!("Failed to execute git clone retry for {}", final_repo_url))?;

                    if !retry_output.status.success() {
                        let retry_error_msg = String::from_utf8_lossy(&retry_output.stderr);
                        span_clone.record("operation.success", false);
                        span_clone.record("error.message", retry_error_msg.to_string());
                        crate::observability::metrics::increment_git_clone_errors_total();
                        return Err(anyhow::anyhow!(
                            "Failed to clone repository {repo_url} after retry: {retry_error_msg}"
                        ));
                    }
                    // Success on retry, continue with checkout
                } else {
                    // Other error, fail immediately
                    span_clone.record("operation.success", false);
                    span_clone.record("error.message", error_msg.to_string());
                    crate::observability::metrics::increment_git_clone_errors_total();
                    return Err(anyhow::anyhow!(
                        "Failed to clone repository {repo_url}: {error_msg}"
                    ));
                }
            }

            // Fetch the specific revision if needed
            let mut fetch_cmd = tokio::process::Command::new("git");
            fetch_cmd
                .arg("-C")
                .arg(&clone_path)
                .arg("fetch")
                .arg("--depth")
                .arg("50")
                .arg("origin")
                .arg(target_revision);

            if let Some(ref env_vars) = git_env {
                for (key, value) in env_vars {
                    fetch_cmd.env(key, value);
                }
            }

            let _fetch_output = fetch_cmd.output().await;

            // Checkout specific revision
            let checkout_output = tokio::process::Command::new("git")
                .arg("-C")
                .arg(&clone_path)
                .arg("checkout")
                .arg(target_revision)
                .output()
                .await
                .context(format!(
                    "Failed to checkout revision {target_revision} in repository {repo_url}"
                ))?;

            if !checkout_output.status.success() {
                let error_msg = String::from_utf8_lossy(&checkout_output.stderr);
                span_clone.record("operation.success", false);
                span_clone.record("error.message", error_msg.to_string());
                crate::observability::metrics::increment_git_clone_errors_total();
                return Err(anyhow::anyhow!(
                    "Failed to checkout revision {target_revision} in repository {repo_url}: {error_msg}"
                ));
            }
        }

        Ok(())
    }
    .instrument(span)
    .await;

    // Lock will be released automatically when _lock_guard is dropped
    match clone_result {
        Ok(_) => {
            span_clone_for_match
                .record("operation.duration_ms", start.elapsed().as_millis() as u64);
            span_clone_for_match.record("operation.success", true);
            crate::observability::metrics::increment_git_clone_total();
            crate::observability::metrics::observe_git_clone_duration(
                start.elapsed().as_secs_f64(),
            );
            info!(
                "Successfully cloned ArgoCD repository to {} (revision: {})",
                clone_path_for_match, target_revision
            );

            // Clean up old revisions - keep only the 3 newest revisions per namespace/name
            // This prevents disk space from growing unbounded
            if let Some(parent_dir) = path_buf_for_match.parent() {
                if let Err(e) = cleanup_old_revisions(parent_dir).await {
                    warn!("Failed to cleanup old ArgoCD revisions: {}", e);
                    // Don't fail reconciliation if cleanup fails
                }
            } else {
                warn!(
                    "Cannot cleanup old ArgoCD revisions: path {} has no parent directory",
                    path_buf_for_match.display()
                );
            }

            Ok(path_buf_for_match)
        }
        Err(e) => {
            span_clone_for_match
                .record("operation.duration_ms", start.elapsed().as_millis() as u64);
            span_clone_for_match.record("operation.success", false);
            Err(e)
        }
    }
}
