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
#[allow(
    clippy::missing_errors_doc,
    reason = "Error documentation is provided in doc comments"
)]
pub async fn parse_secrets(
    app_files: &ApplicationFiles,
    sops_private_key: Option<&str>,
) -> Result<HashMap<String, String>, ParseSecretsError> {
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

pub(crate) async fn parse_env_file(
    path: &Path,
    sops_private_key: Option<&str>,
) -> Result<HashMap<String, String>, ParseSecretsError> {
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

pub(crate) async fn parse_yaml_secrets(
    path: &Path,
    sops_private_key: Option<&str>,
) -> Result<HashMap<String, String>, ParseSecretsError> {
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

    let mut secrets = HashMap::new();
    flatten_yaml_value(&yaml, String::new(), &mut secrets);

    Ok(secrets)
}

pub(crate) fn flatten_yaml_value(
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
