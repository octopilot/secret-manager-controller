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
use prometheus::{Counter, Histogram, HistogramVec, IntCounter, IntCounterVec, IntGauge, Registry};
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

static SOPS_DECRYPTION_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_sops_decryption_total",
        "Total number of SOPS decryption operations (attempts)",
    )
    .expect("Failed to create SOPS_DECRYPTION_TOTAL metric - this should never happen")
});

static SOPS_DECRYPTION_SUCCESS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_sops_decrypt_success_total",
        "Total number of successful SOPS decryption operations",
    )
    .expect("Failed to create SOPS_DECRYPTION_SUCCESS_TOTAL metric - this should never happen")
});

static SOPS_DECRYPTION_DURATION: LazyLock<Histogram> = LazyLock::new(|| {
    Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "secret_manager_sops_decrypt_duration_seconds",
            "Duration of SOPS decryption operations in seconds",
        )
        .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0]),
    )
    .expect("Failed to create SOPS_DECRYPTION_DURATION metric - this should never happen")
});

static SOPS_DECRYPTION_ERRORS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_sops_decryption_errors_total",
        "Total number of SOPS decryption errors",
    )
    .expect("Failed to create SOPS_DECRYPTION_ERRORS_TOTAL metric - this should never happen")
});

static SOPS_DECRYPTION_ERRORS_TOTAL_BY_REASON: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        prometheus::Opts::new(
            "secret_manager_sops_decryption_errors_total_by_reason",
            "Total number of SOPS decryption errors by failure reason",
        ),
        &["reason"],
    )
    .expect(
        "Failed to create SOPS_DECRYPTION_ERRORS_TOTAL_BY_REASON metric - this should never happen",
    )
});

static KUSTOMIZE_BUILD_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_kustomize_build_total",
        "Total number of kustomize build operations",
    )
    .expect("Failed to create KUSTOMIZE_BUILD_TOTAL metric - this should never happen")
});

static KUSTOMIZE_BUILD_DURATION: LazyLock<Histogram> = LazyLock::new(|| {
    Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "secret_manager_kustomize_build_duration_seconds",
            "Duration of kustomize build operations in seconds",
        )
        .buckets(vec![0.5, 1.0, 2.0, 5.0, 10.0, 30.0]),
    )
    .expect("Failed to create KUSTOMIZE_BUILD_DURATION metric - this should never happen")
});

static KUSTOMIZE_BUILD_ERRORS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_kustomize_build_errors_total",
        "Total number of kustomize build errors",
    )
    .expect("Failed to create KUSTOMIZE_BUILD_ERRORS_TOTAL metric - this should never happen")
});

static GIT_CLONE_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_git_clone_total",
        "Total number of git clone operations",
    )
    .expect("Failed to create GIT_CLONE_TOTAL metric - this should never happen")
});

static GIT_CLONE_DURATION: LazyLock<Histogram> = LazyLock::new(|| {
    Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "secret_manager_git_clone_duration_seconds",
            "Duration of git clone operations in seconds",
        )
        .buckets(vec![1.0, 2.0, 5.0, 10.0, 30.0, 60.0]),
    )
    .expect("Failed to create GIT_CLONE_DURATION metric - this should never happen")
});

static GIT_CLONE_ERRORS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_git_clone_errors_total",
        "Total number of git clone errors",
    )
    .expect("Failed to create GIT_CLONE_ERRORS_TOTAL metric - this should never happen")
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

// Artifact download and extraction metrics
static ARTIFACT_DOWNLOADS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_artifact_downloads_total",
        "Total number of artifact downloads (FluxCD/ArgoCD)",
    )
    .expect("Failed to create ARTIFACT_DOWNLOADS_TOTAL metric - this should never happen")
});

static ARTIFACT_DOWNLOAD_DURATION: LazyLock<Histogram> = LazyLock::new(|| {
    Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "secret_manager_artifact_download_duration_seconds",
            "Duration of artifact downloads in seconds",
        )
        .buckets(vec![0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]),
    )
    .expect("Failed to create ARTIFACT_DOWNLOAD_DURATION metric - this should never happen")
});

