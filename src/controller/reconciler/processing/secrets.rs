//! # Secret Storage
//!
//! Handles storing secrets in cloud provider secret stores, including enabled/disabled state management.

use crate::controller::parser;
use crate::controller::reconciler::utils::construct_secret_name;
use crate::crd::SecretManagerConfig;
use crate::observability;
use crate::provider::SecretManagerProvider;
use anyhow::Result;
use tracing::{debug, error, info, warn};

/// Process and store secrets (enabled and disabled)
pub async fn store_secrets(
    provider: &dyn SecretManagerProvider,
    config: &SecretManagerConfig,
    parsed_secrets: &parser::ParsedSecrets,
    secret_prefix: &str,
    provider_name: &str,
) -> Result<i32> {
    let mut count = 0;
    let mut updated_count = 0;
    let mut disabled_count = 0;
    let mut enabled_count = 0;

    // Process all secrets (both enabled and disabled)
    for (key, entry) in &parsed_secrets.secrets {
        let secret_name = construct_secret_name(
            Some(secret_prefix),
            key.as_str(),
            config.spec.secrets.suffix.as_deref(),
        );

        if entry.enabled {
            // Enabled secret: create/update as normal, and ensure it's enabled
            match provider
                .create_or_update_secret(&secret_name, &entry.value)
                .await
            {
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

            // Ensure secret is enabled (in case it was previously disabled)
            if let Err(e) = provider.enable_secret(&secret_name).await {
                warn!(
                    "Failed to enable secret {} (may have been disabled): {}",
                    secret_name, e
                );
                // Don't fail the entire operation, just log a warning
            } else {
                enabled_count += 1;
            }
        } else {
            // Disabled secret: update value if changed, then disable
            // First, check if secret exists and update value if needed
            let current_value = provider.get_secret_value(&secret_name).await?;
            let value_changed = current_value
                .as_ref()
                .map(|v| v != &entry.value)
                .unwrap_or(true);

            if value_changed {
                // Update the value even though it's disabled
                // This handles the case: #FOO_SECRET=baz (disabled but value updated)
                match provider
                    .create_or_update_secret(&secret_name, &entry.value)
                    .await
                {
                    Ok(_) => {
                        debug!("Updated disabled secret {} value from git", secret_name);
                    }
                    Err(e) => {
                        // If secret doesn't exist, that's okay - we'll just disable it when it's created later
                        if !e.to_string().contains("not found") && !e.to_string().contains("404") {
                            warn!(
                                "Failed to update disabled secret {} value: {}",
                                secret_name, e
                            );
                        }
                    }
                }
            }

            // Disable the secret
            match provider.disable_secret(&secret_name).await {
                Ok(was_disabled) => {
                    if was_disabled {
                        disabled_count += 1;
                        info!("Disabled secret {} (commented out in git)", secret_name);
                    }
                }
                Err(e) => {
                    // If secret doesn't exist, that's okay - it's already effectively disabled
                    if !e.to_string().contains("not found") && !e.to_string().contains("404") {
                        warn!("Failed to disable secret {}: {}", secret_name, e);
                        // Don't fail the entire operation, just log a warning
                    }
                }
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

    if disabled_count > 0 {
        info!(
            "Disabled {} secret(s) (commented out in git)",
            disabled_count
        );
    }

    if enabled_count > 0 {
        info!(
            "Re-enabled {} secret(s) (uncommented in git)",
            enabled_count
        );
    }

    Ok(count)
}
