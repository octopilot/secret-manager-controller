//! # Download and Extraction
//!
//! Shared download and extraction logic for artifacts.
//! Used by both FluxCD and ArgoCD artifact handling.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};
use tracing::{debug, error, info, info_span, warn};

/// Download artifact from URL to temporary file
///
/// Returns the path to the downloaded file and the downloaded size in bytes.
pub async fn download_artifact(artifact_url: &str, temp_file: &Path) -> Result<(PathBuf, u64)> {
    let download_span = info_span!(
        "artifact.download",
        artifact.url = artifact_url,
        artifact.temp_file = temp_file.display().to_string()
    );
    let download_start = Instant::now();
    crate::observability::metrics::increment_artifact_downloads_total();

    info!("Downloading artifact from {}", artifact_url);

    // Create parent directory if needed
    if let Some(parent) = temp_file.parent() {
        tokio::fs::create_dir_all(parent).await.context(format!(
            "Failed to create parent directory: {}",
            parent.display()
        ))?;
    }

    // Download tar.gz file to temporary location
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .context("Failed to create HTTP client")?;

    let response = match client.get(artifact_url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            // Provide detailed error information for debugging network issues
            let error_msg = format!("{:?}", e);
            let error_str = format!("{}", e);

            // Check error type and provide specific guidance
            let is_timeout = error_msg.contains("timeout")
                || error_msg.contains("timed out")
                || error_str.contains("timeout")
                || error_str.contains("timed out");
            let is_dns = error_msg.contains("dns")
                || error_msg.contains("resolve")
                || error_msg.contains("Dns")
                || error_str.contains("dns")
                || error_str.contains("resolve");
            let is_connection = error_msg.contains("connection")
                || error_msg.contains("connect")
                || error_msg.contains("Connection")
                || error_str.contains("connection")
                || error_str.contains("connect");
            let is_builder = error_msg.contains("builder") || error_msg.contains("Builder");

            error!("Failed to download artifact from {}: {}", artifact_url, e);
            error!("Error details: {:?}", e);

            if is_timeout {
                error!("Network timeout detected - source-controller may be unreachable or slow to respond");
                error!("Troubleshooting:");
                error!("  1. Check service: kubectl get svc source-controller -n flux-system");
                error!("  2. Check pods: kubectl get pods -n flux-system -l app=source-controller");
                error!(
                    "  3. Check endpoints: kubectl get endpoints source-controller -n flux-system"
                );
                error!("  4. Test connectivity from controller pod");
            } else if is_dns {
                error!("DNS resolution failed - check if source-controller.flux-system.svc.cluster.local resolves");
                error!("Troubleshooting:");
                error!("  1. Check DNS: kubectl exec -n octopilot-system <pod> -- nslookup source-controller.flux-system.svc.cluster.local");
                error!(
                    "  2. Verify service exists: kubectl get svc source-controller -n flux-system"
                );
            } else if is_connection {
                error!("Connection failed - check network policies and service endpoints");
                error!("Troubleshooting:");
                error!(
                    "  1. Check endpoints: kubectl get endpoints source-controller -n flux-system"
                );
                error!("  2. Check network policies: kubectl get networkpolicies -A");
                error!("  3. Verify service targetPort matches pod containerPort");
            } else if is_builder {
                error!("HTTP client builder error - check reqwest configuration");
            } else {
                error!("Unknown network error - full error: {:?}", e);
                error!("Troubleshooting:");
                error!("  1. Verify source-controller is running: kubectl get pods -n flux-system -l app=source-controller");
                error!("  2. Check service: kubectl get svc source-controller -n flux-system");
                error!("  3. Test from controller pod: kubectl exec -n octopilot-system <pod> -- curl -v <url>");
            }

            crate::observability::metrics::increment_artifact_download_errors_total();
            download_span.record("operation.success", false);
            download_span.record("error.message", format!("{}", e));
            return Err(anyhow::anyhow!(
                "Failed to download artifact from {}: {} (details: {:?})",
                artifact_url,
                e,
                e
            ));
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let status_code = status.as_u16();
        let status_text = response.status().canonical_reason().unwrap_or("Unknown");
        crate::observability::metrics::increment_artifact_download_errors_total();
        download_span.record("operation.success", false);
        download_span.record("error.status_code", status_code as u64);

        // Provide detailed error message with context
        let error_msg = if status_code == 404 {
            format!(
                "Artifact not found (404) at URL: {}\n\
                This may indicate:\n\
                1. GitRepository has not been reconciled yet by FluxCD source-controller\n\
                2. Artifact has not been generated yet (check GitRepository status)\n\
                3. Artifact URL is incorrect\n\
                4. Source-controller is not running or not accessible\n\
                Troubleshooting:\n\
                - Check source-controller: kubectl get pods -n flux-system -l app=source-controller\n\
                - Check GitRepository status: kubectl get gitrepository -A\n\
                - Verify artifact URL is correct in GitRepository status.artifact.url",
                artifact_url
            )
        } else {
            format!(
                "Artifact download failed: HTTP {} {} from URL: {}\n\
                Troubleshooting:\n\
                - Verify source-controller is running: kubectl get pods -n flux-system\n\
                - Check network connectivity from controller pod\n\
                - Verify artifact URL is accessible",
                status_code, status_text, artifact_url
            )
        };

        error!("{}", error_msg);
        return Err(anyhow::anyhow!("{}", error_msg));
    }

    // Verify Content-Length matches actual download size (detect partial downloads)
    let expected_size = response.content_length();
    let mut file = tokio::fs::File::create(temp_file).await.context(format!(
        "Failed to create temp file: {}",
        temp_file.display()
    ))?;

    // Stream download to detect partial downloads and verify size
    let mut downloaded_size: u64 = 0;
    let mut stream = response.bytes_stream();
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.context("Failed to read chunk from download stream")?;
        downloaded_size += chunk.len() as u64;
        file.write_all(&chunk)
            .await
            .context("Failed to write chunk to file")?;
    }

    drop(file); // Close file before verification

    // Verify download size matches Content-Length (if provided)
    if let Some(expected) = expected_size {
        if downloaded_size != expected {
            // Clean up partial download
            let _ = tokio::fs::remove_file(temp_file).await;
            return Err(anyhow::anyhow!(
                "Partial download detected: expected {} bytes, got {} bytes",
                expected,
                downloaded_size
            ));
        }
    }

    // Verify file is not empty
    if downloaded_size == 0 {
        crate::observability::metrics::increment_artifact_download_errors_total();
        download_span.record("operation.success", false);
        download_span.record("error.message", "Downloaded artifact is empty");
        let _ = tokio::fs::remove_file(temp_file).await;
        return Err(anyhow::anyhow!("Downloaded artifact is empty"));
    }

    // Record successful download metrics and span
    let download_duration = download_start.elapsed().as_secs_f64();
    crate::observability::metrics::observe_artifact_download_duration(download_duration);
    download_span.record(
        "operation.duration_ms",
        download_start.elapsed().as_millis() as u64,
    );
    download_span.record("operation.success", true);
    download_span.record("artifact.size_bytes", downloaded_size);

    Ok((temp_file.to_path_buf(), downloaded_size))
}

