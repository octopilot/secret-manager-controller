//! # Watch Loop
//!
//! Controller watch loop that monitors SecretManagerConfig resources and triggers
//! reconciliation when changes are detected.

use crate::config::SharedControllerConfig;
use crate::controller::reconciler::status::update_status_phase;
use crate::controller::reconciler::validation::parse_kubernetes_duration;
use crate::controller::reconciler::{Reconciler, TriggerSource, reconcile, retry};
use crate::controller::server::ServerState;
use crate::crd::SecretManagerConfig;
use crate::runtime::error_policy::{handle_reconciliation_error, handle_watch_stream_error};
use futures::StreamExt;
use kube::api::{Api, ApiResource, DynamicObject};
use kube::core::GroupVersionKind;
use kube::runtime::reflector::ObjectRef;
use kube_runtime::{Controller, controller::Action, watcher};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
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
        let mut controller =
            Controller::new(configs.clone(), watcher::Config::default().any_semantic());
        // Pair with Flux source-controller: a new artifact on a GitRepository triggers the
        // SecretManagerConfigs that reference it, instead of waiting for reconcileInterval.
        // Skipped when the cluster has no Flux source API (for example ArgoCD only).
        let client = configs.clone().into_client();
        if flux_git_repository_available(&client).await {
            info!("Watching Flux GitRepository artifacts (source.toolkit.fluxcd.io/v1)");
            let ar = flux_git_repository_resource();
            let store = controller.store();
            controller = controller.watches_with(
                Api::<DynamicObject>::all_with(client, &ar),
                ar,
                watcher::Config::default(),
                move |git_repository: DynamicObject| {
                    configs_for_new_artifact(&git_repository, &store.state())
                },
            );
        } else {
            warn!(
                "Flux GitRepository API (source.toolkit.fluxcd.io/v1) not found: \
                 source changes are picked up on reconcileInterval only"
            );
        }
        let controller_future = controller
            .shutdown_on_signal()
            .run(
                |obj, ctx| create_reconcile_fn(obj, ctx, controller_config_for_reconcile.clone()),
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
        // A new artifact on the referenced GitRepository (see configs_for_new_artifact).
        let is_source_trigger = take_source_trigger(&namespace, &name);
        // A retry asked for by an earlier run that failed or had to wait.
        let is_retry = retry::take_if_due(&namespace, &name);

        let reason = reconcile_reason(
            generation,
            observed_generation,
            is_manual_trigger,
            is_source_trigger,
            is_retry,
            is_periodic_reconcile,
        );

        if reason.is_none() {
            debug!(
                resource.name = name.as_str(),
                resource.namespace = namespace.as_str(),
                generation = generation,
                observed_generation = observed_generation,
                "Skipping reconciliation - only status changed, spec unchanged (no manual trigger, not periodic)"
            );
            // Not due yet. This run was most likely caused by our own status update, and
            // kube-runtime drops the requeue a reconcile returns when another run of the
            // same object is already pending. Returning await_change() here therefore ended
            // periodic reconciliation after the first success: schedule the periodic run.
            // A pending retry is sooner than the periodic run: wake for whichever is first.
            let wake_in = retry::time_until(&namespace, &name).map_or_else(
                || time_until_next_reconcile(&obj),
                |retry_in| retry_in.min(time_until_next_reconcile(&obj)),
            );
            return Ok(Action::requeue(
                wake_in.max(std::time::Duration::from_secs(1)),
            ));
        }

        // This run supersedes any retry still pending.
        retry::clear(&namespace, &name);
        info!(
            resource.name = name.as_str(),
            resource.namespace = namespace.as_str(),
            reason = ?reason,
            "Reconciling"
        );

        if is_source_trigger {
            info!(
                resource.name = name.as_str(),
                resource.namespace = namespace.as_str(),
                "Source artifact changed - reconciling"
            );
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
            obj.clone(),
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
                // Whatever failed, the resource must not keep showing Ready.
                if let Err(status_err) =
                    update_status_phase(&reconciler, &obj, "Failed", Some(&e.to_string())).await
                {
                    warn!(resource.name = name.as_str(), error = %status_err, "Failed to record Failed status");
                }
            }
        }

        result
    }
}

