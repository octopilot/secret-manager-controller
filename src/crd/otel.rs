//! # OpenTelemetry Configuration
//!
//! OpenTelemetry configuration types for distributed tracing.

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// OpenTelemetry configuration
/// Supports both OTLP exporter and Datadog direct export
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum OtelConfig {
    /// Use OTLP exporter to send traces to an OpenTelemetry Collector
    Otlp {
        /// OTLP endpoint URL (e.g., "http://otel-collector:4317")
        endpoint: String,
        /// Service name for traces (defaults to "secret-manager-controller")
        #[serde(default, rename = "serviceName")]
        service_name: Option<String>,
        /// Service version for traces (defaults to Cargo package version)
        #[serde(default, rename = "serviceVersion")]
        service_version: Option<String>,
        /// Deployment environment (e.g., "dev", "prod")
        #[serde(default)]
        environment: Option<String>,
    },
    /// Use Datadog OpenTelemetry exporter (direct to Datadog)
    Datadog {
        /// Service name for traces (defaults to "secret-manager-controller")
        #[serde(default, rename = "serviceName")]
        service_name: Option<String>,
        /// Service version for traces (defaults to Cargo package version)
        #[serde(default, rename = "serviceVersion")]
        service_version: Option<String>,
        /// Deployment environment (e.g., "dev", "prod")
        #[serde(default)]
        environment: Option<String>,
        /// Datadog site (e.g., "datadoghq.com", "us3.datadoghq.com")
        /// If not specified, uses DD_SITE environment variable or defaults to "datadoghq.com"
        #[serde(default)]
        site: Option<String>,
        /// Datadog API key
        /// If not specified, uses DD_API_KEY environment variable
        #[serde(default, rename = "apiKey")]
        api_key: Option<String>,
    },
}

impl JsonSchema for OtelConfig {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("OtelConfig")
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        // Use a single schema with all properties optional
        // The "type" field acts as a discriminator (handled by serde's tag = "type")
        // This avoids the schema conflict when kube-core tries to merge oneOf schemas
        let schema_value = serde_json::json!({
            "type": "object",
            "description": "OpenTelemetry configuration - supports both OTLP exporter and Datadog direct export",
            "properties": {
                "type": {
                    "type": "string",
                    "enum": ["otlp", "datadog"],
                    "description": "OpenTelemetry exporter type: 'otlp' for OTLP exporter, 'datadog' for Datadog direct export"
                },
                "endpoint": {
                    "type": "string",
                    "description": "OTLP endpoint URL (e.g., \"http://otel-collector:4317\") - required when type is 'otlp'"
                },
                "serviceName": {
                    "type": "string",
                    "description": "Service name for traces (defaults to \"secret-manager-controller\")"
                },
                "serviceVersion": {
                    "type": "string",
                    "description": "Service version for traces (defaults to Cargo package version)"
                },
                "environment": {
                    "type": "string",
                    "description": "Deployment environment (e.g., \"dev\", \"prod\")"
                },
                "site": {
                    "type": "string",
                    "description": "Datadog site (e.g., \"datadoghq.com\", \"us3.datadoghq.com\") - used when type is 'datadog'"
                },
                "apiKey": {
                    "type": "string",
                    "description": "Datadog API key - used when type is 'datadog'"
                }
            },
            "required": ["type"]
        });
        Schema::try_from(schema_value).expect("Failed to create Schema for OtelConfig")
    }
}
