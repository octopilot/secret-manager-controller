//! # Kustomize Secret Processing
//!
//! Handles processing secrets extracted from kustomize builds.

use crate::controller::reconciler::utils::construct_secret_name;
use crate::crd::{ProviderConfig, SecretManagerConfig};
use crate::observability;
use crate::provider::SecretManagerProvider;
use anyhow::Result;
use std::collections::HashMap;
use std::time::Instant;
use tracing::{error, info, info_span, warn};

/// Process Kustomize secrets
/// Extracts secrets from kustomize-generated Secret resources and stores them in cloud provider
pub async fn process_kustomize_secrets(
    provider: &dyn SecretManagerProvider,
    config: &SecretManagerConfig,
    secrets: &HashMap<String, String>,
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
