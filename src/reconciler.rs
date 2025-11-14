//! # Reconciler
//!
//! Core reconciliation logic for `SecretManagerConfig` resources.
//!
//! The reconciler:
//! - Watches `SecretManagerConfig` resources across all namespaces
//! - Fetches GitRepository or Application artifacts
//! - Processes application secret files or kustomize builds
//! - Syncs secrets to Google Cloud Secret Manager
//! - Updates resource status with reconciliation results
//!
//! ## Reconciliation Flow
//!
//! 1. Get source (FluxCD GitRepository or ArgoCD Application)
//! 2. Extract artifact path
//! 3. Choose mode:
//!    - **Kustomize Build Mode**: Run `kustomize build` and extract secrets
//!    - **Raw File Mode**: Parse `application.secrets.env` files directly
//! 4. Decrypt SOPS-encrypted files if needed
//! 5. Sync secrets to GCP Secret Manager
//! 6. Update status

use crate::{gcp::SecretManagerClient, parser, SecretManagerConfig, metrics};
use kube_runtime::controller::Action;
use anyhow::{Context, Result};
use kube::Client;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use tracing::{error, info, warn};

#[derive(Debug, Error)]
pub enum ReconcilerError {
    #[error("Reconciliation failed: {0}")]
    ReconciliationFailed(#[from] anyhow::Error),
}

#[derive(Clone)]
pub struct Reconciler {
    client: Client,
    secret_manager: Arc<SecretManagerClient>,
    sops_private_key: Option<String>,
}

impl Reconciler {
    pub async fn new(client: Client) -> Result<Self> {
        let secret_manager = SecretManagerClient::new().await?;
        
        // Load SOPS private key from Kubernetes secret
        let sops_private_key = Self::load_sops_private_key(&client).await?;
        
        Ok(Self {
            client,
            secret_manager: Arc::new(secret_manager),
            sops_private_key,
        })
    }

    /// Load SOPS private key from Kubernetes secret in flux-system namespace
    async fn load_sops_private_key(client: &Client) -> Result<Option<String>> {
        use kube::Api;
        use kube::core::ObjectMeta;
        use k8s_openapi::api::core::v1::Secret;
        
        let secrets: Api<Secret> = Api::namespaced(client.clone(), "flux-system");
        
        // Try to get the SOPS private key secret
        // Expected secret name: sops-private-key (or similar)
        let secret_names = vec!["sops-private-key", "sops-gpg-key", "gpg-key"];
        
        for secret_name in secret_names {
            match secrets.get(secret_name).await {
                Ok(secret) => {
                    // Extract private key from secret data
                    // The key might be in different fields: "private-key", "key", "gpg-key", etc.
                    if let Some(ref data_map) = secret.data {
                        if let Some(data) = data_map.get("private-key")
                            .or_else(|| data_map.get("key"))
                            .or_else(|| data_map.get("gpg-key"))
                        {
                            let key = String::from_utf8(data.0.clone())
                                .map_err(|e| anyhow::anyhow!("Failed to decode private key: {}", e))?;
                            info!("Loaded SOPS private key from secret: {}", secret_name);
                            return Ok(Some(key));
                        }
                    }
                }
                Err(kube::Error::Api(api_err)) if api_err.code == 404 => {
                    continue; // Try next secret name
                }
                Err(e) => {
                    warn!("Failed to get secret {}: {}", secret_name, e);
                }
            }
        }
        
        warn!("SOPS private key not found in flux-system namespace, SOPS decryption will be disabled");
        Ok(None)
    }

