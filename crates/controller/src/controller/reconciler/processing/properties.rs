//! # Property Storage
//!
//! Handles storing properties in config stores (Parameter Store, App Configuration) or as secrets.

use crate::controller::reconciler::types::Reconciler;
use crate::controller::reconciler::utils::construct_secret_name;
use crate::crd::{ConfigStoreType, ProviderConfig, ResourceSyncState, SecretManagerConfig};
use crate::observability;
use crate::provider::aws::AwsParameterStore;
use crate::provider::azure::AzureAppConfiguration;
use crate::provider::gcp::create_gcp_parameter_manager_provider;
use crate::provider::{ConfigStoreProvider, SecretManagerProvider};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Store properties in appropriate store (config store if enabled, otherwise secret store)
/// Returns (count, synced_properties_map) where synced_properties tracks push state
pub async fn store_properties(
    reconciler: &Arc<Reconciler>,
    provider: &dyn SecretManagerProvider,
    config: &SecretManagerConfig,
    properties: HashMap<String, String>,
    secret_prefix: &str,
) -> Result<(i32, std::collections::HashMap<String, ResourceSyncState>)> {
    if properties.is_empty() {
        return Ok((0, std::collections::HashMap::new()));
    }

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

    // Initialize synced_properties map from existing status (preserve state across reconciliations)
    let mut synced_properties = config
        .status
        .as_ref()
        .and_then(|s| s.sync.as_ref())
        .and_then(|sync| sync.properties.clone())
        .unwrap_or_default();

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
            ProviderConfig::Gcp(gcp_config) => {
                // Check if Parameter Manager is configured
                let use_parameter_manager = config
                    .spec
                    .configs
                    .as_ref()
                    .and_then(|c| c.store.as_ref())
                    .map(|store| matches!(store, ConfigStoreType::ParameterManager))
                    .unwrap_or(false);

                if use_parameter_manager {
                    // Use Parameter Manager for configs
                    info!("Using GCP Parameter Manager for configs");

                    // Extract auth config similar to how it's done in provider.rs
                    let (auth_type, service_account_email_owned) =
                        if let Some(ref auth_config) = gcp_config.auth {
                            match serde_json::to_value(auth_config)
                                .context("Failed to serialize gcpAuth config")
                            {
                                Ok(auth_json) => {
                                    let auth_type_str =
                                        auth_json.get("authType").and_then(|t| t.as_str());
                                    if let Some("WorkloadIdentity") = auth_type_str {
                                        match auth_json
                                            .get("serviceAccountEmail")
                                            .and_then(|e| e.as_str())
                                        {
                                            Some(email) => {
                                                (Some("WorkloadIdentity"), Some(email.to_string()))
                                            }
                                            None => (Some("WorkloadIdentity"), None),
                                        }
                                    } else {
                                        (Some("WorkloadIdentity"), None)
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to serialize GCP auth config: {}", e);
                                    return Err(anyhow::anyhow!(
                                        "Failed to serialize GCP auth config: {}",
                                        e
                                    ));
                                }
                            }
                        } else {
                            (Some("WorkloadIdentity"), None)
                        };

                    let param_provider = create_gcp_parameter_manager_provider(
                        gcp_config.project_id.clone(),
                        auth_type,
                        service_account_email_owned.as_deref(),
                    )
                    .await
                    .context("Failed to create GCP Parameter Manager provider")?;

                    for (key, value) in properties {
                        let config_name = construct_secret_name(
                            Some(secret_prefix),
                            key.as_str(),
                            config.spec.secrets.suffix.as_deref(),
                        );
                        match param_provider
                            .create_or_update_config(&config_name, &value)
                            .await
                        {
                            Ok(was_updated) => {
                                config_count += 1;

                                // Update push state: track existence and update count
                                let sync_state = synced_properties
                                    .entry(config_name.clone())
                                    .or_insert_with(|| ResourceSyncState {
                                        exists: false,
                                        update_count: 0,
                                    });
                                sync_state.exists = true;

                                if was_updated {
                                    sync_state.update_count += 1;
                                    config_updated_count += 1;
                                    info!(
                                        "✅ Updated config '{}' in Parameter Manager (GitOps source of truth) - update_count={}",
                                        config_name, sync_state.update_count
                                    );
                                } else {
                                    info!(
                                        "✅ Config '{}' unchanged (no update needed) - exists={}, update_count={}",
                                        config_name, sync_state.exists, sync_state.update_count
                                    );
                                }
                            }
                            Err(e) => {
                                error!("Failed to store config {}: {}", config_name, e);
                                return Err(
                                    e.context(format!("Failed to store config: {config_name}"))
                                );
                            }
                        }
                    }
                } else {
                    // Default: reuse Secret Manager provider (store configs as individual secrets)
                    // This maintains backward compatibility
                    for (key, value) in properties {
                        let config_name = construct_secret_name(
                            Some(secret_prefix),
                            key.as_str(),
                            config.spec.secrets.suffix.as_deref(),
                        );
                        match provider
                            .create_or_update_secret(&config_name, &value, environment, &location)
                            .await
                        {
                            Ok(was_updated) => {
                                config_count += 1;

                                // Update push state: track existence and update count
                                let sync_state = synced_properties
                                    .entry(config_name.clone())
                                    .or_insert_with(|| ResourceSyncState {
                                        exists: false,
                                        update_count: 0,
                                    });
                                sync_state.exists = true;

                                if was_updated {
                                    sync_state.update_count += 1;
                                    config_updated_count += 1;
                                    info!(
                                        "Updated config {} from git (GitOps source of truth) - update_count={}",
                                        config_name, sync_state.update_count
                                    );
                                } else {
                                    debug!(
                                        "Config {} unchanged (no update needed) - exists={}, update_count={}",
                                        config_name, sync_state.exists, sync_state.update_count
                                    );
                                }
                            }
                            Err(e) => {
                                error!("Failed to store config {}: {}", config_name, e);
                                return Err(
                                    e.context(format!("Failed to store config: {config_name}"))
                                );
                            }
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

                            // Update push state: track existence and update count
                            let sync_state =
                                synced_properties.entry(key.clone()).or_insert_with(|| {
                                    ResourceSyncState {
                                        exists: false,
                                        update_count: 0,
                                    }
                                });
                            sync_state.exists = true;

                            if was_updated {
                                sync_state.update_count += 1;
                                config_updated_count += 1;
                                info!("Updated config {} from git (GitOps source of truth) - update_count={}", key, sync_state.update_count);
                            } else {
                                debug!(
                                    "Config {} unchanged (no update needed) - exists={}, update_count={}",
                                    key, sync_state.exists, sync_state.update_count
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

                            // Update push state: track existence and update count
                            let sync_state =
                                synced_properties.entry(key.clone()).or_insert_with(|| {
                                    ResourceSyncState {
                                        exists: false,
                                        update_count: 0,
                                    }
                                });
                            sync_state.exists = true;

                            if was_updated {
                                sync_state.update_count += 1;
                                config_updated_count += 1;
                                info!("✅ Updated config '{}' from git (GitOps source of truth) - update_count={}", key, sync_state.update_count);
                            } else {
                                info!(
                                    "✅ Config '{}' unchanged (no update needed) - exists={}, update_count={}",
                                    key, sync_state.exists, sync_state.update_count
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

        if config_updated_count > 0 {
            observability::metrics::increment_secrets_updated(i64::from(config_updated_count));
            warn!(
                "Updated {} configs from git (GitOps source of truth). Manual changes in cloud provider were overwritten.",
                config_updated_count
            );
        }

        Ok((config_count, synced_properties))
    } else {
        // Backward compatibility: store properties as a single secret (JSON encoded)
        let properties_json = serde_json::to_string(&properties)?;
        let secret_name = construct_secret_name(
            Some(secret_prefix),
            "properties",
            config.spec.secrets.suffix.as_deref(),
        );
        match provider
            .create_or_update_secret(&secret_name, &properties_json, environment, &location)
            .await
        {
            Ok(was_updated) => {
                // Update push state for properties secret
                let sync_state =
                    synced_properties
                        .entry(secret_name.clone())
                        .or_insert_with(|| ResourceSyncState {
                            exists: false,
                            update_count: 0,
                        });
                sync_state.exists = true;

                if was_updated {
                    sync_state.update_count += 1;
                    observability::metrics::increment_secrets_updated(1);
                    info!(
                        "✅ Updated properties secret '{}' from git - update_count={}",
                        secret_name, sync_state.update_count
                    );
                } else {
                    info!(
                        "✅ Properties secret '{}' unchanged (no update needed) - exists={}, update_count={}",
                        secret_name, sync_state.exists, sync_state.update_count
                    );
                }
                Ok((1, synced_properties))
            }
            Err(e) => {
                error!("Failed to store properties: {}", e);
                Err(e.context("Failed to store properties"))
            }
        }
    }
}
