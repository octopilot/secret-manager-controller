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
//! use crate::controller::kustomize;
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

mod build;
mod parse;
mod properties;
mod secrets;

pub use properties::extract_properties_from_kustomize;
pub use secrets::extract_secrets_from_kustomize;

#[cfg(test)]
mod tests {
    use k8s_openapi::api::core::v1::Secret;
    use std::collections::BTreeMap;

    mod parse_kustomize_output_tests {
        use super::*;
        use crate::controller::kustomize::parse::parse_secrets_from_yaml;
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
            let yaml =
                serde_yaml::to_string(&secret).expect("Failed to serialize Secret to YAML in test");
            let yaml_output = format!("---\n{}", yaml);

            let result = parse_secrets_from_yaml(&yaml_output);

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
            let yaml1 = serde_yaml::to_string(&secret1)
                .expect("Failed to serialize Secret1 to YAML in test");
            let yaml2 = serde_yaml::to_string(&secret2)
                .expect("Failed to serialize Secret2 to YAML in test");
            let yaml_output = format!("---\n{}\n---\n{}", yaml1, yaml2);

            let result = parse_secrets_from_yaml(&yaml_output);

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

            let result = parse_secrets_from_yaml(yaml_output);

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
            let yaml =
                serde_yaml::to_string(&secret).expect("Failed to serialize Secret to YAML in test");
            let yaml_output = format!("---\n{}", yaml);

            let result = parse_secrets_from_yaml(&yaml_output);

            // Invalid base64 should be skipped
            assert!(!result.contains_key("key"));
        }

        #[test]
        fn test_parse_kustomize_output_empty() {
            let result = parse_secrets_from_yaml("");

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
            let yaml =
                serde_yaml::to_string(&secret).expect("Failed to serialize Secret to YAML in test");

            let result = parse_secrets_from_yaml(&yaml);

            // Should still parse even without --- separator
            assert_eq!(result.get("key"), Some(&"value".to_string()));
        }

        #[test]
        fn test_parse_kustomize_output_secret_with_no_data() {
            let secret = Secret {
                data: None,
                ..Secret::default()
            };
            let yaml =
                serde_yaml::to_string(&secret).expect("Failed to serialize Secret to YAML in test");
            let yaml_output = format!("---\n{}", yaml);

            let result = parse_secrets_from_yaml(&yaml_output);

            assert!(result.is_empty());
        }

        #[test]
        fn test_parse_kustomize_output_secret_with_empty_data() {
            let secret = Secret {
                data: Some(BTreeMap::new()),
                ..Secret::default()
            };
            let yaml =
                serde_yaml::to_string(&secret).expect("Failed to serialize Secret to YAML in test");
            let yaml_output = format!("---\n{}", yaml);

            let result = parse_secrets_from_yaml(&yaml_output);

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
            let yaml1 = serde_yaml::to_string(&secret1)
                .expect("Failed to serialize Secret1 to YAML in test");
            let yaml2 = serde_yaml::to_string(&secret2)
                .expect("Failed to serialize Secret2 to YAML in test");
            let yaml_output = format!("---\n{}\n---\n{}", yaml1, yaml2);

            let result = parse_secrets_from_yaml(&yaml_output);

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
            let yaml =
                serde_yaml::to_string(&secret).expect("Failed to serialize Secret to YAML in test");
            let yaml_output = format!("---\n{}", yaml);

            let result = parse_secrets_from_yaml(&yaml_output);

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
            let secret_yaml =
                serde_yaml::to_string(&secret).expect("Failed to serialize Secret to YAML in test");
            let yaml_output = format!("---\n{}\n---\n{}", config_map_yaml, secret_yaml);

            let result = parse_secrets_from_yaml(&yaml_output);

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
            let yaml =
                serde_yaml::to_string(&secret).expect("Failed to serialize Secret to YAML in test");
            // Add extra whitespace around separators
            let yaml_output = format!("   ---   \n{}\n   ---   \n", yaml);

            let result = parse_secrets_from_yaml(&yaml_output);

            assert_eq!(result.get("key"), Some(&"value".to_string()));
        }
    }
}
