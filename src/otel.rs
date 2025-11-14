//! # OpenTelemetry Support
//!
//! Provides OpenTelemetry tracing integration with support for:
//! - OTLP exporter (to OpenTelemetry Collector)
//! - Datadog direct export via OTLP
//!
//! Configuration is done via the CRD's `otel` field or environment variables.
//!
//! Note: This is a simplified implementation. Full Otel support will be added
//! as the opentelemetry-otlp API stabilizes.

use anyhow::{Context, Result};
use tracing::{error, info};

use crate::OtelConfig;

/// Initialize OpenTelemetry tracing based on configuration
/// 
/// Returns `Ok(None)` if OpenTelemetry is not configured (no CRD config and no env vars).
/// This allows users to skip Otel entirely if they don't have an Otel endpoint.
/// 
/// Currently logs the configuration. Full implementation pending API stabilization.
pub fn init_otel(config: Option<&OtelConfig>) -> Result<Option<()>> {
    match config {
        Some(OtelConfig::Otlp { endpoint, service_name, service_version, environment }) => {
            info!(
                "OpenTelemetry OTLP configured: endpoint={}, service={}, version={}, env={:?}",
                endpoint,
                service_name.as_deref().unwrap_or("secret-manager-controller"),
                service_version.as_deref().unwrap_or(env!("CARGO_PKG_VERSION")),
                environment
            );
            info!("Note: Full Otel implementation pending - configuration logged only");
            Ok(Some(()))
        }
        Some(OtelConfig::Datadog { service_name, service_version, environment, site, api_key }) => {
            info!(
                "Datadog OpenTelemetry configured: service={}, version={}, env={:?}, site={:?}",
                service_name.as_deref().unwrap_or("secret-manager-controller"),
                service_version.as_deref().unwrap_or(env!("CARGO_PKG_VERSION")),
                environment,
                site.as_deref().unwrap_or("datadoghq.com")
            );
            if api_key.is_some() {
                info!("Datadog API key provided (hidden in logs)");
            }
            info!("Note: Full Otel implementation pending - configuration logged only");
            Ok(Some(()))
        }
        None => {
            // Check environment variables
            if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() 
                || std::env::var("DD_API_KEY").is_ok() 
                || std::env::var("DD_SITE").is_ok() {
                info!("OpenTelemetry environment variables detected");
                info!("Note: Full Otel implementation pending - environment variables logged only");
                return Ok(Some(()));
            }
            info!("No OpenTelemetry configuration provided, skipping Otel initialization");
            Ok(None)
        }
    }
}

/// Shutdown OpenTelemetry tracer provider
pub fn shutdown_otel(_tracer_provider: Option<()>) {
    info!("OpenTelemetry shutdown called (no-op in current implementation)");
}
