//! # Source and Secrets Configuration
//!
//! Types for GitOps source references and secrets/configs sync configuration.

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// Source reference for GitOps repositories
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    /// Source kind: "GitRepository" (FluxCD) or "Application" (ArgoCD)
    #[serde(default = "crate::crd::spec::default_source_kind")]
    pub kind: String,
    /// Source name
    pub name: String,
    /// Source namespace
    pub namespace: String,
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
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
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
