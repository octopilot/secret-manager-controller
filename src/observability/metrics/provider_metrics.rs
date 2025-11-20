//! # Provider Metrics
//!
//! Metrics for provider operations: GCP, generic provider operations, and secret publishing.

use crate::observability::metrics::registry::REGISTRY;
use anyhow::Result;
use prometheus::{Counter, Histogram, HistogramVec, IntCounterVec};
use std::sync::LazyLock;

// GCP-specific metrics (maintained for backward compatibility)
static GCP_SECRET_MANAGER_OPERATIONS_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    Counter::new(
        "secret_manager_gcp_operations_total",
        "Total number of GCP Secret Manager operations",
    )
    .expect(
        "Failed to create GCP_SECRET_MANAGER_OPERATIONS_TOTAL metric - this should never happen",
    )
});

static GCP_SECRET_MANAGER_OPERATION_DURATION: LazyLock<Histogram> = LazyLock::new(|| {
    Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "secret_manager_gcp_operation_duration_seconds",
            "Duration of GCP Secret Manager operations in seconds",
        )
        .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0]),
    )
    .expect(
        "Failed to create GCP_SECRET_MANAGER_OPERATION_DURATION metric - this should never happen",
    )
});

// Provider-specific metrics with provider label
static PROVIDER_OPERATIONS_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        prometheus::Opts::new(
            "secret_manager_provider_operations_total",
            "Total number of provider operations by provider type",
        ),
        &["provider"],
    )
    .expect("Failed to create PROVIDER_OPERATIONS_TOTAL metric - this should never happen")
});

static PROVIDER_OPERATION_DURATION: LazyLock<HistogramVec> = LazyLock::new(|| {
    HistogramVec::new(
        prometheus::HistogramOpts::new(
            "secret_manager_provider_operation_duration_seconds",
            "Duration of provider operations in seconds by provider type",
        )
        .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0]),
        &["provider"],
    )
    .expect("Failed to create PROVIDER_OPERATION_DURATION metric - this should never happen")
});

static PROVIDER_OPERATION_ERRORS_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        prometheus::Opts::new(
            "secret_manager_provider_operation_errors_total",
            "Total number of provider operation errors by provider type",
        ),
        &["provider"],
    )
    .expect("Failed to create PROVIDER_OPERATION_ERRORS_TOTAL metric - this should never happen")
});

// Secret publishing metrics
static SECRETS_PUBLISHED_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        prometheus::Opts::new(
            "secret_manager_secrets_published_total",
            "Total number of secrets published to providers",
        ),
        &["provider"],
    )
    .expect("Failed to create SECRETS_PUBLISHED_TOTAL metric - this should never happen")
});

static SECRETS_SKIPPED_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        prometheus::Opts::new(
            "secret_manager_secrets_skipped_total",
            "Total number of secrets skipped (no changes or errors)",
        ),
        &["provider", "reason"],
    )
    .expect("Failed to create SECRETS_SKIPPED_TOTAL metric - this should never happen")
});

static SECRETS_DIFF_DETECTED_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        prometheus::Opts::new(
            "secret_manager_secrets_diff_detected_total",
            "Total number of secrets where differences were detected between Git and cloud provider",
        ),
        &["provider"],
    )
    .expect("Failed to create SECRETS_DIFF_DETECTED_TOTAL metric - this should never happen")
});

