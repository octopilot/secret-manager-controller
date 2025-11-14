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
    Client, CustomResource,
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

pub mod aws;
pub mod azure;
pub mod gcp;
pub mod kustomize;
pub mod metrics;
pub mod otel;
pub mod parser;
pub mod provider;
pub mod reconciler;
pub mod server;

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
///     namespace: microscaler-system
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
    /// Cloud provider configuration - supports GCP, AWS, and Azure
    pub provider: ProviderConfig,
    /// Secrets sync configuration
    pub secrets: SecretsConfig,
    /// OpenTelemetry configuration for distributed tracing (optional)
    /// Supports OTLP exporter (to OpenTelemetry Collector) and Datadog direct export
    /// If not specified, OpenTelemetry is disabled and standard tracing is used
    #[serde(default)]
    pub otel: Option<OtelConfig>,
}

/// Cloud provider configuration
/// Supports GCP, AWS, and Azure Secret Manager
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ProviderConfig {
    /// Google Cloud Platform Secret Manager
    Gcp(GcpConfig),
    /// Amazon Web Services Secrets Manager
    Aws(AwsConfig),
    /// Microsoft Azure Key Vault (stub - not yet implemented)
    Azure(AzureConfig),
}

/// GCP configuration for Secret Manager
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GcpConfig {
    /// GCP project ID for Secret Manager
    pub project_id: String,
    /// GCP authentication configuration. If not specified, defaults to Workload Identity (recommended).
    /// JSON credentials are available but will be deprecated once GCP deprecates them.
    #[serde(default)]
    pub auth: Option<GcpAuthConfig>,
}

/// AWS configuration for Secrets Manager
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AwsConfig {
    /// AWS region for Secrets Manager (e.g., "us-east-1", "eu-west-1")
    pub region: String,
    /// AWS authentication configuration. If not specified, defaults to IRSA (IAM Roles for Service Accounts) - recommended.
    /// Access Keys are available but will be deprecated once AWS deprecates them.
    #[serde(default)]
    pub auth: Option<AwsAuthConfig>,
}

/// Azure configuration for Key Vault (stub)
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AzureConfig {
    /// Azure Key Vault name
    pub vault_name: String,
    /// Azure authentication configuration. If not specified, defaults to Workload Identity (recommended).
    /// Service Principal credentials are available but will be deprecated once Azure deprecates them.
    #[serde(default)]
    pub auth: Option<AzureAuthConfig>,
}

/// Secrets sync configuration
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretsConfig {
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
    /// Matches kustomize-google-secret-manager prefix behavior
    #[serde(default)]
    pub prefix: Option<String>,
    /// Secret name suffix (optional)
    /// Matches kustomize-google-secret-manager suffix behavior
    /// Common use cases: environment identifiers, tags, etc.
    #[serde(default)]
    pub suffix: Option<String>,
}

/// GCP authentication configuration
/// Supports both JSON credentials and Workload Identity
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum GcpAuthConfig {
    /// Use JSON credentials from a Kubernetes secret
    /// 
    /// ⚠️ DEPRECATED: JSON credentials are available but will be deprecated once GCP deprecates them.
    /// Workload Identity is the recommended and default authentication method.
    #[deprecated(note = "JSON credentials will be deprecated once GCP deprecates them. Use WorkloadIdentity instead.")]
    JsonCredentials {
        /// Secret name containing the JSON credentials
        /// Defaults to "gcp-secret-manager-credentials" if not specified
        #[serde(default = "default_json_secret_name")]
        secret_name: String,
        /// Secret namespace
        /// Defaults to controller namespace (microscaler-system) if not specified
        #[serde(default)]
        secret_namespace: Option<String>,
        /// Key in the secret containing the JSON credentials
        /// Defaults to "key.json" if not specified
        #[serde(default = "default_json_secret_key")]
        secret_key: String,
    },
    /// Use Workload Identity for authentication (DEFAULT)
    /// Requires GKE cluster with Workload Identity enabled
    /// This is the recommended authentication method and is used by default when auth is not specified
    WorkloadIdentity {
        /// GCP service account email to impersonate
        /// Format: <service-account-name>@<project-id>.iam.gserviceaccount.com
        service_account_email: String,
    },
}

fn default_json_secret_name() -> String {
    "gcp-secret-manager-credentials".to_string()
}

fn default_json_secret_key() -> String {
    "key.json".to_string()
}

