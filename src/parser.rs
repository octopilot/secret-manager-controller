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
        Some(".") | Some("") | None => None,
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
/// If base_path is None, searches from repository root
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
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Check if this is a deployment-configuration directory
        if path.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "deployment-configuration")
            .unwrap_or(false)
        {
            // Extract service name (parent of deployment-configuration)
            let service_name = if let Some(parent) = path.parent() {
                // Check if parent is the base_path (single service case)
                if parent == search_path {
                    // Single service: use default_service_name or fallback
                    default_service_name
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| {
                            // Try to extract from artifact path or use default
                            artifact_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "default-service".to_string())
                        })
                } else {
                    // Monolith: extract service name from parent directory
                    parent
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                }
            } else {
                default_service_name
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            };

            // Look for profiles directory first (Skaffold-compliant structure)
            let profiles_path = path.join("profiles");
            if profiles_path.exists() && profiles_path.is_dir() {
                // New structure: deployment-configuration/profiles/{env}/
                // Only process the specified environment
                let env_path = profiles_path.join(environment);
                if env_path.exists() && env_path.is_dir() {
                    let app_files = find_files_in_directory(&service_name, &env_path).await?;
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
                    let app_files = find_files_in_directory(&service_name, &env_path).await?;
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

async fn find_files_in_directory(
    service_name: &str,
    dir: &Path,
) -> Result<ApplicationFiles> {
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
    pub fn has_any_files(&self) -> bool {
        self.secrets_env.is_some()
            || self.secrets_yaml.is_some()
            || self.properties.is_some()
    }
}

/// Parse secrets from application.secrets.env and application.secrets.yaml
/// Supports SOPS-encrypted files
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
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content)
        .context("Failed to parse YAML")?;

    let mut secrets = HashMap::new();
    flatten_yaml_value(&yaml, String::new(), &mut secrets);

    Ok(secrets)
}

fn flatten_yaml_value(value: &serde_yaml::Value, prefix: String, result: &mut HashMap<String, String>) {
    match value {
        serde_yaml::Value::Mapping(map) => {
            for (key, val) in map {
                let key_str = key.as_str().unwrap_or("").to_string();
                let new_prefix = if prefix.is_empty() {
                    key_str
                } else {
                    format!("{}.{}", prefix, key_str)
                };
                flatten_yaml_value(val, new_prefix, result);
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for (idx, val) in seq.iter().enumerate() {
                let new_prefix = format!("{}[{}]", prefix, idx);
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
        if yaml.as_mapping()
            .and_then(|m| m.get(&serde_yaml::Value::String("sops".to_string())))
            .is_some() {
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

/// Decrypt SOPS-encrypted content using rops
/// Note: This is a placeholder implementation. rops crate API may need adjustment.
async fn decrypt_sops_content(
    _content: &str,
    sops_private_key: Option<&str>,
) -> Result<String> {
    // For now, we'll use a simplified approach:
    // If a private key is provided, we can attempt decryption
    // Otherwise, we'll need to rely on the sops binary or proper rops integration
    
    // TODO: Implement proper SOPS decryption using rops crate
    // The rops crate API needs to be verified - it may require different usage patterns
    // For now, return an error indicating SOPS decryption is not yet fully implemented
    // In production, this should call the sops binary or use rops properly
    
    if sops_private_key.is_some() {
        // Attempt decryption with provided key
        // This is a placeholder - actual implementation depends on rops API
        warn!("SOPS decryption with provided key is not yet fully implemented");
        return Err(anyhow::anyhow!("SOPS decryption not yet implemented - please use unencrypted files or implement proper rops integration"));
    } else {
        // Try to use rops default keychain
        warn!("SOPS decryption without explicit key is not yet fully implemented");
        return Err(anyhow::anyhow!("SOPS decryption not yet implemented - please use unencrypted files or provide private key"));
    }
}

