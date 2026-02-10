//! # Kustomize Secret Processing
//!
//! Handles processing secrets extracted from kustomize builds.

use crate::controller::reconciler::utils::construct_secret_name;
use crate::crd::{ProviderConfig, ResourceSyncState, SecretManagerConfig};
use crate::observability;
use crate::provider::SecretManagerProvider;
use anyhow::Result;
use std::collections::HashMap;
use std::time::Instant;
use tracing::{error, info, info_span, warn};

/// Process Kustomize secrets
/// Extracts secrets from kustomize-generated Secret resources and stores them in cloud provider
/// Returns (count, synced_secrets_map) where synced_secrets tracks push state
pub async fn process_kustomize_secrets(
    provider: &dyn SecretManagerProvider,
    config: &SecretManagerConfig,
    secrets: &HashMap<String, String>,
    secret_prefix: &str,
) -> Result<(i32, std::collections::HashMap<String, ResourceSyncState>)> {
    // Initialize synced_secrets map from existing status
    let mut synced_secrets = config
        .status
        .as_ref()
        .and_then(|s| s.sync.as_ref())
        .and_then(|sync| sync.secrets.clone())
        .unwrap_or_default();
    // Store secrets in cloud provider (GitOps: Git is source of truth)
    // Get provider name for metrics
    let provider_name = match &config.spec.provider {
        ProviderConfig::Gcp(_) => "gcp",
        ProviderConfig::Aws(_) => "aws",
        ProviderConfig::Azure(_) => "azure",
    };

    // Extract environment and location from config
    let environment = &config.spec.secrets.environment;
    // For GCP, location is required in the config (enforced by CRD validation)
    // "automatic" is not a valid GCP location - automatic replication means no specific location (NULL in DB)
    // If location is empty string, treat it as automatic replication (NULL in DB)
    let location = match &config.spec.provider {
        ProviderConfig::Gcp(gcp_config) => {
            // Location is required, but if it's empty string, treat as automatic replication
            let loc = gcp_config.location.clone();
            if loc.is_empty() || loc == "automatic" {
                "".to_string() // Empty means automatic replication (NULL in DB)
            } else {
                loc
            }
        }
        ProviderConfig::Aws(aws_config) => aws_config.region.clone(),
        ProviderConfig::Azure(azure_config) => {
            // Location is required in the config (enforced by CRD validation)
            azure_config.location.clone()
        }
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
        match provider
            .create_or_update_secret(&secret_name, value, environment, &location)
            .await
        {
            Ok(was_updated) => {
                count += 1;
                observability::metrics::increment_secrets_published_total(provider_name, 1);

                // Update push state: track existence and update count
                let sync_state = synced_secrets
                    .entry(secret_name.clone())
                    .or_insert_with(|| ResourceSyncState {
                        exists: false,
                        update_count: 0,
                    });

                // Mark as existing (successfully pushed)
                sync_state.exists = true;

                // Only increment update_count if value actually changed
                if was_updated {
                    sync_state.update_count += 1;
                    updated_count += 1;
                    info!(
                        "✅ Updated secret '{}' from kustomize build (GitOps source of truth) - update_count={}",
                        secret_name, sync_state.update_count
                    );
                } else {
                    info!(
                        "✅ Secret '{}' unchanged (no update needed) - exists={}, update_count={}",
                        secret_name, sync_state.exists, sync_state.update_count
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
    Ok((count, synced_secrets))
}
