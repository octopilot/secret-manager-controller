//! # Kustomize Secret Extraction
//!
//! Extracts secrets from Kubernetes Secret resources in kustomize output.

use crate::observability::metrics;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use tracing::{debug, info, info_span};

use super::build::run_kustomize_build;
use super::parse::parse_secrets_from_yaml;

/// Run kustomize build on the specified path and extract secrets from Secret resources
#[allow(
    clippy::missing_errors_doc,
    reason = "Error documentation is provided in doc comments"
)]
pub fn extract_secrets_from_kustomize(
    artifact_path: &Path,
    kustomize_path: &str,
) -> Result<HashMap<String, String>> {
    let span = info_span!("kustomize.build", kustomize.path = kustomize_path);
    let span_clone = span.clone();
    let start = Instant::now();

    let result = (|| -> Result<HashMap<String, String>> {
        let yaml_output = run_kustomize_build(artifact_path, kustomize_path)?;

        debug!("Kustomize build succeeded, parsing output...");

        // Parse YAML stream (multiple resources separated by ---)
        let secrets = parse_secrets_from_yaml(&yaml_output);

        span_clone.record("secrets.count", secrets.len() as u64);
        span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
        span_clone.record("operation.success", true);
        metrics::increment_kustomize_build_total();
        metrics::observe_kustomize_build_duration(start.elapsed().as_secs_f64());

        info!("Extracted {} secrets from kustomize output", secrets.len());
        Ok(secrets)
    })();

    // Record span attributes even on error
    if let Err(ref e) = result {
        span_clone.record("operation.success", false);
        span_clone.record("error.message", e.to_string());
        metrics::increment_kustomize_build_errors_total();
    }

    result
}
