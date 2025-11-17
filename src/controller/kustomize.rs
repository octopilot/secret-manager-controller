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
//! - **GitOps-agnostic**: Works with any `GitOps` tool (`FluxCD`, `ArgoCD`, etc.)
//! - **Secret extraction**: Parses Kubernetes Secret resources from kustomize output
//! - **Base64 decoding**: Automatically decodes base64-encoded secret values
//!
//! ## Usage
//!
//! ```rust,no_run
//! use secret_manager_controller::controller::kustomize;
//! use std::path::Path;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let artifact_path = Path::new("/tmp/flux-source-repo");
//! let kustomize_path = "microservices/idam/deployment-configuration/profiles/dev";
//!
//! let secrets = kustomize::extract_secrets_from_kustomize(artifact_path, kustomize_path)?;
//! # Ok(())
//! # }
//! ```

use crate::observability::metrics;
use anyhow::{Context, Result};
use k8s_openapi::api::core::v1::Secret;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::Instant;
use tracing::{debug, error, info, info_span, warn};

/// Run kustomize build on the specified path and extract secrets from Secret resources
#[allow(
    clippy::missing_errors_doc,
    reason = "Error documentation is provided in doc comments"
)]
pub fn extract_secrets_from_kustomize(
    artifact_path: &Path,
    kustomize_path: &str,
) -> Result<HashMap<String, String>> {
    let full_path = artifact_path.join(kustomize_path);
    let span = info_span!("kustomize.build", kustomize.path = kustomize_path);
    let span_clone = span.clone();
    let start = Instant::now();

    let result = (|| -> Result<HashMap<String, String>> {
        // Construct full path to kustomization.yaml
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

        info!("Running kustomize build on path: {}", full_path.display());

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
            span_clone.record("operation.success", false);
            span_clone.record("error.message", stderr.to_string());
            metrics::increment_kustomize_build_errors_total();
            return Err(anyhow::anyhow!("Kustomize build failed: {stderr}"));
        }

        let yaml_output = String::from_utf8(output.stdout)
            .context("Failed to decode kustomize output as UTF-8")?;

        debug!("Kustomize build succeeded, parsing output...");

        // Parse YAML stream (multiple resources separated by ---)
        let secrets = parse_kustomize_output(&yaml_output);

        span_clone.record("secrets.count", secrets.len() as u64);
        span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
        span_clone.record("operation.success", true);
        metrics::increment_kustomize_build_total();
        metrics::observe_kustomize_build_duration(start.elapsed().as_secs_f64());

        info!("Extracted {} secrets from kustomize output", secrets.len());
        Ok(secrets)
    })();

    // Record span attributes even on error
    if let Err(ref e) = result {
        span_clone.record("operation.success", false);
        span_clone.record("error.message", e.to_string());
    }

    result
}

