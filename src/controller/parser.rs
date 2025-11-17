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

use crate::observability::metrics;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info_span, warn, Instrument};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct ApplicationFiles {
    pub service_name: String,
    pub base_path: PathBuf,
    pub secrets_env: Option<PathBuf>,
    pub secrets_yaml: Option<PathBuf>,
    pub properties: Option<PathBuf>,
}

/// Normalize base path - handle "." and "" as empty/root
fn normalize_base_path(base_path: Option<&str>) -> Option<&str> {
    match base_path {
        Some("." | "") | None => None,
        Some(path) => Some(path),
    }
}

/// Find application files for a specific environment/profile
/// Supports both monolith and single service structures:
/// - Monolith: {basePath}/{service}/deployment-configuration/profiles/{env}/
/// - Single service: deployment-configuration/profiles/{env}/
/// - Backward compatible: deployment-configuration/{env}/ (without profiles)
///
/// Only processes the specified environment name - does not scan all environments
///
/// If `base_path` is None, searches from repository root
#[allow(
    clippy::unused_async,
    clippy::missing_errors_doc,
    reason = "May be called from async contexts in the future, error docs in comments"
)]
pub async fn find_application_files(
    artifact_path: &Path,
    base_path: Option<&str>,
    environment: &str,
    default_service_name: Option<&str>,
) -> Result<Vec<ApplicationFiles>> {
    // Normalize base path - handle "." and "" as root
    let normalized_base = normalize_base_path(base_path);
    let search_path = match normalized_base {
        None => artifact_path.to_path_buf(),
        Some(path) => artifact_path.join(path),
    };

    if !search_path.exists() {
        warn!("Base path does not exist: {}", search_path.display());
        return Ok(vec![]);
    }

    let mut application_files = Vec::new();

    // Walk through directory structure
    // Expected structures:
    // - Monolith: microservices/{service}/deployment-configuration/profiles/{env}/
    // - Single service: deployment-configuration/profiles/{env}/
    // - Legacy: deployment-configuration/{env}/ (backward compatibility)
    for entry in WalkDir::new(&search_path)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();

        // Check if this is a deployment-configuration directory
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "deployment-configuration")
        {
            // Extract service name (parent of deployment-configuration)
            let service_name = if let Some(parent) = path.parent() {
                // Check if parent is the base_path (single service case)
                if parent == search_path {
                    // Single service: use default_service_name or fallback
                    default_service_name
                        .map(ToString::to_string)
                        .or_else(|| {
                            // Try to extract from artifact path or use default
                            artifact_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(ToString::to_string)
                        })
                        .unwrap_or_else(|| "default-service".to_string())
                } else {
                    // Monolith: extract service name from parent directory
                    parent
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map_or_else(|| "unknown".to_string(), ToString::to_string)
                }
            } else {
                default_service_name.map_or("unknown".to_string(), ToString::to_string)
            };

            // Look for profiles directory first (Skaffold-compliant structure)
            let profiles_path = path.join("profiles");
            if profiles_path.exists() && profiles_path.is_dir() {
                // New structure: deployment-configuration/profiles/{env}/
                // Only process the specified environment
                let env_path = profiles_path.join(environment);
                if env_path.exists() && env_path.is_dir() {
                    let app_files = find_files_in_directory(&service_name, &env_path)?;
                    if app_files.has_any_files() {
                        application_files.push(app_files);
                    } else {
                        warn!(
                            "No application files found in environment '{}' at {}",
                            environment,
                            env_path.display()
                        );
                    }
                } else {
                    warn!(
                        "Environment '{}' not found in profiles directory: {}",
                        environment,
                        profiles_path.display()
                    );
                }
            } else {
                // Backward compatibility: deployment-configuration/{env}/ (without profiles)
                // Only process the specified environment
                let env_path = path.join(environment);
                if env_path.exists() && env_path.is_dir() {
                    let app_files = find_files_in_directory(&service_name, &env_path)?;
                    if app_files.has_any_files() {
                        application_files.push(app_files);
                    } else {
                        warn!(
                            "No application files found in environment '{}' at {}",
                            environment,
                            env_path.display()
                        );
                    }
                } else {
                    warn!(
                        "Environment '{}' not found in deployment-configuration directory: {}",
                        environment,
                        path.display()
                    );
                }
            }
        }
    }

    Ok(application_files)
}

fn find_files_in_directory(service_name: &str, dir: &Path) -> Result<ApplicationFiles> {
    let mut app_files = ApplicationFiles {
        service_name: service_name.to_string(),
        base_path: dir.to_path_buf(),
        secrets_env: None,
        secrets_yaml: None,
        properties: None,
    };

    // Look for application files
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            match file_name {
                "application.secrets.env" => {
                    app_files.secrets_env = Some(path);
                }
                "application.secrets.yaml" => {
                    app_files.secrets_yaml = Some(path);
                }
                "application.properties" => {
                    app_files.properties = Some(path);
                }
                _ => {}
            }
        }
    }

    Ok(app_files)
}

impl ApplicationFiles {
    #[must_use]
    pub fn has_any_files(&self) -> bool {
        self.secrets_env.is_some() || self.secrets_yaml.is_some() || self.properties.is_some()
    }
}

