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
    api::{Api, ListParams},
    Client, CustomResource,
};
use kube_runtime::{controller::Action, watcher, Controller};

mod constants;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

pub mod controller;
pub mod observability;
pub mod provider;

use controller::reconciler::{reconcile, Reconciler, TriggerSource};
use controller::server::{start_server, ServerState};

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
    shortname = "smc",
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}, {"name":"Description", "type":"string", "jsonPath":".status.description"}, {"name":"Ready", "type":"string", "jsonPath":".status.conditions[?(@.type==\"Ready\")].status"}"#
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
    /// Config store configuration for routing application.properties to config stores
    /// When enabled, properties are stored individually in config stores instead of as a JSON blob in secret stores
    #[serde(default)]
    pub configs: Option<ConfigsConfig>,
    /// OpenTelemetry configuration for distributed tracing (optional)
    /// Supports OTLP exporter (to OpenTelemetry Collector) and Datadog direct export
    /// If not specified, OpenTelemetry is disabled and standard tracing is used
    #[serde(default)]
    pub otel: Option<OtelConfig>,
    /// GitRepository pull update interval
    /// How often to check for updates from the GitRepository source
    /// Format: Kubernetes duration string (e.g., "1m", "5m", "1h")
    /// Minimum: 1m (60 seconds) - shorter intervals may hit API rate limits
    /// Default: "5m" (5 minutes)
    /// Recommended: 5m or greater to avoid rate limiting
    #[serde(default = "default_git_repository_pull_interval")]
    pub git_repository_pull_interval: String,
    /// Reconcile interval
    /// How often to reconcile secrets between Git and cloud providers (Secret Manager or Parameter Manager)
    /// Format: Kubernetes duration string (e.g., "1m", "30s", "5m")
    /// Default: "1m" (1 minute)
    #[serde(default = "default_reconcile_interval")]
    pub reconcile_interval: String,
    /// Enable diff discovery
    /// When enabled, detects if secrets have been tampered with in Secret Manager or Parameter Manager
    /// and logs warnings when differences are found between Git (source of truth) and cloud provider
    /// Default: true (enabled)
    #[serde(default = "default_true")]
    pub diff_discovery: bool,
    /// Enable update triggers
    /// When enabled, automatically updates cloud provider secrets if Git values have changed since last pull
    /// This ensures Git remains the source of truth
    /// Default: true (enabled)
    #[serde(default = "default_true")]
    pub trigger_update: bool,
    /// Suspend reconciliation
    /// When true, the controller will skip reconciliation for this resource
    /// Useful for troubleshooting or during intricate CI/CD transitions where secrets need to be carefully managed
    /// Manual reconciliation via msmctl will also be blocked when suspended
    /// Default: false (reconciliation enabled)
    #[serde(default = "default_false")]
    pub suspend: bool,
    /// Suspend GitRepository pulls
    /// When true, suspends Git pulls from the referenced GitRepository but continues reconciliation with the last pulled commit
    /// This is useful when you want to freeze the Git state but keep syncing secrets from the current commit
    /// The controller will automatically patch the GitRepository resource to set suspend: true/false
    /// Default: false (Git pulls enabled)
    #[serde(default = "default_false")]
    pub suspend_git_pulls: bool,
}

/// Cloud provider configuration
/// Supports GCP, AWS, and Azure Secret Manager
/// Kubernetes sends data in format: {"type": "gcp", "gcp": {...}}
/// We use externally tagged format and ignore the "type" field during deserialization
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ProviderConfig {
    /// Google Cloud Platform Secret Manager
    #[serde(rename = "gcp")]
    Gcp(GcpConfig),
    /// Amazon Web Services Secrets Manager
    #[serde(rename = "aws")]
    Aws(AwsConfig),
    /// Microsoft Azure Key Vault
    #[serde(rename = "azure")]
    Azure(AzureConfig),
}

