//! # Reconciliation Logic
//!
//! Main reconciliation loop for SecretManagerConfig resources.

use crate::constants;
use crate::controller::parser;
use crate::controller::reconciler::artifact::{
    get_argocd_artifact_path, get_flux_artifact_path, get_flux_git_repository,
};
use crate::controller::reconciler::processing::{
    process_application_files, process_kustomize_secrets,
};
use crate::controller::reconciler::source::suspend_git_repository;
use crate::controller::reconciler::status::{
    calculate_progressive_backoff, clear_manual_trigger_annotation, clear_parsing_error_count,
    get_parsing_error_count, increment_parsing_error_count, update_status, update_status_phase,
};
use crate::controller::reconciler::types::{Reconciler, ReconcilerError, TriggerSource};
use crate::controller::reconciler::validation::{
    parse_kubernetes_duration, validate_duration_interval, validate_secret_manager_config,
};
use crate::observability;
use crate::provider::aws::AwsSecretManager;
use crate::provider::azure::AzureKeyVault;
use crate::provider::gcp::create_gcp_provider;
use crate::provider::SecretManagerProvider;
use crate::{ProviderConfig, SecretManagerConfig};
use anyhow::Context;
use kube_runtime::controller::Action;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// Main reconciliation function
/// Reconcile internal logic - errors are handled by error_policy() in main.rs
/// This separation prevents blocking watch/timer paths when many resources fail
/// Backoff logic is now in error_policy() layer as recommended by kube-rs best practices
pub async fn reconcile(
    config: Arc<SecretManagerConfig>,
    ctx: Arc<Reconciler>,
    trigger_source: TriggerSource,
) -> Result<Action, ReconcilerError> {
    reconcile_internal(config, ctx, trigger_source).await
}

