//! # Path Validation
//!
//! Validates file paths and AWS Parameter Store paths.

use anyhow::Result;
use regex::Regex;

/// Validate file path
/// Must be a valid relative or absolute path
/// Cannot contain null bytes or invalid path characters
pub fn validate_path(path: &str, field_name: &str) -> Result<()> {
    let path_trimmed = path.trim();

    if path_trimmed.is_empty() {
        return Err(anyhow::anyhow!("{field_name} cannot be empty"));
    }

    // Check for null bytes
    if path_trimmed.contains('\0') {
        return Err(anyhow::anyhow!(
            "{field_name} '{path_trimmed}' cannot contain null bytes"
        ));
    }

    // Basic path validation: no control characters, reasonable length
    if path_trimmed.len() > 4096 {
        return Err(anyhow::anyhow!(
            "{} '{}' exceeds maximum length of 4096 characters (got {})",
            field_name,
            path_trimmed,
            path_trimmed.len()
        ));
    }

    // Check for invalid path patterns (Windows drive letters, etc.)
    // Allow relative paths (starting with .), absolute paths, and normal paths
    // Exclude: < > : " | ? * and control characters (\x00-\x1f)
    // Use a simpler validation: just check for null bytes and control characters
    // Paths can contain most characters except control chars
    for ch in path_trimmed.chars() {
        if ch.is_control() {
            return Err(anyhow::anyhow!(
                "{field_name} '{path_trimmed}' contains control characters"
            ));
        }
    }

    Ok(())
}

/// Validate AWS Parameter Store path
/// Format: /path/to/parameter (must start with /)
pub fn validate_aws_parameter_path(path: &str, field_name: &str) -> Result<()> {
    let path_trimmed = path.trim();

    if path_trimmed.is_empty() {
        return Err(anyhow::anyhow!("{field_name} cannot be empty"));
    }

    if !path_trimmed.starts_with('/') {
        return Err(anyhow::anyhow!(
            "{field_name} '{path_trimmed}' must start with '/' (e.g., '/my-service/dev')"
        ));
    }

    // AWS Parameter Store path: /[a-zA-Z0-9._-]+(/[a-zA-Z0-9._-]+)*
    let param_path_regex = Regex::new(r"^/[a-zA-Z0-9._-]+(/[a-zA-Z0-9._-]+)*$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if !param_path_regex.is_match(path_trimmed) {
        return Err(anyhow::anyhow!(
            "{field_name} '{path_trimmed}' must be a valid AWS Parameter Store path (e.g., '/my-service/dev')"
        ));
    }

    Ok(())
}

/// Validate URL format
pub fn validate_url(url: &str, field_name: &str) -> Result<()> {
    let url_trimmed = url.trim();

    if url_trimmed.is_empty() {
        return Err(anyhow::anyhow!("{field_name} cannot be empty"));
    }

    // Basic URL validation: must start with http:// or https://
    let url_regex = Regex::new(r"^https?://[^\s/$.?#].[^\s]*$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if !url_regex.is_match(url_trimmed) {
        return Err(anyhow::anyhow!(
            "{field_name} '{url_trimmed}' must be a valid URL starting with http:// or https://"
        ));
    }

    Ok(())
}