static ARTIFACT_DOWNLOAD_ERRORS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_artifact_download_errors_total",
        "Total number of artifact download errors",
    )
    .expect("Failed to create ARTIFACT_DOWNLOAD_ERRORS_TOTAL metric - this should never happen")
});

static ARTIFACT_EXTRACTIONS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_artifact_extractions_total",
        "Total number of artifact extractions",
    )
    .expect("Failed to create ARTIFACT_EXTRACTIONS_TOTAL metric - this should never happen")
});

static ARTIFACT_EXTRACTION_DURATION: LazyLock<Histogram> = LazyLock::new(|| {
    Histogram::with_opts(
        prometheus::HistogramOpts::new(
            "secret_manager_artifact_extraction_duration_seconds",
            "Duration of artifact extractions in seconds",
        )
        .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0]),
    )
    .expect("Failed to create ARTIFACT_EXTRACTION_DURATION metric - this should never happen")
});

static ARTIFACT_EXTRACTION_ERRORS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "secret_manager_artifact_extraction_errors_total",
        "Total number of artifact extraction errors",
    )
    .expect("Failed to create ARTIFACT_EXTRACTION_ERRORS_TOTAL metric - this should never happen")
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

// Requeue metrics
static REQUEUES_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        prometheus::Opts::new(
            "secret_manager_requeues_total",
            "Total number of reconciliation requeues",
        ),
        &["reason"],
    )
    .expect("Failed to create REQUEUES_TOTAL metric - this should never happen")
});

