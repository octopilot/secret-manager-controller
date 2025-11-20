//! # Parsers
//!
//! Parse application configuration files (env, yaml, properties).

use crate::controller::parser::sops::error::SopsDecryptionError;
use crate::controller::parser::sops::{decrypt_sops_content, is_sops_encrypted_impl};
use crate::controller::parser::types::ApplicationFiles;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;
use tracing::debug;

/// Represents a secret with its enabled/disabled state
#[derive(Debug, Clone, PartialEq)]
pub struct SecretEntry {
    /// The secret value
    pub value: String,
    /// Whether the secret is enabled (true) or disabled (false, i.e., commented out)
    pub enabled: bool,
}

/// Collection of secrets with their enabled/disabled state
#[derive(Debug, Clone, Default)]
pub struct ParsedSecrets {
    /// Map of secret key to SecretEntry
    pub secrets: HashMap<String, SecretEntry>,
}

/// Error type for parsing secrets
/// Wraps SOPS decryption errors and file I/O errors
#[derive(Debug, Error)]
pub enum ParseSecretsError {
    #[error("SOPS decryption failed: {0}")]
    SopsDecryption(#[from] SopsDecryptionError),
    #[error("File I/O error: {0}")]
    Io(#[from] anyhow::Error),
}

impl ParseSecretsError {
    /// Check if this is a SOPS decryption error
    pub fn as_sops_error(&self) -> Option<&SopsDecryptionError> {
        match self {
            ParseSecretsError::SopsDecryption(e) => Some(e),
            _ => None,
        }
    }

    /// Check if this error is transient (should retry)
    pub fn is_transient(&self) -> bool {
        match self {
            ParseSecretsError::SopsDecryption(e) => e.is_transient,
            ParseSecretsError::Io(_) => false, // File I/O errors are usually permanent
        }
    }

    /// Get remediation guidance for this error
    pub fn remediation(&self) -> String {
        match self {
            ParseSecretsError::SopsDecryption(e) => e.remediation(),
            ParseSecretsError::Io(e) => format!("File I/O error: {}", e),
        }
    }
}

/// Parse secrets from application.secrets.env and application.secrets.yaml
/// Supports SOPS-encrypted files
///
/// Returns `ParseSecretsError` which can be either a `SopsDecryptionError` or file I/O error.
/// This allows callers to properly classify errors as transient vs permanent.
///
/// **Note**: This function now tracks disabled secrets (commented lines) separately.
/// Use `parse_secrets_with_state()` to get enabled/disabled state information.
#[allow(
    clippy::missing_errors_doc,
    reason = "Error documentation is provided in doc comments"
)]
pub async fn parse_secrets(
    app_files: &ApplicationFiles,
    sops_private_key: Option<&str>,
) -> Result<HashMap<String, String>, ParseSecretsError> {
    let parsed = parse_secrets_with_state(app_files, sops_private_key).await?;
    // Return only enabled secrets for backward compatibility
    Ok(parsed
        .secrets
        .into_iter()
        .filter_map(|(k, v)| if v.enabled { Some((k, v.value)) } else { None })
        .collect())
}

/// Parse secrets from application.secrets.env and application.secrets.yaml with enabled/disabled state
/// Supports SOPS-encrypted files
///
/// Returns secrets with their enabled/disabled state. Disabled secrets are those that are commented out
/// (e.g., `#FOO_SECRET=value`). Disabled secrets should be disabled in the provider but not deleted.
///
/// Returns `ParseSecretsError` which can be either a `SopsDecryptionError` or file I/O error.
#[allow(
    clippy::missing_errors_doc,
    reason = "Error documentation is provided in doc comments"
)]
pub async fn parse_secrets_with_state(
    app_files: &ApplicationFiles,
    sops_private_key: Option<&str>,
) -> Result<ParsedSecrets, ParseSecretsError> {
    let mut parsed = ParsedSecrets::default();

    // Parse application.secrets.env
    if let Some(ref path) = app_files.secrets_env {
        debug!("Parsing secrets from: {}", path.display());
        let env_secrets = parse_env_file_with_state(path, sops_private_key).await?;
        // Merge secrets (later files override earlier ones)
        for (key, entry) in env_secrets.secrets {
            parsed.secrets.insert(key, entry);
        }
    }

    // Parse application.secrets.yaml
    if let Some(ref path) = app_files.secrets_yaml {
        debug!("Parsing secrets from: {}", path.display());
        let yaml_secrets = parse_yaml_secrets_with_state(path, sops_private_key).await?;
        // Merge secrets (later files override earlier ones)
        for (key, entry) in yaml_secrets.secrets {
            parsed.secrets.insert(key, entry);
        }
    }

    Ok(parsed)
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

#[allow(dead_code)]
pub(crate) async fn parse_env_file(
    path: &Path,
    sops_private_key: Option<&str>,
) -> Result<HashMap<String, String>, ParseSecretsError> {
    let parsed = parse_env_file_with_state(path, sops_private_key).await?;
    // Return only enabled secrets for backward compatibility
    Ok(parsed
        .secrets
        .into_iter()
        .filter_map(|(k, v)| if v.enabled { Some((k, v.value)) } else { None })
        .collect())
}

pub(crate) async fn parse_env_file_with_state(
    path: &Path,
    sops_private_key: Option<&str>,
) -> Result<ParsedSecrets, ParseSecretsError> {
    let content = tokio::fs::read_to_string(path)
        .await
        .context(format!("Failed to read: {}", path.display()))
        .map_err(ParseSecretsError::Io)?;

    // Check if file is SOPS-encrypted
    // SECURITY: Decrypted content exists only in memory, never written to disk
    let content = if is_sops_encrypted_impl(&content) {
        debug!("Detected SOPS-encrypted file: {}", path.display());
        decrypt_sops_content(&content, Some(path), sops_private_key)
            .await
            .map_err(ParseSecretsError::SopsDecryption)?
    } else {
        content
    };

    // Parse .env format from in-memory buffer (no disk writes)
    // Parse line-by-line from the in-memory content string
    // SECURITY: All parsing happens in memory, no temp files
    let mut parsed = ParsedSecrets::default();

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines
        if line.is_empty() {
            continue;
        }

        // Check if line is commented out
        let is_disabled = line.starts_with('#');
        let line_to_parse = if is_disabled {
            // Remove the leading '#' and any whitespace after it
            line.strip_prefix('#').unwrap_or(line).trim()
        } else {
            line
        };

        // Parse KEY=VALUE format (works for both enabled and disabled)
        if let Some((key, value)) = line_to_parse.split_once('=') {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            parsed.secrets.insert(
                key,
                SecretEntry {
                    value,
                    enabled: !is_disabled,
                },
            );
        }
    }

    Ok(parsed)
}

