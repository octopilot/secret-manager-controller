//! # Utilities
//!
//! Utility functions for secret name construction and path sanitization.

/// Base directory for Secret Manager Controller cache and artifacts
/// Cluster owners can mount a PVC at this path for persistent storage
pub const SMC_BASE_PATH: &str = "/tmp/smc";

/// Sanitize a string for use in filesystem paths
/// Replaces characters that are problematic in filenames with safe alternatives
#[cfg(test)]
pub fn sanitize_path_component(s: &str) -> String {
    sanitize_path_component_impl(s)
}

#[cfg(not(test))]
pub(crate) fn sanitize_path_component(s: &str) -> String {
    sanitize_path_component_impl(s)
}

fn sanitize_path_component_impl(s: &str) -> String {
    s.replace(['@', '/', ':', '\\', ' ', '\t', '\n', '\r'], "-")
        .replace("..", "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect()
}

/// Construct secret name with prefix, key, and suffix
/// Matches kustomize-google-secret-manager naming convention for drop-in replacement
///
/// Format: {prefix}-{key}-{suffix} (if both prefix and suffix exist)
///         {prefix}-{key} (if only prefix exists)
///         {key}-{suffix} (if only suffix exists)
///         {key} (if neither exists)
///
/// Invalid characters (`.`, `/`, etc.) are replaced with `_` to match GCP Secret Manager requirements
#[must_use]
#[allow(
    clippy::doc_markdown,
    reason = "Markdown formatting in doc comments is intentional"
)]
#[cfg(test)]
pub fn construct_secret_name(prefix: Option<&str>, key: &str, suffix: Option<&str>) -> String {
    construct_secret_name_impl(prefix, key, suffix)
}

#[cfg(not(test))]
pub(crate) fn construct_secret_name(
    prefix: Option<&str>,
    key: &str,
    suffix: Option<&str>,
) -> String {
    construct_secret_name_impl(prefix, key, suffix)
}

fn construct_secret_name_impl(prefix: Option<&str>, key: &str, suffix: Option<&str>) -> String {
    let mut parts = Vec::new();

    if let Some(p) = prefix {
        if !p.is_empty() {
            parts.push(p);
        }
    }

    parts.push(key);

    if let Some(s) = suffix {
        if !s.is_empty() {
            // Strip leading dashes from suffix to avoid double dashes when joining
            let suffix_trimmed = s.trim_start_matches('-');
            if !suffix_trimmed.is_empty() {
                parts.push(suffix_trimmed);
            }
        }
    }

    let name = parts.join("-");
    sanitize_secret_name(&name)
}

/// Sanitize secret name to comply with GCP Secret Manager naming requirements
/// Replaces invalid characters (`.`, `/`, etc.) with `_`
/// Matches kustomize-google-secret-manager character sanitization behavior
#[must_use]
#[cfg(test)]
pub fn sanitize_secret_name(name: &str) -> String {
    sanitize_secret_name_impl(name)
}

#[cfg(not(test))]
pub(crate) fn sanitize_secret_name(name: &str) -> String {
    sanitize_secret_name_impl(name)
}

fn sanitize_secret_name_impl(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            // GCP Secret Manager allows: [a-zA-Z0-9_-]+
            // Replace common invalid characters with underscore
            '.' | '/' | ' ' => '_',
            // Keep valid characters
            c if c.is_alphanumeric() || c == '-' || c == '_' => c,
            // Replace any other invalid character with underscore
            _ => '_',
        })
        .collect();

    // Remove consecutive dashes (double dashes, triple dashes, etc.)
    // This handles cases where sanitization creates multiple dashes in a row
    let mut result = String::with_capacity(sanitized.len());
    let mut prev_was_dash = false;

    for c in sanitized.chars() {
        if c == '-' {
            if !prev_was_dash {
                result.push(c);
                prev_was_dash = true;
            }
        } else {
            result.push(c);
            prev_was_dash = false;
        }
    }

    // Remove leading and trailing dashes
    result.trim_matches('-').to_string()
}