#[allow(
    clippy::too_many_lines,
    clippy::missing_errors_doc,
    reason = "Reconciliation logic is complex and error docs are in comments"
)]
async fn reconcile_internal(
    config: Arc<SecretManagerConfig>,
    ctx: Arc<Reconciler>,
    trigger_source: TriggerSource,
) -> Result<Action, ReconcilerError> {
    let start = Instant::now();
    let name = config.metadata.name.as_deref().unwrap_or("unknown");

    // Log trigger source at start of reconciliation
    info!(
        "🔄 Reconciling SecretManagerConfig: {} (trigger source: {})",
        name,
        trigger_source.as_str()
    );

    // Create OpenTelemetry span for this reconciliation
    // This provides distributed tracing when Datadog/OTel is configured
    // The span will automatically be exported to Datadog if configured
    let provider_type = match &config.spec.provider {
        ProviderConfig::Gcp(_) => "gcp",
        ProviderConfig::Aws(_) => "aws",
        ProviderConfig::Azure(_) => "azure",
    };
    let span = tracing::span!(
        tracing::Level::INFO,
        "reconcile",
        resource.name = name,
        resource.namespace = config.metadata.namespace.as_deref().unwrap_or("default"),
        resource.kind = "SecretManagerConfig",
        resource.provider = provider_type
    );
    let _guard = span.enter();

    // Comprehensive validation of all CRD fields
    if let Err(e) = validate_secret_manager_config(&config) {
        error!("Validation error for {}: {}", name, e);
        // Update status to Failed with validation error
        let _ = update_status_phase(
            &ctx,
            &config,
            "Failed",
            Some(&format!("Validation failed: {e}")),
        )
        .await;
        return Err(ReconcilerError::ReconciliationFailed(e));
    }

    // Validate GitRepository pull interval - must be at least 1 minute to avoid rate limits
    if let Err(e) = validate_duration_interval(
        &config.spec.git_repository_pull_interval,
        "gitRepositoryPullInterval",
        constants::MIN_GITREPOSITORY_PULL_INTERVAL_SECS,
    ) {
        let err = anyhow::anyhow!(
            "Invalid gitRepositoryPullInterval '{}': {}",
            config.spec.git_repository_pull_interval,
            e
        );
        error!("Validation error for {}: {}", name, err);
        // Update status to Failed with validation error
        let _ = update_status_phase(
            &ctx,
            &config,
            "Failed",
            Some(&format!("Invalid gitRepositoryPullInterval: {e}")),
        )
        .await;
        return Err(ReconcilerError::ReconciliationFailed(err));
    }

    // Validate reconcile interval - must be at least 1 minute to avoid rate limits
    if let Err(e) = validate_duration_interval(
        &config.spec.reconcile_interval,
        "reconcileInterval",
        constants::MIN_RECONCILE_INTERVAL_SECS,
    ) {
        let err = anyhow::anyhow!(
            "Invalid reconcileInterval '{}': {}",
            config.spec.reconcile_interval,
            e
        );
        error!("Validation error for {}: {}", name, err);
        // Update status to Failed with validation error
        let _ = update_status_phase(
            &ctx,
            &config,
            "Failed",
            Some(&format!("Invalid reconcileInterval: {e}")),
        )
        .await;
        return Err(ReconcilerError::ReconciliationFailed(err));
    }

    // Check if reconciliation is suspended
    // Suspended resources skip reconciliation entirely, even for manual triggers
    // This is useful for troubleshooting or during intricate CI/CD transitions
    if config.spec.suspend {
        info!(
            "Reconciliation suspended for SecretManagerConfig: {} - skipping reconciliation",
            name
        );
        // Update status to indicate suspended state
        if let Err(e) = update_status_phase(
            &ctx,
            &config,
            "Suspended",
            Some("Reconciliation is suspended - no secrets will be synced"),
        )
        .await
        {
            warn!("Failed to update status to Suspended: {}", e);
        }
        // Return Action::await_change() to wait for suspend to be cleared
        // This ensures we don't reconcile until suspend is set to false
        return Ok(Action::await_change());
    }

    // Check if this is a manual reconciliation trigger (via annotation)
    let is_manual_trigger = config
        .metadata
        .annotations
        .as_ref()
        .and_then(|ann| ann.get("secret-management.microscaler.io/reconcile"))
        .is_some();

    // Determine trigger source for logging
    let trigger_source_str = if is_manual_trigger {
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
        name, trigger_source_str
    );

    observability::metrics::increment_reconciliations();

    // Update status to Started
    if let Err(e) =
        update_status_phase(&ctx, &config, "Started", Some("Starting reconciliation")).await
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
        config.spec.source_ref.kind, config.spec.source_ref.name, config.spec.source_ref.namespace
    );

    // Determine artifact path based on source type (GitRepository vs Application)
    // This path points to the cloned/checked-out repository directory containing secrets
    let artifact_path = match config.spec.source_ref.kind.as_str() {
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
                if let Err(e) = suspend_git_repository(&ctx, &config.spec.source_ref, true).await {
                    warn!("Failed to suspend GitRepository: {}", e);
                    // Continue anyway - GitRepository might already be suspended
                }
            } else {
                // Ensure GitRepository is not suspended if suspendGitPulls is false
                if let Err(e) = suspend_git_repository(&ctx, &config.spec.source_ref, false).await {
                    warn!("Failed to resume GitRepository: {}", e);
                    // Continue anyway - GitRepository might already be active
                }
            }

            // Update status to Cloning - indicates we're fetching the GitRepository
            if let Err(e) = update_status_phase(
                &ctx,
                &config,
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

            let git_repo = match get_flux_git_repository(&ctx, &config.spec.source_ref).await {
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
                            &ctx,
                            &config,
                            "Pending",
                            Some("GitRepository not found, waiting for creation"),
                        )
                        .await;
                        // Return await_change() to wait for watch event instead of blocking timer loop
                        // This prevents the kube-rs controller deadlock where timer-based reconcilers
                        // stop firing after hitting a requeue in an error branch.
                        //
                        // How reconciliation resumes:
                        // 1. Periodic timer-based reconciliation continues to work - the controller's
                        //    timer mechanism will trigger reconciliation based on reconcile_interval
                        //    even when Action::await_change() is returned, allowing periodic checks
                        //    for the GitRepository to appear.
                        // 2. When FluxCD creates the GitRepository, it updates the GitRepository's
                        //    status field. While the controller watches SecretManagerConfig (not
                        //    GitRepository), periodic reconciliation will detect the GitRepository
                        //    on the next scheduled check.
                        // 3. Manual reconciliation triggers (via annotation) will also work,
                        //    allowing immediate retry when the GitRepository is created.
                        //
                        // This approach ensures timer-based reconciliation continues working for
                        // all resources, preventing the deadlock while still allowing periodic
                        // checks for missing dependencies.
                        return Ok(Action::await_change());
                    }

                    // For other errors, log and fail
                    error!(
                        "❌ Failed to get FluxCD GitRepository: {}/{} - {}",
                        config.spec.source_ref.namespace, config.spec.source_ref.name, e
                    );
                    observability::metrics::increment_reconciliation_errors();
                    // Update status to Failed
                    let _ = update_status_phase(
                        &ctx,
                        &config,
                        "Failed",
                        Some(&format!("Clone failed, repo unavailable: {e}")),
                    )
                    .await;
                    return Err(ReconcilerError::ReconciliationFailed(e));
                }
            };

            // Extract artifact path from GitRepository status
            // Downloads and extracts tar.gz artifact from FluxCD source-controller
            // Returns path to extracted directory
            match get_flux_artifact_path(&ctx, &git_repo).await {
                Ok(path) => {
                    info!(
                        "Found FluxCD artifact path: {} for GitRepository: {}",
                        path.display(),
                        config.spec.source_ref.name
                    );
                    path
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
                                &ctx,
                                &config,
                                "Pending",
                                Some("GitRepository is reconciling, waiting for artifact"),
                            )
                            .await;
                            // Wait for watch event - GitRepository status updates will trigger reconciliation
                            return Ok(Action::await_change());
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
                                &ctx,
                                &config,
                                "Failed",
                                Some(&format!("GitRepository not ready: {}", reason)),
                            )
                            .await;
                            return Err(ReconcilerError::ReconciliationFailed(anyhow::anyhow!(
                                "GitRepository not ready: {}",
                                reason
                            )));
                        }
                    }

                    // GitRepository is ready but artifact path extraction failed
                    // This is unexpected - log error and fail
                    error!("Failed to get FluxCD artifact path: {}", e);
                    observability::metrics::increment_reconciliation_errors();
                    // Update status to Failed
                    let _ = update_status_phase(
                        &ctx,
                        &config,
                        "Failed",
                        Some(&format!("Failed to get artifact path: {e}")),
                    )
                    .await;
                    return Err(ReconcilerError::ReconciliationFailed(e));
                }
            }
        }
        "Application" => {
            // ArgoCD Application: Clone repository directly
            // Unlike FluxCD, ArgoCD doesn't expose artifact paths, so we clone ourselves
            // This supports both GitRepository and Helm sources
            match get_argocd_artifact_path(&ctx, &config.spec.source_ref).await {
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

    // Create provider client based on provider configuration
    // Each provider has different authentication methods:
    // - GCP: Workload Identity (default)
    // - AWS: IRSA - IAM Roles for Service Accounts (default)
    // - Azure: Workload Identity or Managed Identity (default)
    // Provider is created per-reconciliation to support per-resource auth config
    let provider: Box<dyn SecretManagerProvider> = match &config.spec.provider {
        ProviderConfig::Gcp(gcp_config) => {
            // GCP Secret Manager provider
            // Validate required GCP configuration
            if gcp_config.project_id.is_empty() {
                let err = anyhow::anyhow!("GCP projectId is required but is empty");
                error!("Validation error for {}: {}", name, err);
                return Err(ReconcilerError::ReconciliationFailed(err));
            }

            // Determine authentication method from config
            // Default to Workload Identity when auth is not specified
            // Workload Identity requires GKE with WI enabled and service account annotation
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
                                Some(email) => (Some("WorkloadIdentity"), Some(email.to_string())),
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
            match create_gcp_provider(
                gcp_config.project_id.clone(),
                auth_type,
                service_account_email,
            )
            .await
            {
                Ok(gcp_client) => gcp_client,
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

    // Determine sync mode: secrets vs configs (properties)
    // Configs are stored in config stores (Parameter Store, App Configuration)
    // Secrets are stored in secret stores (Secret Manager, Key Vault)
    let is_configs_enabled = config
        .spec
        .configs
        .as_ref()
        .map(|c| c.enabled)
        .unwrap_or(false);

    // Generate status description based on what we're syncing
    // This helps users understand what the controller is doing
    let description = if is_configs_enabled {
        // Syncing to config stores (non-secret configuration values)
        match &config.spec.provider {
            ProviderConfig::Gcp(_) => "Reconciling properties to Parameter Manager",
            ProviderConfig::Aws(_) => "Reconciling properties to Parameter Store",
            ProviderConfig::Azure(_) => "Reconciling properties to App Configuration",
        }
    } else {
        // Syncing to secret stores (sensitive values)
        match &config.spec.provider {
            ProviderConfig::Gcp(_) => "Reconciling secrets to Secret Manager",
            ProviderConfig::Aws(_) => "Reconciling secrets to Secrets Manager",
            ProviderConfig::Azure(_) => "Reconciling secrets to Key Vault",
        }
    };

    // Update status to Updating - indicates we're actively syncing secrets/configs
    if let Err(e) = update_status_phase(&ctx, &config, "Updating", Some(description)).await {
        warn!("Failed to update status to Updating: {}", e);
    }

    let mut secrets_synced = 0;

    // Determine processing mode: kustomize build vs raw file parsing
    // Kustomize mode: Extract secrets from kustomize-generated Secret resources
    // Raw file mode: Parse application.secrets.env files directly
    if let Some(kustomize_path) = &config.spec.secrets.kustomize_path {
        // Kustomize Build Mode
        // Runs `kustomize build` to generate Kubernetes manifests, then extracts Secret resources
        // Supports overlays, patches, generators, and other kustomize features
        // This is the recommended mode for complex deployments with multiple environments
        info!("Using kustomize build mode on path: {}", kustomize_path);

        match crate::controller::kustomize::extract_secrets_from_kustomize(
            &artifact_path,
            kustomize_path,
        ) {
            Ok(secrets) => {
                let secret_prefix = config.spec.secrets.prefix.as_deref().unwrap_or("default");
                match process_kustomize_secrets(&*provider, &config, &secrets, secret_prefix).await
                {
                    Ok(count) => {
                        secrets_synced += count;
                        info!("✅ Synced {} secrets from kustomize build", count);
                    }
                    Err(e) => {
                        error!("Failed to process kustomize secrets: {}", e);
                        observability::metrics::increment_reconciliation_errors();
                        // Update status to Failed
                        let _ = update_status_phase(
                            &ctx,
                            &config,
                            "Failed",
                            Some(&format!("Failed to process kustomize secrets: {e}")),
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
                let _ = update_status_phase(
                    &ctx,
                    &config,
                    "Failed",
                    Some(&format!("Failed to extract secrets from kustomize: {e}")),
                )
                .await;
                return Err(ReconcilerError::ReconciliationFailed(e));
            }
        }
    } else {
        // Raw File Mode
        // Directly parses application.secrets.env, application.secrets.yaml, and application.properties files
        // Simpler than kustomize mode but doesn't support overlays or generators
        // Suitable for simple deployments or when kustomize isn't needed
        info!("Using raw file mode");

        // Find application files for the specified environment
        // Searches for files matching patterns like:
        // - {basePath}/profiles/{environment}/application.secrets.env
        // - {basePath}/{service}/profiles/{environment}/application.secrets.env
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
                let _ = update_status_phase(
                    &ctx,
                    &config,
                    "Failed",
                    Some(&format!("Failed to find application files: {e}")),
                )
                .await;
                return Err(ReconcilerError::ReconciliationFailed(e));
            }
        };

        info!(
            "📋 Found {} application file set(s) to process",
            application_files.len()
        );

        // Process each application file set
        for app_files in application_files {
            match process_application_files(&ctx, &*provider, &config, &app_files).await {
                Ok(count) => {
                    secrets_synced += count;
                    info!(
                        "✅ Synced {} secrets for service: {}",
                        count, app_files.service_name
                    );
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    // Check if this is a transient SOPS decryption error
                    let is_transient = error_msg.contains("transient");

                    if is_transient {
                        // Transient error - log warning and return action to retry
                        warn!(
                            "⏳ Transient error processing service {}: {}. Will retry.",
                            app_files.service_name, error_msg
                        );
                        observability::metrics::increment_reconciliation_errors();
                        // Update status to indicate retry
                        let _ = update_status_phase(
                            &ctx,
                            &config,
                            "Retrying",
                            Some(&format!("Transient error: {}. Retrying...", error_msg)),
                        )
                        .await;
                        // Return action to retry after a delay
                        return Ok(Action::requeue(std::time::Duration::from_secs(30)));
                    } else {
                        // Permanent error - log error and continue with other services
                        // This allows partial success when multiple services are configured
                        error!(
                            "❌ Permanent error processing service {}: {}",
                            app_files.service_name, error_msg
                        );
                        observability::metrics::increment_reconciliation_errors();
                        // Update status to indicate failure for this service
                        let _ = update_status_phase(
                            &ctx,
                            &config,
                            "PartialFailure",
                            Some(&format!(
                                "Failed to process service {}: {}",
                                app_files.service_name, error_msg
                            )),
                        )
                        .await;
                    }
                }
            }
        }
    }

    // Update status
    if let Err(e) = update_status(&ctx, &config, secrets_synced).await {
        error!("Failed to update status: {}", e);
        observability::metrics::increment_reconciliation_errors();
        return Err(ReconcilerError::ReconciliationFailed(e));
    }

    // Clear manual trigger annotation if present (msmctl reconcile)
    // This prevents the annotation from triggering repeated reconciliations
    if is_manual_trigger {
        if let Err(e) = clear_manual_trigger_annotation(&ctx, &config).await {
            warn!("Failed to clear manual trigger annotation: {}", e);
            // Don't fail reconciliation if annotation clearing fails
        } else {
            debug!("Cleared manual trigger annotation after successful reconciliation");
        }
    }

    // Update metrics
    observability::metrics::observe_reconciliation_duration(start.elapsed().as_secs_f64());
    observability::metrics::set_secrets_managed(i64::from(secrets_synced));

    // Success - reset backoff state for this resource
    // On successful reconciliation, reset the backoff timer to use the resource's reconcile_interval
    // This ensures that after a successful reconciliation (even during backoff), we return to
    // the normal schedule defined in the resource spec
    // Note: Backoff state is managed in error_policy() layer, but we reset it here on success
    // to ensure clean state for next reconciliation
    let resource_key = format!(
        "{}/{}",
        config.metadata.namespace.as_deref().unwrap_or("default"),
        name
    );
    let was_in_backoff = if let Ok(mut states) = ctx.backoff_states.lock() {
        if let Some(state) = states.get_mut(&resource_key) {
            let had_errors = state.error_count > 0;
            state.reset();
            had_errors
        } else {
            false
        }
    } else {
        false
    };

    info!(
        "✅ Reconciliation complete for {} (synced {} secrets, duration: {:.2}s)",
        name,
        secrets_synced,
        start.elapsed().as_secs_f64()
    );

    // Use reconcile_interval from CRD spec for successful reconciliations
    // Each resource has its own reconcileInterval and maintains its own error count
    // Parse the reconcile interval and requeue after that duration
    // This ensures we don't reconcile more frequently than specified per resource
    match parse_kubernetes_duration(&config.spec.reconcile_interval) {
        Ok(duration) => {
            // Successfully parsed - use the specified interval for THIS resource
            // Reset any parsing error count by clearing the annotation if it exists
            // This resets backoff when parsing succeeds again for this specific resource
            let _ = clear_parsing_error_count(&ctx, &config).await;

            let next_trigger_time = chrono::Utc::now()
                + chrono::Duration::from_std(duration).unwrap_or(chrono::Duration::seconds(60));

            if was_in_backoff {
                info!(
                    "🔄 Backoff reset: Returning to normal schedule using reconcileInterval '{}'",
                    config.spec.reconcile_interval
                );
            }

            info!(
                "📅 Next scheduled reconciliation: {} (in {}s, trigger source: timer-based)",
                next_trigger_time.to_rfc3339(),
                duration.as_secs()
            );

            debug!(
                "Requeueing reconciliation for {} after {}s (reconcileInterval: {})",
                name,
                duration.as_secs(),
                config.spec.reconcile_interval
            );
            observability::metrics::increment_requeues_total("timer-based");
            Ok(Action::requeue(duration))
        }
        Err(e) => {
            // Parsing failed for THIS resource - use Fibonacci-based progressive backoff
            // Each resource tracks its own error count independently via annotations
            // Track parsing errors in metrics and use Fibonacci backoff (1m -> 1m -> 2m -> 3m -> 5m -> ... -> 60m max)
            observability::metrics::increment_duration_parsing_errors();

            // Get current parsing error count for THIS resource from its annotations
            // Each resource maintains its own error count, so resources don't affect each other
            let error_count = get_parsing_error_count(&config);
            let backoff_duration = calculate_progressive_backoff(error_count);

            // Update error count in THIS resource's annotations for next reconciliation
            // This persists the error count across controller restarts, per resource
            let _ = increment_parsing_error_count(&ctx, &config, error_count).await;

            error!(
                "Failed to parse reconcileInterval '{}' for resource {}: {}. Using Fibonacci backoff: {}s (error count: {})",
                config.spec.reconcile_interval, name, e, backoff_duration.as_secs(), error_count + 1
            );

            observability::metrics::increment_requeues_total("duration-parsing-error");
            Ok(Action::requeue(backoff_duration))
        }
    }
}
