//! # Azure App Configuration Operations
//!
//! Implements the `ConfigStoreProvider` trait for Azure App Configuration.

use super::auth::get_token;
use super::client::ClientComponents;
use super::types::KeyValue;
use crate::observability::metrics;
use crate::provider::ConfigStoreProvider;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::time::Instant;
use tracing::{Instrument, debug, info, info_span};

/// Azure App Configuration provider operations
pub struct AzureAppConfigurationOperations {
    pub(crate) components: ClientComponents,
}

impl AzureAppConfigurationOperations {
    /// Construct full key name from config key
    /// Format: {prefix}:{environment}:{key}
    pub fn construct_key_name(&self, key: &str) -> String {
        format!("{}{}", self.components.key_prefix, key)
    }
}

#[async_trait]
impl ConfigStoreProvider for AzureAppConfigurationOperations {
    async fn create_or_update_config(&self, config_key: &str, config_value: &str) -> Result<bool> {
        let key_name = self.construct_key_name(config_key);
        let vault_name = self
            .components
            .endpoint
            .strip_prefix("https://")
            .and_then(|s| s.strip_suffix(".azconfig.io"))
            .unwrap_or("unknown");
        let span = info_span!(
            "azure.appconfig.create_or_update",
            key.name = key_name,
            vault.name = vault_name
        );
        let span_clone = span.clone();
        let start = Instant::now();

        async move {
            // Get access token
            let token = get_token(&self.components.credential).await?;

            // Check if key exists
            let get_url = format!("{}/kv/{}", self.components.endpoint, key_name);
            let get_response = self
                .components
                .http_client
                .get(&get_url)
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .send()
                .await
                .context("Failed to check if Azure App Configuration key exists")?;

            let key_exists = get_response.status().is_success();

            let operation_type = if !key_exists {
                // Create key-value
                info!("Creating Azure App Configuration key: {}", key_name);
                let kv = KeyValue {
                    key: key_name.clone(),
                    value: config_value.to_string(),
                    label: None,
                    content_type: Some("text/plain".to_string()),
                };

                let put_url = format!("{}/kv", self.components.endpoint);
                let response = self
                    .components
                    .http_client
                    .put(&put_url)
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .json(&kv)
                    .send()
                    .await
                    .context("Failed to create Azure App Configuration key-value")?;

                if !response.status().is_success() {
                    let status = response.status();
                    let error_text = response.text().await.unwrap_or_default();
                    span_clone.record("operation.success", false);
                    span_clone.record("operation.type", "create");
                    span_clone.record("error.message", format!("HTTP {}: {}", status, error_text));
                    span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                    metrics::increment_provider_operation_errors("azure");
                    return Err(anyhow::anyhow!(
                        "Failed to create Azure App Configuration key-value: {status} - {error_text}"
                    ));
                }

                metrics::record_secret_operation("azure", "create", start.elapsed().as_secs_f64());
                span_clone.record("operation.type", "create");
                span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                span_clone.record("operation.success", true);
                return Ok(true);
            } else {
                // Get current value
                let current_value = self.get_config_value(config_key).await?;

                if let Some(current) = current_value {
                    if current == config_value {
                        debug!(
                            "Azure App Configuration key {} unchanged, skipping update",
                            key_name
                        );
                        metrics::record_secret_operation("azure", "no_change", start.elapsed().as_secs_f64());
                        span_clone.record("operation.type", "no_change");
                        span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        span_clone.record("operation.success", true);
                        return Ok(false);
                    }
                }

                // Update key-value
                info!("Updating Azure App Configuration key: {}", key_name);
                let kv = KeyValue {
                    key: key_name.clone(),
                    value: config_value.to_string(),
                    label: None,
                    content_type: Some("text/plain".to_string()),
                };

                let put_url = format!("{}/kv", self.components.endpoint);
                let response = self
                    .components
                    .http_client
                    .put(&put_url)
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .json(&kv)
                    .send()
                    .await
                    .context("Failed to update Azure App Configuration key-value")?;

                if !response.status().is_success() {
                    let status = response.status();
                    let error_text = response.text().await.unwrap_or_default();
                    span_clone.record("operation.success", false);
                    span_clone.record("operation.type", "update");
                    span_clone.record("error.message", format!("HTTP {}: {}", status, error_text));
                    span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                    metrics::increment_provider_operation_errors("azure");
                    return Err(anyhow::anyhow!(
                        "Failed to update Azure App Configuration key-value: {status} - {error_text}"
                    ));
                }

                metrics::record_secret_operation("azure", "update", start.elapsed().as_secs_f64());
                "update"
            };

            span_clone.record("operation.type", operation_type);
            span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
            span_clone.record("operation.success", true);
            Ok(true)
        }
        .instrument(span)
        .await
    }

