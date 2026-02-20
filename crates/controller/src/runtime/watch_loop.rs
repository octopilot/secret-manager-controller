//! # Watch Loop
//!
//! Controller watch loop that monitors SecretManagerConfig resources and triggers
//! reconciliation when changes are detected.

use crate::config::SharedControllerConfig;
use crate::controller::reconciler::{Reconciler, TriggerSource, reconcile};
use crate::controller::server::ServerState;
use crate::crd::SecretManagerConfig;
use crate::runtime::error_policy::{handle_reconciliation_error, handle_watch_stream_error};
use futures::StreamExt;
use kube::api::Api;
use kube_runtime::{Controller, controller::Action, watcher};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Run the controller watch loop
///
/// This function sets up the Kubernetes controller to watch SecretManagerConfig
/// resources and trigger reconciliation on changes. It handles graceful shutdown
/// and automatic restart on watch stream errors.
pub async fn run_watch_loop(
    configs: Api<SecretManagerConfig>,
    reconciler: Arc<Reconciler>,
    server_state: Arc<ServerState>,
    controller_config: SharedControllerConfig,
) -> Result<(), anyhow::Error> {
    // Create controller with any_semantic() to watch for all semantic changes (create, update, delete)
    // This ensures the controller picks up newly created resources
    info!("Starting controller watch loop...");

    // Set up graceful shutdown handler - mark server as not ready when shutting down
    let server_state_shutdown = server_state.clone();

    // Load configuration (hot-reloadable)
    let config = controller_config.read().await;
    let backoff_start_ms = config.backoff_start_ms;
    // Note: max_backoff_ms, watch_restart_delay_secs, and watch_restart_delay_after_end_secs
    // are reloaded inside their respective closures/loops to ensure we use the latest config values
    drop(config); // Release lock before async operations

    // Use Arc for shared backoff state
    let backoff_duration_ms = Arc::new(std::sync::atomic::AtomicU64::new(backoff_start_ms));

    // Set up shutdown signal handler - mark server as not ready when SIGTERM/SIGINT received
    // Note: SIGHUP handling removed - Tilt uses restart_container() which sends SIGTERM
    // SIGHUP can cause issues in some environments, so we only handle standard shutdown signals
    let shutdown_server_state = server_state_shutdown.clone();
    tokio::spawn(async move {
        // Handle SIGTERM/SIGINT (standard shutdown signals)
        // These are sent by Kubernetes (SIGTERM) and manual interruption (SIGINT)
        let _ = tokio::signal::ctrl_c().await;
        info!("Received shutdown signal (SIGINT/SIGTERM), initiating graceful shutdown...");

        shutdown_server_state
            .is_ready
            .store(false, std::sync::atomic::Ordering::Relaxed);
        info!("Marked server as not ready, waiting for in-flight reconciliations to complete...");
    });

    // Run controller with improved error handling and automatic restart
    loop {
        // Check if we should shut down before starting/restarting watch
        if !server_state_shutdown
            .is_ready
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            info!("Shutdown requested, exiting watch loop");
            break;
        }

        let backoff_clone = backoff_duration_ms.clone();
        let controller_config_for_reconcile = controller_config.clone();
        let controller_config_for_filter = controller_config.clone();
        let watch_span = tracing::span!(
            tracing::Level::INFO,
            "controller.watch",
            operation = "watch_loop"
        );
        let _watch_guard = watch_span.enter();

        info!("Starting controller watch loop...");
        let controller_future =
            Controller::new(configs.clone(), watcher::Config::default().any_semantic())
                .shutdown_on_signal()
                .run(
                    |obj, ctx| {
                        create_reconcile_fn(obj, ctx, controller_config_for_reconcile.clone())
                    },
                    |obj, error, ctx| handle_reconciliation_error(obj, error, ctx),
                    reconciler.clone(),
                )
                .filter_map(move |x| {
                    let backoff = backoff_clone.clone();
                    let config_clone = controller_config_for_filter.clone();
                    async move {
                        match &x {
                            Ok(_) => {
                                // Successful event, reset backoff on success
                                // Reload config in case it changed
                                let config = config_clone.read().await;
                                let backoff_start = config.backoff_start_ms;
                                drop(config);
                                backoff.store(backoff_start, std::sync::atomic::Ordering::Relaxed);
                                debug!("watch.event.success");
                                Some(x)
                            }
                            Err(e) => {
                                // Convert the controller error to a string for classification
                                let error_string = format!("{e:?}");
                                // Reload config in case it changed
                                let config = config_clone.read().await;
                                let max_backoff = config.backoff_max_ms;
                                let watch_restart_delay = config.watch_restart_delay_secs;
                                drop(config);
                                match handle_watch_stream_error(
                                    &error_string,
                                    &backoff,
                                    max_backoff,
                                    watch_restart_delay,
                                )
                                .await
                                {
                                    Some(_) => Some(x), // Continue with this event
                                    None => None,       // Filter out to allow restart
                                }
                            }
                        }
                    }
                })
                .for_each(|_| futures::future::ready(()));

        // Run controller - check for shutdown before and after
        controller_future.await;

        // Check if shutdown was requested
        if !server_state_shutdown
            .is_ready
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            info!("Shutdown requested, exiting watch loop");
            break;
        }

        // Controller stream ended - restart watch
        // Reload config in case it changed
        let config = controller_config.read().await;
        let delay_secs = config.watch_restart_delay_after_end_secs;
        drop(config);
        warn!(
            "Controller watch stream ended, restarting in {} seconds...",
            delay_secs
        );
        tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
    }

    info!("Controller stopped gracefully");
    Ok(())
}

