//! # Reconciler
//!
//! Core reconciliation logic for `SecretManagerConfig` resources.
//!
//! The reconciler:
//! - Watches `SecretManagerConfig` resources across all namespaces
//! - Fetches `GitRepository` or `Application` artifacts
//! - Processes application secret files or kustomize builds
//! - Syncs secrets to Google Cloud Secret Manager
//! - Updates resource status with reconciliation results
//!
//! ## Reconciliation Flow
//!
//! 1. Get source (`FluxCD` `GitRepository` or `ArgoCD` `Application`)
//! 2. Extract artifact path
//! 3. Choose mode:
//!    - **Kustomize Build Mode**: Run `kustomize build` and extract secrets
//!    - **Raw File Mode**: Parse `application.secrets.env` files directly
//! 4. Decrypt SOPS-encrypted files if needed
//! 5. Sync secrets to GCP Secret Manager
//! 6. Update status

use crate::controller::backoff::FibonacciBackoff;
use crate::controller::parser;
use crate::provider::aws::AwsParameterStore;
use crate::provider::aws::AwsSecretManager;
use crate::provider::azure::AzureAppConfiguration;
use crate::provider::azure::AzureKeyVault;
use crate::provider::gcp::SecretManagerClient as GcpSecretManagerClient;
use crate::provider::{ConfigStoreProvider, SecretManagerProvider};
use crate::{
    observability, Condition, ProviderConfig, SecretManagerConfig, SecretManagerConfigStatus,
    SourceRef,
};
use anyhow::{Context, Result};
use kube::Client;
use kube_runtime::controller::Action;
use md5;
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use thiserror::Error;
use tracing::{error, info, warn};

/// Construct secret name with prefix, key, and suffix
/// Matches kustomize-google-secret-manager naming convention for drop-in replacement
///
/// Format: {prefix}-{key}-{suffix} (if both prefix and suffix exist)
///         {prefix}-{key} (if only prefix exists)
///         {key}-{suffix} (if only suffix exists)
///         {key} (if neither exists)
///
/// Invalid characters (`.`, `/`, etc.) are replaced with `_` to match GCP Secret Manager requirements
#[must_use]
#[allow(clippy::doc_markdown)]
#[cfg(test)]
pub fn construct_secret_name(prefix: Option<&str>, key: &str, suffix: Option<&str>) -> String {
    construct_secret_name_impl(prefix, key, suffix)
}

#[cfg(not(test))]
fn construct_secret_name(prefix: Option<&str>, key: &str, suffix: Option<&str>) -> String {
    construct_secret_name_impl(prefix, key, suffix)
}

fn construct_secret_name_impl(prefix: Option<&str>, key: &str, suffix: Option<&str>) -> String {
    let mut parts = Vec::new();

    if let Some(p) = prefix {
        if !p.is_empty() {
            parts.push(p);
        }
    }

    parts.push(key);

    if let Some(s) = suffix {
        if !s.is_empty() {
            // Strip leading dashes from suffix to avoid double dashes when joining
            let suffix_trimmed = s.trim_start_matches('-');
            if !suffix_trimmed.is_empty() {
                parts.push(suffix_trimmed);
            }
        }
    }

    let name = parts.join("-");
    sanitize_secret_name(&name)
}

/// Sanitize secret name to comply with GCP Secret Manager naming requirements
/// Replaces invalid characters (`.`, `/`, etc.) with `_`
/// Matches kustomize-google-secret-manager character sanitization behavior
#[must_use]
#[cfg(test)]
pub fn sanitize_secret_name(name: &str) -> String {
    sanitize_secret_name_impl(name)
}

#[cfg(not(test))]
fn sanitize_secret_name(name: &str) -> String {
    sanitize_secret_name_impl(name)
}

fn sanitize_secret_name_impl(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            // GCP Secret Manager allows: [a-zA-Z0-9_-]+
            // Replace common invalid characters with underscore
            '.' | '/' | ' ' => '_',
            // Keep valid characters
            c if c.is_alphanumeric() || c == '-' || c == '_' => c,
            // Replace any other invalid character with underscore
            _ => '_',
        })
        .collect();

    // Remove consecutive dashes (double dashes, triple dashes, etc.)
    // This handles cases where sanitization creates multiple dashes in a row
    let mut result = String::with_capacity(sanitized.len());
    let mut prev_was_dash = false;

    for c in sanitized.chars() {
        if c == '-' {
            if !prev_was_dash {
                result.push(c);
                prev_was_dash = true;
            }
        } else {
            result.push(c);
            prev_was_dash = false;
        }
    }

    // Remove leading and trailing dashes
    result.trim_matches('-').to_string()
}

#[derive(Debug, Error)]
pub enum ReconcilerError {
    #[error("Reconciliation failed: {0}")]
    ReconciliationFailed(#[from] anyhow::Error),
}

/// Backoff state for a specific resource
/// Tracks error count and backoff calculator for progressive retries
#[derive(Debug, Clone)]
struct BackoffState {
    backoff: FibonacciBackoff,
    error_count: u32,
}

impl BackoffState {
    fn new() -> Self {
        Self {
            backoff: FibonacciBackoff::new(30, 300), // 30s min, 5m max
            error_count: 0,
        }
    }

    fn increment_error(&mut self) {
        self.error_count += 1;
    }