/// Parse secrets from application.secrets.env and application.secrets.yaml
/// Supports SOPS-encrypted files
#[allow(
    clippy::missing_errors_doc,
    reason = "Error documentation is provided in doc comments"
)]
pub async fn parse_secrets(
    app_files: &ApplicationFiles,
    sops_private_key: Option<&str>,
) -> Result<HashMap<String, String>> {
    let mut secrets = HashMap::new();

    // Parse application.secrets.env
    if let Some(ref path) = app_files.secrets_env {
        debug!("Parsing secrets from: {}", path.display());
        let env_secrets = parse_env_file(path, sops_private_key).await?;
        secrets.extend(env_secrets);
    }

    // Parse application.secrets.yaml
    if let Some(ref path) = app_files.secrets_yaml {
        debug!("Parsing secrets from: {}", path.display());
        let yaml_secrets = parse_yaml_secrets(path, sops_private_key).await?;
        secrets.extend(yaml_secrets);
    }

    Ok(secrets)
}

/// Parse properties from application.properties
#[allow(
    clippy::missing_errors_doc,
    reason = "Error documentation is provided in doc comments"
)]
pub async fn parse_properties(app_files: &ApplicationFiles) -> Result<HashMap<String, String>> {
    if let Some(ref path) = app_files.properties {
        debug!("Parsing properties from: {}", path.display());
        parse_properties_file(path).await
    } else {
        Ok(HashMap::new())
    }
}

async fn parse_env_file(
    path: &Path,
    sops_private_key: Option<&str>,
) -> Result<HashMap<String, String>> {
    let content = tokio::fs::read_to_string(path)
        .await
        .context(format!("Failed to read: {}", path.display()))?;

    // Check if file is SOPS-encrypted
    // SECURITY: Decrypted content exists only in memory, never written to disk
    let content = if is_sops_encrypted(&content) {
        debug!("Detected SOPS-encrypted file: {}", path.display());
        decrypt_sops_content(&content, sops_private_key)
            .await
            .context("Failed to decrypt SOPS file")?
    } else {
        content
    };

    // Parse .env format from in-memory buffer (no disk writes)
    // Parse line-by-line from the in-memory content string
    // SECURITY: All parsing happens in memory, no temp files
    let mut secrets = HashMap::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse KEY=VALUE format
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            secrets.insert(key, value);
        }
    }

    Ok(secrets)
}

async fn parse_yaml_secrets(
    path: &Path,
    sops_private_key: Option<&str>,
) -> Result<HashMap<String, String>> {
    let content = tokio::fs::read_to_string(path)
        .await
        .context(format!("Failed to read: {}", path.display()))?;

    // Check if file is SOPS-encrypted
    // SECURITY: Decrypted content exists only in memory, never written to disk
    let content = if is_sops_encrypted(&content) {
        debug!("Detected SOPS-encrypted file: {}", path.display());
        decrypt_sops_content(&content, sops_private_key)
            .await
            .context("Failed to decrypt SOPS file")?
    } else {
        content
    };

    // Parse YAML from in-memory buffer (no disk writes)
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content).context("Failed to parse YAML")?;

    let mut secrets = HashMap::new();
    flatten_yaml_value(&yaml, String::new(), &mut secrets);

    Ok(secrets)
}

fn flatten_yaml_value(
    value: &serde_yaml::Value,
    prefix: String,
    result: &mut HashMap<String, String>,
) {
    match value {
        serde_yaml::Value::Mapping(map) => {
            for (key, val) in map {
                let key_str = key.as_str().unwrap_or("").to_string();
                let new_prefix = if prefix.is_empty() {
                    key_str
                } else {
                    format!("{prefix}.{key_str}")
                };
                flatten_yaml_value(val, new_prefix, result);
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for (idx, val) in seq.iter().enumerate() {
                let new_prefix = format!("{prefix}[{idx}]");
                flatten_yaml_value(val, new_prefix, result);
            }
        }
        serde_yaml::Value::String(s) => {
            result.insert(prefix, s.clone());
        }
        serde_yaml::Value::Number(n) => {
            result.insert(prefix, n.to_string());
        }
        serde_yaml::Value::Bool(b) => {
            result.insert(prefix, b.to_string());
        }
        serde_yaml::Value::Null => {
            result.insert(prefix, String::new());
        }
        serde_yaml::Value::Tagged(_) => {
            // Skip tagged values (SOPS metadata, etc.)
            // These are typically metadata and not actual secret values
        }
    }
}

async fn parse_properties_file(path: &Path) -> Result<HashMap<String, String>> {
    let content = tokio::fs::read_to_string(path)
        .await
        .context(format!("Failed to read: {}", path.display()))?;

    let mut properties = HashMap::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse KEY=VALUE format
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            properties.insert(key, value);
        }
    }

    Ok(properties)
}

/// Check if content is SOPS-encrypted by looking for SOPS metadata
fn is_sops_encrypted(content: &str) -> bool {
    // SOPS files have a specific structure with sops metadata
    // Check for common SOPS indicators:
    // 1. YAML files start with "sops:" key
    // 2. JSON files have "sops" key at root
    // 3. ENV files might have SOPS metadata comments

    // Try parsing as YAML first (most common)
    if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(content) {
        if yaml
            .as_mapping()
            .and_then(|m| m.get(serde_yaml::Value::String("sops".to_string())))
            .is_some()
        {
            return true;
        }
    }

    // Try parsing as JSON
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
        if json.get("sops").is_some() {
            return true;
        }
    }

    // Check for SOPS metadata in comments (for ENV files)
    if content.contains("sops_version") || content.contains("sops_encrypted") {
        return true;
    }

    false
}

