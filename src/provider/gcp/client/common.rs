//! Common utilities and helpers for GCP Secret Manager clients
//!
//! This module contains shared logic between REST and gRPC implementations
//! to reduce code duplication and maintain consistency.

use crate::observability::metrics;
use std::time::{Duration, Instant};
use tracing::Span;

/// Determines the operation type based on existing secret state
///
/// Returns:
/// - `Some("no_change")` if the secret exists and value hasn't changed
/// - `Some("create")` if the secret doesn't exist
/// - `Some("update")` if the secret exists but value has changed
/// - `None` if there's an error (should not happen in normal flow)
pub fn determine_operation_type(
    existing_secret: &Option<String>,
    new_value: &str,
) -> Option<&'static str> {
    match existing_secret {
        None => Some("create"),
        Some(existing_value) => {
            if existing_value == new_value {
                Some("no_change")
            } else {
                Some("update")
            }
        }
    }
}

/// Records operation metrics and span attributes for a successful operation
pub fn record_success_metrics(span: &Span, operation_type: &str, duration: Duration) {
    span.record("operation.type", operation_type);
    span.record("operation.duration_ms", duration.as_millis() as u64);
    span.record("operation.success", true);
    metrics::record_secret_operation("gcp", operation_type, duration.as_secs_f64());
}

/// Records operation metrics and span attributes for a no-change operation
pub fn record_no_change_metrics(span: &Span, duration: Duration) {
    span.record("operation.type", "no_change");
    span.record("operation.duration_ms", duration.as_millis() as u64);
    span.record("operation.success", true);
    metrics::record_secret_operation("gcp", "no_change", duration.as_secs_f64());
}

/// Records operation metrics and span attributes for a failed operation
pub fn record_error_metrics(
    span: &Span,
    operation_type: Option<&str>,
    error_message: &str,
    duration: Duration,
) {
    span.record("operation.success", false);
    if let Some(op_type) = operation_type {
        span.record("operation.type", op_type);
    }
    span.record("error.message", error_message);
    span.record("operation.duration_ms", duration.as_millis() as u64);
    metrics::increment_provider_operation_errors("gcp");
}

/// Formats a GCP secret resource path
pub fn format_secret_path(project_id: &str, secret_name: &str) -> String {
    format!("projects/{}/secrets/{}", project_id, secret_name)
}

/// Formats a GCP secret version path
pub fn format_secret_version_path(project_id: &str, secret_name: &str) -> String {
    format!(
        "projects/{}/secrets/{}/versions/latest",
        project_id, secret_name
    )
}

/// Helper struct for tracking operation state
pub struct OperationTracker {
    start: Instant,
    span: Span,
}

impl OperationTracker {
    /// Create a new operation tracker
    pub fn new(span: Span) -> Self {
        Self {
            start: Instant::now(),
            span,
        }
    }

    /// Record success metrics
    pub fn record_success(&self, operation_type: &str) {
        record_success_metrics(&self.span, operation_type, self.start.elapsed());
    }

    /// Record no-change metrics
    pub fn record_no_change(&self) {
        record_no_change_metrics(&self.span, self.start.elapsed());
    }

    /// Record error metrics
    pub fn record_error(&self, operation_type: Option<&str>, error_message: &str) {
        record_error_metrics(
            &self.span,
            operation_type,
            error_message,
            self.start.elapsed(),
        );
    }

    /// Get elapsed duration
    ///
    /// Reserved for future use if we need to access elapsed time directly.
    #[allow(dead_code)]
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Get a reference to the span
    ///
    /// Reserved for future use if we need to access the span directly.
    #[allow(dead_code)]
    pub fn span(&self) -> &Span {
        &self.span
    }
}
