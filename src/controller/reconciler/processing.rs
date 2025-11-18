//! # Secret Processing
//!
//! Handles parsing application files and processing Kustomize builds to extract secrets and properties.

use crate::controller::parser;
use crate::controller::parser::sops::is_sops_encrypted_impl;
use crate::controller::reconciler::status::update_decryption_status;
use crate::controller::reconciler::types::Reconciler;
use crate::controller::reconciler::utils::construct_secret_name;
use crate::observability;
use crate::provider::aws::AwsParameterStore;
use crate::provider::azure::AzureAppConfiguration;
use crate::provider::{ConfigStoreProvider, SecretManagerProvider};
use crate::{ProviderConfig, SecretManagerConfig};
use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, info_span, warn, Instrument};

/// Process application files (secrets and properties)
/// Parses application.secrets.env, application.secrets.yaml, and application.properties files
/// Stores secrets in cloud provider secret store and properties in config store (if enabled)
#[allow(
    clippy::too_many_lines,
    reason = "Complex file processing logic with multiple provider paths"
)]
pub async fn process_application_files(
    reconciler: &Arc<Reconciler>,
    provider: &dyn SecretManagerProvider,
    config: &SecretManagerConfig,
    app_files: &parser::ApplicationFiles,
) -> Result<i32> {
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
            let sops_key_available = config.status
                .as_ref()
                .and_then(|s| s.sops_key_available)
                .unwrap_or(false);

            if !sops_key_available {
                // Key not available for this resource - check if we need to update status
                let resource_namespace = config.metadata.namespace.as_deref().unwrap_or("default");

                // If status is None, check once and update
                if config.status.as_ref().and_then(|s| s.sops_key_available).is_none() {
                    let (key_available, secret_name) = crate::controller::reconciler::status::check_sops_key_availability(
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

        // Parse secrets - handle SOPS decryption errors with proper classification
        let secrets = match parser::parse_secrets(app_files, sops_private_key.as_deref()).await {
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
        if !secrets.is_empty() {
            let secret_keys: Vec<&String> = secrets.keys().collect();
            debug!(
                "📋 Found {} secret key(s) in application.secrets files: {:?}",
                secret_keys.len(),
                secret_keys
            );
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
            secret.count = secrets.len(),
            secret.prefix = secret_prefix
        );
        let publish_start = Instant::now();

        let mut count = 0;
        let mut updated_count = 0;

        for (key, value) in secrets {
            let secret_name = construct_secret_name(
                Some(secret_prefix),
                key.as_str(),
                config.spec.secrets.suffix.as_deref(),
            );
            match provider.create_or_update_secret(&secret_name, &value).await {
                Ok(was_updated) => {
                    count += 1;
                    observability::metrics::increment_secrets_published_total(provider_name, 1);
                    if was_updated {
                        updated_count += 1;
                        info!(
                            "Updated secret {} from git (GitOps source of truth)",
                            secret_name
                        );
                    }
                }
                Err(e) => {
                    observability::metrics::increment_secrets_skipped_total(provider_name, "error");
                    error!("Failed to store secret {}: {}", secret_name, e);
                    return Err(e.context(format!("Failed to store secret: {secret_name}")));
                }
            }
        }

        if updated_count > 0 {
            observability::metrics::increment_secrets_updated(i64::from(updated_count));
            warn!(
                "Updated {} secrets from git (GitOps source of truth). Manual changes in cloud provider were overwritten.",
                updated_count
            );
        }

        // Record successful publish metrics and span
        publish_span.record("operation.duration_ms", publish_start.elapsed().as_millis() as u64);
        publish_span.record("operation.success", true);
        publish_span.record("secrets.published", count as u64);

        // Store properties - route to config store if enabled, otherwise store as JSON blob in secret store
        if !properties.is_empty() {
            let configs_enabled = config
                .spec
                .configs
                .as_ref()
                .map(|c| c.enabled)
                .unwrap_or(false);

            if configs_enabled {
                // Route properties to config store (store individually)
                info!(
                    "Config store enabled: storing {} properties individually",
                    properties.len()
                );
                let mut config_count = 0;
                let mut config_updated_count = 0;

                // Route to appropriate config store based on provider
                match &config.spec.provider {
                    ProviderConfig::Gcp(_gcp_config) => {
                        // For GCP, reuse Secret Manager provider (store configs as individual secrets)
                        // This is an interim solution until Parameter Manager support is contributed to ESO
                        for (key, value) in properties {
                            let config_name = construct_secret_name(
                                Some(secret_prefix),
                                key.as_str(),
                                config.spec.secrets.suffix.as_deref(),
                            );
                            match provider.create_or_update_secret(&config_name, &value).await {
                                Ok(was_updated) => {
                                    config_count += 1;
                                    if was_updated {
                                        config_updated_count += 1;
                                        info!(
                                            "Updated config {} from git (GitOps source of truth)",
                                            config_name
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to store config {}: {}", config_name, e);
                                    return Err(e.context(format!(
                                        "Failed to store config: {config_name}"
                                    )));
                                }
                            }
                        }
                    }
                    ProviderConfig::Aws(aws_config) => {
                        // For AWS, use Parameter Store
                        let parameter_path = config
                            .spec
                            .configs
                            .as_ref()
                            .and_then(|c| c.parameter_path.as_deref());
                        let aws_param_store = AwsParameterStore::new(
                            aws_config,
                            parameter_path,
                            secret_prefix,
                            &config.spec.secrets.environment,
                            &reconciler.client,
                        )
                        .await
                        .context("Failed to create AWS Parameter Store client")?;

                        for (key, value) in properties {
                            match aws_param_store.create_or_update_config(&key, &value).await {
                                Ok(was_updated) => {
                                    config_count += 1;
                                    if was_updated {
                                        config_updated_count += 1;
                                        info!(
                                            "Updated config {} from git (GitOps source of truth)",
                                            key
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to store config {}: {}", key, e);
                                    return Err(e.context(format!("Failed to store config: {key}")));
                                }
                            }
                        }
                    }
                    ProviderConfig::Azure(azure_config) => {
                        // For Azure, use App Configuration
                        let app_config_endpoint = config
                            .spec
                            .configs
                            .as_ref()
                            .and_then(|c| c.app_config_endpoint.as_deref());
                        let azure_app_config = AzureAppConfiguration::new(
                            azure_config,
                            app_config_endpoint,
                            secret_prefix,
                            &config.spec.secrets.environment,
                            &reconciler.client,
                        )
                        .await
                        .context("Failed to create Azure App Configuration client")?;

                        for (key, value) in properties {
                            match azure_app_config.create_or_update_config(&key, &value).await {
                                Ok(was_updated) => {
                                    config_count += 1;
                                    if was_updated {
                                        config_updated_count += 1;
                                        info!(
                                            "Updated config {} from git (GitOps source of truth)",
                                            key
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to store config {}: {}", key, e);
                                    return Err(e.context(format!("Failed to store config: {key}")));
                                }
                            }
                        }
                    }
                }

                count += config_count;
                if config_updated_count > 0 {
                    observability::metrics::increment_secrets_updated(i64::from(
                        config_updated_count,
                    ));
                    warn!(
                        "Updated {} configs from git (GitOps source of truth). Manual changes in cloud provider were overwritten.",
                        config_updated_count
                    );
                }
            } else {
                // Backward compatibility: store properties as a single secret (JSON encoded)
                let properties_json = serde_json::to_string(&properties)?;
                let secret_name = construct_secret_name(
                    Some(secret_prefix),
                    "properties",
                    config.spec.secrets.suffix.as_deref(),
                );
                match provider
                    .create_or_update_secret(&secret_name, &properties_json)
                    .await
                {
                    Ok(was_updated) => {
                        count += 1;
                        if was_updated {
                            observability::metrics::increment_secrets_updated(1);
                            info!("Updated properties secret {} from git", secret_name);
                        }
                    }
                    Err(e) => {
                        error!("Failed to store properties: {}", e);
                        return Err(e.context("Failed to store properties"));
                    }
                }
            }
        }

        observability::metrics::increment_secrets_synced(i64::from(count));
        Ok(count)
    }
    .instrument(span)
    .await;

    match &result {
        Ok(count) => {
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

/// Process Kustomize secrets
/// Extracts secrets from kustomize-generated Secret resources and stores them in cloud provider
pub async fn process_kustomize_secrets(
    provider: &dyn SecretManagerProvider,
    config: &SecretManagerConfig,
    secrets: &std::collections::HashMap<String, String>,
    secret_prefix: &str,
) -> Result<i32> {
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
        secret.count = secrets.len(),
        secret.prefix = secret_prefix
    );
    let publish_start = Instant::now();

    let mut count = 0;
    let mut updated_count = 0;

    for (key, value) in secrets {
        let secret_name = construct_secret_name(
            Some(secret_prefix),
            key.as_str(),
            config.spec.secrets.suffix.as_deref(),
        );
        match provider.create_or_update_secret(&secret_name, value).await {
            Ok(was_updated) => {
                count += 1;
                observability::metrics::increment_secrets_published_total(provider_name, 1);
                if was_updated {
                    updated_count += 1;
                    info!(
                        "Updated secret {} from kustomize build (GitOps source of truth)",
                        secret_name
                    );
                }
            }
            Err(e) => {
                observability::metrics::increment_secrets_skipped_total(provider_name, "error");
                publish_span.record("operation.success", false);
                publish_span.record("error.message", e.to_string());
                error!("Failed to store secret {}: {}", secret_name, e);
                return Err(e.context(format!("Failed to store secret: {secret_name}")));
            }
        }
    }

    if updated_count > 0 {
        observability::metrics::increment_secrets_updated(i64::from(updated_count));
        warn!(
            "Updated {} secrets from kustomize build (GitOps source of truth). Manual changes in cloud provider were overwritten.",
            updated_count
        );
    }

    // Record successful publish metrics and span
    publish_span.record(
        "operation.duration_ms",
        publish_start.elapsed().as_millis() as u64,
    );
    publish_span.record("operation.success", true);
    publish_span.record("secrets.published", count as u64);

    observability::metrics::increment_secrets_synced(i64::from(count));
    Ok(count)
}