    fn reset(&mut self) {
        self.error_count = 0;
        self.backoff.reset();
    }
}

#[derive(Clone)]
pub struct Reconciler {
    client: Client,
    // Note: secret_manager is created per-reconciliation to support per-resource auth config
    // In the future, we might want to cache clients per auth config
    sops_private_key: Option<String>,
    // Backoff state per resource (identified by namespace/name)
    backoff_states: Arc<Mutex<HashMap<String, BackoffState>>>,
}

impl Reconciler {
    #[allow(clippy::missing_errors_doc)]
    pub async fn new(client: Client) -> Result<Self> {
        // Provider is created per-reconciliation based on provider config
        // Per-resource auth config is handled in reconcile()

        // Load SOPS private key from Kubernetes secret
        let sops_private_key = Self::load_sops_private_key(&client).await?;

        Ok(Self {
            client,
            sops_private_key,
            backoff_states: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Load SOPS private key from Kubernetes secret in controller namespace
    /// Defaults to microscaler-system namespace
    async fn load_sops_private_key(client: &Client) -> Result<Option<String>> {
        use k8s_openapi::api::core::v1::Secret;
        use kube::Api;

        // Use controller namespace (defaults to microscaler-system)
        // Can be overridden via POD_NAMESPACE environment variable
        let namespace =
            std::env::var("POD_NAMESPACE").unwrap_or_else(|_| "microscaler-system".to_string());

        let secrets: Api<Secret> = Api::namespaced(client.clone(), &namespace);

        // Try to get the SOPS private key secret
        // Expected secret name: sops-private-key (or similar)
        let secret_names = vec!["sops-private-key", "sops-gpg-key", "gpg-key"];

        for secret_name in secret_names {
            match secrets.get(secret_name).await {
                Ok(secret) => {
                    // Extract private key from secret data
                    // The key might be in different fields: "private-key", "key", "gpg-key", etc.
                    if let Some(ref data_map) = secret.data {
                        if let Some(data) = data_map
                            .get("private-key")
                            .or_else(|| data_map.get("key"))
                            .or_else(|| data_map.get("gpg-key"))
                        {
                            let key = String::from_utf8(data.0.clone()).map_err(|e| {
                                anyhow::anyhow!("Failed to decode private key: {e}")
                            })?;
                            info!("Loaded SOPS private key from secret: {}", secret_name);
                            return Ok(Some(key));
                        }
                    }
                }
                Err(kube::Error::Api(api_err)) if api_err.code == 404 => {
                    // Try next secret name
                }
                Err(e) => {
                    warn!("Failed to get secret {}: {}", secret_name, e);
                }
            }
        }

        warn!(
            "SOPS private key not found in {} namespace, SOPS decryption will be disabled",
            namespace
        );
        Ok(None)
    }

    #[allow(clippy::too_many_lines, clippy::missing_errors_doc)]
    pub async fn reconcile(
        config: std::sync::Arc<SecretManagerConfig>,
        ctx: std::sync::Arc<Reconciler>,
    ) -> Result<Action, ReconcilerError> {
        // Clone config for use in error handler
        let config_clone = config.clone();
        
        // Wrap entire reconciliation in error handling to prevent panics
        match Self::reconcile_internal(config, ctx.clone()).await {
            Ok(action) => Ok(action),
            Err(e) => {
                let name = config_clone.metadata.name.as_deref().unwrap_or("unknown");
                let resource_key = format!(
                    "{}/{}",
                    config_clone.metadata.namespace.as_deref().unwrap_or("default"),
                    name
                );

                // Get or create backoff state for this resource
                let backoff_seconds = {
                    let mut states = ctx.backoff_states.lock().unwrap();
                    let state = states.entry(resource_key.clone()).or_insert_with(BackoffState::new);
                    state.increment_error();
                    let backoff = state.backoff.next_backoff_seconds();
                    let error_count = state.error_count;
                    (backoff, error_count)
                };

                let next_trigger_time = chrono::Utc::now() + chrono::Duration::seconds(backoff_seconds.0 as i64);
                
                error!(
                    "❌ Reconciliation failed for {} (error count: {}): {}",
                    name, backoff_seconds.1, e
                );
                info!(
                    "🔄 Retrying with Fibonacci backoff: {}s (trigger source: error-backoff)",
                    backoff_seconds.0
                );
                info!(
                    "📅 Next retry scheduled: {}",
                    next_trigger_time.to_rfc3339()
                );
                
                observability::metrics::increment_reconciliation_errors();
                
                // Return requeue action with backoff duration
                // Note: We return Ok here to use backoff scheduling instead of error path
                Ok(Action::requeue(std::time::Duration::from_secs(backoff_seconds.0)))
            }
        }
    }

    #[allow(clippy::too_many_lines, clippy::missing_errors_doc)]
    async fn reconcile_internal(
        config: std::sync::Arc<SecretManagerConfig>,
        ctx: std::sync::Arc<Reconciler>,
    ) -> Result<Action, ReconcilerError> {
        let start = Instant::now();
        let name = config.metadata.name.as_deref().unwrap_or("unknown");

        // Comprehensive validation of all CRD fields
        if let Err(e) = Self::validate_secret_manager_config(&config) {
            error!("Validation error for {}: {}", name, e);
            // Update status to Failed with validation error
            let _ = ctx
                .update_status_phase(
                    &config,
                    "Failed",
                    Some(&format!("Validation failed: {}", e)),
                )
                .await;
            return Err(ReconcilerError::ReconciliationFailed(e));
        }

        // Validate GitRepository pull interval - must be at least 1 minute to avoid rate limits
        if let Err(e) = Self::validate_duration_interval(
            &config.spec.git_repository_pull_interval,
            "gitRepositoryPullInterval",
            60,
        ) {
            let err = anyhow::anyhow!(
                "Invalid gitRepositoryPullInterval '{}': {}",
                config.spec.git_repository_pull_interval,
                e
            );
            error!("Validation error for {}: {}", name, err);
            // Update status to Failed with validation error
            let _ = ctx
                .update_status_phase(
                    &config,
                    "Failed",
                    Some(&format!("Invalid gitRepositoryPullInterval: {}", e)),
                )
                .await;
            return Err(ReconcilerError::ReconciliationFailed(err));
        }

        // Validate reconcile interval - must be at least 1 minute to avoid rate limits
        if let Err(e) = Self::validate_duration_interval(
            &config.spec.reconcile_interval,
            "reconcileInterval",
            60,
        ) {
            let err = anyhow::anyhow!(
                "Invalid reconcileInterval '{}': {}",
                config.spec.reconcile_interval,
                e
            );
            error!("Validation error for {}: {}", name, err);
            // Update status to Failed with validation error
            let _ = ctx
                .update_status_phase(
                    &config,
                    "Failed",
                    Some(&format!("Invalid reconcileInterval: {}", e)),
                )
                .await;
            return Err(ReconcilerError::ReconciliationFailed(err));
        }

        // Check if this is a manual reconciliation trigger (via annotation)
        let is_manual_trigger = config
            .metadata
            .annotations
            .as_ref()
            .and_then(|ann| ann.get("secret-management.microscaler.io/reconcile"))
            .is_some();

        // Determine trigger source for logging
        let trigger_source = if is_manual_trigger {
            "manual-cli"
        } else {
            // Check if this was triggered by an event or timer
            // Since we can't easily distinguish between event-based and timer-based here,
            // we'll log based on whether there's an existing backoff state
            let resource_key = format!(
                "{}/{}",
                config.metadata.namespace.as_deref().unwrap_or("default"),
                name
            );
            let has_errors = ctx
                .backoff_states
                .lock()
                .ok()
                .and_then(|states| states.get(&resource_key).map(|s| s.error_count > 0))
                .unwrap_or(false);
            
            if has_errors {
                "retry-after-error"
            } else {
                "timer-or-event"
            }
        };

        info!(
            "🔄 Reconciling SecretManagerConfig: {} (trigger source: {})",
            name, trigger_source
        );

        observability::metrics::increment_reconciliations();

        // Update status to Started
        if let Err(e) = ctx
            .update_status_phase(&config, "Started", Some("Starting reconciliation"))
            .await
        {
            warn!("Failed to update status to Started: {}", e);
        }

        // Validate and log SecretManagerConfig resource first
        info!(
            "📋 SecretManagerConfig resource details: name={}, namespace={}, sourceRef.kind={}, sourceRef.name={}, sourceRef.namespace={}",
            name,
            config.metadata.namespace.as_deref().unwrap_or("default"),
            config.spec.source_ref.kind,
            config.spec.source_ref.name,
            config.spec.source_ref.namespace
        );

        info!(
            "📋 Secrets config: environment={}, prefix={}, basePath={:?}",
            config.spec.secrets.environment,
            config.spec.secrets.prefix.as_deref().unwrap_or("none"),
            config.spec.secrets.base_path
        );

        info!(
            "📋 Provider config: type={:?}",
            match &config.spec.provider {
                ProviderConfig::Gcp(_) => "gcp",
                ProviderConfig::Aws(_) => "aws",
                ProviderConfig::Azure(_) => "azure",
            }
        );

        // Get source and artifact path based on source type
        info!(
            "🔍 Checking source: {} '{}' in namespace '{}'",
            config.spec.source_ref.kind,
            config.spec.source_ref.name,
            config.spec.source_ref.namespace
        );

        let artifact_path = match config.spec.source_ref.kind.as_str() {
            "GitRepository" => {
                // Update status to Cloning
                if let Err(e) = ctx
                    .update_status_phase(
                        &config,
                        "Cloning",
                        Some("Fetching GitRepository artifact"),
                    )
                    .await
                {
                    warn!("Failed to update status to Cloning: {}", e);
                }

                // FluxCD GitRepository - get artifact path from status
                info!(
                    "📦 Fetching FluxCD GitRepository: {}/{}",
                    config.spec.source_ref.namespace, config.spec.source_ref.name
                );

                let git_repo = match Reconciler::get_flux_git_repository(
                    &ctx,
                    &config.spec.source_ref,
                )
                .await
                {
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
                            if let Some(kube_err) = err.downcast_ref::<kube::Error>() {
                                if let kube::Error::Api(api_err) = kube_err {
                                    return api_err.code == 404;
                                }
                            }
                            false
                        });

                        if is_404 {
                            let next_trigger_time = chrono::Utc::now() + chrono::Duration::seconds(30);
                            warn!(
                                "⏳ GitRepository {}/{} not found yet, will retry in 30s",
                                config.spec.source_ref.namespace, config.spec.source_ref.name
                            );
                            info!(
                                "📅 Next retry scheduled: {} (trigger source: waiting-for-resource)",
                                next_trigger_time.to_rfc3339()
                            );
                            // Update status to Pending (waiting for GitRepository)
                            let _ = ctx
                                .update_status_phase(
                                    &config,
                                    "Pending",
                                    Some("GitRepository not found, waiting for creation"),
                                )
                                .await;
                            // Return requeue action - don't treat as error, just wait for resource
                            return Ok(Action::requeue(std::time::Duration::from_secs(30)));
                        }

                        // For other errors, log and fail
                        error!(
                            "❌ Failed to get FluxCD GitRepository: {}/{} - {}",
                            config.spec.source_ref.namespace, config.spec.source_ref.name, e
                        );
                        observability::metrics::increment_reconciliation_errors();
                        // Update status to Failed
                        let _ = ctx
                            .update_status_phase(
                                &config,
                                "Failed",
                                Some(&format!("Clone failed, repo unavailable: {}", e)),
                            )
                            .await;
                        return Err(ReconcilerError::ReconciliationFailed(e));
                    }
                };

                match Reconciler::get_flux_artifact_path(&ctx, &git_repo) {
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
                        observability::metrics::increment_reconciliation_errors();
                        // Update status to Failed
                        let _ = ctx
                            .update_status_phase(
                                &config,
                                "Failed",
                                Some(&format!("Failed to get artifact path: {}", e)),
                            )
                            .await;
                        return Err(ReconcilerError::ReconciliationFailed(e));
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
                        observability::metrics::increment_reconciliation_errors();
                        return Err(ReconcilerError::ReconciliationFailed(e));
                    }
                }
            }
            _ => {
                error!("Unsupported source kind: {}", config.spec.source_ref.kind);
                observability::metrics::increment_reconciliation_errors();
                return Err(ReconcilerError::ReconciliationFailed(anyhow::anyhow!(
                    "Unsupported source kind: {}",
                    config.spec.source_ref.kind
                )));
            }
        };

