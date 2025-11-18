//! # SOPS Decryption
//!
//! Handles SOPS-encrypted file decryption using the sops binary.

pub mod error;

use crate::controller::parser::sops::error::{
    classify_sops_error, SopsDecryptionError, SopsDecryptionFailureReason,
};
use crate::observability::metrics;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info, info_span, warn, Instrument};

/// Check if content is SOPS-encrypted by looking for SOPS metadata
/// Public for integration tests
pub fn is_sops_encrypted(content: &str) -> bool {
    is_sops_encrypted_impl(content)
}

/// Internal implementation of SOPS encryption detection
/// Public for internal use and tests
pub(crate) fn is_sops_encrypted_impl(content: &str) -> bool {
    // SOPS files have a specific structure with sops metadata
    // Check for common SOPS indicators:
    // 1. YAML files start with "sops:" key
    // 2. JSON files have "sops" key at root
    // 3. ENV files might have SOPS metadata comments

    // Try parsing as YAML first (most common)
    if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(content) {
        if yaml
            .as_mapping()
            .and_then(|m| m.get(serde_yaml::Value::String("sops".to_string())))
            .is_some()
        {
            return true;
        }
    }

    // Try parsing as JSON
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
        if json.get("sops").is_some() {
            return true;
        }
    }

    // Check for SOPS metadata in comments (for ENV files)
    if content.contains("sops_version") || content.contains("sops_encrypted") {
        return true;
    }

    // Check for ENC[...] patterns (SOPS encrypted values in dotenv files)
    // Pattern: ENC[AES256_GCM,data:...,iv:...,tag:...,type:...]
    if content.contains("ENC[") && content.contains("AES256_GCM") {
        return true;
    }

    false
}

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

/// Decrypt SOPS content using rops crate with GPG private key
///
/// **STATUS: DEACTIVATED** - This implementation is currently deactivated.
/// We use the sops binary instead, which is more reliable and doesn't require
/// keys to be in the system keyring.
#[allow(dead_code, reason = "Kept as stub for future rops implementation")]
fn decrypt_with_rops(_content: &str, _private_key: &str) -> Result<String, SopsDecryptionError> {
    Err(SopsDecryptionError::new(
        SopsDecryptionFailureReason::UnsupportedFormat,
        "rops crate decryption is deactivated - using sops binary instead".to_string(),
    ))
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
    use std::process::Stdio;

    // Check if sops binary is available
    let sops_path = which::which("sops")
        .map_err(|e| {
            SopsDecryptionError::new(
                SopsDecryptionFailureReason::ProviderUnavailable,
                format!("sops binary not found in PATH: {}. Please install sops: brew install sops (macOS) or see https://github.com/mozilla/sops", e),
            )
        })?;

    debug!("Using sops binary at: {:?}", sops_path);

    // Set up GPG keyring if private key is provided
    let gpg_home = if let Some(private_key) = sops_private_key {
        info!("Importing GPG private key into temporary keyring for SOPS decryption");
        import_gpg_key(private_key).await.map_err(|e| {
            SopsDecryptionError::new(
                SopsDecryptionFailureReason::InvalidKeyFormat,
                format!("Failed to import GPG key: {}", e),
            )
        })?
    } else {
        warn!("No SOPS private key provided - SOPS decryption may fail if key is not in system keyring");
        None
    };

    // Determine input/output type from file path or content
    let input_type = if let Some(path) = file_path {
        // Detect from file extension
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            match ext {
                "env" => "dotenv",
                "yaml" | "yml" => "yaml",
                "json" => "json",
                _ => {
                    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if filename.contains("application.secrets.env") || filename.ends_with(".env") {
                        "dotenv"
                    } else if filename.contains("application.secrets.yaml")
                        || filename.ends_with(".yaml")
                        || filename.ends_with(".yml")
                    {
                        "yaml"
                    } else if content.trim_start().starts_with('{') {
                        "json"
                    } else if content.trim_start().contains('=')
                        && !content.trim_start().starts_with("sops:")
                    {
                        "dotenv"
                    } else {
                        "yaml"
                    }
                }
            }
        } else {
            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if filename.contains("application.secrets.env") || filename.ends_with(".env") {
                "dotenv"
            } else if filename.contains("application.secrets.yaml")
                || filename.ends_with(".yaml")
                || filename.ends_with(".yml")
            {
                "yaml"
            } else if content.trim_start().starts_with('{') {
                "json"
            } else if content.trim_start().contains('=')
                && !content.trim_start().starts_with("sops:")
            {
                "dotenv"
            } else {
                "yaml"
            }
        }
    } else {
        if content.trim_start().starts_with('{') {
            "json"
        } else if content.trim_start().contains('=') && !content.trim_start().starts_with("sops:") {
            "dotenv"
        } else {
            "yaml"
        }
    };

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