/// Decrypt SOPS-encrypted content using sops binary
///
/// This function uses the sops binary for decryption, which is the current
/// production implementation. The rops crate implementation is deactivated
/// (see `decrypt_with_rops` for details).
pub async fn decrypt_sops_content(content: &str, sops_private_key: Option<&str>) -> Result<String> {
    let content_size = content.len();
    let encryption_method = if sops_private_key.is_some() {
        "gpg"
    } else {
        "system_keyring"
    };

    let span = info_span!(
        "sops.decrypt",
        file.size = content_size,
        encryption.method = encryption_method
    );
    let span_clone = span.clone();
    let start = Instant::now();

    async move {
        // Use sops binary (current implementation)
        debug!("Attempting SOPS decryption using sops binary");
        let result = decrypt_with_sops_binary(content, sops_private_key).await;

        match &result {
            Ok(_) => {
                span_clone.record("decryption.method", "sops_binary");
                span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                span_clone.record("operation.success", true);
                metrics::increment_sops_decryption_total();
                metrics::observe_sops_decryption_duration(start.elapsed().as_secs_f64());
            }
            Err(e) => {
                span_clone.record("decryption.method", "sops_binary");
                span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                span_clone.record("operation.success", false);
                span_clone.record("error.message", e.to_string());
                metrics::increment_sops_decryption_errors_total();
            }
        }

        result
    }
    .instrument(span)
    .await
}

/// Decrypt SOPS content using rops crate with GPG private key
///
/// **STATUS: DEACTIVATED** - This implementation is currently deactivated.
/// We use the sops binary instead, which is more reliable and doesn't require
/// keys to be in the system keyring.
///
/// The rops crate API is complex and requires:
/// 1. Parsing SOPS file format (YAML/JSON) with proper type system
/// 2. Handling GPG keys via integration modules
/// 3. Decrypting with proper file format types (YamlFileFormat, JsonFileFormat, etc.)
///
/// For now, we use the sops binary which handles all of this automatically.
/// This stub is kept for future reference if we decide to implement rops support.
#[allow(dead_code, reason = "Kept as stub for future rops implementation")]
fn decrypt_with_rops(_content: &str, _private_key: &str) -> Result<String> {
    // DEACTIVATED: rops crate decryption is not implemented
    // We use sops binary instead (see decrypt_with_sops_binary)
    Err(anyhow::anyhow!(
        "rops crate decryption is deactivated - using sops binary instead"
    ))
}

/// Decrypt SOPS content using sops binary via stdin/stdout
///
/// **SECURITY**: This implementation pipes encrypted content directly to SOPS stdin
/// and captures decrypted output from stdout. This ensures:
/// - No encrypted content written to disk
/// - No decrypted content written to disk (SOPS processes in memory)
/// - Decrypted content only exists in process memory
async fn decrypt_with_sops_binary(content: &str, sops_private_key: Option<&str>) -> Result<String> {
    use std::process::Stdio;

    // Check if sops binary is available
    let sops_path = which::which("sops")
        .context("sops binary not found in PATH. Please install sops: brew install sops (macOS) or see https://github.com/mozilla/sops")?;

    debug!("Using sops binary at: {:?}", sops_path);

    // Set up GPG keyring if private key is provided
    let gpg_home = if let Some(private_key) = sops_private_key {
        debug!("Importing GPG private key into temporary keyring");
        import_gpg_key(private_key).await?
    } else {
        None
    };

    // Prepare sops command to read from stdin (/dev/stdin)
    // This ensures SOPS never writes decrypted content to disk
    let mut cmd = tokio::process::Command::new(sops_path);
    cmd.arg("-d") // Decrypt
        .arg("/dev/stdin") // Read encrypted content from stdin
        .stdin(Stdio::piped()) // Pipe encrypted content to stdin
        .stdout(Stdio::piped()) // Capture decrypted content from stdout
        .stderr(Stdio::piped());

    // Set GPG home directory if we created a temporary one
    if let Some(ref gpg_home_path) = gpg_home {
        cmd.env("GNUPGHOME", gpg_home_path);
        debug!("Using temporary GPG home: {:?}", gpg_home_path);
    }

    // Spawn the process
    let mut child = cmd.spawn().context("Failed to spawn sops command")?;

    // Write encrypted content to stdin (never touches disk)
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(content.as_bytes())
            .await
            .context("Failed to write encrypted content to sops stdin")?;
        stdin
            .shutdown()
            .await
            .context("Failed to close sops stdin")?;
    }

    // Wait for process to complete and capture output
    let output = child
        .wait_with_output()
        .await
        .context("Failed to wait for sops command")?;

    // Clean up temporary GPG home directory
    if let Some(ref gpg_home_path) = gpg_home {
        let _ = tokio::fs::remove_dir_all(gpg_home_path).await;
    }

    if output.status.success() {
        // SECURITY: Decrypted content exists only in memory (from stdout pipe)
        // Never written to disk - only exists in this String
        let decrypted =
            String::from_utf8(output.stdout).context("sops output is not valid UTF-8")?;
        Ok(decrypted)
    } else {
        // SECURITY: Only log error message, never log decrypted content
        // Truncate error message to avoid potential secret leakage in error output
        let error_msg = String::from_utf8_lossy(&output.stderr);
        let safe_error = if error_msg.len() > 200 {
            format!("{}... (truncated)", &error_msg[..200])
        } else {
            error_msg.to_string()
        };
        Err(anyhow::anyhow!(
            "sops decryption failed: {} (exit code: {})",
            safe_error,
            output.status.code().unwrap_or(-1)
        ))
    }
}