        // Create provider based on provider config
        let provider: Box<dyn SecretManagerProvider> = match &config.spec.provider {
            ProviderConfig::Gcp(gcp_config) => {
                // Validate GCP config
                if gcp_config.project_id.is_empty() {
                    let err = anyhow::anyhow!("GCP projectId is required but is empty");
                    error!("Validation error for {}: {}", name, err);
                    return Err(ReconcilerError::ReconciliationFailed(err));
                }

                // Determine authentication method from config
                // Default to Workload Identity when auth is not specified
                let (auth_type, service_account_email_owned) = if let Some(ref auth_config) =
                    gcp_config.auth
                {
                    match serde_json::to_value(auth_config)
                        .context("Failed to serialize gcpAuth config")
                    {
                        Ok(auth_json) => {
                            let auth_type_str = auth_json.get("authType").and_then(|t| t.as_str());
                            if let Some("WorkloadIdentity") = auth_type_str {
                                match auth_json
                                    .get("serviceAccountEmail")
                                    .and_then(|e| e.as_str())
                                {
                                    Some(email) => {
                                        (Some("WorkloadIdentity"), Some(email.to_string()))
                                    }
                                    None => {
                                        warn!("WorkloadIdentity specified but serviceAccountEmail is missing, using default");
                                        (Some("WorkloadIdentity"), None)
                                    }
                                }
                            } else {
                                // Default to Workload Identity
                                info!("No auth type specified, defaulting to Workload Identity");
                                (Some("WorkloadIdentity"), None)
                            }
                        }
                        Err(e) => {
                            error!("Failed to serialize GCP auth config: {}", e);
                            return Err(ReconcilerError::ReconciliationFailed(e));
                        }
                    }
                } else {
                    // Default to Workload Identity when auth is not specified
                    info!("No auth configuration specified, defaulting to Workload Identity");
                    (Some("WorkloadIdentity"), None)
                };

                let service_account_email = service_account_email_owned.as_deref();
                match GcpSecretManagerClient::new(
                    gcp_config.project_id.clone(),
                    auth_type,
                    service_account_email,
                )
                .await
                {
                    Ok(gcp_client) => Box::new(gcp_client),
                    Err(e) => {
                        error!("Failed to create GCP Secret Manager client: {}", e);
                        return Err(ReconcilerError::ReconciliationFailed(e));
                    }
                }
            }
            ProviderConfig::Aws(aws_config) => {
                match AwsSecretManager::new(aws_config, &ctx.client).await {
                    Ok(aws_provider) => Box::new(aws_provider),
                    Err(e) => {
                        error!("Failed to create AWS Secrets Manager client: {}", e);
                        return Err(ReconcilerError::ReconciliationFailed(
                            e.context("Failed to create AWS Secrets Manager client"),
                        ));
                    }
                }
            }
            ProviderConfig::Azure(azure_config) => {
                match AzureKeyVault::new(azure_config, &ctx.client).await {
                    Ok(azure_provider) => Box::new(azure_provider),
                    Err(e) => {
                        error!("Failed to create Azure Key Vault client: {}", e);
                        return Err(ReconcilerError::ReconciliationFailed(
                            e.context("Failed to create Azure Key Vault client"),
                        ));
                    }
                }
            }
        };

        // Determine what we're syncing and update status accordingly
        let is_configs_enabled = config
            .spec
            .configs
            .as_ref()
            .map(|c| c.enabled)
            .unwrap_or(false);
        let description = if is_configs_enabled {
            // Check provider to determine config store type
            match &config.spec.provider {
                ProviderConfig::Gcp(_) => "Reconciling properties to Parameter Manager",
                ProviderConfig::Aws(_) => "Reconciling properties to Parameter Store",
                ProviderConfig::Azure(_) => "Reconciling properties to App Configuration",
            }
        } else {
            // Syncing secrets
            match &config.spec.provider {
                ProviderConfig::Gcp(_) => "Reconciling secrets to Secret Manager",
                ProviderConfig::Aws(_) => "Reconciling secrets to Secrets Manager",
                ProviderConfig::Azure(_) => "Reconciling secrets to Key Vault",
            }
        };

        // Update status to Updating before syncing
        if let Err(e) = ctx
            .update_status_phase(&config, "Updating", Some(description))
            .await
        {
            warn!("Failed to update status to Updating: {}", e);
        }

        let mut secrets_synced = 0;

