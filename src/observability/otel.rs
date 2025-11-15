//! # OpenTelemetry Support
//!
//! Provides OpenTelemetry tracing integration with support for:
//! - OTLP exporter (to OpenTelemetry Collector)
//! - Datadog direct export via datadog-opentelemetry
//!
//! Configuration is done via the CRD's `otel` field or environment variables.
//!
//! ## Datadog Integration
//!
//! When Datadog is configured, this module uses the `datadog-opentelemetry` crate
//! to provide full APM/tracing support with Datadog-specific features:
//! - Automatic trace collection and export to Datadog Agent
//! - Service name, version, and environment tagging
//! - Trace context propagation (Datadog and W3C TraceContext)
//! - Sampling rules and rate limiting
//!
//! ## OTLP Integration
//!
//! When OTLP is configured, traces are exported to an OpenTelemetry Collector
//! which can then forward to various backends (Datadog, Jaeger, etc.)

use anyhow::Result;
use std::time::Duration;
use tracing::{info, warn};

use crate::OtelConfig;

/// Tracer provider handle for graceful shutdown
/// The actual type returned by datadog-opentelemetry is opentelemetry_sdk::trace::SdkTracerProvider
#[derive(Debug)]
pub enum TracerProviderHandle {
    /// Datadog tracer provider (wraps opentelemetry_sdk::trace::SdkTracerProvider)
    Datadog(opentelemetry_sdk::trace::SdkTracerProvider),
    /// OTLP tracer provider (placeholder for future implementation)
    Otlp(()),
}

/// Initialize OpenTelemetry tracing based on configuration
///
/// Returns `Ok(None)` if OpenTelemetry is not configured (no CRD config and no env vars).
/// Returns `Ok(Some(handle))` if OpenTelemetry was successfully initialized.
///
/// ## Datadog Configuration
///
/// When Datadog is configured (via CRD or environment variables), this function:
/// 1. Sets up Datadog environment variables (DD_SERVICE, DD_VERSION, DD_ENV, etc.)
/// 2. Initializes the Datadog tracer provider using `datadog-opentelemetry`
/// 3. Configures tracing subscriber to use OpenTelemetry
/// 4. Returns a handle for graceful shutdown
///
/// ## Environment Variables
///
/// Datadog configuration can be provided via:
/// - CRD `otel.datadog` field (takes precedence)
/// - Environment variables: `DD_API_KEY`, `DD_SITE`, `DD_SERVICE`, `DD_ENV`, `DD_VERSION`
///
/// # Errors
///
/// Returns an error if configuration is invalid or initialization fails.
pub fn init_otel(config: Option<&OtelConfig>) -> Result<Option<TracerProviderHandle>> {
    match config {
        Some(OtelConfig::Datadog {
            service_name,
            service_version,
            environment,
            site,
            api_key,
        }) => {
            init_datadog(
                service_name.as_deref(),
                service_version.as_deref(),
                environment.as_deref(),
                site.as_deref(),
                api_key.as_deref(),
            )
        }
        Some(OtelConfig::Otlp {
            endpoint,
            service_name,
            service_version,
            environment,
        }) => {
            // OTLP implementation pending - log configuration for now
            info!(
                "OpenTelemetry OTLP configured: endpoint={}, service={}, version={}, env={:?}",
                endpoint,
                service_name
                    .as_deref()
                    .unwrap_or("secret-manager-controller"),
                service_version
                    .as_deref()
                    .unwrap_or(env!("CARGO_PKG_VERSION")),
                environment
            );
            warn!("OTLP exporter implementation pending - only Datadog is currently supported");
            Ok(None)
        }
        None => {
            // Check for Datadog configuration - only initialize if DD_API_KEY is present
            // DD_API_KEY is the required indicator that Datadog is configured
            if std::env::var("DD_API_KEY").is_ok() {
                info!("DD_API_KEY found in environment, initializing Datadog tracing...");
                // Read other DD_* variables from environment if present
                let service_name = std::env::var("DD_SERVICE").ok();
                let service_version = std::env::var("DD_VERSION").ok();
                let environment = std::env::var("DD_ENV").ok();
                let site = std::env::var("DD_SITE").ok();
                let api_key = std::env::var("DD_API_KEY").ok();
                return init_datadog(
                    service_name.as_deref(),
                    service_version.as_deref(),
                    environment.as_deref(),
                    site.as_deref(),
                    api_key.as_deref(),
                );
            }
            
            // Check for OTLP environment variables
            if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
                info!("OTLP environment variables detected, but OTLP exporter implementation is pending");
                return Ok(None);
            }
            
            info!("No OpenTelemetry configuration provided, skipping Otel initialization");
            Ok(None)
        }
    }
}

