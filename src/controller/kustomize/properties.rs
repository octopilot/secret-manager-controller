//! # Kustomize Property Extraction
//!
//! Extracts properties from Kubernetes ConfigMap resources in kustomize output.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

use super::build::run_kustomize_build;
use super::parse::parse_properties_from_yaml;

/// Extract properties from kustomize output (from `ConfigMap` resources)
#[allow(
    clippy::missing_errors_doc,
    reason = "Error documentation is provided in doc comments"
)]
pub fn extract_properties_from_kustomize(
    artifact_path: &Path,
    kustomize_path: &str,
) -> Result<HashMap<String, String>> {
    // Construct full path to kustomization.yaml
    let full_path = artifact_path.join(kustomize_path);

    if !full_path.exists() {
        return Err(anyhow::anyhow!(
            "Kustomize path does not exist: {}",
            full_path.display()
        ));
    }

    info!(
        "Running kustomize build on path: {} (for properties)",
        full_path.display()
    );

    let yaml_output = run_kustomize_build(artifact_path, kustomize_path)?;

    let all_properties = parse_properties_from_yaml(&yaml_output);

    Ok(all_properties)
}
