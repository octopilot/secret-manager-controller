//! # SecretManagerConfig Spec
//!
//! Main CRD specification types and default values.

use serde::{Deserialize, Serialize};

/// SecretManagerConfig Custom Resource Definition
///
/// This CRD defines the configuration for syncing secrets from GitOps repositories
/// to cloud secret managers (GCP, AWS, Azure).
///
/// # Example
///
/// ```yaml
/// apiVersion: secret-management.microscaler.io/v1beta1
/// kind: SecretManagerConfig
/// metadata:
///   name: my-service-secrets
///   namespace: default
/// spec:
///   sourceRef:
///     kind: GitRepository
///     name: my-repo
///     namespace: microscaler-system
///   provider:
///     gcp:
///       projectId: my-gcp-project
///   secrets:
///     environment: dev
///     kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
/// ```
#[derive(kube::CustomResource, Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[kube(
    kind = "SecretManagerConfig",
    group = "secret-management.microscaler.io",
    version = "v1beta1",
    namespaced,
    status = "crate::crd::SecretManagerConfigStatus",
    shortname = "smc",
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}, {"name":"Description", "type":"string", "jsonPath":".status.description"}, {"name":"Ready", "type":"string", "jsonPath":".status.conditions[?(@.type==\"Ready\")].status"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct SecretManagerConfigSpec {
    /// Source reference - supports FluxCD GitRepository and ArgoCD Application
    /// This makes the controller GitOps-agnostic
    pub source_ref: crate::crd::SourceRef,
    /// Cloud provider configuration - supports GCP, AWS, and Azure
    pub provider: crate::crd::ProviderConfig,
    /// Secrets sync configuration
    pub secrets: crate::crd::SecretsConfig,
    /// Config store configuration for routing application.properties to config stores
    /// When enabled, properties are stored individually in config stores instead of as a JSON blob in secret stores
    #[serde(default)]
    pub configs: Option<crate::crd::ConfigsConfig>,
    /// OpenTelemetry configuration for distributed tracing (optional)
    /// Supports OTLP exporter (to OpenTelemetry Collector) and Datadog direct export
    /// If not specified, OpenTelemetry is disabled and standard tracing is used
    #[serde(default)]
    pub otel: Option<crate::crd::OtelConfig>,
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
    /// Notification configuration for drift detection alerts
    /// Supports both FluxCD (via Provider reference) and ArgoCD (via Application annotations)
    /// When drift is detected, notifications are sent according to this configuration
    #[serde(default)]
    pub notifications: Option<crate::crd::NotificationConfig>,
    /// Logging configuration for fine-grained control over log verbosity
    /// Allows setting different log levels for different operations (secrets, properties, reconciliation, etc.)
    /// Default: INFO for most operations, WARN for diff discovery, DEBUG for SOPS and provider operations
    #[serde(default)]
    pub logging: Option<crate::crd::LoggingConfig>,
    /// Hot reload configuration for controller-level settings
    /// Controls whether the controller watches for ConfigMap changes and hot-reloads configuration
    /// When enabled, watches the specified ConfigMap and reloads configuration without pod restart
    /// Environment variables are populated from the ConfigMap using `envFrom` in the deployment
    /// Default: disabled (false) - most users rely on pod restarts via Reloader or manual updates
    #[serde(default)]
    pub hot_reload: Option<crate::crd::HotReloadConfig>,
}

/// Default value for source kind
pub fn default_source_kind() -> String {
    "GitRepository".to_string()
}

/// Default value for GitRepository pull interval
pub fn default_git_repository_pull_interval() -> String {
    "5m".to_string()
}

/// Default value for reconcile interval
pub fn default_reconcile_interval() -> String {
    "1m".to_string()
}

/// Default value for boolean true
pub fn default_true() -> bool {
    true
}

/// Default value for boolean false
pub fn default_false() -> bool {
    false
}
