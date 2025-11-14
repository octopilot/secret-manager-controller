//! # Kustomize Integration
//!
//! Executes `kustomize build` and extracts secrets from generated Kubernetes Secret resources.
//!
//! This module provides GitOps-agnostic secret extraction by running `kustomize build`
//! on the specified path and parsing the output to find Secret resources. This ensures
//! that overlays, patches, and generators are properly applied before secret extraction.
//!
//! ## Features
//!
//! - **Full Kustomize support**: Handles overlays, patches, and generators
//! - **GitOps-agnostic**: Works with any GitOps tool (FluxCD, ArgoCD, etc.)
//! - **Secret extraction**: Parses Kubernetes Secret resources from kustomize output
//! - **Base64 decoding**: Automatically decodes base64-encoded secret values
//!
//! ## Usage
//!
//! ```rust,no_run
//! use secret_manager_controller::kustomize;
//! use std::path::Path;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let artifact_path = Path::new("/tmp/flux-source-repo");
//! let kustomize_path = "microservices/idam/deployment-configuration/profiles/dev";
//!
//! let secrets = kustomize::extract_secrets_from_kustomize(artifact_path, kustomize_path).await?;
//! # Ok(())
//! # }
//! ```

use anyhow::{Context, Result};
use k8s_openapi::api::core::v1::Secret;
use serde_yaml;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use tracing::{debug, error, info, warn};

/// Run kustomize build on the specified path and extract secrets from Secret resources
pub async fn extract_secrets_from_kustomize(
    artifact_path: &Path,
    kustomize_path: &str,
) -> Result<HashMap<String, String>> {
    // Construct full path to kustomization.yaml
    let full_path = artifact_path.join(kustomize_path);
    
    if !full_path.exists() {
        return Err(anyhow::anyhow!(
            "Kustomize path does not exist: {}",
            full_path.display()
        ));
    }

    // Check if kustomization.yaml exists
    let kustomization_file = full_path.join("kustomization.yaml");
    if !kustomization_file.exists() {
        return Err(anyhow::anyhow!(
            "kustomization.yaml not found at: {}",
            kustomization_file.display()
        ));
    }

    info!(
        "Running kustomize build on path: {}",
        full_path.display()
    );

    // Run kustomize build
    let output = Command::new("kustomize")
        .arg("build")
        .arg(&full_path)
        .current_dir(artifact_path)
        .output()
        .context("Failed to execute kustomize build")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("Kustomize build failed: {}", stderr);
        return Err(anyhow::anyhow!(
            "Kustomize build failed: {}",
            stderr
        ));
    }

    let yaml_output = String::from_utf8(output.stdout)
        .context("Failed to decode kustomize output as UTF-8")?;

    debug!("Kustomize build succeeded, parsing output...");

    // Parse YAML stream (multiple resources separated by ---)
    let secrets = parse_kustomize_output(&yaml_output)?;

    info!("Extracted {} secrets from kustomize output", secrets.len());
    Ok(secrets)
}

/// Parse kustomize build output and extract secrets from Secret resources
fn parse_kustomize_output(yaml_output: &str) -> Result<HashMap<String, String>> {
    let mut all_secrets = HashMap::new();

    // Split YAML stream by --- separator
    let documents: Vec<&str> = yaml_output
        .split("---")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    for doc in documents {
        // Try to parse as Kubernetes Secret
        match serde_yaml::from_str::<Secret>(doc) {
            Ok(secret) => {
                // Extract secret data
                if let Some(data) = &secret.data {
                    for (key, value) in data.iter() {
                        // Decode base64 value
                        use base64::{Engine as _, engine::general_purpose};
                        match general_purpose::STANDARD.decode(&value.0) {
                            Ok(decoded) => {
                                match String::from_utf8(decoded) {
                                    Ok(secret_value) => {
                                        all_secrets.insert(key.clone(), secret_value);
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to decode secret value for {} as UTF-8: {}",
                                            key, e
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to decode base64 secret value for {}: {}",
                                    key, e
                                );
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // Not a Secret resource, skip
                debug!("Skipping non-Secret resource in kustomize output");
            }
        }
    }

    Ok(all_secrets)
}

/// Extract properties from kustomize output (from ConfigMap resources)
pub async fn extract_properties_from_kustomize(
    artifact_path: &Path,
    kustomize_path: &str,
) -> Result<HashMap<String, String>> {
    use k8s_openapi::api::core::v1::ConfigMap;

    // Construct full path to kustomization.yaml
    let full_path = artifact_path.join(kustomize_path);
    
    if !full_path.exists() {
        return Err(anyhow::anyhow!(
            "Kustomize path does not exist: {}",
            full_path.display()
        ));
    }

    info!(
        "Running kustomize build on path: {} (for properties)",
        full_path.display()
    );

    // Run kustomize build
    let output = Command::new("kustomize")
        .arg("build")
        .arg(&full_path)
        .current_dir(artifact_path)
        .output()
        .context("Failed to execute kustomize build")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("Kustomize build failed: {}", stderr);
        return Err(anyhow::anyhow!(
            "Kustomize build failed: {}",
            stderr
        ));
    }

    let yaml_output = String::from_utf8(output.stdout)
        .context("Failed to decode kustomize output as UTF-8")?;

    let mut all_properties = HashMap::new();

    // Split YAML stream by --- separator
    let documents: Vec<&str> = yaml_output
        .split("---")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    for doc in documents {
        // Try to parse as Kubernetes ConfigMap
        match serde_yaml::from_str::<ConfigMap>(doc) {
            Ok(config_map) => {
                // Extract data from ConfigMap
                if let Some(data) = &config_map.data {
                    all_properties.extend(data.clone());
                }
            }
            Err(_) => {
                // Not a ConfigMap resource, skip
                debug!("Skipping non-ConfigMap resource in kustomize output");
            }
        }
    }

    Ok(all_properties)
}

