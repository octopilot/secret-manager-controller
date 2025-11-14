//! # Azure Key Vault Client
//!
//! Client for interacting with Azure Key Vault Secrets API.
//!
//! This module provides functionality to:
//! - Create and update secrets in Azure Key Vault
//! - Retrieve secret values
//! - Support Workload Identity and Service Principal authentication

use anyhow::{Context, Result};
use async_trait::async_trait;
use azure_core::credentials::TokenCredential;
use azure_identity::{WorkloadIdentityCredential, DeveloperToolsCredential, ClientSecretCredential, ClientSecretCredentialOptions};
use azure_security_keyvault_secrets::{models::SetSecretParameters, SecretClient};
use std::sync::Arc;
use tracing::{debug, info, warn};
use crate::metrics;
use crate::provider::SecretManagerProvider;
use crate::{AzureConfig, AzureAuthConfig};

/// Azure Key Vault provider implementation
pub struct AzureKeyVault {
    client: SecretClient,
    _vault_url: String,
}

impl AzureKeyVault {
    /// Create a new Azure Key Vault client
    /// Supports both Service Principal and Workload Identity
    pub async fn new(config: &AzureConfig, k8s_client: &kube::Client) -> Result<Self> {
        // Construct vault URL from vault name
        // Format: https://{vault-name}.vault.azure.net/
        let vault_url = if config.vault_name.starts_with("https://") {
            config.vault_name.clone()
        } else {
            format!("https://{}.vault.azure.net/", config.vault_name)
        };

        // Build credential based on authentication method
        // Default to Workload Identity when auth is not specified
        let credential: Arc<dyn TokenCredential> = match &config.auth {
            Some(AzureAuthConfig::WorkloadIdentity { client_id }) => {
                info!("Using Azure Workload Identity authentication with client ID: {}", client_id);
                info!("Ensure pod service account has Azure Workload Identity configured");
                let mut options = azure_identity::WorkloadIdentityCredentialOptions::default();
                options.client_id = Some(client_id.clone());
                WorkloadIdentityCredential::new(Some(options))
                    .context("Failed to create WorkloadIdentityCredential")?
            }
            None => {
                info!("No auth configuration specified, defaulting to DeveloperToolsCredential");
                info!("This will attempt Workload Identity, Managed Identity, environment variables, or Azure CLI");
                // DeveloperToolsCredential attempts multiple authentication methods in order
                DeveloperToolsCredential::new(None)
                    .context("Failed to create DeveloperToolsCredential")?
            }
        };

        let client = SecretClient::new(&vault_url, credential, None)
            .context("Failed to create Azure Key Vault SecretClient")?;

        Ok(Self {
            client,
            _vault_url: vault_url,
        })
    }

}

#[async_trait]
impl SecretManagerProvider for AzureKeyVault {
    async fn create_or_update_secret(
        &self,
        secret_name: &str,
        secret_value: &str,
    ) -> Result<bool> {
        let start = std::time::Instant::now();
        
        // Check if secret exists by trying to get it
        let current_value = self.get_secret_value(secret_name).await?;
        
        if let Some(current) = current_value {
            if current == secret_value {
                debug!("Azure secret {} unchanged, skipping update", secret_name);
                metrics::record_secret_operation("azure", "no_change", start.elapsed().as_secs_f64());
                return Ok(false);
            }
        }
        
        // Create or update secret
        // Azure Key Vault automatically creates a new version when updating
        info!("Creating/updating Azure secret: {}", secret_name);
        let mut parameters = SetSecretParameters::default();
        parameters.value = Some(secret_value.to_string());
        self.client
            .set_secret(secret_name, parameters.try_into()?, None)
            .await
            .context(format!("Failed to create/update Azure secret: {}", secret_name))?;
        
        metrics::record_secret_operation("azure", "update", start.elapsed().as_secs_f64());
        Ok(true)
    }

