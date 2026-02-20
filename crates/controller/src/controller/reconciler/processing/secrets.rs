//! # Secret Storage
//!
//! Handles storing secrets in cloud provider secret stores, including enabled/disabled state management.

use crate::controller::parser;
use crate::controller::reconciler::processing::diff_discovery::detect_secret_diff;
use crate::controller::reconciler::utils::construct_secret_name;
use crate::crd::{ProviderConfig, ResourceSyncState, SecretManagerConfig};
use crate::observability;
use crate::provider::SecretManagerProvider;
use anyhow::Result;
use tracing::{error, info, warn};

/// Process and store secrets (enabled and disabled)
/// Returns (secrets_count, drift_detected, synced_secrets_map)
/// synced_secrets_map tracks which secrets have been successfully pushed and how many times updated
pub async fn store_secrets(
    provider: &dyn SecretManagerProvider,
    config: &SecretManagerConfig,
    parsed_secrets: &parser::ParsedSecrets,
    secret_prefix: &str,
    provider_name: &str,
) -> Result<(
    i32,
    bool,
    std::collections::HashMap<String, ResourceSyncState>,
)> {
    let mut count = 0;
    let mut updated_count = 0;
    let mut disabled_count = 0;
    let mut enabled_count = 0;
    let mut drift_detected = false;
    let mut errors = Vec::new();

    // Initialize synced_secrets map from existing status (preserve state across reconciliations)
    let mut synced_secrets = config
        .status
        .as_ref()
        .and_then(|s| s.sync.as_ref())
        .and_then(|sync| sync.secrets.clone())
        .unwrap_or_default();

    // Extract environment and location from config
    let environment = &config.spec.secrets.environment;
    // For GCP, location is required in the config (enforced by CRD validation)
    // "automatic" is not a valid GCP location - automatic replication means no specific location (NULL in DB)
    // GCP Secret Manager uses replication: { automatic: {} } which is valid, but location should be NULL
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

    // Process all secrets (both enabled and disabled)
    for (key, entry) in &parsed_secrets.secrets {
        let secret_name = construct_secret_name(
            Some(secret_prefix),
            key.as_str(),
            config.spec.secrets.suffix.as_deref(),
        );

        if entry.enabled {
            // Diff discovery: Compare Git value with cloud provider value
            // This detects if secrets were tampered with externally
            // Only checks if secret has been pushed before (prevents chicken-and-egg on first push)
            if config.spec.diff_discovery {
                if let Ok(has_diff) =
                    detect_secret_diff(provider, config, &secret_name, &entry.value).await
                {
                    if has_diff {
                        drift_detected = true;
                        observability::metrics::increment_secrets_diff_detected_total(
                            provider_name,
                        );
                    }
                }
            }

            // Check if secret exists in cloud provider
            let secret_exists = provider
                .get_secret_value(&secret_name)
                .await
                .map(|v| v.is_some())
                .unwrap_or(false);

            // triggerUpdate logic: Only update if flag is enabled OR secret doesn't exist
            // When triggerUpdate is false, we only create missing secrets, don't update existing ones
            let should_update = if config.spec.trigger_update {
                // Always update when triggerUpdate is enabled (default behavior)
                true
            } else {
                // When triggerUpdate is disabled, only create missing secrets
                !secret_exists
            };

            if should_update {
                // Enabled secret: create/update as normal, and ensure it's enabled
                match provider
                    .create_or_update_secret(&secret_name, &entry.value, environment, &location)
                    .await
                {
                    Ok(was_updated) => {
                        count += 1;
                        observability::metrics::increment_secrets_published_total(provider_name, 1);

                        // Update push state: track existence and update count
                        let sync_state =
                            synced_secrets
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
                            if secret_exists {
                                info!(
                                    provider = provider_name,
                                    secret_name = secret_name,
                                    environment = config.spec.secrets.environment,
                                    operation = "update",
                                    update_count = sync_state.update_count,
                                    "✅ Updated secret '{}' from git (GitOps source of truth) - provider={}, environment={}, update_count={}",
                                    secret_name,
                                    provider_name,
                                    config.spec.secrets.environment,
                                    sync_state.update_count
                                );
                            } else {
                                info!(
                                    provider = provider_name,
                                    secret_name = secret_name,
                                    environment = config.spec.secrets.environment,
                                    operation = "create",
                                    update_count = sync_state.update_count,
                                    "✅ Created secret '{}' from git - provider={}, environment={}, update_count={}",
                                    secret_name,
                                    provider_name,
                                    config.spec.secrets.environment,
                                    sync_state.update_count
                                );
                            }
                        } else {
                            info!(
                                provider = provider_name,
                                secret_name = secret_name,
                                environment = config.spec.secrets.environment,
                                operation = "no_change",
                                exists = sync_state.exists,
                                update_count = sync_state.update_count,
                                "✅ Secret '{}' unchanged (no update needed) - provider={}, environment={}, exists={}, update_count={}",
                                secret_name,
                                provider_name,
                                config.spec.secrets.environment,
                                sync_state.exists,
                                sync_state.update_count
                            );
                        }
                    }
                    Err(e) => {
                        observability::metrics::increment_secrets_skipped_total(
                            provider_name,
                            "error",
                        );
                        error!("Failed to store secret {}: {}", secret_name, e);
                        errors.push(format!("Failed to store secret {}: {}", secret_name, e));
                        // Continue processing other secrets instead of returning early
                    }
                }
            } else {
                // triggerUpdate is disabled and secret exists - skip update
                info!(
                    "⏭️  Skipping update for secret '{}' (triggerUpdate disabled, secret already exists)",
                    secret_name
                );
                count += 1; // Count as processed even though we didn't update
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
                    .create_or_update_secret(&secret_name, &entry.value, environment, &location)
                    .await
                {
                    Ok(was_updated) => {
                        // Update push state for disabled secrets too
                        let sync_state =
                            synced_secrets
                                .entry(secret_name.clone())
                                .or_insert_with(|| ResourceSyncState {
                                    exists: false,
                                    update_count: 0,
                                });
                        sync_state.exists = true;
                        if was_updated {
                            sync_state.update_count += 1;
                        }
                        info!(
                            "✅ Updated disabled secret '{}' value from git - update_count={}",
                            secret_name, sync_state.update_count
                        );
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
                        info!(
                            "🚫 Disabled secret '{}' (commented out in git)",
                            secret_name
                        );
                    } else {
                        info!("✅ Secret '{}' already disabled", secret_name);
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

    Ok((count, drift_detected, synced_secrets))
}
