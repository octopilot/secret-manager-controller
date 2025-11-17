//! # Parser
//!
//! Parses application configuration files and extracts secrets.
//!
//! ## Supported File Formats
//!
//! - **`.env` files**: Key-value pairs in `KEY=value` format
//! - **`.yaml` files**: YAML format with nested structures (flattened)
//! - **`.properties` files**: Java properties format
//!
//! ## Features
//!
//! - **SOPS decryption**: Automatically detects and decrypts SOPS-encrypted files
//! - **Multi-environment support**: Processes specific environment directories
//! - **Flexible project structures**: Supports monolith and single-service layouts
//! - **Skaffold compliance**: Works with `profiles/` directory structure
//!
//! ## Directory Structure Support
//!
//! The parser supports multiple project structures:
//!
//! - **Monolith**: `{basePath}/{service}/deployment-configuration/profiles/{env}/`
//! - **Single Service**: `deployment-configuration/profiles/{env}/`
//! - **Backward Compatible**: `deployment-configuration/{env}/` (without profiles)

pub mod file_finder;
pub mod parsers;
pub mod sops;
pub mod types;

// Re-export public API
pub use file_finder::find_application_files;
pub use parsers::{parse_properties, parse_secrets};
pub use types::ApplicationFiles;

// Re-export for backward compatibility (used by tests)
#[cfg(test)]
pub use file_finder::normalize_base_path;
// Note: Test-only functions are pub(crate) and can be imported directly from parsers module
pub use sops::decrypt_sops_content;
pub(crate) use sops::is_sops_encrypted;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    mod normalize_base_path_tests {
        use super::super::file_finder::normalize_base_path;

        #[test]
        fn test_normalize_base_path_none() {
            assert_eq!(normalize_base_path(None), None);
        }

        #[test]
        fn test_normalize_base_path_empty_string() {
            assert_eq!(normalize_base_path(Some("")), None);
        }

        #[test]
        fn test_normalize_base_path_dot() {
            assert_eq!(normalize_base_path(Some(".")), None);
        }

        #[test]
        fn test_normalize_base_path_valid() {
            assert_eq!(
                normalize_base_path(Some("microservices")),
                Some("microservices")
            );
        }
    }

    mod is_sops_encrypted_tests {
        use super::super::sops::is_sops_encrypted;

        #[test]
        fn test_is_sops_encrypted_yaml() {
            let content = r#"sops:
    kms: []
    gcp_kms: []
    azure_kv: []
    hc_vault: []
    age: []
    lastmodified: '2024-01-01T00:00:00Z'
    mac: ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]
    encrypted_suffix: _encrypted
    version: 3.8.0
data:
    secret: ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]"#;
            assert!(is_sops_encrypted(content));
        }

        #[test]
        fn test_is_sops_encrypted_json() {
            let content = r#"{
  "sops": {
    "kms": [],
    "lastmodified": "2024-01-01T00:00:00Z",
    "mac": "ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]",
    "encrypted_suffix": "_encrypted",
    "version": "3.8.0"
  },
  "data": {
    "secret": "ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]"
  }
}"#;
            assert!(is_sops_encrypted(content));
        }

        #[test]
        fn test_is_sops_encrypted_env_with_metadata() {
            let content =
                "# sops_version=3.8.0\nSECRET=ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]";
            assert!(is_sops_encrypted(content));
        }

        #[test]
        fn test_is_not_sops_encrypted_plain_yaml() {
            let content = r#"data:
    secret: plaintext_value"#;
            assert!(!is_sops_encrypted(content));
        }

        #[test]
        fn test_is_not_sops_encrypted_plain_env() {
            let content = "SECRET=plaintext_value";
            assert!(!is_sops_encrypted(content));
        }
    }

    mod find_application_files_tests {
        use super::super::file_finder::find_application_files;
        use super::*;

        #[tokio::test]
        async fn test_find_application_files_monolith() {
            let temp_dir = TempDir::new().unwrap();
            let base = temp_dir.path();

            // Create monolith structure
            let service_dir = base.join("microservices").join("my-service");
            let profile_dir = service_dir
                .join("deployment-configuration")
                .join("profiles")
                .join("dev");
            fs::create_dir_all(&profile_dir).unwrap();

            // Create application files
            fs::write(profile_dir.join("application.secrets.env"), "KEY=value").unwrap();
            fs::write(profile_dir.join("application.properties"), "prop=val").unwrap();

            let files = find_application_files(base, Some("microservices"), "dev", None)
                .await
                .unwrap();

            assert_eq!(files.len(), 1);
            assert_eq!(files[0].service_name, "my-service");
            assert!(files[0].secrets_env.is_some());
            assert!(files[0].properties.is_some());
        }

        #[tokio::test]
        async fn test_find_application_files_single_service() {
            let temp_dir = TempDir::new().unwrap();
            let base = temp_dir.path();

            // Create single service structure
            let profile_dir = base
                .join("deployment-configuration")
                .join("profiles")
                .join("dev");
            fs::create_dir_all(&profile_dir).unwrap();

            fs::write(profile_dir.join("application.secrets.env"), "KEY=value").unwrap();

            let files = find_application_files(base, None, "dev", Some("my-service"))
                .await
                .unwrap();

            assert_eq!(files.len(), 1);
            assert_eq!(files[0].service_name, "my-service");
            assert!(files[0].secrets_env.is_some());
        }

        #[tokio::test]
        async fn test_find_application_files_legacy() {
            let temp_dir = TempDir::new().unwrap();
            let base = temp_dir.path();

            // Create legacy structure (without profiles)
            let env_dir = base.join("deployment-configuration").join("dev");
            fs::create_dir_all(&env_dir).unwrap();

            fs::write(env_dir.join("application.secrets.env"), "KEY=value").unwrap();

            let files = find_application_files(base, None, "dev", Some("my-service"))
                .await
                .unwrap();

            assert_eq!(files.len(), 1);
            assert!(files[0].secrets_env.is_some());
        }
    }

    mod parse_secrets_tests {
        use super::super::parsers::parse_secrets;
        use super::super::types::ApplicationFiles;
        use super::*;
        use std::path::PathBuf;

        #[tokio::test]
        async fn test_parse_secrets_env() {
            let temp_dir = TempDir::new().unwrap();
            let env_file = temp_dir.path().join("application.secrets.env");
            fs::write(&env_file, "KEY1=value1\nKEY2=value2").unwrap();

            let app_files = ApplicationFiles {
                service_name: "test".to_string(),
                base_path: temp_dir.path().to_path_buf(),
                secrets_env: Some(env_file),
                secrets_yaml: None,
                properties: None,
            };

            let secrets = parse_secrets(&app_files, None).await.unwrap();
            assert_eq!(secrets.get("KEY1"), Some(&"value1".to_string()));
            assert_eq!(secrets.get("KEY2"), Some(&"value2".to_string()));
        }

        #[tokio::test]
        async fn test_parse_secrets_yaml() {
            let temp_dir = TempDir::new().unwrap();
            let yaml_file = temp_dir.path().join("application.secrets.yaml");
            fs::write(
                &yaml_file,
                r#"nested:
  key1: value1
  key2: value2"#,
            )
            .unwrap();

            let app_files = ApplicationFiles {
                service_name: "test".to_string(),
                base_path: temp_dir.path().to_path_buf(),
                secrets_env: None,
                secrets_yaml: Some(yaml_file),
                properties: None,
            };

            let secrets = parse_secrets(&app_files, None).await.unwrap();
            assert_eq!(secrets.get("nested.key1"), Some(&"value1".to_string()));
            assert_eq!(secrets.get("nested.key2"), Some(&"value2".to_string()));
        }
    }

    mod parse_properties_tests {
        use super::super::parsers::parse_properties;
        use super::super::types::ApplicationFiles;
        use super::*;

        #[tokio::test]
        async fn test_parse_properties() {
            let temp_dir = TempDir::new().unwrap();
            let props_file = temp_dir.path().join("application.properties");
            fs::write(&props_file, "key1=value1\nkey2=value2").unwrap();

            let app_files = ApplicationFiles {
                service_name: "test".to_string(),
                base_path: temp_dir.path().to_path_buf(),
                secrets_env: None,
                secrets_yaml: None,
                properties: Some(props_file),
            };

            let properties = parse_properties(&app_files).await.unwrap();
            assert_eq!(properties.get("key1"), Some(&"value1".to_string()));
            assert_eq!(properties.get("key2"), Some(&"value2".to_string()));
        }

        #[tokio::test]
        async fn test_parse_properties_no_file() {
            let app_files = ApplicationFiles {
                service_name: "test".to_string(),
                base_path: PathBuf::from("/tmp"),
                secrets_env: None,
                secrets_yaml: None,
                properties: None,
            };

            let properties = parse_properties(&app_files).await.unwrap();
            assert!(properties.is_empty());
        }
    }
}
