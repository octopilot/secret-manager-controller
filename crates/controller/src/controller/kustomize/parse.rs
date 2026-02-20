//! # Kustomize Output Parsing
//!
//! Parses YAML output from kustomize build to extract secrets and properties.

use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use std::collections::HashMap;
use tracing::{debug, warn};

/// Parse kustomize build output and extract secrets from Secret resources
pub fn parse_secrets_from_yaml(yaml_output: &str) -> HashMap<String, String> {
    let mut all_secrets = HashMap::new();

    // Split YAML stream by --- separator
    let documents: Vec<&str> = yaml_output
        .split("---")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    for doc in documents {
        // Try to parse as Kubernetes Secret
        match serde_yaml::from_str::<Secret>(doc) {
            Ok(secret) => {
                // Extract secret data
                if let Some(data) = &secret.data {
                    for (key, value) in data {
                        // Decode base64 value
                        use base64::{Engine as _, engine::general_purpose};
                        match general_purpose::STANDARD.decode(&value.0) {
                            Ok(decoded) => match String::from_utf8(decoded) {
                                Ok(secret_value) => {
                                    all_secrets.insert(key.clone(), secret_value);
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to decode secret value for {} as UTF-8: {}",
                                        key, e
                                    );
                                }
                            },
                            Err(e) => {
                                warn!("Failed to decode base64 secret value for {}: {}", key, e);
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

    all_secrets
}

/// Parse kustomize build output and extract properties from ConfigMap resources
pub fn parse_properties_from_yaml(yaml_output: &str) -> HashMap<String, String> {
    let mut all_properties = HashMap::new();

    // Split YAML stream by --- separator
    let documents: Vec<&str> = yaml_output
        .split("---")
        .map(str::trim)
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

    all_properties
}