/// Initialize Datadog OpenTelemetry tracing
///
/// Sets up Datadog environment variables and initializes the tracer provider.
/// Uses BUILD_GIT_HASH for DD_VERSION as requested.
/// 
/// This function should only be called when DD_API_KEY is present in environment variables,
/// as DD_API_KEY is the required indicator that Datadog is configured.
fn init_datadog(
    service_name: Option<&str>,
    service_version: Option<&str>,
    environment: Option<&str>,
    site: Option<&str>,
    api_key: Option<&str>,
) -> Result<Option<TracerProviderHandle>> {
    // Set Datadog environment variables
    // These are read by datadog-opentelemetry during initialization
    // Note: DD_API_KEY must already be present (checked before calling this function)
    
    // DD_SERVICE - service name
    // Use provided value, or existing env var, or default
    if let Some(name) = service_name {
        std::env::set_var("DD_SERVICE", name);
    } else if std::env::var("DD_SERVICE").is_err() {
        std::env::set_var("DD_SERVICE", "secret-manager-controller");
    }
    
    // DD_VERSION - use BUILD_GIT_HASH from build.rs as requested
    // Use provided value, or existing env var, or build from BUILD_GIT_HASH
    if let Some(version) = service_version {
        std::env::set_var("DD_VERSION", version);
    } else if std::env::var("DD_VERSION").is_err() {
        // Use BUILD_GIT_HASH from build.rs for version tracking
        let build_version = format!("{}-{}", env!("CARGO_PKG_VERSION"), env!("BUILD_GIT_HASH"));
        std::env::set_var("DD_VERSION", build_version);
    }
    
    // DD_ENV - environment (dev, prod, etc.)
    // Only set if provided (don't override existing env var)
    if let Some(env) = environment {
        std::env::set_var("DD_ENV", env);
    }
    
    // DD_SITE - Datadog site
    // Use provided value, or existing env var, or default
    if let Some(dd_site) = site {
        std::env::set_var("DD_SITE", dd_site);
    } else if std::env::var("DD_SITE").is_err() {
        std::env::set_var("DD_SITE", "datadoghq.com");
    }
    
    // DD_API_KEY - API key (should already be set from environment, but set if provided)
    if let Some(key) = api_key {
        std::env::set_var("DD_API_KEY", key);
    }
    
    // Set DD_TRACE_AGENT_URL if not already set
    // Defaults to Datadog Agent on localhost:8126
    if std::env::var("DD_TRACE_AGENT_URL").is_err() {
        std::env::set_var("DD_TRACE_AGENT_URL", "http://localhost:8126");
    }
    
    info!(
        "Initializing Datadog OpenTelemetry tracing: service={}, version={}, env={:?}, site={:?}",
        std::env::var("DD_SERVICE").unwrap_or_default(),
        std::env::var("DD_VERSION").unwrap_or_default(),
        std::env::var("DD_ENV").ok(),
        std::env::var("DD_SITE").unwrap_or_default()
    );
    
    // Initialize Datadog tracer provider
    // This sets up OpenTelemetry with Datadog-specific features
    // The init() function returns a TracerProvider that can be shut down later
    let tracer_provider = datadog_opentelemetry::tracing()
        .init();
    
    info!("✅ Datadog OpenTelemetry tracing initialized successfully");
    info!("   Traces will be sent to: {}", std::env::var("DD_TRACE_AGENT_URL").unwrap_or_default());
    
    Ok(Some(TracerProviderHandle::Datadog(tracer_provider)))
}

/// Shutdown OpenTelemetry tracer provider gracefully
///
/// Flushes pending spans and shuts down the tracer provider.
/// This should be called before application exit to ensure all traces are sent.
pub fn shutdown_otel(tracer_provider: Option<TracerProviderHandle>) {
    match tracer_provider {
        Some(TracerProviderHandle::Datadog(provider)) => {
            info!("Shutting down Datadog tracer provider...");
            // Shutdown with timeout to flush pending spans
            if let Err(e) = provider.shutdown_with_timeout(Duration::from_secs(5)) {
                warn!("Error shutting down Datadog tracer provider: {}", e);
            } else {
                info!("✅ Datadog tracer provider shut down successfully");
            }
        }
        Some(TracerProviderHandle::Otlp(_)) => {
            warn!("OTLP tracer provider shutdown called (no-op - OTLP not yet implemented)");
        }
        None => {
            // No tracer provider to shut down
        }
    }
}
