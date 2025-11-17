//! # Common Provider Utilities
//!
//! Shared utilities and patterns used across all provider implementations.
//!
//! This module reduces code duplication by providing common functionality
//! for secret management operations that are similar across providers.

use crate::observability::metrics;
use anyhow::Result;
use std::time::Instant;
use tracing::debug;

/// Result of a secret comparison operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretComparison {
    /// Secret doesn't exist
    NotFound,
    /// Secret exists but value is unchanged
    Unchanged,
    /// Secret exists and value has changed
    Changed,
}

/// Compare current secret value with new value
///
/// This helper function encapsulates the common pattern of:
/// 1. Getting the current secret value
/// 2. Comparing it with the new value
/// 3. Returning the comparison result
///
/// # Arguments
///
/// * `get_current_value` - Async function that returns the current secret value
/// * `new_value` - The new secret value to compare against
///
/// # Returns
///
/// `SecretComparison` indicating the result of the comparison
pub async fn compare_secret_value<F, Fut>(
    get_current_value: F,
    new_value: &str,
) -> Result<SecretComparison>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<Option<String>>>,
{
    let current_value = get_current_value().await?;

    match current_value {
        None => Ok(SecretComparison::NotFound),
        Some(current) if current == new_value => Ok(SecretComparison::Unchanged),
        Some(_) => Ok(SecretComparison::Changed),
    }
}

/// Record metrics for a secret operation
///
/// This helper function standardizes metric recording across all providers.
///
/// # Arguments
///
/// * `provider` - Provider name (e.g., "gcp", "aws", "azure")
/// * `operation` - Operation type (e.g., "create", "update", "no_change")
/// * `start_time` - Start time of the operation
pub fn record_secret_metrics(provider: &str, operation: &str, start_time: Instant) {
    let duration = start_time.elapsed().as_secs_f64();
    metrics::record_secret_operation(provider, operation, duration);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_compare_secret_value_not_found() {
        let result = compare_secret_value(|| async { Ok(None) }, "new-value")
            .await
            .unwrap();
        assert_eq!(result, SecretComparison::NotFound);
    }

    #[tokio::test]
    async fn test_compare_secret_value_unchanged() {
        let result = compare_secret_value(
            || async { Ok(Some("same-value".to_string())) },
            "same-value",
        )
        .await
        .unwrap();
        assert_eq!(result, SecretComparison::Unchanged);
    }

    #[tokio::test]
    async fn test_compare_secret_value_changed() {
        let result =
            compare_secret_value(|| async { Ok(Some("old-value".to_string())) }, "new-value")
                .await
                .unwrap();
        assert_eq!(result, SecretComparison::Changed);
    }

    #[tokio::test]
    async fn test_compare_secret_value_error_propagation() {
        let result =
            compare_secret_value(|| async { Err(anyhow::anyhow!("Test error")) }, "value").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_secret_comparison_debug() {
        let not_found = SecretComparison::NotFound;
        let debug_str = format!("{:?}", not_found);
        assert!(debug_str.contains("NotFound"));
    }

    #[test]
    fn test_secret_comparison_eq() {
        assert_eq!(SecretComparison::NotFound, SecretComparison::NotFound);
        assert_eq!(SecretComparison::Unchanged, SecretComparison::Unchanged);
        assert_eq!(SecretComparison::Changed, SecretComparison::Changed);
        assert_ne!(SecretComparison::NotFound, SecretComparison::Unchanged);
    }

    #[test]
    fn test_record_secret_metrics() {
        let start = Instant::now();
        // Just verify it doesn't panic
        record_secret_metrics("gcp", "create", start);
    }

    #[test]
    fn test_log_secret_operation() {
        // Just verify it doesn't panic
        log_secret_operation("gcp", "test-secret", SecretComparison::NotFound);
        log_secret_operation("aws", "test-secret", SecretComparison::Unchanged);
        log_secret_operation("azure", "test-secret", SecretComparison::Changed);
    }
}

/// Log secret operation result
///
/// Standardizes logging for secret operations across providers.
///
/// # Arguments
///
/// * `provider` - Provider name
/// * `secret_name` - Name of the secret
/// * `comparison` - Result of secret comparison
pub fn log_secret_operation(provider: &str, secret_name: &str, comparison: SecretComparison) {
    match comparison {
        SecretComparison::NotFound => {
            tracing::info!("Creating new {} secret: {}", provider, secret_name);
        }
        SecretComparison::Unchanged => {
            debug!(
                "{} secret {} unchanged, skipping update",
                provider, secret_name
            );
        }
        SecretComparison::Changed => {
            tracing::info!(
                "Secret value changed, updating {} secret: {}",
                provider,
                secret_name
            );
        }
    }
}

// Common pattern documentation:
//
// Providers can use these utilities to implement create_or_update_secret with consistent behavior:
// 1. Compare current value with new value using `compare_secret_value`
// 2. Return early if unchanged
// 3. Create or update secret (provider-specific)
// 4. Record metrics using `record_secret_metrics`
// 5. Log operations using `log_secret_operation`
//
// Example usage:
// ```rust,ignore
// async fn create_or_update_secret(&self, secret_name: &str, secret_value: &str) -> Result<bool> {
//     let start = Instant::now();
//     let comparison = compare_secret_value(|| self.get_secret_value(secret_name), secret_value).await?;
//     if comparison == SecretComparison::Unchanged {
//         record_secret_metrics("provider", "no_change", start);
//         return Ok(false);
//     }
//     log_secret_operation("provider", secret_name, comparison);
//     // Provider-specific create/update logic here
//     record_secret_metrics("provider", "update", start);
//     Ok(true)
// }
// ```
