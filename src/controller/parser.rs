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

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tracing::{debug, warn};
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
#[allow(clippy::unused_async, clippy::missing_errors_doc)] // May be called from async contexts in the future
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
#[allow(clippy::missing_errors_doc)]
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
#[allow(clippy::missing_errors_doc)]
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
    let content = if is_sops_encrypted(&content) {
        debug!("Detected SOPS-encrypted file: {}", path.display());
        decrypt_sops_content(&content, sops_private_key)
            .await
            .context("Failed to decrypt SOPS file")?
    } else {
        content
    };

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
    let content = if is_sops_encrypted(&content) {
        debug!("Detected SOPS-encrypted file: {}", path.display());
        decrypt_sops_content(&content, sops_private_key)
            .await
            .context("Failed to decrypt SOPS file")?
    } else {
        content
    };

    // Parse YAML as key-value pairs
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

/// Decrypt SOPS-encrypted content using rops crate or sops binary
///
/// Supports two methods:
/// 1. Using rops crate with GPG private key (future implementation)
/// 2. Using sops binary (current implementation - more reliable)
async fn decrypt_sops_content(content: &str, sops_private_key: Option<&str>) -> Result<String> {
    // Try rops crate first if private key is provided (future enhancement)
    if let Some(private_key) = sops_private_key {
        debug!("Attempting SOPS decryption with provided GPG private key");

        // Try using rops crate (currently falls back to sops binary)
        match decrypt_with_rops(content, private_key) {
            Ok(decrypted) => {
                debug!("Successfully decrypted SOPS content using rops");
                return Ok(decrypted);
            }
            Err(e) => {
                debug!("rops decryption failed: {}, trying sops binary", e);
                // Fall through to try sops binary
            }
        }
    }

    // Use sops binary (current implementation)
    debug!("Attempting SOPS decryption using sops binary");
    decrypt_with_sops_binary(content, sops_private_key).await
}

/// Decrypt SOPS content using rops crate with GPG private key
///
/// Note: The rops crate API may require GPG keys to be in the system keyring.
/// For now, we'll primarily use the sops binary which is more reliable.
fn decrypt_with_rops(_content: &str, _private_key: &str) -> Result<String> {
    // TODO: Implement rops crate decryption
    // The rops crate API needs to be verified - it may require:
    // 1. GPG keys to be imported into system keyring
    // 2. Different API than expected
    //
    // For now, return an error to fall back to sops binary
    Err(anyhow::anyhow!(
        "rops crate decryption not yet implemented - using sops binary fallback"
    ))
}

/// Decrypt SOPS content using sops binary (fallback method)
/// This is more reliable as it uses the actual sops tool
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

    // Create temporary file with encrypted content
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("sops-decrypt-{}.tmp", uuid::Uuid::new_v4()));

    // Write encrypted content to temp file
    tokio::fs::write(&temp_file, content)
        .await
        .context("Failed to write encrypted content to temp file")?;

    // Prepare sops command
    let mut cmd = tokio::process::Command::new(sops_path);
    cmd.arg("-d") // Decrypt
        .arg(&temp_file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Set GPG home directory if we created a temporary one
    if let Some(ref gpg_home_path) = gpg_home {
        cmd.env("GNUPGHOME", gpg_home_path);
        debug!("Using temporary GPG home: {:?}", gpg_home_path);
    }

    // Execute sops command
    let output = cmd
        .output()
        .await
        .context("Failed to execute sops command")?;

    // Clean up temp file
    let _ = tokio::fs::remove_file(&temp_file).await;

    // Clean up temporary GPG home directory
    if let Some(ref gpg_home_path) = gpg_home {
        let _ = tokio::fs::remove_dir_all(gpg_home_path).await;
    }

    if output.status.success() {
        let decrypted =
            String::from_utf8(output.stdout).context("sops output is not valid UTF-8")?;
        Ok(decrypted)
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!(
            "sops decryption failed: {} (exit code: {})",
            error_msg,
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
            "Failed to import GPG private key: {}",
            error_msg
        ))
    }
}