/// Import GPG private key into a temporary GPG home directory
/// Returns the path to the temporary GPG home directory if successful
async fn import_gpg_key(private_key: &str) -> Result<Option<PathBuf>> {
    use std::process::Stdio;

    // Check if gpg binary is available
    let gpg_path = match which::which("gpg") {
        Ok(path) => path,
        Err(_) => {
            warn!(
                "gpg binary not found - SOPS decryption may fail if key is not in system keyring"
            );
            return Ok(None);
        }
    };

    // Create temporary GPG home directory
    let temp_dir = std::env::temp_dir();
    let gpg_home = temp_dir.join(format!("gpg-home-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&gpg_home)
        .await
        .context("Failed to create temporary GPG home directory")?;

    debug!("Created temporary GPG home: {:?}", gpg_home);

    // Import private key into temporary keyring
    let mut cmd = tokio::process::Command::new(gpg_path);
    cmd.env("GNUPGHOME", &gpg_home)
        .arg("--batch")
        .arg("--yes")
        .arg("--import")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().context("Failed to spawn gpg import command")?;

    // Write private key to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(private_key.as_bytes())
            .await
            .context("Failed to write GPG private key to stdin")?;
        stdin.shutdown().await.context("Failed to close stdin")?;
    }

    let output = child
        .wait_with_output()
        .await
        .context("Failed to wait for gpg import command")?;

    if output.status.success() {
        debug!("Successfully imported GPG private key");
        Ok(Some(gpg_home))
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        warn!("Failed to import GPG private key: {}", error_msg);
        // Clean up on failure
        let _ = tokio::fs::remove_dir_all(&gpg_home).await;
        Err(anyhow::anyhow!(
            "Failed to import GPG private key: {error_msg}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    mod normalize_base_path_tests {
        use super::*;

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
        use super::*;

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
    pgp: []
    encrypted_regex: ^(data|stringData)$
    version: 3.8.1
database:
    url: ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]
"#;
            assert!(is_sops_encrypted(content));
        }

        #[test]
        fn test_is_sops_encrypted_json() {
            let content = r#"{
  "sops": {
    "kms": [],
    "gcp_kms": [],
    "lastmodified": "2024-01-01T00:00:00Z",
    "mac": "ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]",
    "version": "3.8.1"
  },
  "database": {
    "url": "ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]"
  }
}"#;
            assert!(is_sops_encrypted(content));
        }

        #[test]
        fn test_is_sops_encrypted_env_with_metadata() {
            let content = r#"# sops_version=3.8.1
# sops_encrypted=true
DATABASE_URL=ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]
"#;
            assert!(is_sops_encrypted(content));
        }

        #[test]
        fn test_is_sops_encrypted_plain_yaml() {
            let content = r#"database:
    url: postgres://localhost:5432/mydb
    user: admin
"#;
            assert!(!is_sops_encrypted(content));
        }

        #[test]
        fn test_is_sops_encrypted_plain_env() {
            let content = r#"DATABASE_URL=postgres://localhost:5432/mydb
DATABASE_USER=admin
"#;
            assert!(!is_sops_encrypted(content));
        }
    }

    mod flatten_yaml_value_tests {
        use super::*;

        #[test]
        fn test_flatten_yaml_value_simple() {
            let yaml: serde_yaml::Value = serde_yaml::from_str(
                r#"database:
  url: postgres://localhost:5432/mydb
  user: admin
"#,
            )
            .unwrap();
            let mut result = HashMap::new();
            flatten_yaml_value(&yaml, String::new(), &mut result);

            assert_eq!(
                result.get("database.url"),
                Some(&"postgres://localhost:5432/mydb".to_string())
            );
            assert_eq!(result.get("database.user"), Some(&"admin".to_string()));
        }

        #[test]
        fn test_flatten_yaml_value_nested() {
            let yaml: serde_yaml::Value = serde_yaml::from_str(
                r#"api:
  keys:
    service1: key1
    service2: key2
"#,
            )
            .unwrap();
            let mut result = HashMap::new();
            flatten_yaml_value(&yaml, String::new(), &mut result);

            assert_eq!(result.get("api.keys.service1"), Some(&"key1".to_string()));
            assert_eq!(result.get("api.keys.service2"), Some(&"key2".to_string()));
        }

        #[test]
        fn test_flatten_yaml_value_with_prefix() {
            let yaml: serde_yaml::Value = serde_yaml::from_str(
                r#"database:
  url: postgres://localhost:5432/mydb
"#,
            )
            .unwrap();
            let mut result = HashMap::new();
            flatten_yaml_value(&yaml, "prefix".to_string(), &mut result);

            assert_eq!(
                result.get("prefix.database.url"),
                Some(&"postgres://localhost:5432/mydb".to_string())
            );
        }

        #[test]
        fn test_flatten_yaml_value_array() {
            let yaml: serde_yaml::Value = serde_yaml::from_str(
                r#"servers:
  - host: server1
    port: 8080
  - host: server2
    port: 8081
"#,
            )
            .unwrap();
            let mut result = HashMap::new();
            flatten_yaml_value(&yaml, String::new(), &mut result);

            // Arrays should be flattened with indices
            assert!(result.contains_key("servers[0].host"));
            assert!(result.contains_key("servers[0].port"));
            assert!(result.contains_key("servers[1].host"));
            assert!(result.contains_key("servers[1].port"));
        }

        #[test]
        fn test_flatten_yaml_value_number() {
            let yaml: serde_yaml::Value = serde_yaml::from_str(
                r#"timeout: 30
retries: 3
"#,
            )
            .unwrap();
            let mut result = HashMap::new();
            flatten_yaml_value(&yaml, String::new(), &mut result);

            assert_eq!(result.get("timeout"), Some(&"30".to_string()));
            assert_eq!(result.get("retries"), Some(&"3".to_string()));
        }

        #[test]
        fn test_flatten_yaml_value_boolean() {
            let yaml: serde_yaml::Value = serde_yaml::from_str(
                r#"enabled: true
disabled: false
"#,
            )
            .unwrap();
            let mut result = HashMap::new();
            flatten_yaml_value(&yaml, String::new(), &mut result);

            assert_eq!(result.get("enabled"), Some(&"true".to_string()));
            assert_eq!(result.get("disabled"), Some(&"false".to_string()));
        }

        #[test]
        fn test_flatten_yaml_value_null() {
            let yaml: serde_yaml::Value = serde_yaml::from_str(
                r#"optional: null
required: value
"#,
            )
            .unwrap();
            let mut result = HashMap::new();
            flatten_yaml_value(&yaml, String::new(), &mut result);

            assert_eq!(result.get("optional"), Some(&String::new()));
            assert_eq!(result.get("required"), Some(&"value".to_string()));
        }
    }

    mod parse_env_file_tests {
        use super::*;

        #[tokio::test]
        async fn test_parse_env_file_simple() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.env");
            fs::write(
                &file_path,
                "DATABASE_URL=postgres://localhost:5432/mydb\nDATABASE_USER=admin\n",
            )
            .unwrap();

            let result = parse_env_file(&file_path, None).await.unwrap();

            assert_eq!(
                result.get("DATABASE_URL"),
                Some(&"postgres://localhost:5432/mydb".to_string())
            );
            assert_eq!(result.get("DATABASE_USER"), Some(&"admin".to_string()));
        }

        #[tokio::test]
        async fn test_parse_env_file_with_comments() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.env");
            fs::write(
                &file_path,
                "# Database configuration\nDATABASE_URL=postgres://localhost:5432/mydb\n# End of file\n",
            )
            .unwrap();

            let result = parse_env_file(&file_path, None).await.unwrap();

            assert_eq!(
                result.get("DATABASE_URL"),
                Some(&"postgres://localhost:5432/mydb".to_string())
            );
            assert!(!result.contains_key("# Database configuration"));
        }

        #[tokio::test]
        async fn test_parse_env_file_with_empty_lines() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.env");
            fs::write(&file_path, "KEY1=value1\n\nKEY2=value2\n\n").unwrap();

            let result = parse_env_file(&file_path, None).await.unwrap();

            assert_eq!(result.get("KEY1"), Some(&"value1".to_string()));
            assert_eq!(result.get("KEY2"), Some(&"value2".to_string()));
        }

        #[tokio::test]
        async fn test_parse_env_file_with_whitespace() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.env");
            fs::write(&file_path, "  KEY1  =  value1  \nKEY2=value2\n").unwrap();

            let result = parse_env_file(&file_path, None).await.unwrap();

            assert_eq!(result.get("KEY1"), Some(&"value1".to_string()));
            assert_eq!(result.get("KEY2"), Some(&"value2".to_string()));
        }

        #[tokio::test]
        async fn test_parse_env_file_nonexistent() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("nonexistent.env");

            let result = parse_env_file(&file_path, None).await;

            assert!(result.is_err());
        }
    }

    mod parse_properties_file_tests {
        use super::*;

        #[tokio::test]
        async fn test_parse_properties_file_simple() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.properties");
            fs::write(
                &file_path,
                "database.url=postgres://localhost:5432/mydb\ndatabase.user=admin\n",
            )
            .unwrap();

            let result = parse_properties_file(&file_path).await.unwrap();

            assert_eq!(
                result.get("database.url"),
                Some(&"postgres://localhost:5432/mydb".to_string())
            );
            assert_eq!(result.get("database.user"), Some(&"admin".to_string()));
        }

        #[tokio::test]
        async fn test_parse_properties_file_with_comments() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.properties");
            fs::write(
                &file_path,
                "# Properties file\ndatabase.url=postgres://localhost:5432/mydb\n",
            )
            .unwrap();

            let result = parse_properties_file(&file_path).await.unwrap();

            assert_eq!(
                result.get("database.url"),
                Some(&"postgres://localhost:5432/mydb".to_string())
            );
        }

        #[tokio::test]
        async fn test_parse_properties_file_nonexistent() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("nonexistent.properties");

            let result = parse_properties_file(&file_path).await;

            assert!(result.is_err());
        }
    }

    mod parse_yaml_secrets_tests {
        use super::*;

        #[tokio::test]
        async fn test_parse_yaml_secrets_simple() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.yaml");
            fs::write(
                &file_path,
                r#"database:
  url: postgres://localhost:5432/mydb
  user: admin
"#,
            )
            .unwrap();

            let result = parse_yaml_secrets(&file_path, None).await.unwrap();

            assert_eq!(
                result.get("database.url"),
                Some(&"postgres://localhost:5432/mydb".to_string())
            );
            assert_eq!(result.get("database.user"), Some(&"admin".to_string()));
        }

        #[tokio::test]
        async fn test_parse_yaml_secrets_nested() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.yaml");
            fs::write(
                &file_path,
                r#"api:
  keys:
    service1: key1
    service2: key2
"#,
            )
            .unwrap();

            let result = parse_yaml_secrets(&file_path, None).await.unwrap();

            assert_eq!(result.get("api.keys.service1"), Some(&"key1".to_string()));
            assert_eq!(result.get("api.keys.service2"), Some(&"key2".to_string()));
        }

        #[tokio::test]
        async fn test_parse_yaml_secrets_invalid_yaml() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.yaml");
            fs::write(&file_path, "invalid: yaml: content: [").unwrap();

            let result = parse_yaml_secrets(&file_path, None).await;

            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_parse_yaml_secrets_nonexistent() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("nonexistent.yaml");

            let result = parse_yaml_secrets(&file_path, None).await;

            assert!(result.is_err());
        }
    }

    mod application_files_tests {
        use super::*;

        #[test]
        fn test_application_files_has_any_files_all_none() {
            let files = ApplicationFiles {
                service_name: "test".to_string(),
                base_path: PathBuf::from("/tmp"),
                secrets_env: None,
                secrets_yaml: None,
                properties: None,
            };
            assert!(!files.has_any_files());
        }

        #[test]
        fn test_application_files_has_any_files_with_env() {
            let files = ApplicationFiles {
                service_name: "test".to_string(),
                base_path: PathBuf::from("/tmp"),
                secrets_env: Some(PathBuf::from("/tmp/secrets.env")),
                secrets_yaml: None,
                properties: None,
            };
            assert!(files.has_any_files());
        }

        #[test]
        fn test_application_files_has_any_files_with_yaml() {
            let files = ApplicationFiles {
                service_name: "test".to_string(),
                base_path: PathBuf::from("/tmp"),
                secrets_env: None,
                secrets_yaml: Some(PathBuf::from("/tmp/secrets.yaml")),
                properties: None,
            };
            assert!(files.has_any_files());
        }

        #[test]
        fn test_application_files_has_any_files_with_properties() {
            let files = ApplicationFiles {
                service_name: "test".to_string(),
                base_path: PathBuf::from("/tmp"),
                secrets_env: None,
                secrets_yaml: None,
                properties: Some(PathBuf::from("/tmp/properties")),
            };
            assert!(files.has_any_files());
        }
    }

    mod parse_secrets_tests {
        use super::*;

        #[tokio::test]
        async fn test_parse_secrets_empty_files() {
            let files = ApplicationFiles {
                service_name: "test".to_string(),
                base_path: PathBuf::from("/tmp"),
                secrets_env: None,
                secrets_yaml: None,
                properties: None,
            };
            let result = parse_secrets(&files, None).await.unwrap();
            assert!(result.is_empty());
        }

        #[tokio::test]
        async fn test_parse_secrets_with_env_only() {
            let temp_dir = TempDir::new().unwrap();
            let env_path = temp_dir.path().join("secrets.env");
            fs::write(&env_path, "KEY1=value1\nKEY2=value2\n").unwrap();

            let files = ApplicationFiles {
                service_name: "test".to_string(),
                base_path: temp_dir.path().to_path_buf(),
                secrets_env: Some(env_path),
                secrets_yaml: None,
                properties: None,
            };
            let result = parse_secrets(&files, None).await.unwrap();
            assert_eq!(result.get("KEY1"), Some(&"value1".to_string()));
            assert_eq!(result.get("KEY2"), Some(&"value2".to_string()));
        }

        #[tokio::test]
        async fn test_parse_secrets_with_yaml_only() {
            let temp_dir = TempDir::new().unwrap();
            let yaml_path = temp_dir.path().join("secrets.yaml");
            fs::write(
                &yaml_path,
                r#"database:
  url: postgres://localhost:5432/mydb
"#,
            )
            .unwrap();

            let files = ApplicationFiles {
                service_name: "test".to_string(),
                base_path: temp_dir.path().to_path_buf(),
                secrets_env: None,
                secrets_yaml: Some(yaml_path),
                properties: None,
            };
            let result = parse_secrets(&files, None).await.unwrap();
            assert_eq!(
                result.get("database.url"),
                Some(&"postgres://localhost:5432/mydb".to_string())
            );
        }

        #[tokio::test]
        async fn test_parse_secrets_with_both_env_and_yaml() {
            let temp_dir = TempDir::new().unwrap();
            let env_path = temp_dir.path().join("secrets.env");
            let yaml_path = temp_dir.path().join("secrets.yaml");
            fs::write(&env_path, "ENV_KEY=env_value\n").unwrap();
            fs::write(
                &yaml_path,
                r#"yaml:
  key: yaml_value
"#,
            )
            .unwrap();

            let files = ApplicationFiles {
                service_name: "test".to_string(),
                base_path: temp_dir.path().to_path_buf(),
                secrets_env: Some(env_path),
                secrets_yaml: Some(yaml_path),
                properties: None,
            };
            let result = parse_secrets(&files, None).await.unwrap();
            assert_eq!(result.get("ENV_KEY"), Some(&"env_value".to_string()));
            assert_eq!(result.get("yaml.key"), Some(&"yaml_value".to_string()));
        }
    }

    mod parse_properties_tests {
        use super::*;

        #[tokio::test]
        async fn test_parse_properties_no_properties_file() {
            let files = ApplicationFiles {
                service_name: "test".to_string(),
                base_path: PathBuf::from("/tmp"),
                secrets_env: None,
                secrets_yaml: None,
                properties: None,
            };
            let result = parse_properties(&files).await.unwrap();
            assert!(result.is_empty());
        }

        #[tokio::test]
        async fn test_parse_properties_with_file() {
            let temp_dir = TempDir::new().unwrap();
            let props_path = temp_dir.path().join("properties");
            fs::write(
                &props_path,
                "database.url=postgres://localhost:5432/mydb\ndatabase.user=admin\n",
            )
            .unwrap();

            let files = ApplicationFiles {
                service_name: "test".to_string(),
                base_path: temp_dir.path().to_path_buf(),
                secrets_env: None,
                secrets_yaml: None,
                properties: Some(props_path),
            };
            let result = parse_properties(&files).await.unwrap();
            assert_eq!(
                result.get("database.url"),
                Some(&"postgres://localhost:5432/mydb".to_string())
            );
            assert_eq!(result.get("database.user"), Some(&"admin".to_string()));
        }
    }

    mod flatten_yaml_value_edge_cases_tests {
        use super::*;

        #[test]
        fn test_flatten_yaml_value_empty_prefix() {
            let mut result = HashMap::new();
            let value = serde_yaml::from_str::<serde_yaml::Value>("key: value").unwrap();
            flatten_yaml_value(&value, String::new(), &mut result);
            assert_eq!(result.get("key"), Some(&"value".to_string()));
        }

        #[test]
        fn test_flatten_yaml_value_nested_arrays() {
            let mut result = HashMap::new();
            let value = serde_yaml::from_str::<serde_yaml::Value>(
                r#"items:
  - first
  - second
"#,
            )
            .unwrap();
            flatten_yaml_value(&value, String::new(), &mut result);
            assert_eq!(result.get("items[0]"), Some(&"first".to_string()));
            assert_eq!(result.get("items[1]"), Some(&"second".to_string()));
        }

        #[test]
        fn test_flatten_yaml_value_numbers() {
            let mut result = HashMap::new();
            let value = serde_yaml::from_str::<serde_yaml::Value>("port: 8080").unwrap();
            flatten_yaml_value(&value, String::new(), &mut result);
            assert_eq!(result.get("port"), Some(&"8080".to_string()));
        }

        #[test]
        fn test_flatten_yaml_value_booleans() {
            let mut result = HashMap::new();
            let value = serde_yaml::from_str::<serde_yaml::Value>("enabled: true").unwrap();
            flatten_yaml_value(&value, String::new(), &mut result);
            assert_eq!(result.get("enabled"), Some(&"true".to_string()));
        }

        #[test]
        fn test_flatten_yaml_value_null() {
            let mut result = HashMap::new();
            let value = serde_yaml::from_str::<serde_yaml::Value>("key: null").unwrap();
            flatten_yaml_value(&value, String::new(), &mut result);
            assert_eq!(result.get("key"), Some(&String::new()));
        }

        #[test]
        fn test_flatten_yaml_value_complex_nested() {
            let mut result = HashMap::new();
            let value = serde_yaml::from_str::<serde_yaml::Value>(
                r#"api:
  version: 1
  endpoints:
    - /health
    - /metrics
  enabled: true
"#,
            )
            .unwrap();
            flatten_yaml_value(&value, String::new(), &mut result);
            assert_eq!(result.get("api.version"), Some(&"1".to_string()));
            assert_eq!(result.get("api.endpoints[0]"), Some(&"/health".to_string()));
            assert_eq!(
                result.get("api.endpoints[1]"),
                Some(&"/metrics".to_string())
            );
            assert_eq!(result.get("api.enabled"), Some(&"true".to_string()));
        }

        #[test]
        fn test_flatten_yaml_value_non_string_key() {
            let mut result = HashMap::new();
            // YAML with numeric key (edge case)
            let value = serde_yaml::from_str::<serde_yaml::Value>("123: value").unwrap();
            flatten_yaml_value(&value, String::new(), &mut result);
            // Non-string keys become empty string
            assert_eq!(result.get(""), Some(&"value".to_string()));
        }
    }

    mod find_files_in_directory_tests {
        use super::*;
        use tempfile::TempDir;

        #[test]
        fn test_find_files_in_directory_all_files() {
            let temp_dir = TempDir::new().unwrap();
            let env_path = temp_dir.path().join("application.secrets.env");
            let yaml_path = temp_dir.path().join("application.secrets.yaml");
            let props_path = temp_dir.path().join("application.properties");
            fs::write(&env_path, "KEY=value").unwrap();
            fs::write(&yaml_path, "key: value").unwrap();
            fs::write(&props_path, "key=value").unwrap();

            let result = find_files_in_directory("test-service", temp_dir.path()).unwrap();

            assert_eq!(result.service_name, "test-service");
            assert_eq!(result.secrets_env, Some(env_path));
            assert_eq!(result.secrets_yaml, Some(yaml_path));
            assert_eq!(result.properties, Some(props_path));
        }

        #[test]
        fn test_find_files_in_directory_no_files() {
            let temp_dir = TempDir::new().unwrap();

            let result = find_files_in_directory("test-service", temp_dir.path()).unwrap();

            assert_eq!(result.service_name, "test-service");
            assert!(result.secrets_env.is_none());
            assert!(result.secrets_yaml.is_none());
            assert!(result.properties.is_none());
        }

        #[test]
        fn test_find_files_in_directory_partial_files() {
            let temp_dir = TempDir::new().unwrap();
            let env_path = temp_dir.path().join("application.secrets.env");
            fs::write(&env_path, "KEY=value").unwrap();

            let result = find_files_in_directory("test-service", temp_dir.path()).unwrap();

            assert_eq!(result.secrets_env, Some(env_path));
            assert!(result.secrets_yaml.is_none());
            assert!(result.properties.is_none());
        }

        #[test]
        fn test_find_files_in_directory_ignores_other_files() {
            let temp_dir = TempDir::new().unwrap();
            let env_path = temp_dir.path().join("application.secrets.env");
            let other_file = temp_dir.path().join("other-file.txt");
            fs::write(&env_path, "KEY=value").unwrap();
            fs::write(&other_file, "content").unwrap();

            let result = find_files_in_directory("test-service", temp_dir.path()).unwrap();

            assert_eq!(result.secrets_env, Some(env_path));
            assert!(result.secrets_yaml.is_none());
            assert!(result.properties.is_none());
        }

        #[test]
        fn test_find_files_in_directory_nonexistent_dir() {
            let result =
                find_files_in_directory("test-service", std::path::Path::new("/nonexistent/path"));

            assert!(result.is_err());
        }
    }

    mod parse_env_file_edge_cases_tests {
        use super::*;
        use tempfile::TempDir;

        #[tokio::test]
        async fn test_parse_env_file_with_comments() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.env");
            fs::write(
                &file_path,
                "# Comment line\nKEY1=value1\n# Another comment\nKEY2=value2\n",
            )
            .unwrap();

            let result = parse_env_file(&file_path, None).await.unwrap();

            assert_eq!(result.get("KEY1"), Some(&"value1".to_string()));
            assert_eq!(result.get("KEY2"), Some(&"value2".to_string()));
            assert!(!result.contains_key("# Comment"));
        }

        #[tokio::test]
        async fn test_parse_env_file_with_empty_value() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.env");
            fs::write(&file_path, "KEY1=\nKEY2=value2\n").unwrap();

            let result = parse_env_file(&file_path, None).await.unwrap();

            assert_eq!(result.get("KEY1"), Some(&String::new()));
            assert_eq!(result.get("KEY2"), Some(&"value2".to_string()));
        }

        #[tokio::test]
        async fn test_parse_env_file_with_equals_in_value() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.env");
            fs::write(&file_path, "URL=https://example.com?key=value\n").unwrap();

            let result = parse_env_file(&file_path, None).await.unwrap();

            // split_once only splits on first =
            assert_eq!(
                result.get("URL"),
                Some(&"https://example.com?key=value".to_string())
            );
        }

        #[tokio::test]
        async fn test_parse_env_file_with_no_equals() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.env");
            fs::write(&file_path, "KEY1=value1\nINVALID_LINE\nKEY2=value2\n").unwrap();

            let result = parse_env_file(&file_path, None).await.unwrap();

            assert_eq!(result.get("KEY1"), Some(&"value1".to_string()));
            assert_eq!(result.get("KEY2"), Some(&"value2".to_string()));
            assert!(!result.contains_key("INVALID_LINE"));
        }

        #[tokio::test]
        async fn test_parse_env_file_only_comments() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.env");
            fs::write(&file_path, "# Comment 1\n# Comment 2\n").unwrap();

            let result = parse_env_file(&file_path, None).await.unwrap();

            assert!(result.is_empty());
        }

        #[tokio::test]
        async fn test_parse_env_file_only_empty_lines() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.env");
            fs::write(&file_path, "\n\n\n").unwrap();

            let result = parse_env_file(&file_path, None).await.unwrap();

            assert!(result.is_empty());
        }
    }

    mod parse_properties_file_edge_cases_tests {
        use super::*;
        use tempfile::TempDir;

        #[tokio::test]
        async fn test_parse_properties_file_with_empty_value() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.properties");
            fs::write(&file_path, "key1=\nkey2=value2\n").unwrap();

            let result = parse_properties_file(&file_path).await.unwrap();

            assert_eq!(result.get("key1"), Some(&String::new()));
            assert_eq!(result.get("key2"), Some(&"value2".to_string()));
        }

        #[tokio::test]
        async fn test_parse_properties_file_with_equals_in_value() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.properties");
            fs::write(&file_path, "url=https://example.com?key=value\n").unwrap();

            let result = parse_properties_file(&file_path).await.unwrap();

            assert_eq!(
                result.get("url"),
                Some(&"https://example.com?key=value".to_string())
            );
        }

        #[tokio::test]
        async fn test_parse_properties_file_with_no_equals() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.properties");
            fs::write(&file_path, "key1=value1\ninvalid_line\nkey2=value2\n").unwrap();

            let result = parse_properties_file(&file_path).await.unwrap();

            assert_eq!(result.get("key1"), Some(&"value1".to_string()));
            assert_eq!(result.get("key2"), Some(&"value2".to_string()));
            assert!(!result.contains_key("invalid_line"));
        }

        #[tokio::test]
        async fn test_parse_properties_file_only_comments() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.properties");
            fs::write(&file_path, "# Comment 1\n# Comment 2\n").unwrap();

            let result = parse_properties_file(&file_path).await.unwrap();

            assert!(result.is_empty());
        }

        #[tokio::test]
        async fn test_parse_properties_file_empty_file() {
            let temp_dir = TempDir::new().unwrap();
            let file_path = temp_dir.path().join("test.properties");
            fs::write(&file_path, "").unwrap();

            let result = parse_properties_file(&file_path).await.unwrap();

            assert!(result.is_empty());
        }
    }
}
