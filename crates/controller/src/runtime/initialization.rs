//! # Initialization
//!
//! Controller initialization logic including rustls setup, OpenTelemetry,
//! tracing, metrics, server startup, and Kubernetes client setup.

use crate::config::{
    create_shared_config, start_configmap_watch, SharedControllerConfig, SharedServerConfig,
};
use crate::controller::reconciler::{reconcile, Reconciler, TriggerSource};
use crate::controller::server::{start_server, ServerState};
use crate::crd::SecretManagerConfig;
use crate::observability;
use anyhow::{Context, Result};
use kube::{api::Api, api::ListParams, Client};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Initialization result containing all necessary components for the controller
pub struct InitializationResult {
    /// Kubernetes client
    pub client: Client,
    /// API for SecretManagerConfig CRD
    pub configs: Api<SecretManagerConfig>,
    /// Reconciler context
    pub reconciler: Arc<Reconciler>,
    /// Server state for health checks
    pub server_state: Arc<ServerState>,
    /// OpenTelemetry tracer provider (if initialized)
    pub otel_tracer_provider: Option<crate::observability::otel::TracerProviderHandle>,
    /// Shared controller configuration (hot-reloadable)
    pub controller_config: SharedControllerConfig,
    /// Shared server configuration (hot-reloadable)
    pub server_config: SharedServerConfig,
}

impl std::fmt::Debug for InitializationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InitializationResult")
            .field(
                "server_ready",
                &self
                    .server_state
                    .is_ready
                    .load(std::sync::atomic::Ordering::Relaxed),
            )
            .finish_non_exhaustive()
    }
}

/// Initialize the controller runtime
///
/// This function handles:
/// - rustls crypto provider setup
/// - OpenTelemetry initialization
/// - Tracing subscriber setup
/// - Metrics registration
/// - HTTP server startup
/// - Kubernetes client creation
/// - Reconciler setup
/// - SOPS key watch
/// - Reconcile existing resources
pub async fn initialize() -> Result<InitializationResult> {
    // Configure rustls crypto provider FIRST, before any other operations
    // Required for rustls 0.23+ when no default provider is set via features
    // This must be called synchronously before any async operations that use rustls
    // We use ring as the crypto provider
    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap_or_else(|_| panic!("Failed to install rustls crypto provider"));

    // Initialize PACT_MODE configuration FIRST
    // This must be done before any provider creation
    crate::config::PactModeConfig::init()
        .context("Failed to initialize PACT_MODE configuration")?;

    // Initialize OpenTelemetry first (if configured)
    // This will set up tracing with Otel support
    // Note: Otel config can come from CRD, but we initialize early from env vars
    // Per-resource Otel config is handled in the reconciler
    let otel_tracer_provider =
        observability::otel::init_otel(None).context("Failed to initialize OpenTelemetry")?;

    // If Otel wasn't initialized, use standard tracing subscriber
    // When Datadog is configured, datadog-opentelemetry sets up the tracing subscriber automatically
    if otel_tracer_provider.is_none() {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "secret_manager_controller=info".into()),
            )
            .init();
    } else {
        // When Otel is initialized, we still need to set up the tracing subscriber
        // datadog-opentelemetry handles this automatically, but we ensure env filter is applied
        // The tracing-opentelemetry layer is already set up by datadog-opentelemetry
        if let Err(e) = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "secret_manager_controller=info".into()),
            )
            .try_init()
        {
            // If init fails, it might already be initialized by datadog-opentelemetry
            // This is fine - datadog-opentelemetry sets up its own subscriber
            warn!("Tracing subscriber init returned error (may already be initialized by Datadog): {}", e);
        }
    }

    info!("Starting Secret Manager Controller v2");
    info!(
        "Build info: timestamp={}, datetime={}, git_hash={}",
        env!("BUILD_TIMESTAMP"),
        env!("BUILD_DATETIME"),
        env!("BUILD_GIT_HASH")
    );

    // Initialize metrics
    observability::metrics::register_metrics()?;

    // Create server state
    let server_state = Arc::new(ServerState {
        is_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    });

    // Create shared configuration (hot-reloadable) - must be created before server startup
    let (controller_config, server_config) = create_shared_config();

    // Start HTTP server for metrics and probes
    // We start it in a background task but wait for it to be ready before proceeding
    let server_state_clone = server_state.clone();
    let server_port = {
        let config = server_config.read().await;
        config.metrics_port
    };

    // Start server in background task
    let server_handle = tokio::spawn(async move {
        if let Err(e) = start_server(server_port, server_state_clone).await {
            error!("HTTP server error: {}", e);
        }
    });

    // Poll server startup - wait for it to be ready before proceeding
    // This ensures readiness probes pass immediately after server starts
    let server_config_for_wait = server_config.clone();
    wait_for_server_ready(&server_state, &server_handle, server_config_for_wait).await?;

    // Create Kubernetes client
    let client = Client::try_default().await?;

    // Create API for SecretManagerConfig CRD - watch all namespaces
    // This allows developers to deploy SecretManagerConfig resources in any namespace
    let configs: Api<SecretManagerConfig> = Api::all(client.clone());

    // Create reconciler context
    let reconciler = Arc::new(Reconciler::new(client.clone()).await?);

    // Start watching for SOPS private key secret changes
    // This allows hot-reloading the key without restarting the controller
    crate::controller::reconciler::start_sops_key_watch(reconciler.clone());

    // Check if CRD is queryable and reconcile existing resources before starting the watch
    // This ensures existing resources are reconciled when the controller starts
    // CRITICAL: Without this, resources created before controller deployment won't be reconciled
    // Also check for hot-reload configuration in SecretManagerConfig resources
    let hot_reload_config =
        reconcile_existing_resources(&configs, &reconciler, controller_config.clone()).await?;

    // Start ConfigMap watcher for hot-reload if enabled
    if let Some(hot_reload) = hot_reload_config {
        let controller_namespace =
            std::env::var("POD_NAMESPACE").unwrap_or_else(|_| "octopilot-system".to_string());
        let configmap_namespace = hot_reload
            .config_map_namespace
            .as_deref()
            .unwrap_or(&controller_namespace);

        info!(
            "Hot-reload enabled: watching ConfigMap '{}' in namespace '{}'",
            hot_reload.config_map_name, configmap_namespace
        );
        start_configmap_watch(
            client.clone(),
            configmap_namespace,
            &hot_reload.config_map_name,
            controller_config.clone(),
            server_config.clone(),
        );
    } else {
        info!("Hot-reload disabled (default) - configuration changes require pod restart");
    }

    // Note: GitRepository and ArgoCD Application changes are handled by the main controller watch.
    // When SecretManagerConfig resources are reconciled, they fetch the latest source,
    // ensuring source changes are picked up without restarting the controller.
    // SOPS secrets are watched separately for hot-reloading.

    // Server is already marked as ready by start_server() once it binds
    // This ensures readiness probes pass before we start reconciling
    info!("Controller initialized, starting watch loop...");

    Ok(InitializationResult {
        client,
        configs,
        reconciler,
        controller_config,
        server_config,
        server_state,
        otel_tracer_provider,
    })
}

