//! # Azure Key Vault Operations
//!
//! Implements SecretManagerProvider trait for Azure Key Vault.

use crate::observability::metrics;
use crate::provider::SecretManagerProvider;
use anyhow::{Context, Result};
use async_trait::async_trait;
use azure_core::credentials::TokenRequestOptions;
use azure_security_keyvault_secrets::models::SetSecretParameters;
use serde_json::json;
use std::time::Instant;
use tracing::{Instrument, debug, info, info_span};

use super::AzureKeyVault;

#[async_trait]
impl SecretManagerProvider for AzureKeyVault {
    async fn create_or_update_secret(
        &self,
        secret_name: &str,
        secret_value: &str,
        environment: &str,
        location: &str,
    ) -> Result<bool> {
        let vault_name = self
            ._vault_url
            .strip_prefix("https://")
            .and_then(|s| s.strip_suffix(".vault.azure.net/"))
            .unwrap_or("unknown");
        let span = info_span!(
            "azure.keyvault.secret.create_or_update",
            secret.name = secret_name,
            vault.name = vault_name
        );
        let span_clone = span.clone();
        let start = Instant::now();
        let secret_value_clone = secret_value.to_string();

        async move {
            // Check if secret exists by trying to get it
            let current_value = self.get_secret_value(secret_name).await?;

            let vault_name = self
                ._vault_url
                .strip_prefix("https://")
                .and_then(|s| s.strip_suffix(".vault.azure.net/"))
                .unwrap_or("unknown");

            let operation_type = if let Some(current) = current_value {
                if current == secret_value_clone {
                    debug!(
                        provider = "azure",
                        vault_name = vault_name,
                        secret_name = secret_name,
                        operation = "no_change",
                        "Azure secret {} unchanged, skipping update",
                        secret_name
                    );
                    metrics::record_secret_operation(
                        "azure",
                        "no_change",
                        start.elapsed().as_secs_f64(),
                    );
                    span_clone.record("operation.type", "no_change");
                    span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                    span_clone.record("operation.success", true);
                    return Ok(false);
                }
                "update"
            } else {
                "create"
            };

            // Create or update secret
            // Azure Key Vault automatically creates a new version when updating
            info!(
                provider = "azure",
                vault_name = vault_name,
                secret_name = secret_name,
                operation = operation_type,
                "Creating/updating Azure secret: vault={}, secret={}, operation={}",
                vault_name,
                secret_name,
                operation_type
            );
            // Build tags with environment and location
            let mut tags = std::collections::HashMap::new();
            tags.insert("environment".to_string(), environment.to_string());
            tags.insert("location".to_string(), location.to_string());

            let parameters = SetSecretParameters {
                value: Some(secret_value_clone),
                tags: Some(tags),
                ..Default::default()
            };
            match self
                .client
                .set_secret(secret_name, parameters.try_into()?, None)
                .await
            {
                Ok(_) => {
                    metrics::record_secret_operation(
                        "azure",
                        operation_type,
                        start.elapsed().as_secs_f64(),
                    );
                    span_clone.record("operation.type", operation_type);
                    span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                    span_clone.record("operation.success", true);
                    Ok(true)
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    span_clone.record("operation.success", false);
                    span_clone.record("operation.type", operation_type);
                    span_clone.record("error.message", error_msg.clone());
                    span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                    metrics::increment_provider_operation_errors("azure");
                    Err(anyhow::anyhow!(
                        "Failed to create/update Azure secret {secret_name}: {e}"
                    ))
                }
            }
        }
        .instrument(span)
        .await
    }

