//! # Secret Name Validation
//!
//! Validates secret name components (prefix, suffix) for cloud provider compatibility.

use anyhow::Result;
use regex::Regex;

/// Validate secret name component (prefix or suffix)
/// Must be valid for cloud provider secret names
/// Format: alphanumeric, hyphens, underscores
/// Length: 1-255 characters
pub fn validate_secret_name_component(component: &str, field_name: &str) -> Result<()> {
    let component_trimmed = component.trim();

    if component_trimmed.is_empty() {
        return Err(anyhow::anyhow!("{field_name} cannot be empty"));
    }

    if component_trimmed.len() > 255 {
        return Err(anyhow::anyhow!(
            "{} '{}' exceeds maximum length of 255 characters (got {})",
            field_name,
            component_trimmed,
            component_trimmed.len()
        ));
    }

    // Secret name component: alphanumeric, hyphens, underscores
    let secret_regex = Regex::new(r"^[a-zA-Z0-9_-]+$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if !secret_regex.is_match(component_trimmed) {
        return Err(anyhow::anyhow!(
            "{field_name} '{component_trimmed}' must contain only alphanumeric characters, hyphens, and underscores"
        ));
    }

    Ok(())
}
