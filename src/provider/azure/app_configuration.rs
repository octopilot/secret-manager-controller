//! # Azure App Configuration Client
//!
//! Client for interacting with Azure App Configuration REST API.
//!
//! This module provides functionality to:
//! - Create and update key-value pairs in Azure App Configuration
//! - Retrieve configuration values
//! - Support Workload Identity authentication
//!
//! Azure App Configuration is used for storing configuration values (non-secrets)
//! and provides better integration with AKS via Azure App Configuration Kubernetes Provider.

use crate::observability::metrics;
use crate::provider::ConfigStoreProvider;
use crate::{AzureAuthConfig, AzureConfig};
use anyhow::{Context, Result};
use async_trait::async_trait;
use azure_core::credentials::{TokenCredential, TokenRequestOptions};
use azure_identity::{ManagedIdentityCredential, WorkloadIdentityCredential};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, info_span, Instrument};

/// Azure App Configuration provider implementation
pub struct AzureAppConfiguration {
    client: Client,
    endpoint: String,
    credential: Arc<dyn TokenCredential>,
    key_prefix: String,
}

impl std::fmt::Debug for AzureAppConfiguration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzureAppConfiguration")
            .field("endpoint", &self.endpoint)
            .field("key_prefix", &self.key_prefix)
            .finish_non_exhaustive()
    }
}

impl AzureAppConfiguration {
    /// Create a new Azure App Configuration client
    /// Supports Workload Identity authentication
    /// # Errors
    /// Returns an error if Azure client initialization fails
    #[allow(
        clippy::missing_errors_doc,
        clippy::unused_async,
        reason = "Error documentation is provided in doc comments, async signature may be needed for future credential initialization"
    )]
    pub async fn new(
        config: &AzureConfig,
        app_config_endpoint: Option<&str>,
        secret_prefix: &str,
        environment: &str,
        _k8s_client: &kube::Client,
    ) -> Result<Self> {
        // Construct App Configuration endpoint
        // Format: https://{store-name}.azconfig.io
        let endpoint = if let Some(endpoint) = app_config_endpoint {
            endpoint.to_string()
        } else {
            // Auto-detect from vault name (assume same region/resource group)
            // Extract store name from vault name pattern
            // This is a simple heuristic - users should provide endpoint explicitly
            let store_name = config.vault_name.replace("-vault", "-appconfig");
            format!("https://{store_name}.azconfig.io")
        };

        // Ensure endpoint doesn't have trailing slash
        let endpoint = endpoint.trim_end_matches('/').to_string();

        info!("Azure App Configuration endpoint: {}", endpoint);

        // Build credential based on authentication method
        // Only support Workload Identity or Managed Identity (workload identity equivalents)
        // Note: Credential constructors return Arc<dyn TokenCredential>
        let credential: Arc<dyn TokenCredential> = match &config.auth {
            Some(AzureAuthConfig::WorkloadIdentity { client_id }) => {
                info!(
                    "Using Azure Workload Identity authentication with client ID: {}",
                    client_id
                );
                info!("Ensure pod service account has Azure Workload Identity configured");
                let options = azure_identity::WorkloadIdentityCredentialOptions {
                    client_id: Some(client_id.clone()),
                    ..Default::default()
                };
                WorkloadIdentityCredential::new(Some(options))
                    .context("Failed to create WorkloadIdentityCredential")?
            }
            None => {
                // Default to Managed Identity (works in Azure environments like AKS)
                info!("No auth configuration specified, using Managed Identity");
                info!("This works automatically in Azure environments (AKS, App Service, etc.)");
                ManagedIdentityCredential::new(None)
                    .context("Failed to create ManagedIdentityCredential")?
            }
        };

        // Create HTTP client with rustls
        let client = Client::builder()
            .build()
            .context("Failed to create HTTP client")?;

        // Construct key prefix: {prefix}:{environment}:
        // Azure App Configuration uses colon-separated keys
        let key_prefix = format!("{secret_prefix}:{environment}:");

        Ok(Self {
            client,
            endpoint,
            credential,
            key_prefix,
        })
    }

    /// Get access token for Azure App Configuration
    async fn get_token(&self) -> Result<String> {
        // TokenCredential::get_token signature: get_token(scopes: &[&str], options: &TokenRequestOptions)
        let scope = &["https://appconfig.azure.net/.default"];
        // Create default token request options
        // Try using azure_core::TokenRequestOptions or check if there's a default
        let options = Some(TokenRequestOptions::default());
        let token_response = self
            .credential
            .get_token(scope, options)
            .await
            .context("Failed to get Azure App Configuration access token")?;
        Ok(token_response.token.secret().to_string())
    }

    /// Construct full key name from config key
    /// Format: {prefix}:{environment}:{key}
    fn construct_key_name(&self, key: &str) -> String {
        format!("{}{}", self.key_prefix, key)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct KeyValue {
    key: String,
    value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_type: Option<String>,
}

#[async_trait]
impl ConfigStoreProvider for AzureAppConfiguration {
    async fn create_or_update_config(&self, config_key: &str, config_value: &str) -> Result<bool> {
        let key_name = self.construct_key_name(config_key);
        let vault_name = self.endpoint
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
            let token = self.get_token().await?;

            // Check if key exists
            let get_url = format!("{}/kv/{}", self.endpoint, key_name);
            let get_response = self
                .client
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

                let put_url = format!("{}/kv", self.endpoint);
                let response = self
                    .client
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

                let put_url = format!("{}/kv", self.endpoint);
                let response = self
                    .client
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
        let vault_name = self.endpoint
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
            let token = match self.get_token().await {
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

            let url = format!("{}/kv/{}", self.endpoint, key_name);
            match self
                .client
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
        let token = self.get_token().await?;

        info!("Deleting Azure App Configuration key: {}", key_name);
        let url = format!("{}/kv/{}", self.endpoint, key_name);
        let response = self
            .client
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
