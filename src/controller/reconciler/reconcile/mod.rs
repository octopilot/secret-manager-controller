//! # Reconciliation Logic
//!
//! Main reconciliation loop for SecretManagerConfig resources.

mod artifact_path;
mod finalize;
mod provider;
mod sync;

pub use artifact_path::{resolve_artifact_path, ArtifactPathResult};
pub use finalize::finalize_reconciliation;
pub use provider::create_provider;
pub use sync::{sync_secrets, SyncResult};

use crate::constants;
use crate::controller::reconciler::status::update_status_phase;
use crate::controller::reconciler::types::{Reconciler, ReconcilerError, TriggerSource};
use crate::controller::reconciler::validation::{
    validate_duration_interval, validate_secret_manager_config,
};
use crate::crd::{ProviderConfig, SecretManagerConfig};
use crate::observability;
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
        update_status_phase(&ctx, &config, "Started", Some("Reconciliation started")).await
    {
        warn!("Failed to update status to Started: {}", e);
    }

    // Set up notifications if configured
    // Note: FluxCD and ArgoCD notifications are configured independently:
    // - FluxCD shops only need fluxcd.providerRef (for GitRepository sources)
    // - ArgoCD shops only need argocd (for Application sources)
    // - Both can be configured if using both GitOps tools
    if let Some(notifications) = &config.spec.notifications {
        // FluxCD: Ensure Alert CRD exists for GitRepository sources
        // Only set up if fluxcd.providerRef is configured AND source is GitRepository
        if let Some(fluxcd_config) = &notifications.fluxcd {
            if config.spec.source_ref.kind == "GitRepository" {
                if let Err(e) = crate::controller::reconciler::notifications::ensure_fluxcd_alert(
                    &ctx,
                    &config,
                    &fluxcd_config.provider_ref,
                )
                .await
                {
                    warn!("Failed to set up FluxCD notification alert: {}", e);
                    // Don't fail reconciliation if notification setup fails
                }
            } else {
                // fluxcd configured but source is not GitRepository - skip silently
                // This allows users to have the same config template for both GitOps tools
                debug!(
                    "Skipping FluxCD notification setup - fluxcd configured but source is {} (not GitRepository)",
                    config.spec.source_ref.kind
                );
            }
        }

        // ArgoCD: Add annotations to Application resource
        // Only set up if argocd is configured AND source is Application
        if let Some(argocd_config) = &notifications.argocd {
            if config.spec.source_ref.kind == "Application" {
                if let Err(e) =
                    crate::controller::reconciler::notifications::send_argocd_notification(
                        &ctx,
                        &config.spec.source_ref,
                        &argocd_config.subscriptions,
                    )
                    .await
                {
                    warn!("Failed to set up ArgoCD notification annotations: {}", e);
                    // Don't fail reconciliation if notification setup fails
                }
            } else {
                // argocd configured but source is not Application - skip silently
                // This allows users to have the same config template for both GitOps tools
                debug!(
                    "Skipping ArgoCD notification setup - argocd configured but source is {} (not Application)",
                    config.spec.source_ref.kind
                );
            }
        }
    }

    // Validate and log SecretManagerConfig resource first
    debug!(
        "Reconciling SecretManagerConfig: {} in namespace: {}",
        name,
        config.metadata.namespace.as_deref().unwrap_or("default")
    );

    // Resolve artifact path
    let artifact_path = match resolve_artifact_path(&config, &ctx).await {
        Ok(ArtifactPathResult::Path(path)) => path,
        Ok(ArtifactPathResult::AwaitChange) => {
            // Need to wait for resource - return await_change
            return Ok(Action::await_change());
        }
        Ok(ArtifactPathResult::Error(e)) => return Err(e),
        Err(e) => return Err(e),
    };

    // Create provider client
    let provider = create_provider(&config, &ctx).await?;

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

    // Sync secrets
    let secrets_synced = match sync_secrets(&config, &ctx, &*provider, &artifact_path).await {
        Ok(SyncResult::Success(count)) => count,
        Ok(SyncResult::TransientError) => {
            // Transient error - retry after delay
            return Ok(Action::requeue(std::time::Duration::from_secs(30)));
        }
        Ok(SyncResult::Error(e)) => return Err(e),
        Err(e) => return Err(e),
    };

    // Finalize reconciliation
    finalize_reconciliation(&config, &ctx, start, secrets_synced, is_manual_trigger).await
}
