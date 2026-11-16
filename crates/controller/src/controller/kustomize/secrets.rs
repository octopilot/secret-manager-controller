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

        // kustomize does not decrypt SOPS files: a secretGenerator fed an encrypted file
        // yields ciphertext, which must never be synced as if it were the secret.
        refuse_sops_ciphertext(&secrets)?;

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

/// Fail when the kustomize output still holds SOPS ciphertext.
///
/// Kustomize mode runs the plain `kustomize build`, which knows nothing of SOPS, so the
/// values of a Secret generated from an encrypted file are the `ENC[...]` strings (and
/// the file's `sops_*` metadata turns up as keys). Syncing those would store ciphertext
/// in the secret manager under the real secret names.
fn refuse_sops_ciphertext(secrets: &HashMap<String, String>) -> Result<()> {
    let mut encrypted: Vec<&str> = secrets
        .iter()
        .filter(|(key, value)| key.starts_with("sops_") || value.starts_with("ENC[AES256_GCM,"))
        .map(|(key, _)| key.as_str())
        .collect();
    if encrypted.is_empty() {
        return Ok(());
    }
    encrypted.sort_unstable();
    Err(anyhow::anyhow!(
        "kustomize output contains SOPS-encrypted values ({}): kustomize mode does not decrypt SOPS files. \
         List the encrypted files in spec.secrets.files instead of using kustomizePath.",
        encrypted.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_values_pass() {
        let secrets = HashMap::from([("API_KEY".to_string(), "abc123".to_string())]);
        assert!(refuse_sops_ciphertext(&secrets).is_ok());
    }

    #[test]
    fn ciphertext_and_sops_metadata_are_refused_by_name_only() {
        let secrets = HashMap::from([
            (
                "API_KEY".to_string(),
                "ENC[AES256_GCM,data:Zm9v,iv:YmFy,tag:YmF6,type:str]".to_string(),
            ),
            ("sops_version".to_string(), "3.12.2".to_string()),
            ("PLAIN".to_string(), "value".to_string()),
        ]);
        let message = refuse_sops_ciphertext(&secrets)
            .expect_err("ciphertext")
            .to_string();
        assert!(message.contains("API_KEY, sops_version"));
        assert!(!message.contains("PLAIN"));
        assert!(!message.contains("Zm9v"), "no value in the message");
    }
}
