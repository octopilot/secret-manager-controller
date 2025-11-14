//! # Observability
//!
//! Observability modules for metrics and tracing.
//!
//! - `metrics`: Prometheus metrics collection
//! - `otel`: OpenTelemetry tracing integration

pub mod metrics;
pub mod otel;

// Re-export for convenience
pub use metrics::*;
pub use otel::*;