#[allow(dead_code)]
pub(crate) async fn parse_yaml_secrets(
    path: &Path,
    sops_private_key: Option<&str>,
) -> Result<HashMap<String, String>, ParseSecretsError> {
    let parsed = parse_yaml_secrets_with_state(path, sops_private_key).await?;
    // Return only enabled secrets for backward compatibility
    Ok(parsed
        .secrets
        .into_iter()
        .filter_map(|(k, v)| if v.enabled { Some((k, v.value)) } else { None })
        .collect())
}

pub(crate) async fn parse_yaml_secrets_with_state(
    path: &Path,
    sops_private_key: Option<&str>,
) -> Result<ParsedSecrets, ParseSecretsError> {
    let content = tokio::fs::read_to_string(path)
        .await
        .context(format!("Failed to read: {}", path.display()))
        .map_err(ParseSecretsError::Io)?;

    // Check if file is SOPS-encrypted
    // SECURITY: Decrypted content exists only in memory, never written to disk
    let content = if is_sops_encrypted_impl(&content) {
        debug!("Detected SOPS-encrypted file: {}", path.display());
        decrypt_sops_content(&content, Some(path), sops_private_key)
            .await
            .map_err(ParseSecretsError::SopsDecryption)?
    } else {
        content
    };

    // Parse YAML from in-memory buffer (no disk writes)
    let yaml: serde_yaml::Value = serde_yaml::from_str(&content)
        .context("Failed to parse YAML")
        .map_err(ParseSecretsError::Io)?;

    // For YAML, we don't have a direct comment syntax like ENV files.
    // All YAML secrets are considered enabled for now.
    // Future enhancement: Could support a special key prefix like `_disabled_` or use null values
    let mut parsed = ParsedSecrets::default();
    flatten_yaml_value_with_state(&yaml, String::new(), &mut parsed.secrets);

    Ok(parsed)
}

#[allow(dead_code)]
pub(crate) fn flatten_yaml_value(
    value: &serde_yaml::Value,
    prefix: String,
    result: &mut HashMap<String, String>,
) {
    let mut state_result = HashMap::new();
    flatten_yaml_value_with_state(value, prefix, &mut state_result);
    for (k, v) in state_result {
        result.insert(k, v.value);
    }
}

pub(crate) fn flatten_yaml_value_with_state(
    value: &serde_yaml::Value,
    prefix: String,
    result: &mut HashMap<String, SecretEntry>,
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
                flatten_yaml_value_with_state(val, new_prefix, result);
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for (idx, val) in seq.iter().enumerate() {
                let new_prefix = format!("{prefix}[{idx}]");
                flatten_yaml_value_with_state(val, new_prefix, result);
            }
        }
        serde_yaml::Value::String(s) => {
            result.insert(
                prefix,
                SecretEntry {
                    value: s.clone(),
                    enabled: true, // YAML secrets are always enabled (no comment syntax)
                },
            );
        }
        serde_yaml::Value::Number(n) => {
            result.insert(
                prefix,
                SecretEntry {
                    value: n.to_string(),
                    enabled: true,
                },
            );
        }
        serde_yaml::Value::Bool(b) => {
            result.insert(
                prefix,
                SecretEntry {
                    value: b.to_string(),
                    enabled: true,
                },
            );
        }
        serde_yaml::Value::Null => {
            result.insert(
                prefix,
                SecretEntry {
                    value: String::new(),
                    enabled: true,
                },
            );
        }
        serde_yaml::Value::Tagged(_) => {
            // Skip tagged values (SOPS metadata, etc.)
            // These are typically metadata and not actual secret values
        }
    }
}

pub(crate) async fn parse_properties_file(path: &Path) -> Result<HashMap<String, String>> {
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
