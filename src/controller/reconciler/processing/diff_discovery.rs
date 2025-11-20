//! # Diff Discovery
//!
//! Detects if secrets have been tampered with in cloud providers by comparing
//! Git values (source of truth) with cloud provider values.

use crate::provider::SecretManagerProvider;
use anyhow::Result;
use tracing::{debug, warn};

/// Compare Git secret value with cloud provider value and detect differences
/// Returns true if values differ, false if they match or secret doesn't exist
pub async fn detect_secret_diff(
    provider: &dyn SecretManagerProvider,
    secret_name: &str,
    git_value: &str,
) -> Result<bool> {
    match provider.get_secret_value(secret_name).await {
        Ok(Some(cloud_value)) => {
            if cloud_value != git_value {
                warn!(
                    "⚠️  Secret '{}' differs between Git (source of truth) and cloud provider",
                    secret_name
                );
                debug!(
                    "Git value (source of truth): '{}'",
                    mask_secret_value(git_value)
                );
                debug!(
                    "Cloud provider value (may have been tampered with): '{}'",
                    mask_secret_value(&cloud_value)
                );
                return Ok(true);
            }
            debug!(
                "Secret '{}' matches Git value (no tampering detected)",
                secret_name
            );
            Ok(false)
        }
        Ok(None) => {
            // Secret doesn't exist yet - not a diff, just needs to be created
            debug!(
                "Secret '{}' does not exist in cloud provider (will be created)",
                secret_name
            );
            Ok(false)
        }
        Err(e) => {
            // Error fetching secret - log but don't fail diff detection
            warn!(
                "Failed to fetch secret '{}' for diff detection: {}",
                secret_name, e
            );
            // Return false to avoid blocking reconciliation
            Ok(false)
        }
    }
}

/// Mask secret value for logging (show first and last few characters)
fn mask_secret_value(value: &str) -> String {
    if value.len() <= 8 {
        // Very short values - mask completely
        "*".repeat(value.len().min(4))
    } else {
        // Show first 4 and last 4 characters
        let first = &value[..4.min(value.len())];
        let last_start = value.len().saturating_sub(4);
        let last = &value[last_start..];
        format!("{}...{}", first, last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_secret_value_short() {
        // Values <= 8 chars: mask with 4 asterisks
        assert_eq!(mask_secret_value("abc"), "***");
        assert_eq!(mask_secret_value("short"), "****");
        assert_eq!(mask_secret_value("12345678"), "****");
    }

    #[test]
    fn test_mask_secret_value_long() {
        let value = "this-is-a-very-long-secret-value";
        let masked = mask_secret_value(value);
        assert!(masked.starts_with("this"));
        assert!(masked.ends_with("lue"));
        assert!(masked.contains("..."));
    }
}