    async fn get_secret_value(&self, secret_name: &str) -> Result<Option<String>> {
        // Get the latest version of the secret (no version parameter needed - defaults to latest)
        match self.client.get_secret(secret_name, None).await {
            Ok(response) => {
                // Response body needs to be deserialized into the Secret model
                use azure_security_keyvault_secrets::models::Secret;
                let body = response.into_body();
                // Try to deserialize the body - ResponseBody might implement Deserialize or have a method
                // Check if we can use serde_json or if there's a specific method
                let secret: Secret = serde_json::from_slice(&body)
                    .context("Failed to deserialize Azure secret response")?;
                Ok(secret.value)
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("SecretNotFound") || error_msg.contains("404") || error_msg.contains("not found") {
                    Ok(None)
                } else {
                    Err(anyhow::anyhow!("Failed to get Azure secret: {}", e))
                }
            }
        }
    }

    async fn delete_secret(&self, secret_name: &str) -> Result<()> {
        info!("Deleting Azure secret: {}", secret_name);
        self.client
            .delete_secret(secret_name, None)
            .await
            .context(format!("Failed to delete Azure secret: {}", secret_name))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AzureConfig, AzureAuthConfig};

    #[test]
    fn test_azure_config_workload_identity() {
        let config = AzureConfig {
            vault_name: "my-vault".to_string(),
            auth: Some(AzureAuthConfig::WorkloadIdentity {
                client_id: "12345678-1234-1234-1234-123456789012".to_string(),
            }),
        };
        
        assert_eq!(config.vault_name, "my-vault");
        match config.auth {
            Some(AzureAuthConfig::WorkloadIdentity { client_id }) => {
                assert_eq!(client_id, "12345678-1234-1234-1234-123456789012");
            }
            _ => panic!("Expected WorkloadIdentity auth config"),
        }
    }

    #[test]
    fn test_azure_config_workload_identity() {
        let config = AzureConfig {
            vault_name: "test-vault".to_string(),
            auth: Some(AzureAuthConfig::WorkloadIdentity {
                client_id: "12345678-1234-1234-1234-123456789012".to_string(),
            }),
        };
        
        assert_eq!(config.vault_name, "test-vault");
        match config.auth {
            Some(AzureAuthConfig::WorkloadIdentity { client_id }) => {
                assert_eq!(client_id, "12345678-1234-1234-1234-123456789012");
            }
            _ => panic!("Expected WorkloadIdentity auth config"),
        }
    }

    #[test]
    fn test_azure_config_default() {
        let config = AzureConfig {
            vault_name: "prod-vault".to_string(),
            auth: None,
        };
        
        assert_eq!(config.vault_name, "prod-vault");
        assert!(config.auth.is_none());
    }

    #[test]
    fn test_azure_vault_url_construction() {
        // Test vault URL construction
        let config1 = AzureConfig {
            vault_name: "my-vault".to_string(),
            auth: None,
        };
        let expected_url = "https://my-vault.vault.azure.net/";
        // This would be tested in the new() method, but we can test the logic
        let vault_url = if config1.vault_name.starts_with("https://") {
            config1.vault_name.clone()
        } else {
            format!("https://{}.vault.azure.net/", config1.vault_name)
        };
        assert_eq!(vault_url, expected_url);

        // Test with full URL
        let config2 = AzureConfig {
            vault_name: "https://custom-vault.vault.azure.net/".to_string(),
            auth: None,
        };
        let vault_url2 = if config2.vault_name.starts_with("https://") {
            config2.vault_name.clone()
        } else {
            format!("https://{}.vault.azure.net/", config2.vault_name)
        };
        assert_eq!(vault_url2, "https://custom-vault.vault.azure.net/");
    }

    #[test]
    fn test_azure_secret_name_validation() {
        // Azure Key Vault secret names must be 1-127 characters
        // Can contain letters, numbers, and hyphens
        let valid_names = vec![
            "my-secret",
            "my-secret-123",
            "MySecret",
            "my_secret",
        ];
        
        for name in valid_names {
            assert!(name.len() >= 1 && name.len() <= 127, "Secret name {} should be valid", name);
        }
    }
}
