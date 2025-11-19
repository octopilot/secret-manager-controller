//! # Kubernetes Validation
//!
//! Validates Kubernetes resource names, namespaces, and labels per RFC 1123.

use anyhow::Result;
use regex::Regex;

/// Validate sourceRef.kind
/// Must be "GitRepository" or "Application" (case-sensitive)
pub fn validate_source_ref_kind(kind: &str) -> Result<()> {
    let kind_trimmed = kind.trim();
    match kind_trimmed {
        "GitRepository" | "Application" => Ok(()),
        _ => Err(anyhow::anyhow!(
            "Must be 'GitRepository' or 'Application' (case-sensitive), got '{kind_trimmed}'"
        )),
    }
}

/// Validate Kubernetes resource name (RFC 1123 subdomain)
/// Format: lowercase alphanumeric, hyphens, dots
/// Length: 1-253 characters
/// Cannot start or end with hyphen or dot
pub fn validate_kubernetes_name(name: &str, field_name: &str) -> Result<()> {
    let name_trimmed = name.trim();

    if name_trimmed.is_empty() {
        return Err(anyhow::anyhow!("{field_name} cannot be empty"));
    }

    if name_trimmed.len() > 253 {
        return Err(anyhow::anyhow!(
            "{} '{}' exceeds maximum length of 253 characters (got {})",
            field_name,
            name_trimmed,
            name_trimmed.len()
        ));
    }

    // RFC 1123 subdomain: [a-z0-9]([-a-z0-9]*[a-z0-9])?(\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*
    // Simplified: lowercase alphanumeric, hyphens, dots; cannot start/end with hyphen or dot
    let name_regex =
        Regex::new(r"^[a-z0-9]([-a-z0-9]*[a-z0-9])?(\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$")
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if !name_regex.is_match(name_trimmed) {
        return Err(anyhow::anyhow!(
            "{field_name} '{name_trimmed}' must be a valid Kubernetes name (lowercase alphanumeric, hyphens, dots; cannot start/end with hyphen or dot)"
        ));
    }

    Ok(())
}

/// Validate Kubernetes namespace (RFC 1123 label)
/// Format: lowercase alphanumeric, hyphens
/// Length: 1-63 characters
/// Cannot start or end with hyphen
pub fn validate_kubernetes_namespace(namespace: &str) -> Result<()> {
    let namespace_trimmed = namespace.trim();

    if namespace_trimmed.is_empty() {
        return Err(anyhow::anyhow!("sourceRef.namespace cannot be empty"));
    }

    if namespace_trimmed.len() > 63 {
        return Err(anyhow::anyhow!(
            "sourceRef.namespace '{}' exceeds maximum length of 63 characters (got {})",
            namespace_trimmed,
            namespace_trimmed.len()
        ));
    }

    // RFC 1123 label: [a-z0-9]([-a-z0-9]*[a-z0-9])?
    let namespace_regex = Regex::new(r"^[a-z0-9]([-a-z0-9]*[a-z0-9])?$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if !namespace_regex.is_match(namespace_trimmed) {
        return Err(anyhow::anyhow!(
            "sourceRef.namespace '{namespace_trimmed}' must be a valid Kubernetes namespace (lowercase alphanumeric, hyphens; cannot start/end with hyphen)"
        ));
    }

    Ok(())
}

/// Validate Kubernetes label value
/// Format: lowercase alphanumeric, hyphens, dots, underscores
/// Length: 1-63 characters
/// Cannot start or end with dot
pub fn validate_kubernetes_label(label: &str, field_name: &str) -> Result<()> {
    let label_trimmed = label.trim();

    if label_trimmed.is_empty() {
        return Err(anyhow::anyhow!("{field_name} cannot be empty"));
    }

    if label_trimmed.len() > 63 {
        return Err(anyhow::anyhow!(
            "{} '{}' exceeds maximum length of 63 characters (got {})",
            field_name,
            label_trimmed,
            label_trimmed.len()
        ));
    }

    // Kubernetes label: [a-z0-9]([-a-z0-9_.]*[a-z0-9])?
    let label_regex = Regex::new(r"^[a-z0-9]([-a-z0-9_.]*[a-z0-9])?$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if !label_regex.is_match(label_trimmed) {
        return Err(anyhow::anyhow!(
            "{field_name} '{label_trimmed}' must be a valid Kubernetes label (lowercase alphanumeric, hyphens, dots, underscores; cannot start/end with dot)"
        ));
    }

    Ok(())
}
