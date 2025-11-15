//! Secret Manager Controller Library
//!
//! This library provides the core functionality for the Secret Manager Controller.
//! Tests are included in the module files (e.g., reconciler.rs).

use kube::CustomResource;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

// Re-export modules so they can be tested
pub mod constants;
pub mod controller;
pub mod observability;
pub mod provider;

// Note: GcpAuthConfig is defined in main.rs since main.rs has its own CRD definition
// For library usage, import from the main module

// CRD types - needed by reconciler and tests
// Note: These types must match main.rs exactly
// We define them here for library/tests, main.rs has the actual CRD definition

#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "secret-management.microscaler.io",
    version = "v1",
    kind = "SecretManagerConfig",
    namespaced,
    status = "SecretManagerConfigStatus"
)]
#[serde(rename_all = "camelCase")]
pub struct SecretManagerConfigSpec {
    pub source_ref: SourceRef,
    pub provider: ProviderConfig,
    pub secrets: SecretsConfig,
    /// Config store configuration for routing application.properties to config stores
    /// When enabled, properties are stored individually in config stores instead of as a JSON blob in secret stores
    #[serde(default)]
    pub configs: Option<ConfigsConfig>,
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
/// Kubernetes sends data in format: {"type": "gcp", "gcp": {...}}
/// We use externally tagged format and ignore the "type" field during deserialization
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ProviderConfig {
    #[serde(rename = "gcp")]
    Gcp(GcpConfig),
    #[serde(rename = "aws")]
    Aws(AwsConfig),
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
                            gcp = Some(map.next_value()?);
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

/// AWS configuration for Secrets Manager
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AwsConfig {
    pub region: String,
    #[serde(default)]
    pub auth: Option<AwsAuthConfig>,
}

/// Azure configuration for Key Vault
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AzureConfig {
    pub vault_name: String,
    #[serde(default)]
    pub auth: Option<AzureAuthConfig>,
}

/// AWS authentication configuration
/// Only supports IRSA (IAM Roles for Service Accounts) - recommended and default
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum AwsAuthConfig {
    Irsa { role_arn: String },
}

/// Azure authentication configuration
/// Only supports Workload Identity (recommended and default)
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum AzureAuthConfig {
    WorkloadIdentity { client_id: String },
}

/// GCP configuration for Secret Manager
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GcpConfig {
    pub project_id: String,
    #[serde(default)]
    pub auth: Option<GcpAuthConfig>,
}

/// Secrets sync configuration
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretsConfig {
    pub environment: String,
    #[serde(default)]
    pub kustomize_path: Option<String>,
    #[serde(default)]
    pub base_path: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub suffix: Option<String>,
}

/// Config store configuration for routing application.properties to config stores
/// When enabled, properties are stored individually in config stores instead of as a JSON blob in secret stores
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
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

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    #[serde(default = "default_source_kind")]
    pub kind: String,
    pub name: String,
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

/// GCP authentication configuration
/// Only supports Workload Identity (recommended and default)
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum GcpAuthConfig {
    WorkloadIdentity { service_account_email: String },
}

/// OpenTelemetry configuration
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "otelType")]
pub enum OtelConfig {
    Otlp {
        endpoint: String,
        #[serde(default)]
        service_name: Option<String>,
        #[serde(default)]
        service_version: Option<String>,
        #[serde(default)]
        environment: Option<String>,
    },
    Datadog {
        #[serde(default)]
        service_name: Option<String>,
        #[serde(default)]
        service_version: Option<String>,
        #[serde(default)]
        environment: Option<String>,
        #[serde(default)]
        site: Option<String>,
        #[serde(default)]
        api_key: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, JsonSchema)]
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
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub observed_generation: Option<i64>,
    #[serde(default)]
    pub last_reconcile_time: Option<String>,
    #[serde(default)]
    pub secrets_synced: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    pub r#type: String,
    pub status: String,
    #[serde(default)]
    pub last_transition_time: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}
