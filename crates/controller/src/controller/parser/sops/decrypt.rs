//! # SOPS Decryption
//!
//! Main decryption logic for SOPS-encrypted files.

use crate::controller::parser::sops::error::{
    classify_sops_error, SopsDecryptionError, SopsDecryptionFailureReason,
};
use crate::controller::parser::sops::gpg::import_gpg_key;
use crate::observability::metrics;
use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info, info_span, warn, Instrument};

/// Decrypt SOPS-encrypted content using sops binary
///
/// Returns a Result that can be classified as transient or permanent failure.
/// The error includes classification for proper retry/backoff handling.
///
/// # File Type Detection
///
/// The function detects the file type from the path extension:
/// - `.env` or `application.secrets.env` → `dotenv`
/// - `.yaml` or `.yml` → `yaml`
/// - `.json` → `json`
/// - Otherwise, attempts content-based detection
///
/// The output type matches the input type to preserve the original format
/// for parsing (env files need dotenv format, yaml files need yaml format).
pub async fn decrypt_sops_content(
    content: &str,
    file_path: Option<&Path>,
    sops_private_key: Option<&str>,
) -> Result<String, SopsDecryptionError> {
    let content_size = content.len();
    let encryption_method = if sops_private_key.is_some() {
        "gpg"
    } else {
        "system_keyring"
    };

    let span = info_span!(
        "sops.decrypt",
        file.size = content_size,
        encryption.method = encryption_method
    );
    let span_clone = span.clone();
    let start = Instant::now();

    async move {
        // Use sops binary (current implementation)
        debug!("Attempting SOPS decryption using sops binary");
        let result = decrypt_with_sops_binary(content, file_path, sops_private_key).await;

        match &result {
            Ok(_) => {
                span_clone.record("decryption.method", "sops_binary");
                span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                span_clone.record("operation.success", true);
                metrics::increment_sops_decryption_total();
                metrics::increment_sops_decrypt_success_total();
                metrics::observe_sops_decryption_duration(start.elapsed().as_secs_f64());
            }
            Err(e) => {
                // Classify the error
                let reason = classify_sops_error(&e.to_string(), None);
                let is_transient = reason.is_transient();

                span_clone.record("decryption.method", "sops_binary");
                span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                span_clone.record("operation.success", false);
                span_clone.record("error.reason", reason.as_str());
                span_clone.record("error.is_transient", is_transient);
                span_clone.record("error.message", e.to_string());

                // Record metrics with reason label
                metrics::increment_sops_decryption_errors_total_with_reason(reason.as_str());

                // Log with appropriate level based on error type
                if is_transient {
                    warn!(
                        "SOPS decryption failed (transient): {} - {}",
                        reason.as_str(),
                        e
                    );
                } else {
                    error!(
                        "SOPS decryption failed (permanent): {} - {}",
                        reason.as_str(),
                        e
                    );
                    error!("Remediation: {}", reason.remediation());
                }
            }
        }

        // Convert anyhow::Error to SopsDecryptionError
        result.map_err(|e| {
            let reason = classify_sops_error(&e.to_string(), None);
            SopsDecryptionError::new(reason, e.to_string())
        })
    }
    .instrument(span)
    .await
}