#[allow(
    clippy::missing_errors_doc,
    reason = "Error documentation is provided in doc comments"
)]
pub fn register_metrics() -> Result<()> {
    // Prometheus Registry::register() takes ownership (Box<dyn Collector>)
    // Since metrics are stored in LazyLock, we must clone them.
    // Prometheus metrics internally use Arc for their data, so cloning is cheap:
    // - Cloning just copies the wrapper (small struct)
    // - The actual metric data is shared via Arc (reference count increment)
    // - This only happens once at startup, so performance impact is negligible
    REGISTRY.register(Box::new(RECONCILIATIONS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(RECONCILIATION_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(RECONCILIATION_DURATION.clone()))?;
    REGISTRY.register(Box::new(SECRETS_SYNCED_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SECRETS_UPDATED_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SECRETS_MANAGED.clone()))?;
    REGISTRY.register(Box::new(GCP_SECRET_MANAGER_OPERATIONS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(GCP_SECRET_MANAGER_OPERATION_DURATION.clone()))?;
    REGISTRY.register(Box::new(DURATION_PARSING_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SOPS_DECRYPTION_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SOPS_DECRYPTION_SUCCESS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SOPS_DECRYPTION_DURATION.clone()))?;
    REGISTRY.register(Box::new(SOPS_DECRYPTION_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SOPS_DECRYPTION_ERRORS_TOTAL_BY_REASON.clone()))?;
    REGISTRY.register(Box::new(KUSTOMIZE_BUILD_TOTAL.clone()))?;
    REGISTRY.register(Box::new(KUSTOMIZE_BUILD_DURATION.clone()))?;
    REGISTRY.register(Box::new(KUSTOMIZE_BUILD_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(GIT_CLONE_TOTAL.clone()))?;
    REGISTRY.register(Box::new(GIT_CLONE_DURATION.clone()))?;
    REGISTRY.register(Box::new(GIT_CLONE_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(PROVIDER_OPERATIONS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(PROVIDER_OPERATION_DURATION.clone()))?;
    REGISTRY.register(Box::new(PROVIDER_OPERATION_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(ARTIFACT_DOWNLOADS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(ARTIFACT_DOWNLOAD_DURATION.clone()))?;
    REGISTRY.register(Box::new(ARTIFACT_DOWNLOAD_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(ARTIFACT_EXTRACTIONS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(ARTIFACT_EXTRACTION_DURATION.clone()))?;
    REGISTRY.register(Box::new(ARTIFACT_EXTRACTION_ERRORS_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SECRETS_PUBLISHED_TOTAL.clone()))?;
    REGISTRY.register(Box::new(SECRETS_SKIPPED_TOTAL.clone()))?;
    REGISTRY.register(Box::new(REQUEUES_TOTAL.clone()))?;

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
pub fn record_secret_operation(provider: &str, _operation: &str, duration: f64) {
    // Record provider-specific metrics
    PROVIDER_OPERATIONS_TOTAL
        .with_label_values(&[provider])
        .inc();
    PROVIDER_OPERATION_DURATION
        .with_label_values(&[provider])
        .observe(duration);

    // Also maintain backward compatibility with GCP-specific metrics
    // TODO: Consider deprecating GCP-specific metrics in favor of provider-labeled metrics
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

pub fn increment_duration_parsing_errors() {
    DURATION_PARSING_ERRORS_TOTAL.inc();
}

pub fn increment_sops_decryption_total() {
    SOPS_DECRYPTION_TOTAL.inc();
}

pub fn increment_sops_decrypt_success_total() {
    SOPS_DECRYPTION_SUCCESS_TOTAL.inc();
}

pub fn observe_sops_decryption_duration(duration: f64) {
    SOPS_DECRYPTION_DURATION.observe(duration);
}

pub fn increment_sops_decryption_errors_total() {
    SOPS_DECRYPTION_ERRORS_TOTAL.inc();
}

/// Increment SOPS decryption errors counter with reason label
pub fn increment_sops_decryption_errors_total_with_reason(reason: &str) {
    SOPS_DECRYPTION_ERRORS_TOTAL.inc();
    SOPS_DECRYPTION_ERRORS_TOTAL_BY_REASON
        .with_label_values(&[reason])
        .inc();
}

pub fn increment_kustomize_build_total() {
    KUSTOMIZE_BUILD_TOTAL.inc();
}

pub fn observe_kustomize_build_duration(duration: f64) {
    KUSTOMIZE_BUILD_DURATION.observe(duration);
}

pub fn increment_kustomize_build_errors_total() {
    KUSTOMIZE_BUILD_ERRORS_TOTAL.inc();
}

pub fn increment_git_clone_total() {
    GIT_CLONE_TOTAL.inc();
}

pub fn observe_git_clone_duration(duration: f64) {
    GIT_CLONE_DURATION.observe(duration);
}

pub fn increment_git_clone_errors_total() {
    GIT_CLONE_ERRORS_TOTAL.inc();
}

// Artifact download metrics
pub fn increment_artifact_downloads_total() {
    ARTIFACT_DOWNLOADS_TOTAL.inc();
}

pub fn observe_artifact_download_duration(duration: f64) {
    ARTIFACT_DOWNLOAD_DURATION.observe(duration);
}

pub fn increment_artifact_download_errors_total() {
    ARTIFACT_DOWNLOAD_ERRORS_TOTAL.inc();
}

// Artifact extraction metrics
pub fn increment_artifact_extractions_total() {
    ARTIFACT_EXTRACTIONS_TOTAL.inc();
}

pub fn observe_artifact_extraction_duration(duration: f64) {
    ARTIFACT_EXTRACTION_DURATION.observe(duration);
}

pub fn increment_artifact_extraction_errors_total() {
    ARTIFACT_EXTRACTION_ERRORS_TOTAL.inc();
}

// Secret publishing metrics
pub fn increment_secrets_published_total(provider: &str, count: u64) {
    SECRETS_PUBLISHED_TOTAL
        .with_label_values(&[provider])
        .inc_by(count);
}

pub fn increment_secrets_skipped_total(provider: &str, reason: &str) {
    SECRETS_SKIPPED_TOTAL
        .with_label_values(&[provider, reason])
        .inc();
}

// Requeue metrics
pub fn increment_requeues_total(reason: &str) {
    REQUEUES_TOTAL.with_label_values(&[reason]).inc();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_metrics() {
        // This should not panic - metrics should register successfully
        assert!(register_metrics().is_ok());
    }

    #[test]
    fn test_increment_reconciliations() {
        let before = RECONCILIATIONS_TOTAL.get();
        increment_reconciliations();
        let after = RECONCILIATIONS_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_increment_reconciliation_errors() {
        let before = RECONCILIATION_ERRORS_TOTAL.get();
        increment_reconciliation_errors();
        let after = RECONCILIATION_ERRORS_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_observe_reconciliation_duration() {
        observe_reconciliation_duration(1.5);
        // Just verify it doesn't panic - histogram observation doesn't return a value
    }

    #[test]
    fn test_increment_secrets_synced() {
        let before = SECRETS_SYNCED_TOTAL.get();
        increment_secrets_synced(5);
        let after = SECRETS_SYNCED_TOTAL.get();
        assert_eq!(after, before + 5u64);
    }

    #[test]
    fn test_increment_secrets_synced_negative() {
        let before = SECRETS_SYNCED_TOTAL.get();
        increment_secrets_synced(-5); // Should be clamped to 0
        let after = SECRETS_SYNCED_TOTAL.get();
        assert_eq!(after, before); // No change since negative is clamped
    }

    #[test]
    fn test_increment_secrets_updated() {
        let before = SECRETS_UPDATED_TOTAL.get();
        increment_secrets_updated(3);
        let after = SECRETS_UPDATED_TOTAL.get();
        assert_eq!(after, before + 3u64);
    }

    #[test]
    fn test_set_secrets_managed() {
        set_secrets_managed(10);
        assert_eq!(SECRETS_MANAGED.get(), 10);
        set_secrets_managed(20);
        assert_eq!(SECRETS_MANAGED.get(), 20);
    }

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

    #[test]
    fn test_increment_duration_parsing_errors() {
        let before = DURATION_PARSING_ERRORS_TOTAL.get();
        increment_duration_parsing_errors();
        let after = DURATION_PARSING_ERRORS_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_increment_sops_decryption_total() {
        let before = SOPS_DECRYPTION_TOTAL.get();
        increment_sops_decryption_total();
        let after = SOPS_DECRYPTION_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_increment_sops_decrypt_success_total() {
        let before = SOPS_DECRYPTION_SUCCESS_TOTAL.get();
        increment_sops_decrypt_success_total();
        let after = SOPS_DECRYPTION_SUCCESS_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_observe_sops_decryption_duration() {
        observe_sops_decryption_duration(0.2);
        // Just verify it doesn't panic
    }

    #[test]
    fn test_increment_sops_decryption_errors_total() {
        let before = SOPS_DECRYPTION_ERRORS_TOTAL.get();
        increment_sops_decryption_errors_total();
        let after = SOPS_DECRYPTION_ERRORS_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_increment_kustomize_build_total() {
        let before = KUSTOMIZE_BUILD_TOTAL.get();
        increment_kustomize_build_total();
        let after = KUSTOMIZE_BUILD_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_observe_kustomize_build_duration() {
        observe_kustomize_build_duration(1.0);
        // Just verify it doesn't panic
    }

    #[test]
    fn test_increment_kustomize_build_errors_total() {
        let before = KUSTOMIZE_BUILD_ERRORS_TOTAL.get();
        increment_kustomize_build_errors_total();
        let after = KUSTOMIZE_BUILD_ERRORS_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_increment_git_clone_total() {
        let before = GIT_CLONE_TOTAL.get();
        increment_git_clone_total();
        let after = GIT_CLONE_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }

    #[test]
    fn test_observe_git_clone_duration() {
        observe_git_clone_duration(2.5);
        // Just verify it doesn't panic
    }

    #[test]
    fn test_increment_git_clone_errors_total() {
        let before = GIT_CLONE_ERRORS_TOTAL.get();
        increment_git_clone_errors_total();
        let after = GIT_CLONE_ERRORS_TOTAL.get();
        assert_eq!(after, before + 1u64);
    }
}