/// Wait for the HTTP server to become ready
async fn wait_for_server_ready(
    server_state: &Arc<ServerState>,
    server_handle: &tokio::task::JoinHandle<()>,
    server_config: SharedServerConfig,
) -> Result<()> {
    let config = server_config.read().await;
    let startup_timeout = std::time::Duration::from_secs(config.startup_timeout_secs);
    let poll_interval = std::time::Duration::from_millis(config.poll_interval_ms);
    drop(config);
    let start_time = std::time::Instant::now();

    loop {
        // Check if server task crashed
        if server_handle.is_finished() {
            return Err(anyhow::anyhow!("HTTP server failed to start"));
        }

        // Check if server is ready (set by start_server once bound)
        if server_state
            .is_ready
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            info!("HTTP server is ready and accepting connections");
            break;
        }

        // Check timeout
        if start_time.elapsed() > startup_timeout {
            return Err(anyhow::anyhow!(
                "HTTP server failed to become ready within {} seconds",
                startup_timeout.as_secs()
            ));
        }

        // Wait before next poll
        tokio::time::sleep(poll_interval).await;
    }

    Ok(())
}

/// Reconcile existing SecretManagerConfig resources before starting the watch
///
/// This ensures resources created before controller deployment are processed.
async fn reconcile_existing_resources(
    configs: &Api<SecretManagerConfig>,
    reconciler: &Arc<Reconciler>,
    controller_config: SharedControllerConfig,
) -> Result<Option<crate::crd::HotReloadConfig>> {
    let existing_resources_span = tracing::span!(
        tracing::Level::INFO,
        "controller.startup.reconcile_existing",
        operation = "reconcile_existing_resources"
    );
    let _guard = existing_resources_span.enter();

    match configs.list(&ListParams::default()).await {
        Ok(list) => {
            info!(
                "CRD is queryable, found {} existing SecretManagerConfig resources",
                list.items.len()
            );

            // Check for hot-reload configuration in any SecretManagerConfig resource
            // Hot-reload is controller-level, so we only need to find it once
            let mut hot_reload_config: Option<crate::crd::HotReloadConfig> = None;
            for item in &list.items {
                if let Some(ref hot_reload) = item.spec.hot_reload {
                    if hot_reload.enabled {
                        hot_reload_config = Some(hot_reload.clone());
                        info!(
                            "Found hot-reload configuration in SecretManagerConfig '{}/{}'",
                            item.metadata.namespace.as_deref().unwrap_or("default"),
                            item.metadata.name.as_deref().unwrap_or("unknown")
                        );
                        break; // Use first enabled hot-reload config found
                    }
                }
            }

            if !list.items.is_empty() {
                // Tabulate resources by namespace for operations visibility
                use std::collections::HashMap;
                let mut resources_by_namespace: HashMap<String, Vec<String>> = HashMap::new();

                for item in &list.items {
                    let namespace = item
                        .metadata
                        .namespace
                        .as_deref()
                        .unwrap_or("default")
                        .to_string();
                    let name = item
                        .metadata
                        .name
                        .as_deref()
                        .unwrap_or("unknown")
                        .to_string();
                    resources_by_namespace
                        .entry(namespace)
                        .or_insert_with(Vec::new)
                        .push(name);
                }

                // Sort namespaces for consistent output
                let mut sorted_namespaces: Vec<_> = resources_by_namespace.keys().collect();
                sorted_namespaces.sort();

                // Output startup summary
                info!("Secret Manager Controller - Startup Resource Summary");
                info!("Resource Kind: SecretManagerConfig");
                info!("Total Resources: {}", list.items.len());
                info!("Namespaces: {}", resources_by_namespace.len());

                for namespace in sorted_namespaces.iter() {
                    let resources = resources_by_namespace.get(*namespace)
                        .ok_or_else(|| anyhow::anyhow!(
                            "Internal error: namespace {} not found in resources map (this should never happen)",
                            namespace
                        ))?;
                    let namespace_display = if **namespace == "default" {
                        format!("{} (default)", namespace)
                    } else {
                        (*namespace).clone()
                    };

                    // Sort resource names within each namespace for consistent output
                    let mut sorted_resources = resources.clone();
                    sorted_resources.sort();

                    info!("Namespace: {}", namespace_display);
                    info!(
                        "  Resources ({}): {}",
                        sorted_resources.len(),
                        if sorted_resources.len() <= 3 {
                            sorted_resources.join(", ")
                        } else {
                            format!(
                                "{}, ... ({} total)",
                                sorted_resources[..3].join(", "),
                                sorted_resources.len()
                            )
                        }
                    );
                }
                info!("Reconciling {} existing SecretManagerConfig resources before starting watch...", list.items.len());

                // Explicitly reconcile each existing resource
                // This ensures resources created before controller deployment are processed
                for item in &list.items {
                    let name = item.metadata.name.as_deref().unwrap_or("unknown");
                    let namespace = item.metadata.namespace.as_deref().unwrap_or("default");

                    info!(
                        "Reconciling existing resource: {} in namespace {}",
                        name, namespace
                    );

                    // Create a reconciliation span for each resource
                    let resource_span = tracing::span!(
                        tracing::Level::INFO,
                        "controller.startup.reconcile_resource",
                        resource.name = name,
                        resource.namespace = namespace,
                        resource.kind = "SecretManagerConfig"
                    );
                    let _resource_guard = resource_span.enter();

                    // Startup reconciliation uses timer-based trigger source
                    match reconcile(
                        Arc::new(item.clone()),
                        reconciler.clone(),
                        TriggerSource::TimerBased,
                        controller_config.clone(),
                    )
                    .await
                    {
                        Ok(_action) => {
                            info!(
                                "Successfully reconciled existing resource: {} in namespace {}",
                                name, namespace
                            );
                            info!(
                                resource.name = name,
                                resource.namespace = namespace,
                                "reconciliation.success"
                            );
                        }
                        Err(e) => {
                            error!(
                                "Failed to reconcile existing resource {} in namespace {}: {}",
                                name, namespace, e
                            );
                            error!(resource.name = name, resource.namespace = namespace, error = %e, "reconciliation.error");
                            // Continue with other resources even if one fails
                        }
                    }
                }

                info!(
                    "Completed reconciliation of {} existing resources",
                    list.items.len()
                );
            } else {
                info!("No existing SecretManagerConfig resources found, watch will pick up new resources");
            }

            Ok(hot_reload_config)
        }
        Err(e) => {
            error!("CRD is not queryable; {:?}. Is the CRD installed?", e);
            error!("Installation: kubectl apply -f config/crd/secretmanagerconfig.yaml");
            // Don't exit - let the controller start and it will handle the error gracefully
            warn!("Continuing despite CRD queryability check failure - controller will retry");
            warn!(error = %e, "CRD queryability check failed");
            Ok(None) // Return None if CRD query fails (hot-reload disabled)
        }
    }
}