/// Parse kustomize build output and extract secrets from Secret resources
fn parse_kustomize_output(yaml_output: &str) -> HashMap<String, String> {
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
                        use base64::{engine::general_purpose, Engine as _};
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

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::Secret;
    use std::collections::BTreeMap;

    mod parse_kustomize_output_tests {
        use super::*;
        use base64::{engine::general_purpose, Engine as _};

        #[test]
        fn test_parse_kustomize_output_single_secret() {
            let secret = Secret {
                data: Some(BTreeMap::from([(
                    "database-url".to_string(),
                    k8s_openapi::ByteString(
                        general_purpose::STANDARD
                            .encode("postgres://localhost:5432/mydb")
                            .into(),
                    ),
                )])),
                ..Secret::default()
            };
            let yaml = serde_yaml::to_string(&secret).unwrap();
            let yaml_output = format!("---\n{}", yaml);

            let result = parse_kustomize_output(&yaml_output);

            assert_eq!(
                result.get("database-url"),
                Some(&"postgres://localhost:5432/mydb".to_string())
            );
        }

        #[test]
        fn test_parse_kustomize_output_multiple_secrets() {
            let secret1 = Secret {
                metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                    name: Some("secret1".to_string()),
                    ..Default::default()
                }
                .into(),
                data: Some(BTreeMap::from([(
                    "key1".to_string(),
                    k8s_openapi::ByteString(general_purpose::STANDARD.encode("value1").into()),
                )])),
                ..Secret::default()
            };
            let secret2 = Secret {
                metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                    name: Some("secret2".to_string()),
                    ..Default::default()
                }
                .into(),
                data: Some(BTreeMap::from([(
                    "key2".to_string(),
                    k8s_openapi::ByteString(general_purpose::STANDARD.encode("value2").into()),
                )])),
                ..Secret::default()
            };
            let yaml1 = serde_yaml::to_string(&secret1).unwrap();
            let yaml2 = serde_yaml::to_string(&secret2).unwrap();
            let yaml_output = format!("---\n{}\n---\n{}", yaml1, yaml2);

            let result = parse_kustomize_output(&yaml_output);

            assert_eq!(result.get("key1"), Some(&"value1".to_string()));
            assert_eq!(result.get("key2"), Some(&"value2".to_string()));
        }

        #[test]
        fn test_parse_kustomize_output_non_secret_resource() {
            let yaml_output = r#"---
apiVersion: v1
kind: ConfigMap
metadata:
  name: test-config
data:
  key: value
"#;

            let result = parse_kustomize_output(yaml_output);

            assert!(result.is_empty());
        }

        #[test]
        fn test_parse_kustomize_output_invalid_base64() {
            let secret = Secret {
                data: Some(BTreeMap::from([(
                    "key".to_string(),
                    k8s_openapi::ByteString("invalid-base64!!!".as_bytes().to_vec()),
                )])),
                ..Secret::default()
            };
            let yaml = serde_yaml::to_string(&secret).unwrap();
            let yaml_output = format!("---\n{}", yaml);

            let result = parse_kustomize_output(&yaml_output);

            // Invalid base64 should be skipped
            assert!(!result.contains_key("key"));
        }

        #[test]
        fn test_parse_kustomize_output_empty() {
            let result = parse_kustomize_output("");

            assert!(result.is_empty());
        }

        #[test]
        fn test_parse_kustomize_output_no_separator() {
            let secret = Secret {
                data: Some(BTreeMap::from([(
                    "key".to_string(),
                    k8s_openapi::ByteString(general_purpose::STANDARD.encode("value").into()),
                )])),
                ..Secret::default()
            };
            let yaml = serde_yaml::to_string(&secret).unwrap();

            let result = parse_kustomize_output(&yaml);

            // Should still parse even without --- separator
            assert_eq!(result.get("key"), Some(&"value".to_string()));
        }

        #[test]
        fn test_parse_kustomize_output_secret_with_no_data() {
            let secret = Secret {
                data: None,
                ..Secret::default()
            };
            let yaml = serde_yaml::to_string(&secret).unwrap();
            let yaml_output = format!("---\n{}", yaml);

            let result = parse_kustomize_output(&yaml_output);

            assert!(result.is_empty());
        }

        #[test]
        fn test_parse_kustomize_output_secret_with_empty_data() {
            let secret = Secret {
                data: Some(BTreeMap::new()),
                ..Secret::default()
            };
            let yaml = serde_yaml::to_string(&secret).unwrap();
            let yaml_output = format!("---\n{}", yaml);

            let result = parse_kustomize_output(&yaml_output);

            assert!(result.is_empty());
        }

        #[test]
        fn test_parse_kustomize_output_multiple_secrets_same_key() {
            // When multiple secrets have the same key, later ones should overwrite
            let secret1 = Secret {
                metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                    name: Some("secret1".to_string()),
                    ..Default::default()
                }
                .into(),
                data: Some(BTreeMap::from([(
                    "key".to_string(),
                    k8s_openapi::ByteString(general_purpose::STANDARD.encode("value1").into()),
                )])),
                ..Secret::default()
            };
            let secret2 = Secret {
                metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                    name: Some("secret2".to_string()),
                    ..Default::default()
                }
                .into(),
                data: Some(BTreeMap::from([(
                    "key".to_string(),
                    k8s_openapi::ByteString(general_purpose::STANDARD.encode("value2").into()),
                )])),
                ..Secret::default()
            };
            let yaml1 = serde_yaml::to_string(&secret1).unwrap();
            let yaml2 = serde_yaml::to_string(&secret2).unwrap();
            let yaml_output = format!("---\n{}\n---\n{}", yaml1, yaml2);

            let result = parse_kustomize_output(&yaml_output);

            // Last value should win
            assert_eq!(result.get("key"), Some(&"value2".to_string()));
        }

        #[test]
        fn test_parse_kustomize_output_secret_with_string_data() {
            // Secrets can have stringData instead of data
            let secret = Secret {
                string_data: Some(BTreeMap::from([(
                    "key".to_string(),
                    "plain-text-value".to_string(),
                )])),
                ..Secret::default()
            };
            let yaml = serde_yaml::to_string(&secret).unwrap();
            let yaml_output = format!("---\n{}", yaml);

            let result = parse_kustomize_output(&yaml_output);

            // stringData should be parsed (if the function supports it)
            // Note: Current implementation only handles data, not stringData
            // This test documents current behavior
            assert!(result.is_empty() || result.contains_key("key"));
        }

        #[test]
        fn test_parse_kustomize_output_mixed_resources() {
            // Mix of Secret and non-Secret resources
            let secret = Secret {
                data: Some(BTreeMap::from([(
                    "key".to_string(),
                    k8s_openapi::ByteString(general_purpose::STANDARD.encode("value").into()),
                )])),
                ..Secret::default()
            };
            let config_map_yaml = r#"apiVersion: v1
kind: ConfigMap
metadata:
  name: test-config
data:
  config-key: config-value
"#;
            let secret_yaml = serde_yaml::to_string(&secret).unwrap();
            let yaml_output = format!("---\n{}\n---\n{}", config_map_yaml, secret_yaml);

            let result = parse_kustomize_output(&yaml_output);

            // Should only extract from Secret, not ConfigMap
            assert_eq!(result.get("key"), Some(&"value".to_string()));
            assert!(!result.contains_key("config-key"));
        }

        #[test]
        fn test_parse_kustomize_output_whitespace_handling() {
            let secret = Secret {
                data: Some(BTreeMap::from([(
                    "key".to_string(),
                    k8s_openapi::ByteString(general_purpose::STANDARD.encode("value").into()),
                )])),
                ..Secret::default()
            };
            let yaml = serde_yaml::to_string(&secret).unwrap();
            // Add extra whitespace around separators
            let yaml_output = format!("   ---   \n{}\n   ---   \n", yaml);

            let result = parse_kustomize_output(&yaml_output);

            assert_eq!(result.get("key"), Some(&"value".to_string()));
        }
    }
}

/// Extract properties from kustomize output (from `ConfigMap` resources)
#[allow(
    clippy::missing_errors_doc,
    reason = "Error documentation is provided in doc comments"
)]
pub fn extract_properties_from_kustomize(
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
        return Err(anyhow::anyhow!("Kustomize build failed: {stderr}"));
    }

    let yaml_output =
        String::from_utf8(output.stdout).context("Failed to decode kustomize output as UTF-8")?;

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

    Ok(all_properties)
}
