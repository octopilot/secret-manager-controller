//! # Metrics
//!
//! Prometheus metrics for monitoring the controller.
//!
//! ## Metrics Exposed
//!
//! - `secret_manager_reconciliations_total` - Total number of reconciliations
//! - `secret_manager_reconciliation_errors_total` - Total number of reconciliation errors
//! - `secret_manager_reconciliation_duration_seconds` - Duration of reconciliation operations
//! - `secret_manager_secrets_synced_total` - Total number of secrets synced to GCP
//! - `secret_manager_secrets_updated_total` - Total number of secrets updated (overwritten)
//! - `secret_manager_secrets_managed` - Current number of secrets being managed
//! - `secret_manager_gcp_operations_total` - Total number of GCP Secret Manager operations
//! - `secret_manager_gcp_operation_duration_seconds` - Duration of GCP operations

use anyhow::Result;
use prometheus::{Counter, Histogram, IntCounter, IntGauge, Registry};
use std::sync::LazyLock;

// Metrics
pub(crate) static REGISTRY: LazyLock<Registry> = LazyLock::new(Registry::new);

static RECONCILIATIONS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_reconciliations_total",
        "Total number of reconciliations",
    )
    .expect("Failed to create RECONCILIATIONS_TOTAL metric - this should never happen")
});

static RECONCILIATION_ERRORS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_reconciliation_errors_total",
        "Total number of reconciliation errors",
    )
    .expect("Failed to create RECONCILIATION_ERRORS_TOTAL metric - this should never happen")
});

static RECONCILIATION_DURATION: LazyLock<Histogram> = LazyLock::new(|| {
    Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "secret_manager_reconciliation_duration_seconds",
            "Duration of reconciliation in seconds",
        )
        .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0]),
    )
    .expect("Failed to create RECONCILIATION_DURATION metric - this should never happen")
});

static SECRETS_SYNCED_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_secrets_synced_total",
        "Total number of secrets synced to GCP Secret Manager",
    )
    .expect("Failed to create SECRETS_SYNCED_TOTAL metric - this should never happen")
});

static SECRETS_UPDATED_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_secrets_updated_total",
        "Total number of secrets updated (overwritten from git)",
    )
    .expect("Failed to create SECRETS_UPDATED_TOTAL metric - this should never happen")
});

static SECRETS_MANAGED: LazyLock<IntGauge> = LazyLock::new(|| {
    IntGauge::new(
        "secret_manager_secrets_managed",
        "Current number of secrets being managed",
    )
    .expect("Failed to create SECRETS_MANAGED metric - this should never happen")
});

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

static DURATION_PARSING_ERRORS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_duration_parsing_errors_total",
        "Total number of duration parsing errors (reconcileInterval parsing failures)",
    )
    .expect("Failed to create DURATION_PARSING_ERRORS_TOTAL metric - this should never happen")
});

#[allow(
    clippy::missing_errors_doc,
    reason = "Error documentation is provided in doc comments"
)]
pub fn register_metrics() -> Result<()> {
    REGISTRY.register(Box::new(RECONCILIATIONS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(RECONCILIATION_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(RECONCILIATION_DURATION.clone()))?;
    REGISTRY.register(Box::new(SECRETS_SYNCED_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SECRETS_UPDATED_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SECRETS_MANAGED.clone()))?;
    REGISTRY.register(Box::new(GCP_SECRET_MANAGER_OPERATIONS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(GCP_SECRET_MANAGER_OPERATION_DURATION.clone()))?;
    REGISTRY.register(Box::new(DURATION_PARSING_ERRORS_TOTAL.clone()))?;

    Ok(())
}

pub fn increment_reconciliations() {
    RECONCILIATIONS_TOTAL.inc();
}

pub fn increment_reconciliation_errors() {
    RECONCILIATION_ERRORS_TOTAL.inc();
}

pub fn observe_reconciliation_duration(duration: f64) {
    RECONCILIATION_DURATION.observe(duration);
}

pub fn increment_secrets_synced(count: i64) {
    #[allow(clippy::cast_sign_loss, reason = "We ensure non-negative with max(0)")]
    let count_u64 = count.max(0) as u64;
    SECRETS_SYNCED_TOTAL.inc_by(count_u64);
}

pub fn increment_secrets_updated(count: i64) {
    #[allow(clippy::cast_sign_loss, reason = "We ensure non-negative with max(0)")]
    let count_u64 = count.max(0) as u64;
    SECRETS_UPDATED_TOTAL.inc_by(count_u64);
}

pub fn set_secrets_managed(count: i64) {
    SECRETS_MANAGED.set(count);
}

pub fn increment_gcp_operations() {
    GCP_SECRET_MANAGER_OPERATIONS_TOTAL.inc();
}

pub fn observe_gcp_operation_duration(duration: f64) {
    GCP_SECRET_MANAGER_OPERATION_DURATION.observe(duration);
}

// Generic secret operation metrics for multi-provider support
pub fn record_secret_operation(_provider: &str, _operation: &str, duration: f64) {
    // For now, we'll use the GCP metrics as a generic metric
    // In the future, we might want provider-specific metrics
    GCP_SECRET_MANAGER_OPERATIONS_TOTAL.inc();
    GCP_SECRET_MANAGER_OPERATION_DURATION.observe(duration);
}

pub fn increment_duration_parsing_errors() {
    DURATION_PARSING_ERRORS_TOTAL.inc();
}