    async fn get_secret_value(&self, secret_name: &str) -> Result<Option<String>> {
        let vault_name = self
            ._vault_url
            .strip_prefix("https://")
            .and_then(|s| s.strip_suffix(".vault.azure.net/"))
            .unwrap_or("unknown");
        let span = tracing::debug_span!(
            "azure.keyvault.secret.get",
            secret.name = secret_name,
            vault.name = vault_name
        );
        let span_clone = span.clone();
        let start = Instant::now();

        async move {
            // Get the latest version of the secret (no version parameter needed - defaults to latest)
            match self.client.get_secret(secret_name, None).await {
                Ok(response) => {
                    // Response body needs to be deserialized into the Secret model
                    use azure_security_keyvault_secrets::models::Secret;
                    match serde_json::from_slice::<Secret>(&response.into_body()) {
                        Ok(secret) => {
                            span_clone.record("operation.success", true);
                            span_clone.record("operation.found", secret.value.is_some());
                            span_clone.record(
                                "operation.duration_ms",
                                start.elapsed().as_millis() as u64,
                            );
                            metrics::record_secret_operation(
                                "azure",
                                "get",
                                start.elapsed().as_secs_f64(),
                            );
                            Ok(secret.value)
                        }
                        Err(e) => {
                            let error_msg = e.to_string();
                            span_clone.record("operation.success", false);
                            span_clone.record(
                                "error.message",
                                format!(
                                    "Failed to deserialize Azure secret response: {}",
                                    error_msg
                                ),
                            );
                            span_clone.record(
                                "operation.duration_ms",
                                start.elapsed().as_millis() as u64,
                            );
                            metrics::increment_provider_operation_errors("azure");
                            Err(anyhow::anyhow!(
                                "Failed to deserialize Azure secret response: {e}"
                            ))
                        }
                    }
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    // Treat missing, disabled, or not found secrets as Ok(None) so controller can create them
                    if error_msg.contains("SecretNotFound")
                        || error_msg.contains("404")
                        || error_msg.contains("not found")
                        || error_msg.contains("disabled")
                        || error_msg.contains("is disabled")
                    {
                        span_clone.record("operation.success", true);
                        span_clone.record("operation.found", false);
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::record_secret_operation(
                            "azure",
                            "get",
                            start.elapsed().as_secs_f64(),
                        );
                        Ok(None)
                    } else {
                        span_clone.record("operation.success", false);
                        span_clone.record("error.message", error_msg.clone());
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::increment_provider_operation_errors("azure");
                        Err(anyhow::anyhow!("Failed to get Azure secret: {e}"))
                    }
                }
            }
        }
        .instrument(span)
        .await
    }

    async fn delete_secret(&self, secret_name: &str) -> Result<()> {
        info!("Deleting Azure secret: {}", secret_name);
        self.client
            .delete_secret(secret_name, None)
            .await
            .context(format!("Failed to delete Azure secret: {secret_name}"))?;
        Ok(())
    }

    async fn disable_secret(&self, secret_name: &str) -> Result<bool> {
        info!("Disabling Azure secret: {}", secret_name);

        // Use REST API directly: PATCH /secrets/{name} with attributes.enabled=false
        // Azure Key Vault REST API: https://learn.microsoft.com/en-us/rest/api/keyvault/secrets/update-secret/update-secret

        // Get access token
        let scope = &["https://vault.azure.net/.default"];
        let options = Some(TokenRequestOptions::default());
        let token_response = self
            .credential
            .get_token(scope, options)
            .await
            .context("Failed to get Azure Key Vault access token")?;
        let token = token_response.token.secret().to_string();

        // Construct URL: PATCH {vault_url}/secrets/{name}?api-version=7.4
        let url = format!("{}secrets/{}?api-version=7.4", self._vault_url, secret_name);

        // Request body: { "attributes": { "enabled": false } }
        let body = json!({
            "attributes": {
                "enabled": false
            }
        });

        let response = self
            .http_client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to disable Azure secret")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            // If secret doesn't exist, return false (not an error)
            if status == 404 {
                debug!("Secret {} does not exist, cannot disable", secret_name);
                return Ok(false);
            }

            return Err(anyhow::anyhow!(
                "Failed to disable Azure secret {}: HTTP {} - {}",
                secret_name,
                status,
                error_text
            ));
        }

        Ok(true)
    }

    async fn enable_secret(&self, secret_name: &str) -> Result<bool> {
        info!("Enabling Azure secret: {}", secret_name);

        // Use REST API directly: PATCH /secrets/{name} with attributes.enabled=true
        // Azure Key Vault REST API: https://learn.microsoft.com/en-us/rest/api/keyvault/secrets/update-secret/update-secret

        // Get access token
        let scope = &["https://vault.azure.net/.default"];
        let options = Some(TokenRequestOptions::default());
        let token_response = self
            .credential
            .get_token(scope, options)
            .await
            .context("Failed to get Azure Key Vault access token")?;
        let token = token_response.token.secret().to_string();

        // Construct URL: PATCH {vault_url}/secrets/{name}?api-version=7.4
        let url = format!("{}secrets/{}?api-version=7.4", self._vault_url, secret_name);

        // Request body: { "attributes": { "enabled": true } }
        let body = json!({
            "attributes": {
                "enabled": true
            }
        });

        let response = self
            .http_client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to enable Azure secret")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            // If secret doesn't exist, return false (not an error)
            if status == 404 {
                debug!("Secret {} does not exist, cannot enable", secret_name);
                return Ok(false);
            }

            return Err(anyhow::anyhow!(
                "Failed to enable Azure secret {}: HTTP {} - {}",
                secret_name,
                status,
                error_text
            ));
        }

        Ok(true)
    }
}