/// The Flux `GitRepository` API resource. v1 is the only version served since Flux 2.8.
/// Why an object is reconciled now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconcileReason {
    /// Never reconciled, or its spec changed since it last was.
    SpecChanged,
    /// Asked for with the reconcile annotation.
    Manual,
    /// Its source has a new artifact.
    SourceChanged,
    /// An earlier run failed or had to wait, and its delay is over.
    Retry,
    /// Its reconcile interval is due.
    Periodic,
}

/// Decide whether a run of the reconcile function has work to do.
///
/// Every status update the controller writes comes back as a watch event, so most runs
/// are echoes of its own writes. A run reconciles only when it has one of these reasons;
/// otherwise it is skipped.
fn reconcile_reason(
    generation: i64,
    observed_generation: i64,
    manual: bool,
    source_changed: bool,
    retry_due: bool,
    periodic_due: bool,
) -> Option<ReconcileReason> {
    if observed_generation == 0 || generation != observed_generation {
        Some(ReconcileReason::SpecChanged)
    } else if manual {
        Some(ReconcileReason::Manual)
    } else if source_changed {
        Some(ReconcileReason::SourceChanged)
    } else if retry_due {
        Some(ReconcileReason::Retry)
    } else if periodic_due {
        Some(ReconcileReason::Periodic)
    } else {
        None
    }
}

fn flux_git_repository_resource() -> ApiResource {
    ApiResource::from_gvk(&GroupVersionKind {
        group: "source.toolkit.fluxcd.io".to_string(),
        version: "v1".to_string(),
        kind: "GitRepository".to_string(),
    })
}

/// Whether the cluster serves the Flux `GitRepository` API.
async fn flux_git_repository_available(client: &kube::Client) -> bool {
    let gvk = GroupVersionKind {
        group: "source.toolkit.fluxcd.io".to_string(),
        version: "v1".to_string(),
        kind: "GitRepository".to_string(),
    };
    kube::discovery::pinned_kind(client, &gvk).await.is_ok()
}

/// Configs (`namespace/name`) whose source produced a new artifact and that must
/// reconcile even though their spec is unchanged and their interval has not elapsed.
fn source_triggers() -> &'static Mutex<HashSet<String>> {
    static TRIGGERS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    TRIGGERS.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Last artifact revision seen per GitRepository (`namespace/name`).
fn seen_revisions() -> &'static Mutex<HashMap<String, String>> {
    static SEEN: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Take (and clear) the source trigger of a config.
fn take_source_trigger(namespace: &str, name: &str) -> bool {
    source_triggers()
        .lock()
        .is_ok_and(|mut triggers| triggers.remove(&format!("{namespace}/{name}")))
}

/// The artifact revision of a GitRepository, if it has produced an artifact.
fn artifact_revision(git_repository: &DynamicObject) -> Option<&str> {
    git_repository
        .data
        .get("status")?
        .get("artifact")?
        .get("revision")?
        .as_str()
}

/// Whether `revision` differs from the last one recorded for the GitRepository `key`.
/// Records it. The first revision seen counts as new, so a commit that landed while
/// the controller was down is synced at startup.
fn is_new_revision(key: &str, revision: &str) -> bool {
    let Ok(mut seen) = seen_revisions().lock() else {
        return true;
    };
    if seen.get(key).is_some_and(|last| last == revision) {
        return false;
    }
    seen.insert(key.to_string(), revision.to_string());
    true
}

/// Whether a config takes its source from the GitRepository `namespace/name`.
fn references_git_repository(config: &SecretManagerConfig, namespace: &str, name: &str) -> bool {
    let source = &config.spec.source_ref;
    source.kind == "GitRepository" && source.name == name && source.namespace == namespace
}

/// Mapper of the GitRepository watch: when the artifact revision changed, mark and
/// return every SecretManagerConfig that references the GitRepository.
fn configs_for_new_artifact(
    git_repository: &DynamicObject,
    configs: &[Arc<SecretManagerConfig>],
) -> Vec<ObjectRef<SecretManagerConfig>> {
    let (Some(name), Some(namespace)) = (
        git_repository.metadata.name.as_deref(),
        git_repository.metadata.namespace.as_deref(),
    ) else {
        return Vec::new();
    };
    let Some(revision) = artifact_revision(git_repository) else {
        return Vec::new();
    };
    if !is_new_revision(&format!("{namespace}/{name}"), revision) {
        return Vec::new();
    }

    let referencing: Vec<&Arc<SecretManagerConfig>> = configs
        .iter()
        .filter(|config| references_git_repository(config, namespace, name))
        .collect();
    if let Ok(mut triggers) = source_triggers().lock() {
        for config in &referencing {
            triggers.insert(format!(
                "{}/{}",
                config.metadata.namespace.as_deref().unwrap_or("default"),
                config.metadata.name.as_deref().unwrap_or("unknown")
            ));
        }
    }
    referencing
        .into_iter()
        .map(|config| ObjectRef::from_obj(config.as_ref()))
        .collect()
}

