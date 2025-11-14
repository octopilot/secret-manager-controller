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
use serde::{Deserialize, Serialize};
use schemars::{JsonSchema, SchemaGenerator, Schema};
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
    printcolumn = r#"{"name":"Ready", "type":"string", "jsonPath":".status.conditions[?(@.type==\"Ready\")].status"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct SecretManagerConfigSpec {
    pub source_ref: SourceRef,
    pub provider: ProviderConfig,
    pub secrets: SecretsConfig,
    // Temporarily commented out to avoid schema generation issues with nested discriminated unions
    // The otel field will be added back once we resolve the schema conflict
    // #[serde(default)]
    // pub otel: Option<OtelConfig>,
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
            map.insert("x-kubernetes-preserve-unknown-fields".to_string(), serde_json::json!(true));
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
        map.insert("x-kubernetes-preserve-unknown-fields".to_string(), serde_json::json!(true));
    }
    Schema::try_from(schema_value).expect("Failed to create Schema from modified Value")
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

/// GCP authentication configuration
/// Only supports Workload Identity (recommended and default)
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum GcpAuthConfig {
    /// Use Workload Identity for authentication (DEFAULT)
    /// Requires GKE cluster with Workload Identity enabled
    /// This is the recommended authentication method and is used by default when auth is not specified
    WorkloadIdentity {
        service_account_email: String,
    },
}

/// AWS authentication configuration
/// Only supports IRSA (IAM Roles for Service Accounts) - recommended and default
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum AwsAuthConfig {
    /// Use IRSA (IAM Roles for Service Accounts) for authentication (DEFAULT)
    /// Requires EKS cluster with IRSA enabled and service account annotation
    /// This is the recommended authentication method and is used by default when auth is not specified
    Irsa {
        role_arn: String,
    },
}

/// Azure authentication configuration
/// Only supports Workload Identity (recommended and default)
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum AzureAuthConfig {
    /// Use Workload Identity for authentication (DEFAULT)
    /// Requires AKS cluster with Workload Identity enabled
    /// This is the recommended authentication method and is used by default when auth is not specified
    WorkloadIdentity {
        client_id: String,
    },
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

fn default_json_secret_name() -> String {
    "gcp-secret-manager-credentials".to_string()
}

fn default_json_secret_key() -> String {
    "key.json".to_string()
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

#[derive(Debug, Clone, Deserialize, Serialize, Default, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretManagerConfigStatus {
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

fn main() {
    // Generate CRD YAML
    let crd = SecretManagerConfig::crd();
    
    // Serialize to YAML
    match serde_yaml::to_string(&crd) {
        Ok(yaml) => {
            print!("{}", yaml);
        }
        Err(e) => {
            eprintln!("Failed to serialize CRD to YAML: {}", e);
            std::process::exit(1);
        }
    }
}