impl<'de> serde::Deserialize<'de> for ProviderConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct ProviderConfigVisitor;

        impl<'de> Visitor<'de> for ProviderConfigVisitor {
            type Value = ProviderConfig;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a provider config object with gcp, aws, or azure field")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                // Try to find the variant key (gcp, aws, or azure)
                let mut gcp: Option<GcpConfig> = None;
                let mut aws: Option<AwsConfig> = None;
                let mut azure: Option<AzureConfig> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "gcp" => {
                            if gcp.is_some() {
                                return Err(de::Error::duplicate_field("gcp"));
                            }
                            // Deserialize GcpConfig from the nested object
                            // The JSON has {"projectId": "..."} which should map to project_id via rename_all
                            gcp = Some(map.next_value::<GcpConfig>().map_err(|e| {
                                de::Error::custom(format!("Failed to deserialize GcpConfig: {e}"))
                            })?);
                        }
                        "aws" => {
                            if aws.is_some() {
                                return Err(de::Error::duplicate_field("aws"));
                            }
                            aws = Some(map.next_value()?);
                        }
                        "azure" => {
                            if azure.is_some() {
                                return Err(de::Error::duplicate_field("azure"));
                            }
                            azure = Some(map.next_value()?);
                        }
                        "type" => {
                            // Ignore the "type" field - it's redundant
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                        _ => {
                            // Ignore unknown fields (like "type")
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                match (gcp, aws, azure) {
                    (Some(config), None, None) => Ok(ProviderConfig::Gcp(config)),
                    (None, Some(config), None) => Ok(ProviderConfig::Aws(config)),
                    (None, None, Some(config)) => Ok(ProviderConfig::Azure(config)),
                    (None, None, None) => Err(de::Error::missing_field("gcp, aws, or azure")),
                    _ => Err(de::Error::custom("multiple provider types specified")),
                }
            }
        }

        deserializer.deserialize_map(ProviderConfigVisitor)
    }
}

/// GCP configuration for Secret Manager
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GcpConfig {
    /// GCP project ID for Secret Manager
    pub project_id: String,
    /// GCP authentication configuration. If not specified, defaults to Workload Identity (recommended).
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
    #[serde(default)]
    pub auth: Option<AwsAuthConfig>,
}

/// Azure configuration for Key Vault
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AzureConfig {
    /// Azure Key Vault name
    pub vault_name: String,
    /// Azure authentication configuration. If not specified, defaults to Workload Identity (recommended).
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

/// Config store configuration for routing application.properties to config stores
/// When enabled, properties are stored individually in config stores instead of as a JSON blob in secret stores
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigsConfig {
    /// Enable config store sync (default: false for backward compatibility)
    /// When true, application.properties files are routed to config stores
    /// When false, properties are stored as a JSON blob in secret stores (current behavior)
    #[serde(default)]
    pub enabled: bool,
    /// AWS-specific: Parameter path prefix
    /// Only applies when provider.type == aws
    /// Optional: defaults to /{prefix}/{environment} if not specified
    /// Example: /my-service/dev
    #[serde(default)]
    pub parameter_path: Option<String>,
    /// GCP-specific: Store type (default: SecretManager)
    /// Only applies when provider.type == gcp
    /// - SecretManager: Store configs as individual secrets in Secret Manager (interim solution)
    /// - ParameterManager: Store configs in Parameter Manager (future, after ESO contribution)
    #[serde(default)]
    pub store: Option<ConfigStoreType>,
    /// Azure-specific: App Configuration endpoint
    /// Only applies when provider.type == azure
    /// Optional: defaults to auto-detection from vault region if not specified
    /// Example: https://my-app-config.azconfig.io
    #[serde(default)]
    pub app_config_endpoint: Option<String>,
}

/// GCP config store type
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigStoreType {
    /// Store configs as individual secrets in Secret Manager (interim solution)
    /// This is the default and recommended interim approach until Parameter Manager support is contributed to ESO
    SecretManager,
    /// Store configs in Parameter Manager (future)
    /// Requires ESO contribution for Kubernetes consumption
    #[serde(rename = "ParameterManager")]
    ParameterManager,
}

impl JsonSchema for ConfigStoreType {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("ConfigStoreType")
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        // Generate a structural schema for Kubernetes CRD
        // Use enum with nullable support (not anyOf)
        let schema_value = serde_json::json!({
            "type": "string",
            "enum": ["secretManager", "ParameterManager"],
            "description": "GCP config store type. SecretManager: Store configs as individual secrets in Secret Manager (interim solution). ParameterManager: Store configs in Parameter Manager (future, after ESO contribution)."
        });
        Schema::try_from(schema_value).expect("Failed to create Schema for ConfigStoreType")
    }
}