    pub async fn reconcile(
        config: std::sync::Arc<SecretManagerConfig>,
        ctx: std::sync::Arc<Reconciler>,
    ) -> Result<Action, ReconcilerError> {
        let start = Instant::now();
        let name = config.metadata.name.as_deref().unwrap_or("unknown");
        info!("Reconciling SecretManagerConfig: {}", name);
        
        metrics::increment_reconciliations();

        // Get source and artifact path based on source type
        let artifact_path = match config.spec.source_ref.kind.as_str() {
            "GitRepository" => {
                // FluxCD GitRepository - get artifact path from status
                let git_repo = match Reconciler::get_flux_git_repository(
                    &ctx,
                    &config.spec.source_ref,
                )
                .await
                {
                    Ok(repo) => repo,
                    Err(e) => {
                        error!("Failed to get FluxCD GitRepository: {}", e);
                        metrics::increment_reconciliation_errors();
                        return Err(ReconcilerError::ReconciliationFailed(e.into()));
                    }
                };

                match Reconciler::get_flux_artifact_path(&ctx, &git_repo).await {
                    Ok(path) => {
                        info!(
                            "Found FluxCD artifact path: {} for GitRepository: {}",
                            path.display(),
                            config.spec.source_ref.name
                        );
                        path
                    }
                    Err(e) => {
                        error!("Failed to get FluxCD artifact path: {}", e);
                        metrics::increment_reconciliation_errors();
                        return Err(ReconcilerError::ReconciliationFailed(e.into()));
                    }
                }
            }
            "Application" => {
                // ArgoCD Application - get Git source and clone/access repository
                match Reconciler::get_argocd_artifact_path(&ctx, &config.spec.source_ref).await {
                    Ok(path) => {
                        info!(
                            "Found ArgoCD artifact path: {} for Application: {}",
                            path.display(),
                            config.spec.source_ref.name
                        );
                        path
                    }
                    Err(e) => {
                        error!("Failed to get ArgoCD artifact path: {}", e);
                        metrics::increment_reconciliation_errors();
                        return Err(ReconcilerError::ReconciliationFailed(e.into()));
                    }
                }
            }
            _ => {
                error!("Unsupported source kind: {}", config.spec.source_ref.kind);
                metrics::increment_reconciliation_errors();
                return Err(ReconcilerError::ReconciliationFailed(
                    anyhow::anyhow!("Unsupported source kind: {}", config.spec.source_ref.kind).into()
                ));
            }
        };

        let mut secrets_synced = 0;

        // Check if kustomize_path is specified - use kustomize build mode
        if let Some(ref kustomize_path) = config.spec.kustomize_path {
            // Use kustomize build to extract secrets from generated Secret resources
            // This supports overlays, patches, and generators
            info!("Using kustomize build mode on path: {}", kustomize_path);
            
            match crate::kustomize::extract_secrets_from_kustomize(&artifact_path, kustomize_path).await {
                Ok(secrets) => {
                    let secret_prefix = config.spec.secret_prefix.as_deref().unwrap_or("default");
                    match ctx.process_kustomize_secrets(&config, &secrets, secret_prefix).await {
                        Ok(count) => {
                            secrets_synced += count;
                            info!("Synced {} secrets from kustomize build", count);
                        }
                        Err(e) => {
                            error!("Failed to process kustomize secrets: {}", e);
                            metrics::increment_reconciliation_errors();
                            return Err(ReconcilerError::ReconciliationFailed(e.into()));
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to extract secrets from kustomize build: {}", e);
                    metrics::increment_reconciliation_errors();
                    return Err(ReconcilerError::ReconciliationFailed(e.into()));
                }
            }
        } else {
            // Use raw file mode - read application.secrets.env files directly
            info!("Using raw file mode");
            
            // Find application files for the specified environment
            // Pass secret_prefix as default_service_name for single service deployments
            let default_service_name = config.spec.secret_prefix.as_deref();
            let application_files = match parser::find_application_files(
                &artifact_path,
                config.spec.base_path.as_deref(),
                &config.spec.environment,
                default_service_name,
            )
            .await
            {
                Ok(files) => files,
                Err(e) => {
                    error!("Failed to find application files for environment '{}': {}", 
                        config.spec.environment, e);
                    metrics::increment_reconciliation_errors();
                    return Err(ReconcilerError::ReconciliationFailed(e.into()));
                }
            };

            info!(
                "Found {} application file sets",
                application_files.len()
            );

            // Process each application file set
            for app_files in application_files {
                match ctx.process_application_files(&config, &app_files).await {
                    Ok(count) => {
                        secrets_synced += count;
                        info!("Synced {} secrets for {}", count, app_files.service_name);
                    }
                    Err(e) => {
                        error!("Failed to process {}: {}", app_files.service_name, e);
                    }
                }
            }
        }

        // Update status
        if let Err(e) = ctx.update_status(&config, secrets_synced).await {
            error!("Failed to update status: {}", e);
            metrics::increment_reconciliation_errors();
            return Err(ReconcilerError::ReconciliationFailed(e.into()));
        }

        // Update metrics
        metrics::observe_reconciliation_duration(start.elapsed().as_secs_f64());
        metrics::set_secrets_managed(secrets_synced as i64);

        info!("Reconciliation complete for {} (synced {} secrets)", name, secrets_synced);
        Ok(Action::await_change())
    }

    /// Get FluxCD GitRepository resource
    async fn get_flux_git_repository(
        &self,
        source_ref: &crate::SourceRef,
    ) -> Result<serde_json::Value> {
        // Use Kubernetes API to get GitRepository
        // GitRepository is a CRD from source.toolkit.fluxcd.io/v1beta2
        use kube::core::DynamicObject;
        use kube::api::ApiResource;
        
        let ar = ApiResource::from_gvk(&kube::core::GroupVersionKind {
            group: "source.toolkit.fluxcd.io".to_string(),
            version: "v1beta2".to_string(),
            kind: "GitRepository".to_string(),
        });

        let api: kube::Api<DynamicObject> = kube::Api::namespaced_with(self.client.clone(), &source_ref.namespace, &ar);

        let git_repo = api
            .get(&source_ref.name)
            .await
            .context(format!("Failed to get FluxCD GitRepository: {}/{}", source_ref.namespace, source_ref.name))?;

        Ok(serde_json::to_value(git_repo)?)
    }

    /// Get artifact path from FluxCD GitRepository status
    async fn get_flux_artifact_path(&self, git_repo: &serde_json::Value) -> Result<PathBuf> {
        // Extract artifact path from GitRepository status
        // Flux stores artifacts at: /tmp/flux-source-<namespace>-<name>-<revision>
        // We can also get it from status.artifact.url or status.artifact.path

        let status = git_repo
            .get("status")
            .and_then(|s| s.get("artifact"))
            .context("FluxCD GitRepository has no artifact in status")?;

        // Try to get path from artifact
        if let Some(path) = status.get("path").and_then(|p| p.as_str()) {
            return Ok(PathBuf::from(path));
        }

        // Fallback: construct path from GitRepository metadata
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

        // Default Flux artifact path
        let default_path = format!("/tmp/flux-source-{}-{}", namespace, name);
        warn!("Using default FluxCD artifact path: {}", default_path);
        Ok(PathBuf::from(default_path))
    }

    /// Get artifact path from ArgoCD Application
    /// ArgoCD doesn't store artifacts like FluxCD, so we need to access the Git repository directly
    async fn get_argocd_artifact_path(
        &self,
        source_ref: &crate::SourceRef,
    ) -> Result<PathBuf> {
        use kube::core::DynamicObject;
        use kube::api::ApiResource;
        
        // Get ArgoCD Application CRD
        // Application is from argoproj.io/v1alpha1
        let ar = ApiResource::from_gvk(&kube::core::GroupVersionKind {
            group: "argoproj.io".to_string(),
            version: "v1alpha1".to_string(),
            kind: "Application".to_string(),
        });

        let api: kube::Api<DynamicObject> = kube::Api::namespaced_with(self.client.clone(), &source_ref.namespace, &ar);

        let app = api
            .get(&source_ref.name)
            .await
            .context(format!("Failed to get ArgoCD Application: {}/{}", source_ref.namespace, source_ref.name))?;

        // Extract Git source from Application spec
        let spec = app
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

        let path = source
            .get("path")
            .and_then(|p| p.as_str())
            .unwrap_or("");

        info!(
            "ArgoCD Application source: repo={}, revision={}, path={}",
            repo_url, target_revision, path
        );

        // For ArgoCD, we need to clone the repository or access it via ArgoCD's mechanisms
        // ArgoCD stores repositories in /tmp/apps/<namespace>/<name>/<revision>
        // However, this path might not be accessible. We'll use a similar pattern to FluxCD
        // In production, you might need to clone the repo yourself or use ArgoCD's repo server API
        
        // For now, construct a path similar to FluxCD pattern
        // TODO: Implement proper ArgoCD repository access
        // This might require:
        // 1. Cloning the repository ourselves
        // 2. Using ArgoCD's repo server API
        // 3. Accessing ArgoCD's internal repository cache
        
        let default_path = format!(
            "/tmp/argocd-source-{}-{}-{}",
            source_ref.namespace,
            source_ref.name,
            target_revision.replace('/', "-")
        );
        
        warn!(
            "ArgoCD Application artifact path not directly accessible. Using default path: {}. \
            You may need to clone the repository or configure ArgoCD repository access.",
            default_path
        );
        
        // Check if path exists (ArgoCD might have cloned it)
        let path_buf = PathBuf::from(&default_path);
        if path_buf.exists() {
            Ok(path_buf)
        } else {
            // Return path anyway - caller can handle cloning if needed
            // For now, this will fail gracefully with a clear error message
            Ok(path_buf)
        }
    }

    async fn process_application_files(
        &self,
        config: &SecretManagerConfig,
        app_files: &parser::ApplicationFiles,
    ) -> Result<i32> {
        let secret_prefix = config
            .spec
            .secret_prefix
            .as_deref()
            .unwrap_or(&app_files.service_name);

        // Parse secrets from files (with SOPS decryption if needed)
        let secrets = parser::parse_secrets(&app_files, self.sops_private_key.as_deref()).await?;
        let properties = parser::parse_properties(&app_files).await?;

        // Store secrets in GCP Secret Manager (GitOps: Git is source of truth)
        let mut count = 0;
        let mut updated_count = 0;
        
        for (key, value) in secrets {
            let secret_name = format!("{}-{}", secret_prefix, key);
            match self.secret_manager
                .create_or_update_secret(
                    &config.spec.gcp_project_id,
                    &secret_name,
                    &value,
                )
                .await
            {
                Ok(was_updated) => {
                    count += 1;
                    if was_updated {
                        updated_count += 1;
                        info!("Updated secret {} from git (GitOps source of truth)", secret_name);
                    }
                }
                Err(e) => {
                    error!("Failed to store secret {}: {}", secret_name, e);
                    return Err(e.context(format!("Failed to store secret: {}", secret_name)));
                }
            }
        }
        
        if updated_count > 0 {
            metrics::increment_secrets_updated(updated_count as i64);
            warn!(
                "Updated {} secrets from git (GitOps source of truth). Manual changes in GCP Secret Manager were overwritten.",
                updated_count
            );
        }

        // Store properties as a single secret (JSON encoded)
        if !properties.is_empty() {
            let properties_json = serde_json::to_string(&properties)?;
            let secret_name = format!("{}-properties", secret_prefix);
            match self.secret_manager
                .create_or_update_secret(
                    &config.spec.gcp_project_id,
                    &secret_name,
                    &properties_json,
                )
                .await
            {
                Ok(was_updated) => {
                    count += 1;
                    if was_updated {
                        metrics::increment_secrets_updated(1);
                        info!("Updated properties secret {} from git", secret_name);
                    }
                }
                Err(e) => {
                    error!("Failed to store properties: {}", e);
                    return Err(e.context("Failed to store properties"));
                }
            }
        }

        metrics::increment_secrets_synced(count as i64);
        Ok(count)
    }

    async fn process_kustomize_secrets(
        &self,
        config: &SecretManagerConfig,
        secrets: &std::collections::HashMap<String, String>,
        secret_prefix: &str,
    ) -> Result<i32> {
        // Store secrets in GCP Secret Manager (GitOps: Git is source of truth)
        let mut count = 0;
        let mut updated_count = 0;
        
        for (key, value) in secrets {
            let secret_name = format!("{}-{}", secret_prefix, key);
            match self.secret_manager
                .create_or_update_secret(
                    &config.spec.gcp_project_id,
                    &secret_name,
                    value,
                )
                .await
            {
                Ok(was_updated) => {
                    count += 1;
                    if was_updated {
                        updated_count += 1;
                        info!("Updated secret {} from kustomize build (GitOps source of truth)", secret_name);
                    }
                }
                Err(e) => {
                    error!("Failed to store secret {}: {}", secret_name, e);
                    return Err(e.context(format!("Failed to store secret: {}", secret_name)));
                }
            }
        }
        
        if updated_count > 0 {
            metrics::increment_secrets_updated(updated_count as i64);
            warn!(
                "Updated {} secrets from kustomize build (GitOps source of truth). Manual changes in GCP Secret Manager were overwritten.",
                updated_count
            );
        }

        metrics::increment_secrets_synced(count as i64);
        Ok(count)
    }

    async fn update_status(
        &self,
        config: &SecretManagerConfig,
        secrets_synced: i32,
    ) -> Result<()> {
        use kube::api::PatchParams;
        use kube::core::ObjectMeta;

        let api: kube::Api<SecretManagerConfig> =
            kube::Api::namespaced(self.client.clone(), config.metadata.namespace.as_deref().unwrap_or("default"));

        let status = crate::SecretManagerConfigStatus {
            conditions: vec![crate::Condition {
                r#type: "Ready".to_string(),
                status: "True".to_string(),
                last_transition_time: Some(chrono::Utc::now().to_rfc3339()),
                reason: Some("ReconciliationSucceeded".to_string()),
                message: Some(format!("Synced {} secrets", secrets_synced)),
            }],
            observed_generation: config.metadata.generation,
            last_reconcile_time: Some(chrono::Utc::now().to_rfc3339()),
            secrets_synced: Some(secrets_synced),
        };

        let patch = serde_json::json!({
            "status": status
        });

        api.patch_status(
            config.metadata.name.as_deref().unwrap_or("unknown"),
            &PatchParams::apply("secret-manager-controller"),
            &kube::api::Patch::Merge(patch),
        )
        .await?;

        Ok(())
    }
}

