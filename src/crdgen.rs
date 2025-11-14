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
use schemars::JsonSchema;

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
    pub gcp_project_id: String,
    pub environment: String,
    #[serde(default)]
    pub kustomize_path: Option<String>,
    #[serde(default)]
    pub base_path: Option<String>,
    #[serde(default)]
    pub secret_prefix: Option<String>,
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
