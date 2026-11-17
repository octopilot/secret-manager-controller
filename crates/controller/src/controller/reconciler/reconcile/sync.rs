//! # Secret Syncing
//!
//! Handles syncing secrets from artifact path to cloud provider.

use crate::controller::parser;
use crate::controller::reconciler::processing::{
    process_application_files, process_kustomize_secrets,
};
use crate::controller::reconciler::status::update_status_phase;
use crate::controller::reconciler::types::{Reconciler, ReconcilerError};
use crate::crd::{ResourceSyncState, SecretManagerConfig};
use crate::observability;
use crate::provider::SecretManagerProvider;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Result type for secret syncing
#[derive(Debug)]
pub enum SyncResult {
    /// Successfully synced secrets (count)
    Success(u32),
    /// Transient error - should retry
    TransientError,
    /// Permanent error
    Error(ReconcilerError),
}

/// Sync secrets from artifact path to provider
/// Returns synced_secrets and synced_properties maps tracking which resources have been pushed and their update counts
pub async fn sync_secrets(
    config: &Arc<SecretManagerConfig>,
    ctx: &Arc<Reconciler>,
    provider: &dyn SecretManagerProvider,
    artifact_path: &PathBuf,
) -> Result<
    (
        SyncResult,
        std::collections::HashMap<String, ResourceSyncState>,
        std::collections::HashMap<String, ResourceSyncState>,
    ),
    ReconcilerError,