/// Decrypt SOPS content using sops binary via stdin/stdout pipes
///
/// **SECURITY**: This implementation pipes encrypted content directly to SOPS stdin
/// and captures decrypted output from stdout. This ensures:
/// - No encrypted content written to disk
/// - No decrypted content written to disk (SOPS processes in memory)
/// - Decrypted content only exists in process memory
///
/// **CRITICAL**: Writing secrets to disk (even temporarily) is a security breach
/// that security teams will block. This implementation uses pipes exclusively.
async fn decrypt_with_sops_binary(
    content: &str,
    file_path: Option<&Path>,
    sops_private_key: Option<&str>,
) -> Result<String, SopsDecryptionError> {
    // Check if sops binary is available
    let sops_path = which::which("sops")
        .map_err(|e| {
            SopsDecryptionError::new(
                SopsDecryptionFailureReason::ProviderUnavailable,
                format!("sops binary not found in PATH: {e}. Please install sops: brew install sops (macOS) or see https://github.com/mozilla/sops"),
            )
        })?;

    debug!("Using sops binary at: {:?}", sops_path);

    // Extract required GPG key fingerprints from SOPS metadata before decryption
    let required_fingerprints = extract_sops_pgp_fingerprints(content);
    if let Some(ref fps) = required_fingerprints {
        if !fps.is_empty() {
            info!(
                "🔑 SOPS file requires GPG key fingerprints: {}",
                fps.join(", ")
            );
        }
    }

    // Set up GPG keyring if private key is provided
    let gpg_home = if let Some(private_key) = sops_private_key {
        info!("Importing GPG private key into temporary keyring for SOPS decryption");
        let gpg_result = import_gpg_key(private_key).await.map_err(|e| {
            SopsDecryptionError::new(
                SopsDecryptionFailureReason::InvalidKeyFormat,
                format!("Failed to import GPG key: {}", e),
            )
        })?;

        // Log key fingerprint comparison if we have both
        if let Some(ref required_fps) = required_fingerprints {
            if !required_fps.is_empty() {
                // We can't easily get the imported fingerprint here, but we'll log it in the import function
                debug!(
                    "Will verify imported key matches required fingerprints: {}",
                    required_fps.join(", ")
                );
            }
        }

        gpg_result
    } else {
        warn!("No SOPS private key provided - SOPS decryption may fail if key is not in system keyring");
        None
    };

    // Determine input/output type from file path or content
    let input_type = detect_file_type(content, file_path);

    debug!(
        "Detected SOPS file type: {} (output type matches for parsing)",
        input_type
    );

    // Prepare sops command to read from stdin
    let mut cmd = tokio::process::Command::new(sops_path);
    cmd.arg("-d")
        .arg("--input-type")
        .arg(input_type)
        .arg("--output-type")
        .arg(input_type)
        .arg("/dev/stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(ref gpg_home_path) = gpg_home {
        cmd.env("GNUPGHOME", gpg_home_path);
        cmd.env("GNUPG_TRUST_MODEL", "always");
        debug!("Using temporary GPG home: {:?}", gpg_home_path);
    }

    let mut child = cmd.spawn().map_err(|e| {
        SopsDecryptionError::new(
            SopsDecryptionFailureReason::ProviderUnavailable,
            format!("Failed to spawn sops command: {}", e),
        )
    })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(content.as_bytes()).await.map_err(|e| {
            SopsDecryptionError::new(
                SopsDecryptionFailureReason::Unknown,
                format!("Failed to write encrypted content to sops stdin: {}", e),
            )
        })?;
        stdin.shutdown().await.map_err(|e| {
            SopsDecryptionError::new(
                SopsDecryptionFailureReason::Unknown,
                format!("Failed to close sops stdin: {}", e),
            )
        })?;
    }

    let output = child.wait_with_output().await.map_err(|e| {
        SopsDecryptionError::new(
            SopsDecryptionFailureReason::Unknown,
            format!("Failed to wait for sops command: {}", e),
        )
    })?;

    if let Some(ref gpg_home_path) = gpg_home {
        let _ = tokio::fs::remove_dir_all(gpg_home_path).await;
    }

    if output.status.success() {
        let decrypted = String::from_utf8(output.stdout).map_err(|e| {
            SopsDecryptionError::new(
                SopsDecryptionFailureReason::CorruptedFile,
                format!("sops output is not valid UTF-8: {}", e),
            )
        })?;

        info!(
            "✅ SOPS decryption succeeded - decrypted {} bytes",
            decrypted.len()
        );
        Ok(decrypted)
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        let stdout_msg = String::from_utf8_lossy(&output.stdout);
        let exit_code = output.status.code();

        warn!("SOPS decryption failed with exit code: {:?}", exit_code);
        warn!("SOPS stderr: {}", error_msg);
        if !stdout_msg.trim().is_empty() {
            warn!("SOPS stdout: {}", stdout_msg);
        }

        if let Some(ref required_fps) = required_fingerprints {
            if !required_fps.is_empty() {
                error!(
                    "🔑 SOPS file requires GPG key fingerprints: {}. Verify the imported key matches one of these.",
                    required_fps.join(", ")
                );
            }
        }

        if let Some(ref required_fps) = required_fingerprints {
            if !required_fps.is_empty() {
                error!(
                    "🔑 SOPS file requires GPG key fingerprints: {}. Verify the imported key matches one of these.",
                    required_fps.join(", ")
                );
            }
        }

        if gpg_home.is_some() {
            warn!("GPG keyring was set - verify the key matches the encryption key used in .sops.yaml");
        }

        // Classify the error based on stderr content and exit code
        let reason = classify_sops_error(&error_msg, exit_code);
        let safe_error = if error_msg.len() > 500 {
            format!(
                "{}... (truncated, see logs for full error)",
                &error_msg[..500]
            )
        } else {
            error_msg.to_string()
        };

        Err(SopsDecryptionError::new(
            reason,
            format!(
                "sops decryption failed: {} (exit code: {:?})",
                safe_error, exit_code
            ),
        ))
    }
}

/// Extract GPG key fingerprints from SOPS metadata
/// SOPS files contain metadata with pgp fingerprints of keys used to encrypt
fn extract_sops_pgp_fingerprints(content: &str) -> Option<Vec<String>> {
    // Try parsing as YAML first (most common)
    if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(content) {
        if let Some(sops_meta) = yaml
            .as_mapping()
            .and_then(|m| m.get(serde_yaml::Value::String("sops".to_string())))
        {
            if let Some(pgp) = sops_meta
                .as_mapping()
                .and_then(|m| m.get(serde_yaml::Value::String("pgp".to_string())))
            {
                if let Some(pgp_array) = pgp.as_sequence() {
                    let fingerprints: Vec<String> = pgp_array
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    if !fingerprints.is_empty() {
                        return Some(fingerprints);
                    }
                }
            }
        }
    }

    // Try parsing as JSON
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(sops_meta) = json.get("sops") {
            if let Some(pgp) = sops_meta.get("pgp") {
                if let Some(pgp_array) = pgp.as_array() {
                    let fingerprints: Vec<String> = pgp_array
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    if !fingerprints.is_empty() {
                        return Some(fingerprints);
                    }
                }
            }
        }
    }

    None
}

/// Detect file type from path or content
fn detect_file_type(content: &str, file_path: Option<&Path>) -> &'static str {
    if let Some(path) = file_path {
        // Detect from file extension
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext {
                "env" => return "dotenv",
                "yaml" | "yml" => return "yaml",
                "json" => return "json",
                _ => {}
            }
        }

        // Fallback to filename-based detection
        let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if filename.contains("application.secrets.env") || filename.ends_with(".env") {
            return "dotenv";
        }
        if filename.contains("application.secrets.yaml")
            || filename.ends_with(".yaml")
            || filename.ends_with(".yml")
        {
            return "yaml";
        }
    }

    // Content-based detection
    if content.trim_start().starts_with('{') {
        "json"
    } else if content.trim_start().contains('=') && !content.trim_start().starts_with("sops:") {
        "dotenv"
    } else {
        "yaml"
    }
}
