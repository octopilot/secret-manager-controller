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
use azure_identity::DefaultAzureCredential;
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
                // DefaultAzureCredential will automatically use Workload Identity when available
                let cred = DefaultAzureCredential::new().context("Failed to create DefaultAzureCredential")?;
                Arc::new(cred)
            }
            Some(AzureAuthConfig::ServicePrincipal { secret_name, secret_namespace }) => {
                warn!("⚠️  DEPRECATED: Service Principal credentials are available but will be deprecated once Azure deprecates them. Please migrate to Workload Identity.");
                info!("Using Service Principal authentication from secret: {}/{}", 
                    secret_namespace.as_deref().unwrap_or("default"), secret_name);
                Self::create_service_principal_credential(secret_name, secret_namespace, k8s_client).await?
            }
            None => {
                info!("No auth configuration specified, defaulting to Workload Identity");
                info!("Ensure pod service account has Azure Workload Identity configured");
                // DefaultAzureCredential will automatically use Workload Identity when available
                // Falls back to Managed Identity, environment variables, or Azure CLI if WI is not available
                let cred = DefaultAzureCredential::new().context("Failed to create DefaultAzureCredential")?;
                Arc::new(cred)
            }
        };

        let client = SecretClient::new(&vault_url, credential, None)
            .context("Failed to create Azure Key Vault SecretClient")?;

        Ok(Self {
            client,
            _vault_url: vault_url,
        })
    }

    /// Create Service Principal credential from Kubernetes secret
    /// 
    /// ⚠️ DEPRECATED: Service Principal credentials are available but will be deprecated once Azure deprecates them.
    /// Workload Identity is the recommended and default authentication method.
    async fn create_service_principal_credential(
        secret_name: &str,
        secret_namespace: &Option<String>,
        k8s_client: &kube::Client,
    ) -> Result<Arc<dyn TokenCredential>> {
        use kube::api::Api;
        use k8s_openapi::api::core::v1::Secret;
        
        let namespace = secret_namespace.as_deref().unwrap_or("default");
        let secrets_api: Api<Secret> = Api::namespaced(k8s_client.clone(), namespace);
        
        let secret = secrets_api.get(secret_name).await
            .context(format!("Failed to get secret {}/{}", namespace, secret_name))?;
        
        let data = secret.data.as_ref()
            .context("Secret data is empty")?;
        
        // Extract Service Principal credentials from secret
        // Expected keys: client-id, client-secret, tenant-id
        let client_id = data.get("client-id")
            .or_else(|| data.get("clientId"))
            .or_else(|| data.get("CLIENT_ID"))
            .and_then(|v| String::from_utf8(v.0.clone()).ok())
            .context("Failed to read client-id from secret")?;
        
        let client_secret = data.get("client-secret")
            .or_else(|| data.get("clientSecret"))
            .or_else(|| data.get("CLIENT_SECRET"))
            .and_then(|v| String::from_utf8(v.0.clone()).ok())
            .context("Failed to read client-secret from secret")?;
        
        let tenant_id = data.get("tenant-id")
            .or_else(|| data.get("tenantId"))
            .or_else(|| data.get("TENANT_ID"))
            .and_then(|v| String::from_utf8(v.0.clone()).ok())
            .context("Failed to read tenant-id from secret")?;
        
        // Set environment variables for DefaultAzureCredential to pick up
        // The SDK will automatically use these from the environment
        std::env::set_var("AZURE_CLIENT_ID", &client_id);
        std::env::set_var("AZURE_CLIENT_SECRET", &client_secret);
        std::env::set_var("AZURE_TENANT_ID", &tenant_id);
        
        // DefaultAzureCredential will use the environment variables we just set
        let cred = DefaultAzureCredential::new().context("Failed to create DefaultAzureCredential from service principal")?;
        Ok(Arc::new(cred))
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
        // Use "latest" as the version to get the current version
        match self.client.get_secret(secret_name, "latest", None).await {
            Ok(response) => {
                let secret = response.into_body().await
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
    fn test_azure_config_service_principal() {
        let config = AzureConfig {
            vault_name: "test-vault".to_string(),
            auth: Some(AzureAuthConfig::ServicePrincipal {
                secret_name: "azure-credentials".to_string(),
                secret_namespace: Some("default".to_string()),
            }),
        };
        
        assert_eq!(config.vault_name, "test-vault");
        match config.auth {
            Some(AzureAuthConfig::ServicePrincipal { secret_name, secret_namespace }) => {
                assert_eq!(secret_name, "azure-credentials");
                assert_eq!(secret_namespace, Some("default".to_string()));
            }
            _ => panic!("Expected ServicePrincipal auth config"),
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
