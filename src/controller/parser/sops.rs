//! # SOPS Decryption
//!
//! Handles SOPS-encrypted file decryption using the sops binary.

use crate::observability::metrics;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, info_span, warn, Instrument};

/// Check if content is SOPS-encrypted by looking for SOPS metadata
pub(crate) fn is_sops_encrypted(content: &str) -> bool {
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

    false
}

/// Decrypt SOPS-encrypted content using sops binary
///
/// This function uses the sops binary for decryption, which is the current
/// production implementation. The rops crate implementation is deactivated
/// (see `decrypt_with_rops` for details).
pub async fn decrypt_sops_content(content: &str, sops_private_key: Option<&str>) -> Result<String> {
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
        let result = decrypt_with_sops_binary(content, sops_private_key).await;

        match &result {
            Ok(_) => {
                span_clone.record("decryption.method", "sops_binary");
                span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                span_clone.record("operation.success", true);
                metrics::increment_sops_decryption_total();
                metrics::observe_sops_decryption_duration(start.elapsed().as_secs_f64());
            }
            Err(e) => {
                span_clone.record("decryption.method", "sops_binary");
                span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                span_clone.record("operation.success", false);
                span_clone.record("error.message", e.to_string());
                metrics::increment_sops_decryption_errors_total();
            }
        }

        result
    }
    .instrument(span)
    .await
}

/// Decrypt SOPS content using rops crate with GPG private key
///
/// **STATUS: DEACTIVATED** - This implementation is currently deactivated.
/// We use the sops binary instead, which is more reliable and doesn't require
/// keys to be in the system keyring.
///
/// The rops crate API is complex and requires:
/// 1. Parsing SOPS file format (YAML/JSON) with proper type system
/// 2. Handling GPG keys via integration modules
/// 3. Decrypting with proper file format types (YamlFileFormat, JsonFileFormat, etc.)
///
/// For now, we use the sops binary which handles all of this automatically.
/// This stub is kept for future reference if we decide to implement rops support.
#[allow(dead_code, reason = "Kept as stub for future rops implementation")]
fn decrypt_with_rops(_content: &str, _private_key: &str) -> Result<String> {
    // DEACTIVATED: rops crate decryption is not implemented
    // We use sops binary instead (see decrypt_with_sops_binary)
    Err(anyhow::anyhow!(
        "rops crate decryption is deactivated - using sops binary instead"
    ))
}

/// Decrypt SOPS content using sops binary via stdin/stdout
///
/// **SECURITY**: This implementation pipes encrypted content directly to SOPS stdin
/// and captures decrypted output from stdout. This ensures:
/// - No encrypted content written to disk
/// - No decrypted content written to disk (SOPS processes in memory)
/// - Decrypted content only exists in process memory
async fn decrypt_with_sops_binary(content: &str, sops_private_key: Option<&str>) -> Result<String> {
    use std::process::Stdio;

    // Check if sops binary is available
    let sops_path = which::which("sops")
        .context("sops binary not found in PATH. Please install sops: brew install sops (macOS) or see https://github.com/mozilla/sops")?;

    debug!("Using sops binary at: {:?}", sops_path);

    // Set up GPG keyring if private key is provided
    let gpg_home = if let Some(private_key) = sops_private_key {
        info!("Importing GPG private key into temporary keyring for SOPS decryption");
        import_gpg_key(private_key).await?
    } else {
        warn!("No SOPS private key provided - SOPS decryption may fail if key is not in system keyring");
        None
    };

    // Prepare sops command to read from stdin (/dev/stdin)
    // This ensures SOPS never writes decrypted content to disk
    let mut cmd = tokio::process::Command::new(sops_path);
    cmd.arg("-d") // Decrypt
        .arg("/dev/stdin") // Read encrypted content from stdin
        .stdin(Stdio::piped()) // Pipe encrypted content to stdin
        .stdout(Stdio::piped()) // Capture decrypted content from stdout
        .stderr(Stdio::piped());

    // Set GPG home directory if we created a temporary one
    if let Some(ref gpg_home_path) = gpg_home {
        cmd.env("GNUPGHOME", gpg_home_path);
        debug!("Using temporary GPG home: {:?}", gpg_home_path);
    }

    // Spawn the process
    let mut child = cmd.spawn().context("Failed to spawn sops command")?;

    // Write encrypted content to stdin (never touches disk)
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(content.as_bytes())
            .await
            .context("Failed to write encrypted content to sops stdin")?;
        stdin
            .shutdown()
            .await
            .context("Failed to close sops stdin")?;
    }

    // Wait for process to complete and capture output
    let output = child
        .wait_with_output()
        .await
        .context("Failed to wait for sops command")?;

    // Clean up temporary GPG home directory
    if let Some(ref gpg_home_path) = gpg_home {
        let _ = tokio::fs::remove_dir_all(gpg_home_path).await;
    }

    if output.status.success() {
        // SECURITY: Decrypted content exists only in memory (from stdout pipe)
        // Never written to disk - only exists in this String
        let decrypted =
            String::from_utf8(output.stdout).context("sops output is not valid UTF-8")?;
        Ok(decrypted)
    } else {
        // SECURITY: Only log error message, never log decrypted content
        // Truncate error message to avoid potential secret leakage in error output
        let error_msg = String::from_utf8_lossy(&output.stderr);
        let safe_error = if error_msg.len() > 200 {
            format!("{}... (truncated)", &error_msg[..200])
        } else {
            error_msg.to_string()
        };
        Err(anyhow::anyhow!(
            "sops decryption failed: {} (exit code: {})",
            safe_error,
            output.status.code().unwrap_or(-1)
        ))
    }
}

/// Import GPG private key into a temporary GPG home directory
/// Returns the path to the temporary GPG home directory if successful
async fn import_gpg_key(private_key: &str) -> Result<Option<PathBuf>> {
    use std::process::Stdio;

    // Check if gpg binary is available
    let gpg_path = match which::which("gpg") {
        Ok(path) => path,
        Err(_) => {
            warn!(
                "gpg binary not found - SOPS decryption may fail if key is not in system keyring"
            );
            return Ok(None);
        }
    };

    // Create temporary GPG home directory
    let temp_dir = std::env::temp_dir();
    let gpg_home = temp_dir.join(format!("gpg-home-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&gpg_home)
        .await
        .context("Failed to create temporary GPG home directory")?;

    debug!("Created temporary GPG home: {:?}", gpg_home);

    // Import private key into temporary keyring
    // Use --pinentry-mode loopback for non-interactive use in containers
    let mut cmd = tokio::process::Command::new(gpg_path);
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

    // Write private key to stdin
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
        info!(
            "Successfully imported GPG private key into temporary keyring: {:?}",
            gpg_home
        );
        debug!("GPG import output: {}", stdout);
        Ok(Some(gpg_home))
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        warn!("Failed to import GPG private key");
        warn!("GPG stderr: {}", error_msg);
        warn!("GPG stdout: {}", stdout);
        // Clean up on failure
        let _ = tokio::fs::remove_dir_all(&gpg_home).await;
        Err(anyhow::anyhow!(
            "Failed to import GPG private key: {error_msg}"
        ))
    }
}