/// AWS authentication configuration
/// Supports both Access Keys and IRSA (IAM Roles for Service Accounts)
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum AwsAuthConfig {
    /// Use Access Keys from a Kubernetes secret
    /// 
    /// ⚠️ DEPRECATED: Access Keys are available but will be deprecated once AWS deprecates them.
    /// IRSA (IAM Roles for Service Accounts) is the recommended and default authentication method.
    #[deprecated(note = "Access Keys will be deprecated once AWS deprecates them. Use Irsa instead.")]
    AccessKeys {
        /// Secret name containing the AWS credentials
        /// Defaults to "aws-secret-manager-credentials" if not specified
        #[serde(default = "default_aws_secret_name")]
        secret_name: String,
        /// Secret namespace
        /// Defaults to controller namespace (microscaler-system) if not specified
        #[serde(default)]
        secret_namespace: Option<String>,
        /// Key in the secret containing the AWS access key ID
        /// Defaults to "access-key-id" if not specified
        #[serde(default = "default_aws_access_key_id_key")]
        access_key_id_key: String,
        /// Key in the secret containing the AWS secret access key
        /// Defaults to "secret-access-key" if not specified
        #[serde(default = "default_aws_secret_access_key_key")]
        secret_access_key_key: String,
    },
    /// Use IRSA (IAM Roles for Service Accounts) for authentication (DEFAULT)
    /// Requires EKS cluster with IRSA enabled and service account annotation
    /// This is the recommended authentication method and is used by default when auth is not specified
    Irsa {
        /// AWS IAM role ARN to assume
        /// Format: arn:aws:iam::<account-id>:role/<role-name>
        role_arn: String,
    },
}

fn default_aws_secret_name() -> String {
    "aws-secret-manager-credentials".to_string()
}

fn default_aws_access_key_id_key() -> String {
    "access-key-id".to_string()
}

fn default_aws_secret_access_key_key() -> String {
    "secret-access-key".to_string()
}

/// Azure authentication configuration (stub)
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum AzureAuthConfig {
    /// Use Service Principal from a Kubernetes secret
    /// 
    /// ⚠️ DEPRECATED: Service Principal credentials are available but will be deprecated once Azure deprecates them.
    /// Workload Identity is the recommended and default authentication method.
    #[deprecated(note = "Service Principal credentials will be deprecated once Azure deprecates them. Use WorkloadIdentity instead.")]
    ServicePrincipal {
        /// Secret name containing the Azure credentials
        secret_name: String,
        /// Secret namespace
        #[serde(default)]
        secret_namespace: Option<String>,
    },
    /// Use Workload Identity for authentication (DEFAULT)
    /// Requires AKS cluster with Workload Identity enabled
    /// This is the recommended authentication method and is used by default when auth is not specified
    WorkloadIdentity {
        /// Azure service principal client ID
        client_id: String,
    },
}

/// OpenTelemetry configuration
/// Supports both OTLP exporter and Datadog direct export
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum OtelConfig {
    /// Use OTLP exporter to send traces to an OpenTelemetry Collector
    Otlp {
        /// OTLP endpoint URL (e.g., "http://otel-collector:4317")
        endpoint: String,
        /// Service name for traces (defaults to "secret-manager-controller")
        #[serde(default)]
        service_name: Option<String>,
        /// Service version for traces (defaults to Cargo package version)
        #[serde(default)]
        service_version: Option<String>,
        /// Deployment environment (e.g., "dev", "prod")
        #[serde(default)]
        environment: Option<String>,
    },
    /// Use Datadog OpenTelemetry exporter (direct to Datadog)
    Datadog {
        /// Service name for traces (defaults to "secret-manager-controller")
        #[serde(default)]
        service_name: Option<String>,
        /// Service version for traces (defaults to Cargo package version)
        #[serde(default)]
        service_version: Option<String>,
        /// Deployment environment (e.g., "dev", "prod")
        #[serde(default)]
        environment: Option<String>,
        /// Datadog site (e.g., "datadoghq.com", "us3.datadoghq.com")
        /// If not specified, uses DD_SITE environment variable or defaults to "datadoghq.com"
        #[serde(default)]
        site: Option<String>,
        /// Datadog API key
        /// If not specified, uses DD_API_KEY environment variable
        #[serde(default)]
        api_key: Option<String>,
    },
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
    // Initialize OpenTelemetry first (if configured)
    // This will set up tracing with Otel support
    // Note: Otel config can come from CRD, but we initialize early from env vars
    // Per-resource Otel config is handled in the reconciler
    let otel_tracer_provider = otel::init_otel(None)
        .context("Failed to initialize OpenTelemetry")?;

    // If Otel wasn't initialized, use standard tracing subscriber
    if otel_tracer_provider.is_none() {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "secret_manager_controller=info".into()),
            )
            .init();
    }

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

    info!("Controller stopped");
    
    // Shutdown OpenTelemetry tracer provider if it was initialized
    otel::shutdown_otel(otel_tracer_provider);
    
    Ok(())
}

