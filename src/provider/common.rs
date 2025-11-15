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
    metrics::record_secret_operation(provider, operation, start_time.elapsed().as_secs_f64());
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
