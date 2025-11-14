//! Secret Manager Controller Library
//!
//! This library provides the core functionality for the Secret Manager Controller.
//! Tests are included in the module files (e.g., reconciler.rs).

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Re-export modules so they can be tested
pub mod aws;
pub mod azure;
pub mod gcp;
pub mod kustomize;
pub mod metrics;
pub mod otel;
pub mod parser;
pub mod provider;
pub mod reconciler;

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
    #[serde(default)]
    pub otel: Option<OtelConfig>,
}

/// Cloud provider configuration
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ProviderConfig {
    Gcp(GcpConfig),
    Aws(AwsConfig),
    Azure(AzureConfig),
}

/// AWS configuration for Secrets Manager
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AwsConfig {
    pub region: String,
    #[serde(default)]
    pub auth: Option<AwsAuthConfig>,
}

/// Azure configuration for Key Vault (stub)
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
    Irsa {
        role_arn: String,
    },
}

/// Azure authentication configuration
/// Only supports Workload Identity (recommended and default)
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum AzureAuthConfig {
    WorkloadIdentity {
        client_id: String,
    },
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

/// GCP authentication configuration
/// Only supports Workload Identity (recommended and default)
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum GcpAuthConfig {
    WorkloadIdentity {
        service_account_email: String,
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