        // Check if kustomize_path is specified - use kustomize build mode
        if let Some(kustomize_path) = &config.spec.secrets.kustomize_path {
            // Use kustomize build to extract secrets from generated Secret resources
            // This supports overlays, patches, and generators
            info!("Using kustomize build mode on path: {}", kustomize_path);

            match crate::controller::kustomize::extract_secrets_from_kustomize(
                &artifact_path,
                kustomize_path,
            ) {
                Ok(secrets) => {
                    let secret_prefix = config.spec.secrets.prefix.as_deref().unwrap_or("default");
                    match ctx
                        .process_kustomize_secrets(&*provider, &config, &secrets, secret_prefix)
                        .await
                    {
                        Ok(count) => {
                            secrets_synced += count;
                            info!("Synced {} secrets from kustomize build", count);
                        }
                        Err(e) => {
                            error!("Failed to process kustomize secrets: {}", e);
                            observability::metrics::increment_reconciliation_errors();
                            // Update status to Failed
                            let _ = ctx
                                .update_status_phase(
                                    &config,
                                    "Failed",
                                    Some(&format!("Failed to process kustomize secrets: {}", e)),
                                )
                                .await;
                            return Err(ReconcilerError::ReconciliationFailed(e));
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to extract secrets from kustomize build: {}", e);
                    observability::metrics::increment_reconciliation_errors();
                    // Update status to Failed
                    let _ = ctx
                        .update_status_phase(
                            &config,
                            "Failed",
                            Some(&format!("Failed to extract secrets from kustomize: {}", e)),
                        )
                        .await;
                    return Err(ReconcilerError::ReconciliationFailed(e));
                }
            }
        } else {
            // Use raw file mode - read application.secrets.env files directly
            info!("Using raw file mode");

            // Find application files for the specified environment
            // Pass secret_prefix as default_service_name for single service deployments
            let default_service_name = config.spec.secrets.prefix.as_deref();
            let application_files = match parser::find_application_files(
                &artifact_path,
                config.spec.secrets.base_path.as_deref(),
                &config.spec.secrets.environment,
                default_service_name,
            )
            .await
            {
                Ok(files) => files,
                Err(e) => {
                    error!(
                        "Failed to find application files for environment '{}': {}",
                        config.spec.secrets.environment, e
                    );
                    observability::metrics::increment_reconciliation_errors();
                    // Update status to Failed
                    let _ = ctx
                        .update_status_phase(
                            &config,
                            "Failed",
                            Some(&format!("Failed to find application files: {}", e)),
                        )
                        .await;
                    return Err(ReconcilerError::ReconciliationFailed(e));
                }
            };

            info!("Found {} application file sets", application_files.len());

            // Process each application file set
            for app_files in application_files {
                match ctx
                    .process_application_files(&*provider, &config, &app_files, &ctx)
                    .await
                {
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
            observability::metrics::increment_reconciliation_errors();
            return Err(ReconcilerError::ReconciliationFailed(e));
        }

        // Update metrics
        observability::metrics::observe_reconciliation_duration(start.elapsed().as_secs_f64());
        observability::metrics::set_secrets_managed(i64::from(secrets_synced));

        // Success - reset backoff state for this resource
        let resource_key = format!(
            "{}/{}",
            config.metadata.namespace.as_deref().unwrap_or("default"),
            name
        );
        if let Ok(mut states) = ctx.backoff_states.lock() {
            if let Some(state) = states.get_mut(&resource_key) {
                state.reset();
            }
        }

        // Parse reconcile_interval and schedule next reconciliation
        let reconcile_interval_seconds = Self::parse_duration_to_seconds(&config.spec.reconcile_interval)
            .unwrap_or(60); // Default to 60s if parsing fails

        let next_trigger_time = chrono::Utc::now() + chrono::Duration::seconds(reconcile_interval_seconds as i64);
        
        info!(
            "✅ Reconciliation complete for {} (synced {} secrets, duration: {:.2}s)",
            name, secrets_synced, start.elapsed().as_secs_f64()
        );
        info!(
            "📅 Next scheduled reconciliation: {} (in {}s, trigger source: timer-based)",
            next_trigger_time.to_rfc3339(),
            reconcile_interval_seconds
        );

        Ok(Action::requeue(std::time::Duration::from_secs(reconcile_interval_seconds)))
    }

    /// Get FluxCD GitRepository resource
    #[allow(clippy::doc_markdown, clippy::missing_errors_doc)]
    async fn get_flux_git_repository(&self, source_ref: &SourceRef) -> Result<serde_json::Value> {
        // Use Kubernetes API to get GitRepository
        // GitRepository is a CRD from source.toolkit.fluxcd.io/v1beta2
        use kube::api::ApiResource;
        use kube::core::DynamicObject;

        let ar = ApiResource::from_gvk(&kube::core::GroupVersionKind {
            group: "source.toolkit.fluxcd.io".to_string(),
            version: "v1beta2".to_string(),
            kind: "GitRepository".to_string(),
        });

        let api: kube::Api<DynamicObject> =
            kube::Api::namespaced_with(self.client.clone(), &source_ref.namespace, &ar);

        let git_repo = api.get(&source_ref.name).await.context(format!(
            "Failed to get FluxCD GitRepository: {}/{}",
            source_ref.namespace, source_ref.name
        ))?;

        Ok(serde_json::to_value(git_repo)?)
    }

    /// Get artifact path from FluxCD GitRepository status
    #[allow(
        clippy::doc_markdown,
        clippy::unused_async,
        clippy::missing_errors_doc,
        clippy::unused_self
    )]
    fn get_flux_artifact_path(&self, git_repo: &serde_json::Value) -> Result<PathBuf> {
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
        let default_path = format!("/tmp/flux-source-{namespace}-{name}");
        warn!("Using default FluxCD artifact path: {}", default_path);
        Ok(PathBuf::from(default_path))
    }

    /// Get artifact path from ArgoCD Application
    /// Clones the Git repository directly from the Application spec
    #[allow(
        clippy::doc_markdown,
        clippy::missing_errors_doc,
        clippy::unused_async,
        clippy::too_many_lines
    )]
    async fn get_argocd_artifact_path(&self, source_ref: &SourceRef) -> Result<PathBuf> {
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
            kube::Api::namespaced_with(self.client.clone(), &source_ref.namespace, &ar);

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

        // Clone repository to temporary directory
        // Use a deterministic path based on Application name/namespace/revision for caching
        let repo_hash = format!(
            "{:x}",
            md5::compute(format!(
                "{}-{}-{}",
                source_ref.namespace, source_ref.name, target_revision
            ))
        );
        let clone_path = format!("/tmp/argocd-repo-{repo_hash}");
        let path_buf = PathBuf::from(&clone_path);

        // Check if repository already exists and is at the correct revision
        if path_buf.exists() {
            // Verify the revision matches by checking HEAD
            let git_dir = path_buf.join(".git");
            if git_dir.exists() || path_buf.join("HEAD").exists() {
                // Check current HEAD revision
                let output = tokio::process::Command::new("git")
                    .arg("-C")
                    .arg(&path_buf)
                    .arg("rev-parse")
                    .arg("HEAD")
                    .output()
                    .await;

                if let Ok(output) = output {
                    if output.status.success() {
                        let current_rev =
                            String::from_utf8_lossy(&output.stdout).trim().to_string();
                        // Try to resolve target revision
                        let target_output = tokio::process::Command::new("git")
                            .arg("-C")
                            .arg(&path_buf)
                            .arg("rev-parse")
                            .arg(target_revision)
                            .output()
                            .await;

                        if let Ok(target_output) = target_output {
                            if target_output.status.success() {
                                let target_rev = String::from_utf8_lossy(&target_output.stdout)
                                    .trim()
                                    .to_string();
                                if current_rev == target_rev {
                                    info!(
                                        "Using cached ArgoCD repository at {} (revision: {})",
                                        clone_path, target_revision
                                    );
                                    return Ok(path_buf);
                                }
                            }
                        }
                    }
                }
            }
            // Remove stale repository
            if let Err(e) = tokio::fs::remove_dir_all(&path_buf).await {
                warn!("Failed to remove stale repository at {}: {}", clone_path, e);
            }
        }

        // Clone the repository using git command
        info!(
            "Cloning ArgoCD repository: {} (revision: {})",
            repo_url, target_revision
        );

        // Create parent directory
        let parent_dir = path_buf.parent().ok_or_else(|| {
            anyhow::anyhow!("Cannot determine parent directory for path: {}", clone_path)
        })?;
        tokio::fs::create_dir_all(parent_dir)
            .await
            .context(format!(
                "Failed to create parent directory for {}",
                clone_path
            ))?;

        // Clone repository (shallow clone for efficiency)
        // First try shallow clone with branch (works for branch/tag names)
        let clone_output = tokio::process::Command::new("git")
            .arg("clone")
            .arg("--depth")
            .arg("1")
            .arg("--branch")
            .arg(target_revision)
            .arg(repo_url)
            .arg(&clone_path)
            .output()
            .await
            .context(format!("Failed to execute git clone for {repo_url}"))?;

        if !clone_output.status.success() {
            // If branch clone fails, clone default branch and checkout specific revision
            // This handles commit SHAs and other revision types
            let clone_output = tokio::process::Command::new("git")
                .arg("clone")
                .arg("--depth")
                .arg("50") // Deeper clone to ensure revision is available
                .arg(repo_url)
                .arg(&clone_path)
                .output()
                .await
                .context(format!("Failed to execute git clone for {repo_url}"))?;

            if !clone_output.status.success() {
                let error_msg = String::from_utf8_lossy(&clone_output.stderr);
                return Err(anyhow::anyhow!(
                    "Failed to clone repository {repo_url}: {error_msg}"
                ));
            }

            // Fetch the specific revision if needed
            let _fetch_output = tokio::process::Command::new("git")
                .arg("-C")
                .arg(&clone_path)
                .arg("fetch")
                .arg("--depth")
                .arg("50")
                .arg("origin")
                .arg(target_revision)
                .output()
                .await;

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
                return Err(anyhow::anyhow!(
                    "Failed to checkout revision {target_revision} in repository {repo_url}: {error_msg}"
                ));
            }
        }