/// Create the reconciliation function for the controller
fn create_reconcile_fn(
    obj: Arc<SecretManagerConfig>,
    ctx: Arc<Reconciler>,
    controller_config: SharedControllerConfig,
) -> impl std::future::Future<Output = Result<Action, crate::controller::reconciler::ReconcilerError>>
+ Send {
    let reconciler = ctx.clone();
    let controller_config_for_reconcile = controller_config.clone();
    let name = obj
        .metadata
        .name
        .as_deref()
        .unwrap_or("unknown")
        .to_string();
    let namespace = obj
        .metadata
        .namespace
        .as_deref()
        .unwrap_or("default")
        .to_string();
    let resource_version = obj
        .metadata
        .resource_version
        .as_deref()
        .unwrap_or("unknown")
        .to_string();
    let generation = obj.metadata.generation.unwrap_or(0);
    let observed_generation = obj
        .status
        .as_ref()
        .and_then(|s| s.observed_generation)
        .unwrap_or(0);

    // Create span for each reconciliation triggered by watch
    let reconcile_span = tracing::span!(
        tracing::Level::INFO,
        "controller.watch.reconcile",
        resource.name = name.as_str(),
        resource.namespace = namespace.as_str(),
        resource.version = resource_version.as_str(),
        resource.generation = generation,
        resource.observed_generation = observed_generation,
        event.r#type = "watch_triggered"
    );
    let _reconcile_guard = reconcile_span.enter();

    async move {
        // CRITICAL: Check if reconciliation is suspended BEFORE any other checks
        // Suspended resources skip reconciliation entirely, even for manual triggers
        // This check happens early to avoid unnecessary processing
        if obj.spec.suspend {
            debug!(
                resource.name = name.as_str(),
                resource.namespace = namespace.as_str(),
                "Skipping reconciliation - resource is suspended"
            );
            // Return Action::await_change() to wait for suspend to be cleared
            // The reconciler will update status to Suspended
            return Ok(Action::await_change());
        }

        // Check if this is a manual reconciliation trigger (via msmctl annotation)
        // Manual triggers should always be honored, even if generation hasn't changed
        let is_manual_trigger = obj
            .metadata
            .annotations
            .as_ref()
            .and_then(|ann| ann.get("secret-management.octopilot.io/reconcile"))
            .is_some();

        // Check if this is a periodic reconciliation (requeue-triggered)
        // Periodic reconciliations should run even if generation matches, as they check
        // for external state changes (secrets in cloud provider, Git repository updates)
        // We use next_reconcile_time from status to persist the schedule across watch restarts
        // CRITICAL: If generation matches but next_reconcile_time has passed, this is a periodic reconciliation
        let is_periodic_reconcile = if generation == observed_generation && observed_generation > 0
        {
            if let Some(status) = &obj.status {
                if let Some(next_reconcile_time) = &status.next_reconcile_time {
                    // Check if next_reconcile_time has passed (with 2s tolerance for timing)
                    if let Ok(next_time) = chrono::DateTime::parse_from_rfc3339(next_reconcile_time)
                    {
                        let next_time_utc = next_time.with_timezone(&chrono::Utc);
                        let now = chrono::Utc::now();
                        // If current time >= next_reconcile_time (with 2s tolerance), this is a periodic reconciliation
                        let is_periodic = now >= next_time_utc - chrono::Duration::seconds(2);
                        if is_periodic {
                            info!(
                                resource.name = name.as_str(),
                                resource.namespace = namespace.as_str(),
                                next_reconcile_time = next_reconcile_time.as_str(),
                                "Detected periodic reconciliation - next_reconcile_time has passed"
                            );
                        }
                        is_periodic
                    } else {
                        false
                    }
                } else {
                    // No next_reconcile_time means first reconciliation or not yet scheduled - not periodic
                    false
                }
            } else {
                false
            }
        } else {
            // Generation doesn't match, so this is a spec change, not periodic
            false
        };

        // CRITICAL: Only reconcile if spec changed (generation != observed_generation)
        // This prevents infinite loops from status updates triggering reconciliations
        // Status-only updates don't change generation, so we skip them
        // Exceptions:
        // 1. Always reconcile if observed_generation is 0 (first reconciliation)
        // 2. Always reconcile if manual trigger annotation is present (msmctl reconcile)
        // 3. Always reconcile if this is a periodic reconciliation (requeue-triggered)
        if generation == observed_generation
            && observed_generation > 0
            && !is_manual_trigger
            && !is_periodic_reconcile
        {
            debug!(
                resource.name = name.as_str(),
                resource.namespace = namespace.as_str(),
                generation = generation,
                observed_generation = observed_generation,
                "Skipping reconciliation - only status changed, spec unchanged (no manual trigger, not periodic)"
            );
            // Return Action::await_change() to wait for next spec change
            return Ok(Action::await_change());
        }

        if is_periodic_reconcile {
            info!(
                resource.name = name.as_str(),
                resource.namespace = namespace.as_str(),
                "Periodic reconciliation triggered - proceeding despite generation match"
            );
        } else if generation == observed_generation && observed_generation > 0 {
            // Log why periodic reconciliation wasn't detected
            debug!(
                resource.name = name.as_str(),
                resource.namespace = namespace.as_str(),
                has_status = obj.status.is_some(),
                has_next_reconcile_time = obj
                    .status
                    .as_ref()
                    .and_then(|s| s.next_reconcile_time.as_ref())
                    .is_some(),
                next_reconcile_time = obj
                    .status
                    .as_ref()
                    .and_then(|s| s.next_reconcile_time.as_ref())
                    .map(|s| s.as_str())
                    .unwrap_or("none"),
                reconcile_interval = obj.spec.reconcile_interval.as_str(),
                "Periodic reconciliation check - generation matches but next_reconcile_time not yet reached"
            );
        }

        // Determine trigger source for detailed logging
        let trigger_source = if is_manual_trigger {
            TriggerSource::ManualCli
        } else if is_periodic_reconcile {
            TriggerSource::TimerBased
        } else if observed_generation == 0 {
            // First reconciliation
            TriggerSource::TimerBased
        } else {
            // Spec change (generation changed)
            TriggerSource::TimerBased
        };

        if is_manual_trigger {
            debug!(
                resource.name = name.as_str(),
                resource.namespace = namespace.as_str(),
                generation = generation,
                observed_generation = observed_generation,
                "Manual reconciliation trigger detected (msmctl) - proceeding despite generation match"
            );
        }

        debug!(
            resource.name = name.as_str(),
            resource.namespace = namespace.as_str(),
            generation = generation,
            observed_generation = observed_generation,
            trigger_source = trigger_source.as_str(),
            "watch.event.received"
        );

        let result = reconcile(
            obj,
            reconciler.clone(),
            trigger_source,
            controller_config_for_reconcile.clone(),
        )
        .await;

        match &result {
            Ok(action) => {
                debug!(resource.name = name.as_str(), action = ?action, "watch.event.reconciled");
            }
            Err(e) => {
                error!(resource.name = name.as_str(), error = %e, "watch.event.reconciliation_failed");
            }
        }

        result
    }
}