/// Verify artifact checksum if provided
pub fn verify_checksum(temp_file: &Path, expected_digest: &str) -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    // Read file and compute SHA256
    let mut file = std::fs::File::open(temp_file)
        .context("Failed to open downloaded file for checksum verification")?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 8192];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    let computed_hash = format!("sha256:{:x}", hasher.finalize());

    // Extract hash from digest (format: "sha256:...")
    if expected_digest != computed_hash {
        // Clean up invalid artifact
        let _ = std::fs::remove_file(temp_file);
        return Err(anyhow::anyhow!(
            "Checksum mismatch: expected {}, got {}. Artifact may be corrupt or tampered.",
            expected_digest,
            computed_hash
        ));
    }
    debug!("Checksum verified: {}", expected_digest);
    Ok(())
}

/// Verify file is a valid tar.gz by checking magic bytes
pub fn verify_tar_gz_format(temp_file: &Path) -> Result<()> {
    // tar.gz files start with gzip magic bytes: 1f 8b
    // This prevents processing non-tar.gz files that could cause extraction errors
    let mut magic_buffer = [0u8; 2];
    if let Ok(mut file) = std::fs::File::open(temp_file) {
        use std::io::Read;
        if file.read_exact(&mut magic_buffer).is_ok() {
            if magic_buffer != [0x1f, 0x8b] {
                // Clean up invalid file
                let _ = std::fs::remove_file(temp_file);
                return Err(anyhow::anyhow!(
                    "Invalid file format: expected tar.gz (gzip), got magic bytes {:02x}{:02x}. File may be corrupt or wrong format.",
                    magic_buffer[0],
                    magic_buffer[1]
                ));
            }
            debug!("File format verified: valid gzip magic bytes");
        }
    }
    Ok(())
}

