//! # Property Storage
//!
//! Handles storing properties in config stores (Parameter Store, App Configuration) or as secrets.

use crate::controller::reconciler::types::Reconciler;
use crate::controller::reconciler::utils::construct_secret_name;
use crate::crd::{ProviderConfig, SecretManagerConfig};
use crate::observability;
use crate::provider::aws::AwsParameterStore;
use crate::provider::azure::AzureAppConfiguration;
use crate::provider::{ConfigStoreProvider, SecretManagerProvider};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Store properties in appropriate store (config store if enabled, otherwise secret store)
pub async fn store_properties(
    reconciler: &Arc<Reconciler>,
    provider: &dyn SecretManagerProvider,
    config: &SecretManagerConfig,
    properties: HashMap<String, String>,
    secret_prefix: &str,
) -> Result<i32> {
    if properties.is_empty() {
        return Ok(0);
    }

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
                            return Err(e.context(format!("Failed to store config: {config_name}")));
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
                                info!("Updated config {} from git (GitOps source of truth)", key);
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
                                info!("Updated config {} from git (GitOps source of truth)", key);
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

        Ok(config_count)
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
                if was_updated {
                    observability::metrics::increment_secrets_updated(1);
                    info!("Updated properties secret {} from git", secret_name);
                }
                Ok(1)
            }
            Err(e) => {
                error!("Failed to store properties: {}", e);
                Err(e.context("Failed to store properties"))
            }
        }
    }
}
