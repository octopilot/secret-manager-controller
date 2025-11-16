//! # CRD Generator
//!
//! Generates Kubernetes CustomResourceDefinition (CRD) YAML from Rust type definitions.
//!
//! This binary uses the `kube` crate's `CustomResourceExt` trait to generate
//! the CRD YAML for the `SecretManagerConfig` resource.
//!
//! ## Usage
//!
//! ```bash
//! # Generate CRD YAML
//! cargo run --bin crdgen > config/crd/secretmanagerconfig.yaml
//!
//! # Generate and apply directly
//! cargo run --bin crdgen | kubectl apply -f -
//! ```
//!
//! The generated CRD includes:
//! - OpenAPI schema validation
//! - Required fields
//! - Default values
//! - Status subresource

// We need to share the SecretManagerConfig type between binaries
// The simplest approach is to include the type definitions here
// In a production setup, you'd move shared types to lib.rs

use kube::{core::CustomResourceExt, CustomResource};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

// Re-define the types needed for CRD generation
// This matches the types in main.rs
#[derive(CustomResource, Debug, Clone, Deserialize, Serialize, JsonSchema)]
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
    pub source_ref: SourceRef,
    pub provider: ProviderConfig,
    pub secrets: SecretsConfig,
    /// Config store configuration for routing application.properties to config stores
    /// When enabled, properties are stored individually in config stores instead of as a JSON blob in secret stores
    #[serde(default)]
    pub configs: Option<ConfigsConfig>,
    // Temporarily commented out to avoid schema generation issues with nested discriminated unions
    // The otel field will be added back once we resolve the schema conflict
    // #[serde(default)]
    // pub otel: Option<OtelConfig>,
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
/// NOTE: Due to kube-rs limitations with nested discriminated unions, the provider field
/// uses x-kubernetes-preserve-unknown-fields: true to allow the discriminated union structure.
/// Validation is performed at runtime by the controller.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ProviderConfig {
    Gcp(GcpConfig),
    Aws(AwsConfig),
    Azure(AzureConfig),
}

impl JsonSchema for ProviderConfig {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("ProviderConfig")
    }

    fn json_schema(gen: &mut SchemaGenerator) -> Schema {
        // Use x-kubernetes-preserve-unknown-fields to allow discriminated union structure
        // kube-rs cannot generate proper oneOf schemas for nested discriminated unions
        // Validation is performed at runtime by the controller
        // In schemars 1.0+, Schema is a newtype wrapper around serde_json::Value
        let schema = gen.root_schema_for::<serde_json::Value>();
        // Convert to Value, modify, then convert back to Schema
        let mut schema_value: serde_json::Value = schema.into();
        if let serde_json::Value::Object(ref mut map) = schema_value {
            map.insert("description".to_string(), serde_json::json!("Cloud provider configuration. Supports GCP, AWS, and Azure. Must have a 'type' field set to 'gcp', 'aws', or 'azure'."));
            map.insert(
                "x-kubernetes-preserve-unknown-fields".to_string(),
                serde_json::json!(true),
            );
        }
        Schema::try_from(schema_value).expect("Failed to create Schema from modified Value")
    }
}

fn auth_config_schema(gen: &mut SchemaGenerator) -> Schema {
    // Use generic metadata to ensure all auth fields have identical schemas
    // This is required for Kubernetes structural schemas with oneOf
    // In schemars 1.0+, Schema is a newtype wrapper around serde_json::Value
    let schema = gen.root_schema_for::<serde_json::Value>();
    // Convert to Value, modify, then convert back to Schema
    let mut schema_value: serde_json::Value = schema.into();
    if let serde_json::Value::Object(ref mut map) = schema_value {
        map.insert("description".to_string(), serde_json::json!("Authentication configuration. Supports multiple auth types via discriminated union."));
        map.insert(
            "x-kubernetes-preserve-unknown-fields".to_string(),
            serde_json::json!(true),
        );
    }
    Schema::try_from(schema_value).expect("Failed to create Schema from modified Value")
}

fn config_store_type_schema(_gen: &mut SchemaGenerator) -> Schema {
    // Generate a structural schema for Kubernetes CRD
    // Use nullable enum (not anyOf) for Option<ConfigStoreType>
    let schema_value = serde_json::json!({
        "type": "string",
        "enum": ["secretManager", "ParameterManager"],
        "nullable": true,
        "description": "GCP config store type. SecretManager: Store configs as individual secrets in Secret Manager (interim solution). ParameterManager: Store configs in Parameter Manager (future, after ESO contribution)."
    });
    Schema::try_from(schema_value).expect("Failed to create Schema for ConfigStoreType")
}

/// GCP configuration for Secret Manager
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GcpConfig {
    pub project_id: String,
    #[serde(default)]
    #[schemars(schema_with = "auth_config_schema")]
    pub auth: Option<GcpAuthConfig>,
}

/// AWS configuration for Secrets Manager
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AwsConfig {
    pub region: String,
    #[serde(default)]
    #[schemars(schema_with = "auth_config_schema")]
    pub auth: Option<AwsAuthConfig>,
}

/// Azure configuration for Key Vault
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AzureConfig {
    pub vault_name: String,
    #[serde(default)]
    #[schemars(schema_with = "auth_config_schema")]
    pub auth: Option<AzureAuthConfig>,
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
    #[schemars(schema_with = "config_store_type_schema")]
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

/// GCP authentication configuration
/// Only supports Workload Identity (recommended and default)
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum GcpAuthConfig {
    /// Use Workload Identity for authentication (DEFAULT)
    /// Requires GKE cluster with Workload Identity enabled
    /// This is the recommended authentication method and is used by default when auth is not specified
    WorkloadIdentity { service_account_email: String },
}

/// AWS authentication configuration
/// Only supports IRSA (IAM Roles for Service Accounts) - recommended and default
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum AwsAuthConfig {
    /// Use IRSA (IAM Roles for Service Accounts) for authentication (DEFAULT)
    /// Requires EKS cluster with IRSA enabled and service account annotation
    /// This is the recommended authentication method and is used by default when auth is not specified
    Irsa { role_arn: String },
}

/// Azure authentication configuration
/// Only supports Workload Identity (recommended and default)
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum AzureAuthConfig {
    /// Use Workload Identity for authentication (DEFAULT)
    /// Requires AKS cluster with Workload Identity enabled
    /// This is the recommended authentication method and is used by default when auth is not specified
    WorkloadIdentity { client_id: String },
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
    /// Next scheduled reconciliation time (RFC3339)
    /// Used to persist periodic reconciliation schedule across watch restarts
    #[serde(default)]
    pub next_reconcile_time: Option<String>,
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

// main() is only flagged as unused when checking the library target,
// but it's the entry point for the crdgen binary
#[allow(dead_code, reason = "main() is the entry point for the crdgen binary")]
fn main() {
    // Generate CRD YAML
    let crd = SecretManagerConfig::crd();

    // Serialize to YAML
    match serde_yaml::to_string(&crd) {
        Ok(yaml) => {
            // Print header comments warning that this file should not be edited manually
            println!("# This file is auto-generated by crdgen");
            println!("# DO NOT EDIT THIS FILE MANUALLY");
            println!("# If there are malformed YAML issues, fix them in the Rust code (src/controller/crdgen.rs or src/main.rs)");
            println!("# This file will be overwritten on every code update");
            println!("#");
            println!("---");
            print!("{yaml}");
        }
        Err(e) => {
            eprintln!("Failed to serialize CRD to YAML: {e}");
            std::process::exit(1);
        }
    }
}