    async fn get_config_value(&self, config_key: &str) -> Result<Option<String>> {
        let key_name = self.construct_key_name(config_key);
        let vault_name = self
            .components
            .endpoint
            .strip_prefix("https://")
            .and_then(|s| s.strip_suffix(".azconfig.io"))
            .unwrap_or("unknown");
        let span = tracing::debug_span!(
            "azure.appconfig.get",
            key.name = key_name,
            vault.name = vault_name
        );
        let span_clone = span.clone();
        let start = Instant::now();

        async move {
            let token = match get_token(&self.components.credential).await {
                Ok(t) => t,
                Err(e) => {
                    let error_msg = e.to_string();
                    span_clone.record("operation.success", false);
                    span_clone.record("error.message", format!("Failed to get token: {}", error_msg));
                    span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                    metrics::increment_provider_operation_errors("azure");
                    return Err(anyhow::anyhow!("Failed to get Azure App Configuration access token: {e}"));
                }
            };

            let url = format!("{}/kv/{}", self.components.endpoint, key_name);
            match self
                .components
                .http_client
                .get(&url)
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<KeyValue>().await {
                            Ok(kv) => {
                                span_clone.record("operation.success", true);
                                span_clone.record("operation.found", true);
                                span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                                metrics::record_secret_operation("azure", "get", start.elapsed().as_secs_f64());
                                Ok(Some(kv.value))
                            }
                            Err(e) => {
                                let error_msg = e.to_string();
                                span_clone.record("operation.success", false);
                                span_clone.record("error.message", format!("Failed to deserialize response: {}", error_msg));
                                span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                                metrics::increment_provider_operation_errors("azure");
                                Err(anyhow::anyhow!("Failed to deserialize Azure App Configuration response: {e}"))
                            }
                        }
                    } else if response.status() == 404 {
                        span_clone.record("operation.success", true);
                        span_clone.record("operation.found", false);
                        span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::record_secret_operation("azure", "get", start.elapsed().as_secs_f64());
                        Ok(None)
                    } else {
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_default();
                        span_clone.record("operation.success", false);
                        span_clone.record("error.message", format!("HTTP {}: {}", status, error_text));
                        span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::increment_provider_operation_errors("azure");
                        Err(anyhow::anyhow!(
                            "Failed to get Azure App Configuration key-value: {status} - {error_text}"
                        ))
                    }
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    span_clone.record("operation.success", false);
                    span_clone.record("error.message", format!("HTTP request failed: {}", error_msg));
                    span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                    metrics::increment_provider_operation_errors("azure");
                    Err(anyhow::anyhow!("Failed to get Azure App Configuration key-value: {e}"))
                }
            }
        }
        .instrument(span)
        .await
    }

    async fn delete_config(&self, config_key: &str) -> Result<()> {
        let key_name = self.construct_key_name(config_key);
        let token = get_token(&self.components.credential).await?;

        info!("Deleting Azure App Configuration key: {}", key_name);
        let url = format!("{}/kv/{}", self.components.endpoint, key_name);
        let response = self
            .components
            .http_client
            .delete(&url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .context("Failed to delete Azure App Configuration key-value")?;

        if !response.status().is_success() && response.status() != 404 {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to delete Azure App Configuration key-value: {status} - {error_text}"
            ));
        }

        Ok(())
    }
}
