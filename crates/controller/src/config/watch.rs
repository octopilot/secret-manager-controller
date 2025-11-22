//! # ConfigMap Watch
//!
//! Watches for ConfigMap changes and hot-reloads controller configuration.

use crate::config::{ControllerConfig, ServerConfig, SharedControllerConfig, SharedServerConfig};
use futures::{pin_mut, StreamExt};
use k8s_openapi::api::core::v1::ConfigMap;
use kube::Api;
use kube_runtime::watcher;
use tracing::{error, info, warn};

/// Start watching for ConfigMap changes and hot-reload configuration
///
/// Watches the specified ConfigMap in the controller namespace.
/// When the ConfigMap changes, reloads configuration and updates the shared config.
///
/// **Note**: This is only useful if you want to avoid pod restarts when ConfigMap changes.
/// If you use a tool like Reloader that automatically restarts pods on ConfigMap changes,
/// hot-reload may be redundant.
pub fn start_configmap_watch(
    client: kube::Client,
    namespace: &str,
    configmap_name: &str,
    controller_config: SharedControllerConfig,
    server_config: SharedServerConfig,
) {
    let namespace = namespace.to_string();
    let configmap_name = configmap_name.to_string();
    tokio::spawn(async move {
        let configmaps: Api<ConfigMap> = Api::namespaced(client.clone(), &namespace);

        info!(
            "Starting watch for ConfigMap '{}' in namespace '{}'",
            configmap_name, namespace
        );

        // Create a watcher with field selector to only watch our ConfigMap
        let configmap_name_clone = configmap_name.clone();
        let watcher_config =
            watcher::Config::default().fields(&format!("metadata.name={configmap_name_clone}"));

        let stream = watcher(configmaps, watcher_config);
        pin_mut!(stream);

        info!("✅ ConfigMap watcher started - configuration will hot-reload on changes");

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    match event {
                        watcher::Event::Apply(configmap) => {
                            if configmap.metadata.name == Some(configmap_name.clone()) {
                                info!(
                                    "ConfigMap '{}' changed, reloading configuration...",
                                    configmap_name
                                );
                                reload_config_from_configmap(
                                    &configmap,
                                    &controller_config,
                                    &server_config,
                                )
                                .await;
                            }
                        }
                        watcher::Event::Delete(configmap) => {
                            if configmap.metadata.name == Some(configmap_name.clone()) {
                                warn!(
                                    "ConfigMap '{}' was deleted, reverting to defaults",
                                    configmap_name
                                );
                                // Reload with defaults (empty ConfigMap data)
                                let empty_configmap = ConfigMap {
                                    data: None,
                                    ..Default::default()
                                };
                                reload_config_from_configmap(
                                    &empty_configmap,
                                    &controller_config,
                                    &server_config,
                                )
                                .await;
                            }
                        }
                        watcher::Event::Init
                        | watcher::Event::InitApply(_)
                        | watcher::Event::InitDone => {
                            // Initial watch events - ignore, we already loaded config at startup
                        }
                    }
                }
                Err(e) => {
                    error!("Error watching ConfigMap: {}", e);
                    // Continue watching - stream will retry automatically
                }
            }
        }

        warn!("ConfigMap watch stream ended");
    });
}

/// Reload configuration from ConfigMap data
///
/// Updates environment variables temporarily, reloads config, then restores env vars.
/// This allows the config module to read the new values.
async fn reload_config_from_configmap(
    configmap: &ConfigMap,
    controller_config: &SharedControllerConfig,
    server_config: &SharedServerConfig,
) {
    // Get current environment variables to restore later
    let mut env_backup = std::collections::HashMap::new();
    let env_vars_to_backup = vec![
        "METRICS_PORT",
        "SERVER_STARTUP_TIMEOUT_SECS",
        "SERVER_POLL_INTERVAL_MS",
        "RECONCILIATION_ERROR_REQUEUE_SECS",
        "BACKOFF_START_MS",
        "BACKOFF_MAX_MS",
        "WATCH_RESTART_DELAY_SECS",
        "WATCH_RESTART_DELAY_AFTER_END_SECS",
        "MIN_GITREPOSITORY_PULL_INTERVAL_SECS",
        "MIN_RECONCILE_INTERVAL_SECS",
        "SOPS_PRIVATE_KEY_SECRET_NAME",
        "SOPS_KEY_WATCH_ENABLED",
        "POD_NAMESPACE",
        "LOG_LEVEL",
        "LOG_FORMAT",
        "LOG_ENABLE_COLOR",
        "ENABLE_METRICS",
        "ENABLE_TRACING",
        "MAX_CONCURRENT_RECONCILIATIONS",
        "MAX_SECRETS_PER_RESOURCE",
        "MAX_SECRET_SIZE_BYTES",
    ];

    // Backup current env vars
    for key in &env_vars_to_backup {
        if let Ok(value) = std::env::var(key) {
            env_backup.insert(key.to_string(), value);
        }
    }

    // Set environment variables from ConfigMap data
    if let Some(data) = &configmap.data {
        for (key, value) in data {
            // Convert ConfigMap key format (lowercase with underscores) to env var format (UPPERCASE)
            let env_key = key.to_uppercase();
            std::env::set_var(&env_key, value);
            info!("  Set {}={}", env_key, value);
        }
    }

    // Reload configuration
    let new_controller_config = ControllerConfig::from_env();
    let new_server_config = ServerConfig::from_env();

    // Update shared configuration
    {
        let mut config = controller_config.write().await;
        *config = new_controller_config.clone();
    }
    {
        let mut config = server_config.write().await;
        *config = new_server_config.clone();
    }

    info!("✅ Configuration reloaded successfully");
    info!("  Controller config: reconciliation_error_requeue={}s, backoff_start={}ms, backoff_max={}ms", 
          new_controller_config.reconciliation_error_requeue_secs,
          new_controller_config.backoff_start_ms,
          new_controller_config.backoff_max_ms);
    info!(
        "  Server config: metrics_port={}, startup_timeout={}s",
        new_server_config.metrics_port, new_server_config.startup_timeout_secs
    );

    // Restore original environment variables
    for key in &env_vars_to_backup {
        if let Some(value) = env_backup.get(*key) {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }
}