> {
    let mut secrets_synced = 0;
    // First permanent failure. The other services are still synced, then the reconcile
    // is reported as failed instead of Ready.
    let mut first_failure: Option<anyhow::Error> = None;
    let mut desired_secrets = std::collections::HashSet::new();
    let mut all_synced_secrets = std::collections::HashMap::new();
    let mut all_synced_properties = std::collections::HashMap::new();

    // Determine processing mode: kustomize build vs raw file parsing
    // Kustomize mode: Extract secrets from kustomize-generated Secret resources
    // Raw file mode: Parse application.secrets.env files directly
    if let Some(kustomize_path) = &config.spec.secrets.kustomize_path {
        // Kustomize Build Mode
        // Runs `kustomize build` to generate Kubernetes manifests, then extracts Secret resources
        // Supports overlays, patches, generators, and other kustomize features
        // This is the recommended mode for complex deployments with multiple environments
        info!("Using kustomize build mode on path: {}", kustomize_path);

        match crate::controller::kustomize::extract_secrets_from_kustomize(
            artifact_path,
            kustomize_path,
        ) {
            Ok(secrets) => {
                let secret_prefix = config.spec.secrets.prefix.as_deref().unwrap_or("default");
                match process_kustomize_secrets(provider, config, &secrets, secret_prefix).await {
                    Ok((count, kustomize_synced_secrets)) => {
                        secrets_synced += count as u32;
                        desired_secrets.extend(secrets.keys().map(|key| {
                            crate::controller::reconciler::construct_secret_name(
                                Some(secret_prefix),
                                key,
                                config.spec.secrets.suffix.as_deref(),
                            )
                        }));
                        // Merge kustomize synced_secrets into overall map
                        for (secret_name, sync_state) in kustomize_synced_secrets {
                            let existing = all_synced_secrets
                                .entry(secret_name.clone())
                                .or_insert_with(|| ResourceSyncState {
                                    exists: false,
                                    update_count: 0,
                                });
                            existing.exists = existing.exists || sync_state.exists;
                            existing.update_count =
                                existing.update_count.max(sync_state.update_count);
                        }
                        info!("✅ Synced {} secrets from kustomize build", count);
                    }
                    Err(e) => {
                        error!("Failed to process kustomize secrets: {}", e);
                        observability::metrics::increment_reconciliation_errors();
                        // Update status to Failed
                        let _ = update_status_phase(
                            ctx,
                            config,
                            "Failed",
                            Some(&format!("Failed to process kustomize secrets: {e}")),
                        )
                        .await;
                        return Ok((
                            SyncResult::Error(ReconcilerError::ReconciliationFailed(e)),
                            all_synced_secrets,
                            all_synced_properties,
                        ));
                    }
                }
            }
            Err(e) => {
                error!("Failed to extract secrets from kustomize build: {}", e);
                observability::metrics::increment_reconciliation_errors();
                // Update status to Failed
                let _ = update_status_phase(
                    ctx,
                    config,
                    "Failed",
                    Some(&format!("Failed to extract secrets from kustomize: {e}")),
                )
                .await;
                return Ok((
                    SyncResult::Error(ReconcilerError::ReconciliationFailed(e)),
                    std::collections::HashMap::new(),
                    std::collections::HashMap::new(),
                ));
            }
        }
    } else {
        // Raw File Mode
        // Directly parses application.secrets.env, application.secrets.yaml, and application.properties files
        // Simpler than kustomize mode but doesn't support overlays or generators
        // Suitable for simple deployments or when kustomize isn't needed
        debug!("Using raw file mode");

        // Find application files for the specified environment
        // Searches for files matching patterns like:
        // - {basePath}/profiles/{environment}/application.secrets.env
        // - {basePath}/{service}/profiles/{environment}/application.secrets.env
        // Pass secret_prefix as default_service_name for single service deployments
        let default_service_name = config.spec.secrets.prefix.as_deref();
        // spec.secrets.files names the files outright; otherwise they are discovered.
        let explicit_files = config
            .spec
            .secrets
            .files
            .as_deref()
            .filter(|files| !files.is_empty());
        let found = if let Some(files) = explicit_files {
            let service_name = default_service_name
                .map(ToString::to_string)
                .or_else(|| config.metadata.name.clone())
                .unwrap_or_else(|| "default-service".to_string());
            parser::explicit_application_files(artifact_path, files, &service_name)
        } else {
            parser::find_application_files(
                artifact_path,
                config.spec.secrets.base_path.as_deref(),
                &config.spec.secrets.environment,
                default_service_name,
            )
            .await
        };
        let application_files = match found {
            Ok(files) => files,
            Err(e) => {
                error!(
                    "Failed to find application files for environment '{}': {}",
                    config.spec.secrets.environment, e
                );
                observability::metrics::increment_reconciliation_errors();
                // Update status to Failed
                let _ = update_status_phase(
                    ctx,
                    config,
                    "Failed",
                    Some(&format!("Failed to find application files: {e}")),
                )
                .await;
                return Ok((
                    SyncResult::Error(ReconcilerError::ReconciliationFailed(e)),
                    std::collections::HashMap::new(),
                    std::collections::HashMap::new(),
                ));
            }
        };

        debug!(
            "📋 Found {} application file set(s) to process",
            application_files.len()
        );

        // Process each application file set
        for app_files in application_files {
            match process_application_files(ctx, provider, config, &app_files).await {
                Ok((count, synced_secrets, synced_properties, desired)) => {
                    secrets_synced += count as u32;
                    desired_secrets.extend(desired);
                    // Merge synced_secrets from this service into the overall map
                    for (secret_name, sync_state) in synced_secrets {
                        // If secret already exists, merge the states (take max update_count, true if either exists)
                        let existing = all_synced_secrets
                            .entry(secret_name.clone())
                            .or_insert_with(|| ResourceSyncState {
                                exists: false,
                                update_count: 0,
                            });
                        existing.exists = existing.exists || sync_state.exists;
                        existing.update_count = existing.update_count.max(sync_state.update_count);
                    }
                    // Merge synced_properties from this service into the overall map
                    for (property_name, sync_state) in synced_properties {
                        let existing = all_synced_properties
                            .entry(property_name.clone())
                            .or_insert_with(|| ResourceSyncState {
                                exists: false,
                                update_count: 0,
                            });
                        existing.exists = existing.exists || sync_state.exists;
                        existing.update_count = existing.update_count.max(sync_state.update_count);
                    }
                    debug!(
                        "✅ Synced {} secrets for service: {}",
                        count, app_files.service_name
                    );
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    // Check if this is a transient SOPS decryption error
                    let is_transient = error_msg.contains("transient");

                    if is_transient {
                        // Transient error - log warning and return action to retry
                        warn!(
                            "⏳ Transient error processing service {}: {}. Will retry.",
                            app_files.service_name, error_msg
                        );
                        observability::metrics::increment_reconciliation_errors();
                        // Update status to indicate retry
                        let _ = update_status_phase(
                            ctx,
                            config,
                            "Retrying",
                            Some(&format!("Transient error: {}. Retrying...", error_msg)),
                        )
                        .await;
                        // Return action to retry after a delay
                        return Ok((
                            SyncResult::TransientError,
                            all_synced_secrets,
                            all_synced_properties,
                        ));
                    } else {
                        // Permanent error - log error and continue with other services
                        // This allows partial success when multiple services are configured
                        error!(
                            "❌ Permanent error processing service {}: {}",
                            app_files.service_name, error_msg
                        );
                        observability::metrics::increment_reconciliation_errors();
                        if first_failure.is_none() {
                            first_failure = Some(e.context(format!(
                                "Failed to process service {}",
                                app_files.service_name
                            )));
                        }
                    }
                }
            }
        }
    }

    if let Some(e) = first_failure {
        return Ok((
            SyncResult::Error(ReconcilerError::ReconciliationFailed(e)),
            all_synced_secrets,
            all_synced_properties,
        ));
    }

    // Every file was read and synced: with the prune annotation, delete what this
    // resource synced earlier and its files no longer define.
    if crate::controller::reconciler::processing::prune::prune_enabled(config) {
        match crate::controller::reconciler::processing::prune::prune_secrets(
            provider,
            config,
            &desired_secrets,
            &mut all_synced_secrets,
        )
        .await
        {
            Ok(0) => {}
            Ok(pruned) => info!(
                "🗑️  Pruned {} secret(s) no longer in the secret files",
                pruned
            ),
            Err(e) => {
                error!("Pruning failed: {}", e);
                observability::metrics::increment_reconciliation_errors();
                return Ok((
                    SyncResult::Error(ReconcilerError::ReconciliationFailed(e)),
                    all_synced_secrets,
                    all_synced_properties,
                ));
            }
        }
    }

    Ok((
        SyncResult::Success(secrets_synced),
        all_synced_secrets,
        all_synced_properties,
    ))
}