/// Time until the next periodic reconciliation of a config: until
/// `status.nextReconcileTime`, else one `reconcileInterval`. Never less than a second.
fn time_until_next_reconcile(config: &SecretManagerConfig) -> std::time::Duration {
    let interval = parse_kubernetes_duration(&config.spec.reconcile_interval)
        .unwrap_or(std::time::Duration::from_secs(60));
    let remaining = config
        .status
        .as_ref()
        .and_then(|status| status.next_reconcile_time.as_deref())
        .and_then(|next| chrono::DateTime::parse_from_rfc3339(next).ok())
        .map(|next| next.with_timezone(&chrono::Utc) - chrono::Utc::now())
        .and_then(|remaining| remaining.to_std().ok())
        .unwrap_or(interval);
    remaining.clamp(
        std::time::Duration::from_secs(1),
        interval.max(std::time::Duration::from_secs(1)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_repository(namespace: &str, name: &str, revision: Option<&str>) -> DynamicObject {
        let mut object =
            DynamicObject::new(name, &flux_git_repository_resource()).within(namespace);
        if let Some(revision) = revision {
            object.data = serde_json::json!({ "status": { "artifact": { "revision": revision } } });
        }
        object
    }

    #[test]
    fn a_status_echo_is_not_reconciled() {
        assert_eq!(reconcile_reason(3, 3, false, false, false, false), None);
    }

    #[test]
    fn a_new_or_changed_spec_is_reconciled() {
        assert_eq!(
            reconcile_reason(1, 0, false, false, false, false),
            Some(ReconcileReason::SpecChanged)
        );
        assert_eq!(
            reconcile_reason(4, 3, false, false, false, false),
            Some(ReconcileReason::SpecChanged)
        );
    }

    /// The regression this guards: a failed sync records the generation as observed, so
    /// its retry looked like a status echo and waited for the reconcile interval.
    #[test]
    fn a_due_retry_is_reconciled_although_the_generation_is_observed() {
        assert_eq!(
            reconcile_reason(3, 3, false, false, true, false),
            Some(ReconcileReason::Retry)
        );
    }

    #[test]
    fn each_trigger_is_a_reason_on_its_own() {
        assert_eq!(
            reconcile_reason(3, 3, true, false, false, false),
            Some(ReconcileReason::Manual)
        );
        assert_eq!(
            reconcile_reason(3, 3, false, true, false, false),
            Some(ReconcileReason::SourceChanged)
        );
        assert_eq!(
            reconcile_reason(3, 3, false, false, false, true),
            Some(ReconcileReason::Periodic)
        );
    }

    #[test]
    fn artifact_revision_reads_status() {
        assert_eq!(
            artifact_revision(&git_repository(
                "flux-system",
                "repo",
                Some("main@sha1:abc")
            )),
            Some("main@sha1:abc")
        );
        assert_eq!(
            artifact_revision(&git_repository("flux-system", "repo", None)),
            None
        );
    }

    #[test]
    fn a_revision_is_new_once() {
        assert!(is_new_revision("test-ns/once", "main@sha1:1"));
        assert!(!is_new_revision("test-ns/once", "main@sha1:1"));
        assert!(is_new_revision("test-ns/once", "main@sha1:2"));
    }

    #[test]
    fn source_trigger_is_taken_once() {
        source_triggers()
            .lock()
            .expect("lock")
            .insert("test-ns/taken".to_string());
        assert!(take_source_trigger("test-ns", "taken"));
        assert!(!take_source_trigger("test-ns", "taken"));
    }

    #[test]
    fn git_repository_without_artifact_triggers_nothing() {
        let refs =
            configs_for_new_artifact(&git_repository("flux-system", "no-artifact", None), &[]);
        assert!(refs.is_empty());
    }
}