/// Register provider metrics with the registry
pub(crate) fn register_provider_metrics() -> Result<()> {
    REGISTRY.register(Box::new(GCP_SECRET_MANAGER_OPERATIONS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(GCP_SECRET_MANAGER_OPERATION_DURATION.clone()))?;
    REGISTRY.register(Box::new(PROVIDER_OPERATIONS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(PROVIDER_OPERATION_DURATION.clone()))?;
    REGISTRY.register(Box::new(PROVIDER_OPERATION_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SECRETS_PUBLISHED_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SECRETS_SKIPPED_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SECRETS_DIFF_DETECTED_TOTAL.clone()))?;
    Ok(())
}

// Public functions for provider metrics

pub fn increment_gcp_operations() {
    GCP_SECRET_MANAGER_OPERATIONS_TOTAL.inc();
}

pub fn observe_gcp_operation_duration(duration: f64) {
    GCP_SECRET_MANAGER_OPERATION_DURATION.observe(duration);
}

// Generic secret operation metrics for multi-provider support
pub fn record_secret_operation(provider: &str, _operation: &str, duration: f64) {
    // Record provider-specific metrics
    PROVIDER_OPERATIONS_TOTAL
        .with_label_values(&[provider])
        .inc();
    PROVIDER_OPERATION_DURATION
        .with_label_values(&[provider])
        .observe(duration);

    // Also maintain backward compatibility with GCP-specific metrics
    // Note: GCP-specific metrics are kept for backward compatibility.
    // Future versions may deprecate these in favor of provider-labeled metrics.
    if provider == "gcp" {
        GCP_SECRET_MANAGER_OPERATIONS_TOTAL.inc();
        GCP_SECRET_MANAGER_OPERATION_DURATION.observe(duration);
    }
}

/// Increment provider operation errors counter
pub fn increment_provider_operation_errors(provider: &str) {
    PROVIDER_OPERATION_ERRORS_TOTAL
        .with_label_values(&[provider])
        .inc();
}

pub fn increment_secrets_published_total(provider: &str, count: u64) {
    SECRETS_PUBLISHED_TOTAL
        .with_label_values(&[provider])
        .inc_by(count);
}

pub fn increment_secrets_diff_detected_total(provider: &str) {
    SECRETS_DIFF_DETECTED_TOTAL
        .with_label_values(&[provider])
        .inc();
}

pub fn increment_secrets_skipped_total(provider: &str, reason: &str) {
    SECRETS_SKIPPED_TOTAL
        .with_label_values(&[provider, reason])
        .inc();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_increment_gcp_operations() {
        let before = GCP_SECRET_MANAGER_OPERATIONS_TOTAL.get();
        increment_gcp_operations();
        let after = GCP_SECRET_MANAGER_OPERATIONS_TOTAL.get();
        assert_eq!(after, before + 1.0);
    }

    #[test]
    fn test_observe_gcp_operation_duration() {
        observe_gcp_operation_duration(0.5);
        // Just verify it doesn't panic
    }

    #[test]
    fn test_record_secret_operation_gcp() {
        let before_ops = GCP_SECRET_MANAGER_OPERATIONS_TOTAL.get();
        let before_provider = PROVIDER_OPERATIONS_TOTAL.with_label_values(&["gcp"]).get();
        record_secret_operation("gcp", "create", 0.3);
        let after_ops = GCP_SECRET_MANAGER_OPERATIONS_TOTAL.get();
        let after_provider = PROVIDER_OPERATIONS_TOTAL.with_label_values(&["gcp"]).get();
        assert_eq!(after_ops, before_ops + 1.0);
        assert_eq!(after_provider, before_provider + 1u64);
    }

    #[test]
    fn test_record_secret_operation_aws() {
        let before_provider = PROVIDER_OPERATIONS_TOTAL.with_label_values(&["aws"]).get();
        record_secret_operation("aws", "create", 0.3);
        let after_provider = PROVIDER_OPERATIONS_TOTAL.with_label_values(&["aws"]).get();
        assert_eq!(after_provider, before_provider + 1u64);
    }

    #[test]
    fn test_increment_provider_operation_errors() {
        let before = PROVIDER_OPERATION_ERRORS_TOTAL
            .with_label_values(&["gcp"])
            .get();
        increment_provider_operation_errors("gcp");
        let after = PROVIDER_OPERATION_ERRORS_TOTAL
            .with_label_values(&["gcp"])
            .get();
        assert_eq!(after, before + 1u64);
    }
}
