//! # Application Files Processing
//!
//! Main orchestration for processing application files (secrets and properties).

use crate::controller::parser;
use crate::controller::parser::sops::is_sops_encrypted_impl;
use crate::controller::reconciler::status::update_decryption_status;
use crate::controller::reconciler::types::Reconciler;
use crate::crd::{ProviderConfig, SecretManagerConfig};
use crate::observability;
use crate::provider::SecretManagerProvider;
use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;
use tracing::{Instrument, debug, error, info_span, warn};

use super::properties::store_properties;
use super::secrets::store_secrets;

/// Process application files (secrets and properties)
/// Parses application.secrets.env, application.secrets.yaml, and application.properties files
/// Stores secrets in cloud provider secret store and properties in config store (if enabled)
#[allow(
    clippy::too_many_lines,
    reason = "Complex file processing logic with SOPS handling and multiple provider paths"
)]
pub async fn process_application_files(
    reconciler: &Arc<Reconciler>,
    provider: &dyn SecretManagerProvider,
    config: &SecretManagerConfig,
    app_files: &parser::ApplicationFiles,
) -> Result<(
    i32,
    std::collections::HashMap<String, crate::crd::ResourceSyncState>,
    std::collections::HashMap<String, crate::crd::ResourceSyncState>,
)> {
    let service_name = config
        .spec
        .secrets
        .prefix
        .as_deref()
        .unwrap_or(&app_files.service_name);
    let span = info_span!("files.process", service.name = service_name);
    let span_clone_for_match = span.clone();
    let start = Instant::now();

    let result = async move {
        let secret_prefix = service_name;

        // Check if any files are SOPS-encrypted to determine if we need to track decryption status
        let has_sops_files = {
            let mut has_sops = false;
            if let Some(ref path) = app_files.secrets_env {
                if let Ok(content) = tokio::fs::read_to_string(path).await {
                    has_sops = is_sops_encrypted_impl(&content);
                }
            }
            if !has_sops {
                if let Some(ref path) = app_files.secrets_yaml {
                    if let Ok(content) = tokio::fs::read_to_string(path).await {
                        has_sops = is_sops_encrypted_impl(&content);
                    }
                }
            }
            has_sops
        };

        // Check SOPS capability and key availability before attempting decryption
        // This separates "system readiness" from "data decryption" concerns
        if has_sops_files {
            // Check global SOPS capability (bootstrap flag)
            if !reconciler.sops_capability_ready.load(std::sync::atomic::Ordering::Relaxed) {
                // SOPS is not configured globally - this is a permanent failure
                let error_msg = format!(
                    "SOPS decryption not available: No GPG key found in controller namespace. \
                     Add 'sops-private-key' secret to enable SOPS decryption."
                );
                error!("{}", error_msg);

                // Update decryption status
                if let Err(e) = update_decryption_status(
                    reconciler,
                    config,
                    "PermanentFailure",
                    Some(&error_msg),
                )
                .await
                {
                    warn!("Failed to update decryption status: {}", e);
                }

                return Err(anyhow::anyhow!("{}", error_msg));
            }

            // Check per-resource key availability (from status field)
            let sops_key_available = config
                .status
                .as_ref()
                .and_then(|s| s.sops_key_available)
                .unwrap_or(false);

            if !sops_key_available {
                // Key not available for this resource - check if we need to update status
                let resource_namespace = config.metadata.namespace.as_deref().unwrap_or("default");

                // If status is None, check once and update
                if config.status.as_ref().and_then(|s| s.sops_key_available).is_none() {
                    let (key_available, secret_name) =
                        crate::controller::reconciler::status::check_sops_key_availability(
                            reconciler,
                            resource_namespace,
                        )
                        .await
                        .unwrap_or((false, None));

                    // Update status with key availability
                    if let Err(e) = crate::controller::reconciler::status::update_sops_key_status(
                        reconciler,
                        config,
                        key_available,
                        secret_name,
                    )
                    .await
                    {
                        warn!("Failed to update SOPS key status: {}", e);
                    }

                    if !key_available {
                        let error_msg = format!(
                            "SOPS decryption not available: No GPG key found in namespace '{}'. \
                             Status shows sops_key_available=false. Add 'sops-private-key' secret to enable SOPS decryption.",
                            resource_namespace
                        );
                        error!("{}", error_msg);

                        // Update decryption status
                        if let Err(e) = update_decryption_status(
                            reconciler,
                            config,
                            "PermanentFailure",
                            Some(&error_msg),
                        )
                        .await
                        {
                            warn!("Failed to update decryption status: {}", e);
                        }

                        return Err(anyhow::anyhow!("{}", error_msg));
                    }
                } else {
                    // Status shows key is not available - permanent failure
                    let error_msg = format!(
                        "SOPS decryption not available: No GPG key found in namespace '{}'. \
                         Status shows sops_key_available=false. Add 'sops-private-key' secret to enable SOPS decryption.",
                        resource_namespace
                    );
                    error!("{}", error_msg);

                    // Update decryption status
                    if let Err(e) = update_decryption_status(
                        reconciler,
                        config,
                        "PermanentFailure",
                        Some(&error_msg),
                    )
                    .await
                    {
                        warn!("Failed to update decryption status: {}", e);
                    }

                    return Err(anyhow::anyhow!("{}", error_msg));
                }
            }
        }

        // Get SOPS private key from Arc<AsyncMutex> for this reconciliation
        // Key is available (either from controller namespace or resource namespace)
        let sops_private_key = {
            let key_guard = reconciler.sops_private_key.lock().await;
            key_guard.clone() // Clone the Option<String> to avoid lifetime issues
        };

        // Parse secrets with enabled/disabled state - handle SOPS decryption errors with proper classification
        let parsed_secrets = match parser::parse_secrets_with_state(app_files, sops_private_key.as_deref()).await {
            Ok(secrets) => {
                // Update decryption status on success (if SOPS files were processed)
                if has_sops_files {
                    if let Err(e) = update_decryption_status(
                        reconciler,
                        config,
                        "Success",
                        None,
                    )
                    .await
                    {
                        warn!("Failed to update decryption status: {}", e);
                    }
                }
                secrets
            }
            Err(parse_err) => {
                // Use proper error type classification instead of string matching
                let is_transient = parse_err.is_transient();
                let error_msg = parse_err.to_string();
                let remediation = parse_err.remediation();

                // Check if this is a SOPS decryption error for detailed status
                if parse_err.as_sops_error().is_some() {
                    // Update decryption status with SOPS-specific information
                    if has_sops_files {
                        let status = if is_transient {
                            "TransientFailure"
                        } else {
                            "PermanentFailure"
                        };
                        if let Err(update_err) = update_decryption_status(
                            reconciler,
                            config,
                            status,
                            Some(&error_msg),
                        )
                        .await
                        {
                            warn!("Failed to update decryption status: {}", update_err);
                        }
                    }

                    if is_transient {
                        warn!("SOPS decryption failed (transient): {}. Will retry.", error_msg);
                        // Return error but mark as transient - reconciler will retry
                        return Err(anyhow::anyhow!("SOPS decryption failed (transient): {}", error_msg));
                    } else {
                        error!("SOPS decryption failed (permanent): {}. Action required.", error_msg);
                        error!("Remediation: {}", remediation);
                        // Return error as permanent - reconciler will mark as Failed
                        return Err(anyhow::anyhow!("SOPS decryption failed (permanent): {}. {}", error_msg, remediation));
                    }
                } else {
                    // Non-SOPS error (file I/O, etc.) - treat as permanent
                    error!("Failed to parse secrets: {}", error_msg);
                    if has_sops_files {
                        if let Err(update_err) = update_decryption_status(
                            reconciler,
                            config,
                            "PermanentFailure",
                            Some(&error_msg),
                        )
                        .await
                        {
                            warn!("Failed to update decryption status: {}", update_err);
                        }
                    }
                    return Err(anyhow::anyhow!("Failed to parse secrets: {}", error_msg));
                }
            }
        };
        let properties = parser::parse_properties(app_files).await?;

        // Debug: Log keys (not values) for debugging
        let enabled_count = parsed_secrets.secrets.values().filter(|e| e.enabled).count();
        let disabled_count = parsed_secrets.secrets.values().filter(|e| !e.enabled).count();
        if !parsed_secrets.secrets.is_empty() {
            let enabled_keys: Vec<&String> = parsed_secrets
                .secrets
                .iter()
                .filter(|(_, e)| e.enabled)
                .map(|(k, _)| k)
                .collect();
            let disabled_keys: Vec<&String> = parsed_secrets
                .secrets
                .iter()
                .filter(|(_, e)| !e.enabled)
                .map(|(k, _)| k)
                .collect();
            debug!(
                "📋 Found {} enabled and {} disabled secret key(s) in application.secrets files",
                enabled_count, disabled_count
            );
            if !enabled_keys.is_empty() {
                debug!("  Enabled: {:?}", enabled_keys);
            }
            if !disabled_keys.is_empty() {
                debug!("  Disabled: {:?}", disabled_keys);
            }
        } else {
            debug!("📋 No secrets found in application.secrets files");
        }

        if !properties.is_empty() {
            let property_keys: Vec<&String> = properties.keys().collect();
            debug!(
                "📋 Found {} property key(s) in application.properties: {:?}",
                property_keys.len(),
                property_keys
            );
        } else {
            debug!("📋 No properties found in application.properties");
        }

        // Store secrets in cloud provider (GitOps: Git is source of truth)
        // Get provider name for metrics
        let provider_name = match &config.spec.provider {
            ProviderConfig::Gcp(_) => "gcp",
            ProviderConfig::Aws(_) => "aws",
            ProviderConfig::Azure(_) => "azure",
        };

        let publish_span = info_span!(
            "secrets.publish",
            provider = provider_name,
            secret.count = parsed_secrets.secrets.len(),
            secret.prefix = secret_prefix
        );
        let publish_start = Instant::now();

        // Store secrets using extracted module
        let (secret_count, _drift_detected, synced_secrets) = store_secrets(
            provider,
            config,
            &parsed_secrets,
            secret_prefix,
            provider_name,
        )
        .await?;
        // Note: drift_detected is returned for future notification support
        // synced_secrets tracks which secrets have been pushed (exists=true) and how many times updated (update_count)

        // Record successful publish metrics and span
        publish_span.record(
            "operation.duration_ms",
            publish_start.elapsed().as_millis() as u64,
        );
        publish_span.record("operation.success", true);
        publish_span.record("secrets.published", secret_count as u64);

        // Store properties using extracted module
        let (property_count, synced_properties) = store_properties(
            reconciler,
            provider,
            config,
            properties,
            secret_prefix,
        )
        .await?;

        let total_count = secret_count + property_count;
        observability::metrics::increment_secrets_synced(i64::from(total_count));
        Ok((total_count, synced_secrets, synced_properties))
    }
    .instrument(span)
    .await;

    match &result {
        Ok((count, _, _)) => {
            span_clone_for_match.record("files.count", *count as u64);
            span_clone_for_match
                .record("operation.duration_ms", start.elapsed().as_millis() as u64);
            span_clone_for_match.record("operation.success", true);
        }
        Err(e) => {
            span_clone_for_match
                .record("operation.duration_ms", start.elapsed().as_millis() as u64);
            span_clone_for_match.record("operation.success", false);
            span_clone_for_match.record("error.message", e.to_string());
        }
    }

    result
}
