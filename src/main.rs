//! # Secret Manager Controller
//!
//! A Kubernetes controller that syncs secrets from GitOps repositories (FluxCD/ArgoCD) to Google Cloud Secret Manager.
//!
//! ## Overview
//!
//! This controller provides GitOps-style secret management by:
//!
//! 1. **Watching GitOps sources** - Monitors FluxCD GitRepository or ArgoCD Application resources
//! 2. **Reading secret files** - Parses `application.secrets.env`, `application.secrets.yaml`, and `application.properties`
//! 3. **SOPS decryption** - Automatically decrypts SOPS-encrypted files using GPG keys from Kubernetes secrets
//! 4. **Kustomize support** - Runs `kustomize build` to extract secrets from generated Kubernetes Secret resources
//! 5. **GCP Secret Manager sync** - Stores secrets in Google Cloud Secret Manager for CloudRun consumption
//!
//! ## Features
//!
//! - **GitOps-agnostic**: Works with FluxCD GitRepository and ArgoCD Application via `sourceRef` pattern
//! - **Kustomize Build Mode**: Supports overlays, patches, and generators by running `kustomize build`
//! - **Raw File Mode**: Direct parsing of application secret files
//! - **SOPS encryption**: Automatic decryption of SOPS-encrypted files
//! - **Multi-namespace**: Watches `SecretManagerConfig` resources across all namespaces
//! - **Prometheus metrics**: Exposes metrics for monitoring and observability
//! - **Health probes**: HTTP endpoints for liveness and readiness checks
//!
//! ## Usage
//!
//! See the [README.md](../README.md) for detailed usage instructions and examples.

use anyhow::{Context, Result};
use futures::StreamExt;
use kube::{
    api::{Api, ListParams, Patch, PatchParams},
    core::CustomResourceExt,
    Client, CustomResource, Resource,
};
use kube_runtime::{
    watcher, Controller,
    controller::Action,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{error, info, warn};

mod gcp;
mod kustomize;
mod metrics;
mod parser;
mod reconciler;
mod server;

use reconciler::Reconciler;
use server::{ServerState, start_server};

/// SecretManagerConfig Custom Resource Definition
///
/// This CRD defines the configuration for syncing secrets from GitOps repositories
/// to Google Cloud Secret Manager.
///
/// # Example
///
/// ```yaml
/// apiVersion: secret-management.microscaler.io/v1
/// kind: SecretManagerConfig
/// metadata:
///   name: my-service-secrets
///   namespace: default
/// spec:
///   sourceRef:
///     kind: GitRepository
///     name: my-repo
///     namespace: flux-system
///   gcpProjectId: my-gcp-project
///   environment: dev
///   kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
/// ```
#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[kube(
    kind = "SecretManagerConfig",
    group = "secret-management.microscaler.io",
    version = "v1",
    namespaced,
    status = "SecretManagerConfigStatus",
    printcolumn = r#"{"name":"Ready", "type":"string", "jsonPath":".status.conditions[?(@.type==\"Ready\")].status"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct SecretManagerConfigSpec {
    /// Source reference - supports FluxCD GitRepository and ArgoCD Application
    /// This makes the controller GitOps-agnostic
    pub source_ref: SourceRef,
    /// GCP project ID for Secret Manager
    pub gcp_project_id: String,
    /// Environment/profile name to sync (e.g., "dev", "dev-cf", "prod-cf", "pp-cf")
    /// This must match the directory name under profiles/
    pub environment: String,
    /// Kustomize path - path to kustomization.yaml file (relative to GitRepository root)
    /// If specified, controller will run `kustomize build` on this path and extract secrets
    /// from the generated Kubernetes Secret resources. This supports kustomize overlays,
    /// patches, and generators. Works with any GitOps tool (FluxCD, ArgoCD, etc.)
    /// Examples: "microservices/idam/deployment-configuration/profiles/dev" or "./deployment-configuration/profiles/dev"
    /// If not specified, controller reads raw application.secrets.env files directly
    #[serde(default)]
    pub kustomize_path: Option<String>,
    /// Base path for application files (optional, used only if kustomize_path is not specified)
    /// If not specified, searches from repository root
    /// Examples: "microservices", "services", "apps", or "." for root
    #[serde(default)]
    pub base_path: Option<String>,
    /// Secret name prefix (default: repository name)
    #[serde(default)]
    pub secret_prefix: Option<String>,
}

/// Source reference for GitOps repositories
///
/// Supports multiple GitOps tools via the `kind` field:
/// - `GitRepository` (FluxCD) - Default
/// - `Application` (ArgoCD)
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    /// Kind of source - supports "GitRepository" (FluxCD) or "Application" (ArgoCD)
    /// Defaults to "GitRepository" for backward compatibility
    #[serde(default = "default_source_kind")]
    pub kind: String,
    /// Name of the source resource
    pub name: String,
    /// Namespace of the source resource
    pub namespace: String,
}

fn default_source_kind() -> String {
    "GitRepository".to_string()
}

/// Status of the SecretManagerConfig resource
///
/// Tracks reconciliation state, errors, and metrics.
#[derive(Debug, Clone, Deserialize, Serialize, Default, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretManagerConfigStatus {
    /// Conditions represent the latest available observations
    #[serde(default)]
    pub conditions: Vec<Condition>,
    /// Observed generation
    #[serde(default)]
    pub observed_generation: Option<i64>,
    /// Last reconciliation time
    #[serde(default)]
    pub last_reconcile_time: Option<String>,
    /// Number of secrets synced
    #[serde(default)]
    pub secrets_synced: Option<i32>,
}

/// Condition represents a status condition for the resource
///
/// Used to track readiness, errors, and other state information.
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    /// Type of condition
    pub r#type: String,
    /// Status of condition (True, False, Unknown)
    pub status: String,
    /// Last transition time
    #[serde(default)]
    pub last_transition_time: Option<String>,
    /// Reason for condition
    #[serde(default)]
    pub reason: Option<String>,
    /// Message describing condition
    #[serde(default)]
    pub message: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "secret_manager_controller=info".into()),
        )
        .init();

    info!("Starting Secret Manager Controller");

    // Initialize metrics
    metrics::register_metrics()?;

    // Create server state
    let server_state = Arc::new(ServerState {
        is_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    });

    // Start HTTP server for metrics and probes
    let server_state_clone = server_state.clone();
    let server_port = std::env::var("METRICS_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .unwrap_or(8080);
    
    tokio::spawn(async move {
        if let Err(e) = start_server(server_port, server_state_clone).await {
            error!("HTTP server error: {}", e);
        }
    });

    // Create Kubernetes client
    let client = Client::try_default().await?;

    // Create API for SecretManagerConfig CRD - watch all namespaces
    // This allows developers to deploy SecretManagerConfig resources in any namespace
    let configs: Api<SecretManagerConfig> = Api::all(client.clone());

    // Create reconciler context
    let reconciler = Arc::new(Reconciler::new(client.clone()).await?);

    // Mark as ready
    server_state.is_ready.store(true, std::sync::atomic::Ordering::Relaxed);

    // Create controller
    Controller::new(configs, watcher::Config::default())
        .shutdown_on_signal()
        .run(
            reconciler::Reconciler::reconcile,
            |obj, error, _ctx| {
                error!("Reconciliation error for {}: {:?}", 
                    obj.metadata.name.as_deref().unwrap_or("unknown"), error);
                metrics::increment_reconciliation_errors();
                Action::requeue(std::time::Duration::from_secs(60))
            },
            reconciler.clone(),
        )
        .for_each(|_| std::future::ready(()))
        .await;

    Ok(())
}