/// GCP authentication configuration
/// Only supports Workload Identity (recommended and default)
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum GcpAuthConfig {
    /// Use Workload Identity for authentication (DEFAULT)
    /// Requires GKE cluster with Workload Identity enabled
    /// This is the recommended authentication method and is used by default when auth is not specified
    WorkloadIdentity {
        /// GCP service account email to impersonate
        /// Format: <service-account-name>@<project-id>.iam.gserviceaccount.com
        service_account_email: String,
    },
}

/// AWS authentication configuration
/// Only supports IRSA (IAM Roles for Service Accounts) - recommended and default
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum AwsAuthConfig {
    /// Use IRSA (IAM Roles for Service Accounts) for authentication (DEFAULT)
    /// Requires EKS cluster with IRSA enabled and service account annotation
    /// This is the recommended authentication method and is used by default when auth is not specified
    Irsa {
        /// AWS IAM role ARN to assume
        /// Format: arn:aws:iam::<account-id>:role/<role-name>
        role_arn: String,
    },
}

/// Azure authentication configuration
/// Only supports Workload Identity (recommended and default)
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum AzureAuthConfig {
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

fn default_git_repository_pull_interval() -> String {
    "5m".to_string()
}

fn default_reconcile_interval() -> String {
    "1m".to_string()
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

/// Status of the SecretManagerConfig resource
///
/// Tracks reconciliation state, errors, and metrics.
#[derive(Debug, Clone, Deserialize, Serialize, Default, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretManagerConfigStatus {
    /// Current phase of reconciliation
    /// Values: Pending, Started, Cloning, Updating, Failed, Ready
    #[serde(default)]
    pub phase: Option<String>,
    /// Human-readable description of current state
    /// Examples: "Clone failed, repo unavailable", "Reconciling secrets to Secret Manager", "Reconciling properties to Parameter Manager"
    #[serde(default)]
    pub description: Option<String>,
    /// Conditions represent the latest available observations
    #[serde(default)]
    pub conditions: Vec<Condition>,
    /// Observed generation
    #[serde(default)]
    pub observed_generation: Option<i64>,
    /// Last reconciliation time
    #[serde(default)]
    pub last_reconcile_time: Option<String>,
    /// Next scheduled reconciliation time (RFC3339)
    /// Used to persist periodic reconciliation schedule across watch restarts
    #[serde(default)]
    pub next_reconcile_time: Option<String>,
    /// Number of secrets synced
    #[serde(default)]
    pub secrets_synced: Option<i32>,
    /// SOPS decryption status
    /// Values: Success, TransientFailure, PermanentFailure, NotApplicable
    /// NotApplicable means no SOPS-encrypted files were processed
    #[serde(default)]
    pub decryption_status: Option<String>,
    /// Timestamp of last SOPS decryption attempt (RFC3339)
    /// Updated whenever a SOPS-encrypted file is processed
    #[serde(default)]
    pub last_decryption_attempt: Option<String>,
    /// Last SOPS decryption error message (if any)
    /// Only set when decryption fails
    #[serde(default)]
    pub last_decryption_error: Option<String>,
    /// Whether SOPS private key is available in the resource namespace
    /// Updated when key secret changes (via watch)
    /// Used to avoid redundant API calls on every reconcile
    #[serde(default)]
    pub sops_key_available: Option<bool>,
    /// Name of the SOPS key secret found in the resource namespace
    /// Example: "sops-private-key"
    #[serde(default)]
    pub sops_key_secret_name: Option<String>,
    /// Namespace where the SOPS key was found
    /// Usually the resource namespace, but could be controller namespace if fallback
    #[serde(default)]
    pub sops_key_namespace: Option<String>,
    /// Last time the SOPS key availability was checked (RFC3339)
    #[serde(default)]
    pub sops_key_last_checked: Option<String>,
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
    // Configure rustls crypto provider FIRST, before any other operations
    // Required for rustls 0.23+ when no default provider is set via features
    // This must be called synchronously before any async operations that use rustls
    // We use ring as the crypto provider
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

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

    // Start HTTP server for metrics and probes
    // We start it in a background task but wait for it to be ready before proceeding
    let server_state_clone = server_state.clone();
    let server_port = std::env::var("METRICS_PORT")
        .unwrap_or_else(|_| constants::DEFAULT_METRICS_PORT.to_string())
        .parse::<u16>()
        .unwrap_or(constants::DEFAULT_METRICS_PORT);

    // Start server in background task
    let server_handle = tokio::spawn(async move {
        if let Err(e) = start_server(server_port, server_state_clone).await {
            error!("HTTP server error: {}", e);
        }
    });

    // Poll server startup - wait for it to be ready before proceeding
    // This ensures readiness probes pass immediately after server starts
    let startup_timeout =
        std::time::Duration::from_secs(constants::DEFAULT_SERVER_STARTUP_TIMEOUT_SECS);
    let poll_interval =
        std::time::Duration::from_millis(constants::DEFAULT_SERVER_POLL_INTERVAL_MS);
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

    // Create Kubernetes client
    let client = Client::try_default().await?;

    // Create API for SecretManagerConfig CRD - watch all namespaces
    // This allows developers to deploy SecretManagerConfig resources in any namespace
    let configs: Api<SecretManagerConfig> = Api::all(client.clone());

    // Create reconciler context
    let reconciler = Arc::new(Reconciler::new(client.clone()).await?);

    // Start watching for SOPS private key secret changes
    // This allows hot-reloading the key without restarting the controller
    controller::reconciler::start_sops_key_watch(reconciler.clone());

    // Note: GitRepository and ArgoCD Application changes are handled by the main controller watch.
    // When SecretManagerConfig resources are reconciled, they fetch the latest source,
    // ensuring source changes are picked up without restarting the controller.
    // SOPS secrets are watched separately for hot-reloading.

    // Check if CRD is queryable and reconcile existing resources before starting the watch
    // This ensures existing resources are reconciled when the controller starts
    // CRITICAL: Without this, resources created before controller deployment won't be reconciled
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
                    let resources = resources_by_namespace.get(*namespace).unwrap();
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
        }
        Err(e) => {
            error!("CRD is not queryable; {:?}. Is the CRD installed?", e);
            error!("Installation: kubectl apply -f config/crd/secretmanagerconfig.yaml");
            // Don't exit - let the controller start and it will handle the error gracefully
            warn!("Continuing despite CRD queryability check failure - controller will retry");
            warn!(error = %e, "CRD queryability check failed");
        }
    }

    // Server is already marked as ready by start_server() once it binds
    // This ensures readiness probes pass before we start reconciling
    info!("Controller initialized, starting watch loop...");

    // Create controller with any_semantic() to watch for all semantic changes (create, update, delete)
    // This ensures the controller picks up newly created resources
    info!("Starting controller watch loop...");

    // Set up graceful shutdown handler - mark server as not ready when shutting down
    let server_state_shutdown = server_state.clone();

    // Use Arc for shared backoff state
    let backoff_duration_ms = Arc::new(std::sync::atomic::AtomicU64::new(
        constants::DEFAULT_BACKOFF_START_MS,
    ));
    let max_backoff_ms = constants::DEFAULT_BACKOFF_MAX_MS;

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
        let watch_span = tracing::span!(
            tracing::Level::INFO,
            "controller.watch",
            operation = "watch_loop"
        );
        let _watch_guard = watch_span.enter();

        info!("Starting controller watch loop...");
        let controller_future = Controller::new(configs.clone(), watcher::Config::default().any_semantic())
            .shutdown_on_signal()
            .run(
                |obj, ctx| {
                    let reconciler = ctx.clone();
                    let name = obj.metadata.name.as_deref().unwrap_or("unknown").to_string();
                    let namespace = obj.metadata.namespace.as_deref().unwrap_or("default").to_string();
                    let resource_version = obj.metadata.resource_version.as_deref().unwrap_or("unknown").to_string();
                    let generation = obj.metadata.generation.unwrap_or(0);
                    let observed_generation = obj.status.as_ref()
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
                        let is_manual_trigger = obj.metadata.annotations.as_ref()
                            .and_then(|ann| ann.get("secret-management.microscaler.io/reconcile"))
                            .is_some();

                        // Check if this is a periodic reconciliation (requeue-triggered)
                        // Periodic reconciliations should run even if generation matches, as they check
                        // for external state changes (secrets in cloud provider, Git repository updates)
                        // We use next_reconcile_time from status to persist the schedule across watch restarts
                        // CRITICAL: If generation matches but next_reconcile_time has passed, this is a periodic reconciliation
                        let is_periodic_reconcile = if generation == observed_generation && observed_generation > 0 {
                            if let Some(status) = &obj.status {
                                if let Some(next_reconcile_time) = &status.next_reconcile_time {
                                    // Check if next_reconcile_time has passed (with 2s tolerance for timing)
                                    if let Ok(next_time) = chrono::DateTime::parse_from_rfc3339(next_reconcile_time) {
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
                        if generation == observed_generation && observed_generation > 0 && !is_manual_trigger && !is_periodic_reconcile {
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
                                has_next_reconcile_time = obj.status.as_ref()
                                    .and_then(|s| s.next_reconcile_time.as_ref())
                                    .is_some(),
                                next_reconcile_time = obj.status.as_ref()
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

                        let result = reconcile(obj, reconciler.clone(), trigger_source).await;

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
                },
                |obj, error, ctx| {
                    let name = obj.metadata.name.as_deref().unwrap_or("unknown");
                    let namespace = obj.metadata.namespace.as_deref().unwrap_or("default");

                    // Create error span for reconciliation errors
                    let error_span = tracing::span!(
                        tracing::Level::ERROR,
                        "controller.watch.reconciliation_error",
                        resource.name = name,
                        resource.namespace = namespace,
                        error = %error
                    );
                    let _error_guard = error_span.enter();

                    error!(
                        "Reconciliation error for {}: {:?}",
                        name,
                        error
                    );
                    observability::metrics::increment_reconciliation_errors();

                    // Calculate Fibonacci backoff based on error count for this resource
                    // This prevents blocking watch/timer paths when many resources fail
                    // Backoff state is tracked per resource to avoid cross-resource interference
                    // Moved from reconciler to error_policy() layer to prevent deadlocks
                    let resource_key = format!("{}/{}", namespace, name);
                    let backoff_seconds = match ctx.backoff_states.lock() {
                        Ok(mut states) => {
                            use crate::controller::backoff::FibonacciBackoff;
                            use crate::controller::reconciler::BackoffState;
                            let state = states
                                .entry(resource_key.clone())
                                .or_insert_with(|| BackoffState {
                                    backoff: FibonacciBackoff::new(1, 10), // 1 minute min, 10 minutes max
                                    error_count: 0,
                                });
                            state.increment_error();
                            let backoff = state.backoff.next_backoff_seconds();
                            let error_count = state.error_count;
                            (backoff, error_count)
                        }
                        Err(e) => {
                            warn!("Failed to lock backoff_states: {}, using default backoff", e);
                            (constants::DEFAULT_RECONCILIATION_ERROR_REQUEUE_SECS, 0)
                        }
                    };

                    let next_trigger_time =
                        chrono::Utc::now() + chrono::Duration::seconds(backoff_seconds.0 as i64);

                    info!(
                        "🔄 Retrying with Fibonacci backoff: {}s (error count: {}, trigger source: error-backoff)",
                        backoff_seconds.0, backoff_seconds.1
                    );
                    info!(
                        "📅 Next retry scheduled: {} (in {}s, trigger source: error-backoff)",
                        next_trigger_time.to_rfc3339(),
                        backoff_seconds.0
                    );

                    observability::metrics::increment_requeues_total("error-backoff");
                    Action::requeue(std::time::Duration::from_secs(backoff_seconds.0))
                },
                reconciler.clone(),
            )
            .filter_map(move |x| {
                let backoff = backoff_clone.clone();
                async move {
                    match &x {
                        Ok(_) => {
                            // Successful event, reset backoff on success
                            backoff.store(constants::DEFAULT_BACKOFF_START_MS, std::sync::atomic::Ordering::Relaxed);
                            debug!("watch.event.success");
                            Some(x)
                        }
                        Err(e) => {
                            // Handle watch errors with proper classification
                            let error_string = format!("{e:?}");
                            let error_span = tracing::span!(
                                tracing::Level::WARN,
                                "controller.watch.error",
                                error = error_string.as_str()
                            );
                            let _error_guard = error_span.enter();

                            // Check for specific error types
                            let is_401 = error_string.contains("401")
                                || error_string.contains("Unauthorized")
                                || error_string.contains("WatchFailed");
                            let is_410 = error_string.contains("410")
                                || error_string.contains("too old resource version")
                                || error_string.contains("Expired")
                                || error_string.contains("Gone");
                            let is_429 = error_string.contains("429")
                                || error_string.contains("storage is (re)initializing")
                                || error_string.contains("TooManyRequests");
                            let is_not_found = error_string.contains("ObjectNotFound")
                                || (error_string.contains("404") && error_string.contains("not found"));

                            if is_401 {
                                // Authentication error - RBAC may have been revoked or token expired
                                error!("❌ Watch authentication failed (401 Unauthorized) - RBAC may have been revoked or token expired");
                                error!("🔍 SRE Diagnostics:");
                                error!("   1. Verify ClusterRole 'secret-manager-controller' still exists:");
                                error!("      kubectl get clusterrole secret-manager-controller");
                                error!("   2. Verify ClusterRoleBinding still binds ServiceAccount:");
                                error!("      kubectl get clusterrolebinding secret-manager-controller -o yaml");
                                error!("   3. Verify ServiceAccount still exists:");
                                error!("      kubectl get sa secret-manager-controller -n microscaler-system");
                                error!("   4. Check if pod ServiceAccount token is valid:");
                                error!("      kubectl get pod -n microscaler-system -l app=secret-manager-controller -o jsonpath='{{{{.spec.serviceAccountName}}}}'");
                                error!("   5. Verify RBAC permissions are still active:");
                                error!("      kubectl auth can-i list secretmanagerconfigs --as=system:serviceaccount:microscaler-system:secret-manager-controller --all-namespaces");
                                error!("   6. If RBAC was recently changed, restart the controller pod:");
                                error!("      kubectl delete pod -n microscaler-system -l app=secret-manager-controller");
                                warn!("⏳ Waiting {}s before retrying watch (RBAC may need time to propagate)...", constants::DEFAULT_WATCH_RESTART_DELAY_SECS);
                                tokio::time::sleep(std::time::Duration::from_secs(constants::DEFAULT_WATCH_RESTART_DELAY_SECS)).await;
                                None // Filter out to allow restart
                            } else if is_410 {
                                // Resource version expired - this is normal during pod restarts
                                warn!("Watch resource version expired (410) - this is normal during pod restarts, watch will restart");
                                warn!(error_type = "410", "watch.error.resource_version_expired");
                                None // Filter out to allow restart
                            } else if is_429 {
                                // Storage reinitializing - back off and let it restart
                                let current_backoff = backoff.load(std::sync::atomic::Ordering::Relaxed);
                                warn!("API server storage reinitializing (429), backing off for {}ms before restart...", current_backoff);
                                tokio::time::sleep(std::time::Duration::from_millis(current_backoff)).await;
                                // Exponential backoff, max configured value
                                let new_backoff = std::cmp::min(current_backoff * 2, max_backoff_ms);
                                backoff.store(new_backoff, std::sync::atomic::Ordering::Relaxed);
                                None // Filter out to allow restart
                            } else if is_not_found {
                                // Resource not found - this is normal for deleted resources
                                warn!("Resource not found (likely deleted), continuing watch...");
                                Some(x) // Continue - this is expected
                            } else {
                                // Other errors - log but continue
                                error!("Controller stream error: {:?}", e);
                                // For unknown errors, wait a bit before restarting
                                tokio::time::sleep(std::time::Duration::from_secs(constants::DEFAULT_WATCH_RESTART_DELAY_SECS)).await;
                                None // Filter out to allow restart
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
        warn!(
            "Controller watch stream ended, restarting in {} seconds...",
            constants::DEFAULT_WATCH_RESTART_DELAY_AFTER_END_SECS
        );
        tokio::time::sleep(std::time::Duration::from_secs(
            constants::DEFAULT_WATCH_RESTART_DELAY_AFTER_END_SECS,
        ))
        .await;
    }

    info!("Controller stopped gracefully");

    // Shutdown OpenTelemetry tracer provider if it was initialized
    observability::otel::shutdown_otel(otel_tracer_provider);

    Ok(())
}