/// Extract tar.gz file to destination directory
pub async fn extract_artifact(temp_file: &Path, destination: &Path) -> Result<()> {
    let extract_span = info_span!(
        "artifact.extract",
        artifact.destination = destination.display().to_string()
    );
    let extract_start = Instant::now();
    crate::observability::metrics::increment_artifact_extractions_total();

    info!("Extracting artifact to {}", destination.display());

    // Create destination directory
    tokio::fs::create_dir_all(destination)
        .await
        .context(format!(
            "Failed to create destination directory: {}",
            destination.display()
        ))?;

    // Use tar command to extract with security flags:
    // - --strip-components=0: Preserve directory structure
    // - --warning=no-unknown-keyword: Suppress warnings for unknown keywords
    // - -C: Extract to specific directory (prevents path traversal)
    // Note: tar automatically prevents extraction outside -C directory on most systems
    let extract_output = tokio::process::Command::new("tar")
        .arg("-xzf")
        .arg(temp_file)
        .arg("-C")
        .arg(destination)
        .arg("--strip-components=0") // Preserve directory structure
        .arg("--warning=no-unknown-keyword") // Suppress warnings
        .output()
        .await
        .context("Failed to execute tar command")?;

    if !extract_output.status.success() {
        let stderr = String::from_utf8_lossy(&extract_output.stderr);
        crate::observability::metrics::increment_artifact_extraction_errors_total();
        extract_span.record("operation.success", false);
        extract_span.record("error.message", stderr.to_string());
        // Clean up on extraction failure
        let _ = tokio::fs::remove_file(temp_file).await;
        // Also clean up partial extraction directory
        let _ = tokio::fs::remove_dir_all(destination).await;
        return Err(anyhow::anyhow!(
            "Failed to extract artifact (corrupt or invalid tar.gz): {}",
            stderr
        ));
    }

    // Verify extraction succeeded by checking if directory contains files
    let mut entries = tokio::fs::read_dir(destination)
        .await
        .context("Failed to read extracted directory")?;
    let has_files = entries.next_entry().await?.is_some();
    if !has_files {
        crate::observability::metrics::increment_artifact_extraction_errors_total();
        extract_span.record("operation.success", false);
        extract_span.record("error.message", "Extraction produced empty directory");
        // Clean up empty extraction
        let _ = tokio::fs::remove_file(temp_file).await;
        let _ = tokio::fs::remove_dir_all(destination).await;
        return Err(anyhow::anyhow!(
            "Artifact extraction produced empty directory - artifact may be corrupt"
        ));
    }

    // Record successful extraction metrics and span
    let extract_duration = extract_start.elapsed().as_secs_f64();
    crate::observability::metrics::observe_artifact_extraction_duration(extract_duration);
    extract_span.record(
        "operation.duration_ms",
        extract_start.elapsed().as_millis() as u64,
    );
    extract_span.record("operation.success", true);

    Ok(())
}

/// Clean up old revisions, keeping only the 3 newest per namespace/name combination
/// Removes the 4th oldest revision and any older ones to prevent unbounded disk growth
pub async fn cleanup_old_revisions(parent_dir: &Path) -> Result<()> {
    // List all revision directories
    let mut entries = Vec::new();
    let mut dir_entries = tokio::fs::read_dir(parent_dir)
        .await
        .context("Failed to read parent directory for cleanup")?;

    while let Some(entry) = dir_entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            // Get modification time to determine age
            let metadata = tokio::fs::metadata(&path).await?;
            let modified = metadata
                .modified()
                .unwrap_or_else(|_| SystemTime::UNIX_EPOCH);

            entries.push((path, modified));
        }
    }

    // If we have 4 or more revisions, remove the oldest ones (keep 3 newest)
    if entries.len() >= 4 {
        // Sort by modification time (newest first)
        entries.sort_by(|a, b| b.1.cmp(&a.1));

        // Remove all but the 3 newest
        let to_remove = entries.split_off(3);

        for (path, _) in to_remove {
            info!("Removing old revision cache: {}", path.display());
            if let Err(e) = tokio::fs::remove_dir_all(&path).await {
                warn!("Failed to remove old revision {}: {}", path.display(), e);
                // Continue removing others even if one fails
            }
        }
    }

    Ok(())
}
