//! # Reconciliation Finalization
//!
//! Handles final status updates, metrics, and requeue logic after secret syncing.

use crate::controller::reconciler::status::{
    calculate_progressive_backoff, clear_manual_trigger_annotation, clear_parsing_error_count,
    get_parsing_error_count, increment_parsing_error_count, update_status,
};
use crate::controller::reconciler::types::{Reconciler, ReconcilerError};
use crate::controller::reconciler::validation::parse_kubernetes_duration;
use crate::crd::{ResourceSyncState, SecretManagerConfig};
use crate::observability;
use kube_runtime::controller::Action;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// Finalize reconciliation: update status, metrics, and determine next action
pub async fn finalize_reconciliation(
    config: &Arc<SecretManagerConfig>,
    ctx: &Arc<Reconciler>,
    start: Instant,
    secrets_synced: u32,
    is_manual_trigger: bool,
    synced_secrets: &std::collections::HashMap<String, ResourceSyncState>,
    synced_properties: &std::collections::HashMap<String, ResourceSyncState>,
) -> Result<Action, ReconcilerError> {
    let name = config.metadata.name.as_deref().unwrap_or("unknown");

    // Update status (includes synced_secrets and synced_properties push state tracking)
    if let Err(e) = update_status(
        ctx,
        config,
        secrets_synced as i32,
        synced_secrets,
        synced_properties,
    )
    .await
    {
        error!("Failed to update status: {}", e);
        observability::metrics::increment_reconciliation_errors();
        return Err(ReconcilerError::ReconciliationFailed(e));
    }

    // Clear manual trigger annotation if present (msmctl reconcile)
    // This prevents the annotation from triggering repeated reconciliations
    if is_manual_trigger {
        if let Err(e) = clear_manual_trigger_annotation(ctx, config).await {
            warn!("Failed to clear manual trigger annotation: {}", e);
            // Don't fail reconciliation if annotation clearing fails
        } else {
            debug!("Cleared manual trigger annotation after successful reconciliation");
        }
    }

    // Update metrics
    observability::metrics::observe_reconciliation_duration(start.elapsed().as_secs_f64());
    observability::metrics::set_secrets_managed(secrets_synced as i64);

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
            let _ = clear_parsing_error_count(ctx, config).await;

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
            let error_count = get_parsing_error_count(config);
            let backoff_duration = calculate_progressive_backoff(error_count);

            // Update error count in THIS resource's annotations for next reconciliation
            // This persists the error count across controller restarts, per resource
            let _ = increment_parsing_error_count(ctx, config, error_count).await;

            error!(
                "Failed to parse reconcileInterval '{}' for resource {}: {}. Using Fibonacci backoff: {}s (error count: {})",
                config.spec.reconcile_interval,
                name,
                e,
                backoff_duration.as_secs(),
                error_count + 1
            );

            observability::metrics::increment_requeues_total("duration-parsing-error");
            Ok(Action::requeue(backoff_duration))
        }
    }
}