        info!(
            "Successfully cloned ArgoCD repository to {} (revision: {})",
            clone_path, target_revision
        );
        Ok(path_buf)
    }

    #[allow(clippy::too_many_lines, clippy::unused_async)]
    async fn process_application_files(
        &self,
        provider: &dyn SecretManagerProvider,
        config: &SecretManagerConfig,
        app_files: &parser::ApplicationFiles,
        ctx: &Reconciler,
    ) -> Result<i32> {
        let secret_prefix = config
            .spec
            .secrets
            .prefix
            .as_deref()
            .unwrap_or(&app_files.service_name);

        // Parse secrets from files (with SOPS decryption if needed)
        let secrets = parser::parse_secrets(app_files, self.sops_private_key.as_deref()).await?;
        let properties = parser::parse_properties(app_files).await?;

        // Store secrets in cloud provider (GitOps: Git is source of truth)
        let mut count = 0;
        let mut updated_count = 0;

        for (key, value) in secrets {
            let secret_name = construct_secret_name(
                Some(secret_prefix),
                key.as_str(),
                config.spec.secrets.suffix.as_deref(),
            );
            match provider.create_or_update_secret(&secret_name, &value).await {
                Ok(was_updated) => {
                    count += 1;
                    if was_updated {
                        updated_count += 1;
                        info!(
                            "Updated secret {} from git (GitOps source of truth)",
                            secret_name
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to store secret {}: {}", secret_name, e);
                    return Err(e.context(format!("Failed to store secret: {secret_name}")));
                }
            }
        }

        if updated_count > 0 {
            observability::metrics::increment_secrets_updated(i64::from(updated_count));
            warn!(
                "Updated {} secrets from git (GitOps source of truth). Manual changes in cloud provider were overwritten.",
                updated_count
            );
        }

        // Store properties - route to config store if enabled, otherwise store as JSON blob in secret store
        if !properties.is_empty() {
            let configs_enabled = config
                .spec
                .configs
                .as_ref()
                .map(|c| c.enabled)
                .unwrap_or(false);

            if configs_enabled {
                // Route properties to config store (store individually)
                info!(
                    "Config store enabled: storing {} properties individually",
                    properties.len()
                );
                let mut config_count = 0;
                let mut config_updated_count = 0;

                // Route to appropriate config store based on provider
                match &config.spec.provider {
                    ProviderConfig::Gcp(_gcp_config) => {
                        // For GCP, reuse Secret Manager provider (store configs as individual secrets)
                        // This is an interim solution until Parameter Manager support is contributed to ESO
                        for (key, value) in properties {
                            let config_name = construct_secret_name(
                                Some(secret_prefix),
                                key.as_str(),
                                config.spec.secrets.suffix.as_deref(),
                            );
                            match provider.create_or_update_secret(&config_name, &value).await {
                                Ok(was_updated) => {
                                    config_count += 1;
                                    if was_updated {
                                        config_updated_count += 1;
                                        info!(
                                            "Updated config {} from git (GitOps source of truth)",
                                            config_name
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to store config {}: {}", config_name, e);
                                    return Err(e.context(format!(
                                        "Failed to store config: {config_name}"
                                    )));
                                }
                            }
                        }
                    }
                    ProviderConfig::Aws(aws_config) => {
                        // For AWS, use Parameter Store
                        let parameter_path = config
                            .spec
                            .configs
                            .as_ref()
                            .and_then(|c| c.parameter_path.as_deref());
                        let aws_param_store = AwsParameterStore::new(
                            aws_config,
                            parameter_path,
                            secret_prefix,
                            &config.spec.secrets.environment,
                            &ctx.client,
                        )
                        .await
                        .context("Failed to create AWS Parameter Store client")?;

                        for (key, value) in properties {
                            match aws_param_store.create_or_update_config(&key, &value).await {
                                Ok(was_updated) => {
                                    config_count += 1;
                                    if was_updated {
                                        config_updated_count += 1;
                                        info!(
                                            "Updated config {} from git (GitOps source of truth)",
                                            key
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to store config {}: {}", key, e);
                                    return Err(e.context(format!("Failed to store config: {key}")));
                                }
                            }
                        }
                    }
                    ProviderConfig::Azure(azure_config) => {
                        // For Azure, use App Configuration
                        let app_config_endpoint = config
                            .spec
                            .configs
                            .as_ref()
                            .and_then(|c| c.app_config_endpoint.as_deref());
                        let azure_app_config = AzureAppConfiguration::new(
                            azure_config,
                            app_config_endpoint,
                            secret_prefix,
                            &config.spec.secrets.environment,
                            &ctx.client,
                        )
                        .await
                        .context("Failed to create Azure App Configuration client")?;

                        for (key, value) in properties {
                            match azure_app_config.create_or_update_config(&key, &value).await {
                                Ok(was_updated) => {
                                    config_count += 1;
                                    if was_updated {
                                        config_updated_count += 1;
                                        info!(
                                            "Updated config {} from git (GitOps source of truth)",
                                            key
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to store config {}: {}", key, e);
                                    return Err(e.context(format!("Failed to store config: {key}")));
                                }
                            }
                        }
                    }
                }

                count += config_count;
                if config_updated_count > 0 {
                    observability::metrics::increment_secrets_updated(i64::from(
                        config_updated_count,
                    ));
                    warn!(
                        "Updated {} configs from git (GitOps source of truth). Manual changes in cloud provider were overwritten.",
                        config_updated_count
                    );
                }
            } else {
                // Backward compatibility: store properties as a single secret (JSON encoded)
                let properties_json = serde_json::to_string(&properties)?;
                let secret_name = construct_secret_name(
                    Some(secret_prefix),
                    "properties",
                    config.spec.secrets.suffix.as_deref(),
                );
                match provider
                    .create_or_update_secret(&secret_name, &properties_json)
                    .await
                {
                    Ok(was_updated) => {
                        count += 1;
                        if was_updated {
                            observability::metrics::increment_secrets_updated(1);
                            info!("Updated properties secret {} from git", secret_name);
                        }
                    }
                    Err(e) => {
                        error!("Failed to store properties: {}", e);
                        return Err(e.context("Failed to store properties"));
                    }
                }
            }
        }

        observability::metrics::increment_secrets_synced(i64::from(count));
        Ok(count)
    }

    async fn process_kustomize_secrets(
        &self,
        provider: &dyn SecretManagerProvider,
        config: &SecretManagerConfig,
        secrets: &std::collections::HashMap<String, String>,
        secret_prefix: &str,
    ) -> Result<i32> {
        // Store secrets in cloud provider (GitOps: Git is source of truth)
        let mut count = 0;
        let mut updated_count = 0;

        for (key, value) in secrets {
            let secret_name = construct_secret_name(
                Some(secret_prefix),
                key.as_str(),
                config.spec.secrets.suffix.as_deref(),
            );
            match provider.create_or_update_secret(&secret_name, value).await {
                Ok(was_updated) => {
                    count += 1;
                    if was_updated {
                        updated_count += 1;
                        info!(
                            "Updated secret {} from kustomize build (GitOps source of truth)",
                            secret_name
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to store secret {}: {}", secret_name, e);
                    return Err(e.context(format!("Failed to store secret: {secret_name}")));
                }
            }
        }

        if updated_count > 0 {
            observability::metrics::increment_secrets_updated(i64::from(updated_count));
            warn!(
                "Updated {} secrets from kustomize build (GitOps source of truth). Manual changes in cloud provider were overwritten.",
                updated_count
            );
        }

        observability::metrics::increment_secrets_synced(i64::from(count));
        Ok(count)
    }

    /// Update status with phase and optional message
    async fn update_status_phase(
        &self,
        config: &SecretManagerConfig,
        phase: &str,
        message: Option<&str>,
    ) -> Result<()> {
        use kube::api::PatchParams;

        let api: kube::Api<SecretManagerConfig> = kube::Api::namespaced(
            self.client.clone(),
            config.metadata.namespace.as_deref().unwrap_or("default"),
        );

        let mut conditions = vec![];
        let ready_status = if phase == "Ready" { "True" } else { "False" };
        let ready_reason = if phase == "Ready" {
            "ReconciliationSucceeded"
        } else if phase == "Failed" {
            "ReconciliationFailed"
        } else {
            "ReconciliationInProgress"
        };

        conditions.push(Condition {
            r#type: "Ready".to_string(),
            status: ready_status.to_string(),
            last_transition_time: Some(chrono::Utc::now().to_rfc3339()),
            reason: Some(ready_reason.to_string()),
            message: message.map(|s| s.to_string()),
        });

        let status = SecretManagerConfigStatus {
            phase: Some(phase.to_string()),
            description: message.map(|s| s.to_string()),
            conditions,
            observed_generation: config.metadata.generation,
            last_reconcile_time: Some(chrono::Utc::now().to_rfc3339()),
            secrets_synced: None,
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

    async fn update_status(&self, config: &SecretManagerConfig, secrets_synced: i32) -> Result<()> {
        use kube::api::PatchParams;

        let api: kube::Api<SecretManagerConfig> = kube::Api::namespaced(
            self.client.clone(),
            config.metadata.namespace.as_deref().unwrap_or("default"),
        );

        // Determine what was synced for the description
        let is_configs_enabled = config
            .spec
            .configs
            .as_ref()
            .map(|c| c.enabled)
            .unwrap_or(false);
        let description = if is_configs_enabled {
            format!("Synced {} properties to config store", secrets_synced)
        } else {
            format!("Synced {} secrets to secret store", secrets_synced)
        };

        let status = SecretManagerConfigStatus {
            phase: Some("Ready".to_string()),
            description: Some(description.clone()),
            conditions: vec![Condition {
                r#type: "Ready".to_string(),
                status: "True".to_string(),
                last_transition_time: Some(chrono::Utc::now().to_rfc3339()),
                reason: Some("ReconciliationSucceeded".to_string()),
                message: Some(description),
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

    /// Parse Kubernetes duration string to seconds
    /// Accepts format: "1m", "5m", "1h", "30s", etc.
    ///
    /// # Arguments
    /// * `interval` - The duration string to parse
    ///
    /// # Returns
    /// * `Ok(seconds)` if valid
    /// * `Err` with descriptive error message if invalid
    fn parse_duration_to_seconds(interval: &str) -> Result<u64> {
        use regex::Regex;

        let interval_trimmed = interval.trim();
        if interval_trimmed.is_empty() {
            return Err(anyhow::anyhow!("Duration cannot be empty"));
        }

        let duration_regex = Regex::new(r"^(?P<number>\d+)(?P<unit>[smhd])$")
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))?;

        let interval_lower = interval_trimmed.to_lowercase();
        let captures = duration_regex.captures(&interval_lower)
            .ok_or_else(|| anyhow::anyhow!(
                "Invalid duration format '{}': must match pattern <number><unit> where unit is s, m, h, or d",
                interval_trimmed
            ))?;

        let number_str = captures.name("number")
            .ok_or_else(|| anyhow::anyhow!("Failed to extract number from duration"))?
            .as_str();

        let unit = captures.name("unit")
            .ok_or_else(|| anyhow::anyhow!("Failed to extract unit from duration"))?
            .as_str();

        let number: u64 = number_str.parse()
            .map_err(|e| anyhow::anyhow!("Invalid duration number: {}", e))?;

        if number == 0 {
            return Err(anyhow::anyhow!("Duration number must be greater than 0"));
        }

        let seconds = match unit {
            "s" => number,
            "m" => number * 60,
            "h" => number * 3600,
            "d" => number * 86400,
            _ => return Err(anyhow::anyhow!("Invalid duration unit: {}", unit)),
        };

        Ok(seconds)
    }

    /// Validate duration interval with regex and minimum value check
    /// Ensures interval matches Kubernetes duration format and meets minimum requirement
    /// Accepts Kubernetes duration format: "1m", "5m", "1h", etc.
    ///
    /// # Arguments
    /// * `interval` - The duration string to validate
    /// * `field_name` - Name of the field for error messages
    /// * `min_seconds` - Minimum allowed duration in seconds
    ///
    /// # Returns
    /// * `Ok(())` if valid
    /// * `Err` with descriptive error message if invalid
    fn validate_duration_interval(
        interval: &str,
        field_name: &str,
        min_seconds: u64,
    ) -> Result<()> {
        use regex::Regex;

        // Trim whitespace
        let interval_trimmed = interval.trim();

        if interval_trimmed.is_empty() {
            return Err(anyhow::anyhow!("{} cannot be empty", field_name));
        }

        // Regex pattern for Kubernetes duration format
        // Matches: <number><unit> where:
        //   - number: one or more digits
        //   - unit: s, m, h, d (case insensitive)
        // Examples: "1m", "5m", "1h", "30m", "2h", "1d"
        // Does NOT match: "30s" (if min_seconds >= 60), "abc", "1", "m", etc.
        let duration_regex = Regex::new(r"^(?P<number>\d+)(?P<unit>[smhd])$")
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))?;

        // Match against trimmed, lowercase version
        let interval_lower = interval_trimmed.to_lowercase();

        let captures = duration_regex.captures(&interval_lower)
            .ok_or_else(|| anyhow::anyhow!(
                "Invalid duration format '{}': must match pattern <number><unit> where unit is s, m, h, or d (e.g., '1m', '5m', '1h')",
                interval_trimmed
            ))?;

        // Extract number and unit from regex captures
        let number_str = captures
            .name("number")
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Failed to extract number from duration '{}'",
                    interval_trimmed
                )
            })?
            .as_str();

        let unit = captures
            .name("unit")
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Failed to extract unit from duration '{}'",
                    interval_trimmed
                )
            })?
            .as_str();

        // Parse number safely
        let number: u64 = number_str.parse().map_err(|e| {
            anyhow::anyhow!(
                "Invalid duration number '{}' in '{}': {}",
                number_str,
                interval_trimmed,
                e
            )
        })?;

        if number == 0 {
            return Err(anyhow::anyhow!(
                "Duration number must be greater than 0, got '{}'",
                interval_trimmed
            ));
        }

        // Convert to seconds based on unit
        let seconds = match unit {
            "s" => number,
            "m" => number * 60,
            "h" => number * 3600,
            "d" => number * 86400,
            _ => {
                // This should never happen due to regex, but handle it safely
                return Err(anyhow::anyhow!(
                    "Invalid duration unit '{}' in '{}': expected s, m, h, or d",
                    unit,
                    interval_trimmed
                ));
            }
        };

        // Enforce minimum
        if seconds < min_seconds {
            let min_duration = if min_seconds == 60 {
                "1 minute (60 seconds)"
            } else {
                &format!("{} seconds", min_seconds)
            };
            return Err(anyhow::anyhow!(
                "{} must be at least {} to avoid API rate limits. Got: '{}' ({} seconds)",
                field_name,
                min_duration,
                interval_trimmed,
                seconds
            ));
        }

        Ok(())
    }

    /// Comprehensive validation of SecretManagerConfig fields
    /// Validates all fields according to CRD schema and Kubernetes conventions
    /// Returns Ok(()) if valid, Err with descriptive message if invalid
    fn validate_secret_manager_config(config: &SecretManagerConfig) -> Result<()> {
        // Validate sourceRef.kind
        if config.spec.source_ref.kind.is_empty() {
            return Err(anyhow::anyhow!("sourceRef.kind is required but is empty"));
        }
        if let Err(e) = Self::validate_source_ref_kind(&config.spec.source_ref.kind) {
            return Err(anyhow::anyhow!(
                "Invalid sourceRef.kind '{}': {}",
                config.spec.source_ref.kind,
                e
            ));
        }

        // Validate sourceRef.name
        if config.spec.source_ref.name.is_empty() {
            return Err(anyhow::anyhow!("sourceRef.name is required but is empty"));
        }
        if let Err(e) =
            Self::validate_kubernetes_name(&config.spec.source_ref.name, "sourceRef.name")
        {
            return Err(anyhow::anyhow!(
                "Invalid sourceRef.name '{}': {}",
                config.spec.source_ref.name,
                e
            ));
        }

        // Validate sourceRef.namespace
        if config.spec.source_ref.namespace.is_empty() {
            return Err(anyhow::anyhow!(
                "sourceRef.namespace is required but is empty"
            ));
        }
        if let Err(e) = Self::validate_kubernetes_namespace(&config.spec.source_ref.namespace) {
            return Err(anyhow::anyhow!(
                "Invalid sourceRef.namespace '{}': {}",
                config.spec.source_ref.namespace,
                e
            ));
        }

        // Validate secrets.environment
        if config.spec.secrets.environment.is_empty() {
            return Err(anyhow::anyhow!(
                "secrets.environment is required but is empty"
            ));
        }
        if let Err(e) =
            Self::validate_kubernetes_label(&config.spec.secrets.environment, "secrets.environment")
        {
            return Err(anyhow::anyhow!(
                "Invalid secrets.environment '{}': {}",
                config.spec.secrets.environment,
                e
            ));
        }

        // Validate optional secrets fields
        if let Some(ref prefix) = config.spec.secrets.prefix {
            if !prefix.is_empty() {
                if let Err(e) = Self::validate_secret_name_component(prefix, "secrets.prefix") {
                    return Err(anyhow::anyhow!(
                        "Invalid secrets.prefix '{}': {}",
                        prefix,
                        e
                    ));
                }
            }
        }

        if let Some(ref suffix) = config.spec.secrets.suffix {
            if !suffix.is_empty() {
                if let Err(e) = Self::validate_secret_name_component(suffix, "secrets.suffix") {
                    return Err(anyhow::anyhow!(
                        "Invalid secrets.suffix '{}': {}",
                        suffix,
                        e
                    ));
                }
            }
        }

        if let Some(ref base_path) = config.spec.secrets.base_path {
            if !base_path.is_empty() {
                if let Err(e) = Self::validate_path(base_path, "secrets.basePath") {
                    return Err(anyhow::anyhow!(
                        "Invalid secrets.basePath '{}': {}",
                        base_path,
                        e
                    ));
                }
            }
        }

        if let Some(ref kustomize_path) = config.spec.secrets.kustomize_path {
            if !kustomize_path.is_empty() {
                if let Err(e) = Self::validate_path(kustomize_path, "secrets.kustomizePath") {
                    return Err(anyhow::anyhow!(
                        "Invalid secrets.kustomizePath '{}': {}",
                        kustomize_path,
                        e
                    ));
                }
            }
        }

        // Validate provider configuration
        if let Err(e) = Self::validate_provider_config(&config.spec.provider) {
            return Err(anyhow::anyhow!("Invalid provider configuration: {}", e));
        }

        // Validate configs configuration if present
        if let Some(ref configs) = config.spec.configs {
            if let Err(e) = Self::validate_configs_config(configs) {
                return Err(anyhow::anyhow!("Invalid configs configuration: {}", e));
            }
        }

        // Boolean fields are validated by serde, but we ensure they're not None
        // diffDiscovery and triggerUpdate have defaults, so they're always present

        Ok(())
    }

    /// Validate sourceRef.kind
    /// Must be "GitRepository" or "Application" (case-sensitive)
    fn validate_source_ref_kind(kind: &str) -> Result<()> {
        let kind_trimmed = kind.trim();
        match kind_trimmed {
            "GitRepository" | "Application" => Ok(()),
            _ => Err(anyhow::anyhow!(
                "Must be 'GitRepository' or 'Application' (case-sensitive), got '{}'",
                kind_trimmed
            )),
        }
    }

    /// Validate Kubernetes resource name (RFC 1123 subdomain)
    /// Format: lowercase alphanumeric, hyphens, dots
    /// Length: 1-253 characters
    /// Cannot start or end with hyphen or dot
    fn validate_kubernetes_name(name: &str, field_name: &str) -> Result<()> {
        let name_trimmed = name.trim();

        if name_trimmed.is_empty() {
            return Err(anyhow::anyhow!("{} cannot be empty", field_name));
        }

        if name_trimmed.len() > 253 {
            return Err(anyhow::anyhow!(
                "{} '{}' exceeds maximum length of 253 characters (got {})",
                field_name,
                name_trimmed,
                name_trimmed.len()
            ));
        }

        // RFC 1123 subdomain: [a-z0-9]([-a-z0-9]*[a-z0-9])?(\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*
        // Simplified: lowercase alphanumeric, hyphens, dots; cannot start/end with hyphen or dot
        let name_regex =
            Regex::new(r"^[a-z0-9]([-a-z0-9]*[a-z0-9])?(\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$")
                .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))?;

        if !name_regex.is_match(name_trimmed) {
            return Err(anyhow::anyhow!(
                "{} '{}' must be a valid Kubernetes name (lowercase alphanumeric, hyphens, dots; cannot start/end with hyphen or dot)",
                field_name,
                name_trimmed
            ));
        }

        Ok(())
    }

    /// Validate Kubernetes namespace (RFC 1123 label)
    /// Format: lowercase alphanumeric, hyphens
    /// Length: 1-63 characters
    /// Cannot start or end with hyphen
    fn validate_kubernetes_namespace(namespace: &str) -> Result<()> {
        let namespace_trimmed = namespace.trim();

        if namespace_trimmed.is_empty() {
            return Err(anyhow::anyhow!("sourceRef.namespace cannot be empty"));
        }

        if namespace_trimmed.len() > 63 {
            return Err(anyhow::anyhow!(
                "sourceRef.namespace '{}' exceeds maximum length of 63 characters (got {})",
                namespace_trimmed,
                namespace_trimmed.len()
            ));
        }

        // RFC 1123 label: [a-z0-9]([-a-z0-9]*[a-z0-9])?
        let namespace_regex = Regex::new(r"^[a-z0-9]([-a-z0-9]*[a-z0-9])?$")
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))?;

        if !namespace_regex.is_match(namespace_trimmed) {
            return Err(anyhow::anyhow!(
                "sourceRef.namespace '{}' must be a valid Kubernetes namespace (lowercase alphanumeric, hyphens; cannot start/end with hyphen)",
                namespace_trimmed
            ));
        }

        Ok(())
    }

    /// Validate Kubernetes label value
    /// Format: lowercase alphanumeric, hyphens, dots, underscores
    /// Length: 1-63 characters
    /// Cannot start or end with dot
    fn validate_kubernetes_label(label: &str, field_name: &str) -> Result<()> {
        let label_trimmed = label.trim();

        if label_trimmed.is_empty() {
            return Err(anyhow::anyhow!("{} cannot be empty", field_name));
        }

        if label_trimmed.len() > 63 {
            return Err(anyhow::anyhow!(
                "{} '{}' exceeds maximum length of 63 characters (got {})",
                field_name,
                label_trimmed,
                label_trimmed.len()
            ));
        }

        // Kubernetes label: [a-z0-9]([-a-z0-9_.]*[a-z0-9])?
        let label_regex = Regex::new(r"^[a-z0-9]([-a-z0-9_.]*[a-z0-9])?$")
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))?;

        if !label_regex.is_match(label_trimmed) {
            return Err(anyhow::anyhow!(
                "{} '{}' must be a valid Kubernetes label (lowercase alphanumeric, hyphens, dots, underscores; cannot start/end with dot)",
                field_name,
                label_trimmed
            ));
        }

        Ok(())
    }

    /// Validate secret name component (prefix or suffix)
    /// Must be valid for cloud provider secret names
    /// Format: alphanumeric, hyphens, underscores
    /// Length: 1-255 characters
    fn validate_secret_name_component(component: &str, field_name: &str) -> Result<()> {
        let component_trimmed = component.trim();

        if component_trimmed.is_empty() {
            return Err(anyhow::anyhow!("{} cannot be empty", field_name));
        }

        if component_trimmed.len() > 255 {
            return Err(anyhow::anyhow!(
                "{} '{}' exceeds maximum length of 255 characters (got {})",
                field_name,
                component_trimmed,
                component_trimmed.len()
            ));
        }

        // Secret name component: alphanumeric, hyphens, underscores
        let secret_regex = Regex::new(r"^[a-zA-Z0-9_-]+$")
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))?;

        if !secret_regex.is_match(component_trimmed) {
            return Err(anyhow::anyhow!(
                "{} '{}' must contain only alphanumeric characters, hyphens, and underscores",
                field_name,
                component_trimmed
            ));
        }

        Ok(())
    }

    /// Validate file path
    /// Must be a valid relative or absolute path
    /// Cannot contain null bytes or invalid path characters
    fn validate_path(path: &str, field_name: &str) -> Result<()> {
        let path_trimmed = path.trim();

        if path_trimmed.is_empty() {
            return Err(anyhow::anyhow!("{} cannot be empty", field_name));
        }

        // Check for null bytes
        if path_trimmed.contains('\0') {
            return Err(anyhow::anyhow!(
                "{} '{}' cannot contain null bytes",
                field_name,
                path_trimmed
            ));
        }

        // Basic path validation: no control characters, reasonable length
        if path_trimmed.len() > 4096 {
            return Err(anyhow::anyhow!(
                "{} '{}' exceeds maximum length of 4096 characters (got {})",
                field_name,
                path_trimmed,
                path_trimmed.len()
            ));
        }

        // Check for invalid path patterns (Windows drive letters, etc.)
        // Allow relative paths (starting with .), absolute paths, and normal paths
        // Exclude: < > : " | ? * and control characters (\x00-\x1f)
        // Use a simpler validation: just check for null bytes and control characters
        // Paths can contain most characters except control chars
        for ch in path_trimmed.chars() {
            if ch.is_control() {
                return Err(anyhow::anyhow!(
                    "{} '{}' contains control characters",
                    field_name,
                    path_trimmed
                ));
            }
        }

        Ok(())
    }

    /// Validate provider configuration
    /// Uses official provider API constraints from:
    /// - GCP: https://cloud.google.com/resource-manager/docs/creating-managing-projects
    /// - AWS: https://docs.aws.amazon.com/general/latest/gr/rande.html
    /// - Azure: https://learn.microsoft.com/en-us/azure/key-vault/general/about-keys-secrets-certificates#vault-name
    fn validate_provider_config(provider: &ProviderConfig) -> Result<()> {
        match provider {
            ProviderConfig::Gcp(gcp) => {
                if gcp.project_id.is_empty() {
                    return Err(anyhow::anyhow!(
                        "provider.gcp.projectId is required but is empty"
                    ));
                }
                // GCP project ID validation per official GCP API constraints:
                // - Length: 6-30 characters
                // - Must start with a lowercase letter
                // - Cannot end with a hyphen
                // - Allowed: lowercase letters, numbers, hyphens
                // Reference: https://cloud.google.com/resource-manager/docs/creating-managing-projects
                let project_id_regex = Regex::new(r"^[a-z][a-z0-9-]{4,28}[a-z0-9]$")
                    .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))?;

                if !project_id_regex.is_match(&gcp.project_id) {
                    return Err(anyhow::anyhow!(
                        "provider.gcp.projectId '{}' must be a valid GCP project ID (6-30 characters, lowercase letters/numbers/hyphens, must start with letter, cannot end with hyphen). See: https://cloud.google.com/resource-manager/docs/creating-managing-projects",
                        gcp.project_id
                    ));
                }
            }
            ProviderConfig::Aws(aws) => {
                if aws.region.is_empty() {
                    return Err(anyhow::anyhow!(
                        "provider.aws.region is required but is empty"
                    ));
                }
                // AWS region validation per official AWS API constraints:
                // - Format: [a-z]{2}-[a-z]+-[0-9]+ (e.g., us-east-1, eu-west-1)
                // - Some regions include -gov or -iso segments (e.g., us-gov-west-1)
                // - Must match valid AWS region codes
                // Reference: https://docs.aws.amazon.com/general/latest/gr/rande.html
                Self::validate_aws_region(&aws.region)?;
            }
            ProviderConfig::Azure(azure) => {
                if azure.vault_name.is_empty() {
                    return Err(anyhow::anyhow!(
                        "provider.azure.vaultName is required but is empty"
                    ));
                }
                // Azure Key Vault name validation per official Azure API constraints:
                // - Length: 3-24 characters
                // - Must start with a letter
                // - Cannot end with a hyphen
                // - Allowed: alphanumeric characters and hyphens
                // - Hyphens cannot be consecutive
                // Reference: https://learn.microsoft.com/en-us/azure/key-vault/general/about-keys-secrets-certificates#vault-name
                let vault_name_regex = Regex::new(r"^[a-zA-Z][a-zA-Z0-9-]{1,22}[a-zA-Z0-9]$")
                    .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))?;

                if !vault_name_regex.is_match(&azure.vault_name) {
                    return Err(anyhow::anyhow!(
                        "provider.azure.vaultName '{}' must be a valid Azure Key Vault name (3-24 characters, alphanumeric/hyphens, must start with letter, cannot end with hyphen). See: https://learn.microsoft.com/en-us/azure/key-vault/general/about-keys-secrets-certificates#vault-name",
                        azure.vault_name
                    ));
                }

                // Check for consecutive hyphens
                if azure.vault_name.contains("--") {
                    return Err(anyhow::anyhow!(
                        "provider.azure.vaultName '{}' cannot contain consecutive hyphens",
                        azure.vault_name
                    ));
                }
            }
        }
        Ok(())
    }

    /// Validate AWS region against official AWS region format
    /// Supports standard regions (us-east-1) and special regions (us-gov-west-1, cn-north-1)
    /// Reference: https://docs.aws.amazon.com/general/latest/gr/rande.html
    fn validate_aws_region(region: &str) -> Result<()> {
        let region_trimmed = region.trim().to_lowercase();

        if region_trimmed.is_empty() {
            return Err(anyhow::anyhow!("provider.aws.region cannot be empty"));
        }

        // AWS region format patterns:
        // Standard: [a-z]{2}-[a-z]+-[0-9]+ (e.g., us-east-1, eu-west-1)
        // Gov: [a-z]{2}-gov-[a-z]+-[0-9]+ (e.g., us-gov-west-1)
        // ISO: [a-z]{2}-iso-[a-z]+-[0-9]+ (e.g., us-iso-east-1)
        // China: cn-[a-z]+-[0-9]+ (e.g., cn-north-1)
        // Local: local (for localstack)

        // Standard region pattern: [a-z]{2}-[a-z]+-[0-9]+
        let standard_pattern = Regex::new(r"^[a-z]{2}-[a-z]+-\d+$")
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))?;

        // Gov region pattern: [a-z]{2}-gov-[a-z]+-[0-9]+
        let gov_pattern = Regex::new(r"^[a-z]{2}-gov-[a-z]+-\d+$")
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))?;

        // ISO region pattern: [a-z]{2}-iso-[a-z]+-[0-9]+
        let iso_pattern = Regex::new(r"^[a-z]{2}-iso-[a-z]+-\d+$")
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))?;

        // China region pattern: cn-[a-z]+-[0-9]+
        let china_pattern = Regex::new(r"^cn-[a-z]+-\d+$")
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))?;

        // Local pattern (for localstack/testing)
        let local_pattern = Regex::new(r"^local$")
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))?;

        if standard_pattern.is_match(&region_trimmed)
            || gov_pattern.is_match(&region_trimmed)
            || iso_pattern.is_match(&region_trimmed)
            || china_pattern.is_match(&region_trimmed)
            || local_pattern.is_match(&region_trimmed)
        {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "provider.aws.region '{}' must be a valid AWS region code (e.g., 'us-east-1', 'eu-west-1', 'us-gov-west-1', 'cn-north-1'). See: https://docs.aws.amazon.com/general/latest/gr/rande.html",
                region
            ))
        }
    }

    /// Validate configs configuration
    fn validate_configs_config(configs: &crate::ConfigsConfig) -> Result<()> {
        // Validate store type if present
        // ConfigStoreType is an enum, so it's already validated by serde
        // No additional validation needed - enum variants are: SecretManager, ParameterManager
        if let Some(ref _store) = configs.store {
            // Enum is already validated by serde deserialization
            // ConfigStoreType::SecretManager or ConfigStoreType::ParameterManager are the only valid values
        }

        // Validate appConfigEndpoint if present
        if let Some(ref endpoint) = configs.app_config_endpoint {
            if !endpoint.is_empty() {
                if let Err(e) = Self::validate_url(endpoint, "configs.appConfigEndpoint") {
                    return Err(anyhow::anyhow!(
                        "Invalid configs.appConfigEndpoint '{}': {}",
                        endpoint,
                        e
                    ));
                }
            }
        }

        // Validate parameterPath if present
        if let Some(ref path) = configs.parameter_path {
            if !path.is_empty() {
                if let Err(e) = Self::validate_aws_parameter_path(path, "configs.parameterPath") {
                    return Err(anyhow::anyhow!(
                        "Invalid configs.parameterPath '{}': {}",
                        path,
                        e
                    ));
                }
            }
        }

        Ok(())
    }

    /// Validate URL format
    fn validate_url(url: &str, field_name: &str) -> Result<()> {
        let url_trimmed = url.trim();

        if url_trimmed.is_empty() {
            return Err(anyhow::anyhow!("{} cannot be empty", field_name));
        }

        // Basic URL validation: must start with http:// or https://
        let url_regex = Regex::new(r"^https?://[^\s/$.?#].[^\s]*$")
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))?;

        if !url_regex.is_match(url_trimmed) {
            return Err(anyhow::anyhow!(
                "{} '{}' must be a valid URL starting with http:// or https://",
                field_name,
                url_trimmed
            ));
        }

        Ok(())
    }

    /// Validate AWS Parameter Store path
    /// Format: /path/to/parameter (must start with /)
    fn validate_aws_parameter_path(path: &str, field_name: &str) -> Result<()> {
        let path_trimmed = path.trim();

        if path_trimmed.is_empty() {
            return Err(anyhow::anyhow!("{} cannot be empty", field_name));
        }

        if !path_trimmed.starts_with('/') {
            return Err(anyhow::anyhow!(
                "{} '{}' must start with '/' (e.g., '/my-service/dev')",
                field_name,
                path_trimmed
            ));
        }

        // AWS Parameter Store path: /[a-zA-Z0-9._-]+(/[a-zA-Z0-9._-]+)*
        let param_path_regex = Regex::new(r"^/[a-zA-Z0-9._-]+(/[a-zA-Z0-9._-]+)*$")
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))?;

        if !param_path_regex.is_match(path_trimmed) {
            return Err(anyhow::anyhow!(
                "{} '{}' must be a valid AWS Parameter Store path (e.g., '/my-service/dev')",
                field_name,
                path_trimmed
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod secret_name_tests {
        use super::*;

        #[test]
        fn test_construct_secret_name_with_prefix_and_suffix() {
            let result = construct_secret_name(Some("my-service"), "database-url", Some("-prod"));
            assert_eq!(result, "my-service-database-url-prod"); // Leading dash stripped from suffix
        }

        #[test]
        fn test_construct_secret_name_with_prefix_only() {
            let result = construct_secret_name(Some("my-service"), "database-url", None);
            assert_eq!(result, "my-service-database-url");
        }

        #[test]
        fn test_construct_secret_name_with_suffix_only() {
            let result = construct_secret_name(None, "database-url", Some("-prod"));
            assert_eq!(result, "database-url-prod"); // Leading dash stripped from suffix
        }

        #[test]
        fn test_construct_secret_name_no_prefix_no_suffix() {
            let result = construct_secret_name(None, "database-url", None);
            assert_eq!(result, "database-url");
        }

        #[test]
        fn test_construct_secret_name_empty_prefix() {
            let result = construct_secret_name(Some(""), "database-url", Some("-prod"));
            assert_eq!(result, "database-url-prod"); // Leading dash stripped from suffix
        }

        #[test]
        fn test_construct_secret_name_empty_suffix() {
            let result = construct_secret_name(Some("my-service"), "database-url", Some(""));
            assert_eq!(result, "my-service-database-url");
        }

        #[test]
        fn test_construct_secret_name_properties_key() {
            let result = construct_secret_name(Some("my-service"), "properties", Some("-prod"));
            assert_eq!(result, "my-service-properties-prod"); // Leading dash stripped from suffix
        }

        #[test]
        fn test_sanitize_secret_name_dots() {
            let result = sanitize_secret_name("my.service.database.url");
            assert_eq!(result, "my_service_database_url");
        }

        #[test]
        fn test_sanitize_secret_name_slashes() {
            let result = sanitize_secret_name("my/service/database/url");
            assert_eq!(result, "my_service_database_url");
        }

        #[test]
        fn test_sanitize_secret_name_spaces() {
            let result = sanitize_secret_name("my service database url");
            assert_eq!(result, "my_service_database_url");
        }

        #[test]
        fn test_sanitize_secret_name_mixed_invalid_chars() {
            let result = sanitize_secret_name("my.service/database url");
            assert_eq!(result, "my_service_database_url");
        }

        #[test]
        fn test_sanitize_secret_name_valid_chars() {
            let result = sanitize_secret_name("my-service_database-url123");
            assert_eq!(result, "my-service_database-url123");
        }

        #[test]
        fn test_sanitize_secret_name_special_chars() {
            let result = sanitize_secret_name("my@service#database$url");
            assert_eq!(result, "my_service_database_url");
        }

        #[test]
        fn test_construct_secret_name_with_sanitization() {
            // Test that construct_secret_name applies sanitization
            let result = construct_secret_name(Some("my.service"), "database/url", Some("-prod"));
            assert_eq!(result, "my_service-database_url-prod"); // Leading dash stripped, invalid chars sanitized
        }

        #[test]
        fn test_construct_secret_name_kustomize_compatibility() {
            // Test compatibility with kustomize-google-secret-manager naming
            let result = construct_secret_name(Some("idam-dev"), "database-url", Some("-prod"));
            assert_eq!(result, "idam-dev-database-url-prod"); // Leading dash stripped from suffix
        }

        #[test]
        fn test_construct_secret_name_edge_cases() {
            // Test edge cases
            assert_eq!(construct_secret_name(None, "", None), "");
            assert_eq!(
                construct_secret_name(Some("prefix"), "", Some("suffix")),
                "prefix-suffix"
            ); // Empty key becomes empty string after trim
            assert_eq!(construct_secret_name(Some("a"), "b", Some("c")), "a-b-c");
            assert_eq!(
                construct_secret_name(Some("prefix"), "key", Some("---suffix")),
                "prefix-key-suffix"
            ); // Multiple leading dashes stripped
        }

        #[test]
        fn test_sanitize_secret_name_edge_cases() {
            // Test edge cases
            assert_eq!(sanitize_secret_name(""), "");
            assert_eq!(sanitize_secret_name("a"), "a");
            assert_eq!(sanitize_secret_name("___"), "___");
            assert_eq!(sanitize_secret_name("---"), ""); // All dashes removed by trim
            assert_eq!(sanitize_secret_name("--test--"), "test"); // Leading/trailing dashes removed
            assert_eq!(sanitize_secret_name("a--b--c"), "a-b-c"); // Consecutive dashes collapsed
        }
    }
}