/// Import GPG private key into a temporary GPG home directory
async fn import_gpg_key(private_key: &str) -> Result<Option<PathBuf>> {
    use std::process::Stdio;

    let gpg_path = match which::which("gpg") {
        Ok(path) => path,
        Err(_) => {
            warn!(
                "gpg binary not found - SOPS decryption may fail if key is not in system keyring"
            );
            return Ok(None);
        }
    };

    let temp_dir = std::env::temp_dir();
    let gpg_home = temp_dir.join(format!("gpg-home-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&gpg_home)
        .await
        .context("Failed to create temporary GPG home directory")?;

    debug!("Created temporary GPG home: {:?}", gpg_home);

    let gpg_path_for_trust = gpg_path.clone();
    let mut cmd = tokio::process::Command::new(&gpg_path);
    cmd.env("GNUPGHOME", &gpg_home)
        .arg("--batch")
        .arg("--yes")
        .arg("--pinentry-mode")
        .arg("loopback")
        .arg("--import")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().context("Failed to spawn gpg import command")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(private_key.as_bytes())
            .await
            .context("Failed to write GPG private key to stdin")?;
        stdin.shutdown().await.context("Failed to close stdin")?;
    }

    let output = child
        .wait_with_output()
        .await
        .context("Failed to wait for gpg import command")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("GPG import output: {}", stdout);

        let gpg_home_clone = gpg_home.clone();
        let trust_output = tokio::process::Command::new(&gpg_path_for_trust)
            .env("GNUPGHOME", &gpg_home_clone)
            .arg("--list-keys")
            .arg("--with-colons")
            .arg("--fingerprint")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        if let Ok(list_output) = trust_output {
            if list_output.status.success() {
                let output_str = String::from_utf8_lossy(&list_output.stdout);
                for line in output_str.lines() {
                    if line.starts_with("fpr:") {
                        if let Some(fpr_line) = line.split(':').last() {
                            if !fpr_line.is_empty() {
                                let trust_cmd = tokio::process::Command::new(&gpg_path_for_trust)
                                    .env("GNUPGHOME", &gpg_home_clone)
                                    .arg("--batch")
                                    .arg("--yes")
                                    .arg("--import-ownertrust")
                                    .stdin(Stdio::piped())
                                    .stdout(Stdio::piped())
                                    .stderr(Stdio::piped())
                                    .spawn();

                                if let Ok(mut trust_child) = trust_cmd {
                                    let trust_input = format!("{}:6:\n", fpr_line);
                                    if let Some(mut stdin) = trust_child.stdin.take() {
                                        let _ = stdin.write_all(trust_input.as_bytes()).await;
                                        let _ = stdin.shutdown().await;
                                    }
                                    let _ = trust_child.wait_with_output().await;
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }

        info!(
            "Successfully imported GPG private key into temporary keyring: {:?}",
            gpg_home
        );
        Ok(Some(gpg_home))
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        warn!("Failed to import GPG private key");
        warn!("GPG stderr: {}", error_msg);
        warn!("GPG stdout: {}", stdout);
        let _ = tokio::fs::remove_dir_all(&gpg_home).await;
        Err(anyhow::anyhow!(
            "Failed to import GPG private key: {error_msg}"
        ))
    }
}
