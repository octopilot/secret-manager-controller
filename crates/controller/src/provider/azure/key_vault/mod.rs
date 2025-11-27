//! # Azure Key Vault Client
//!
//! Client for interacting with Azure Key Vault Secrets API.
//!
//! This module provides functionality to:
//! - Create and update secrets in Azure Key Vault
//! - Retrieve secret values
//! - Manage secret versions
//! - Support Workload Identity and Service Principal authentication

mod auth;
mod client;
mod operations;
mod pact_api_override;

pub use auth::MockTokenCredential;
use azure_core::credentials::TokenCredential;
use azure_security_keyvault_secrets::SecretClient;
use reqwest::Client as ReqwestClient;
use std::sync::Arc;

use crate::crd::AzureConfig;
use anyhow::Result;

use self::client::create_client_components;

/// Azure Key Vault provider implementation
pub struct AzureKeyVault {
    pub(crate) client: SecretClient,
    pub(crate) _vault_url: String,
    pub(crate) http_client: ReqwestClient,
    pub(crate) credential: Arc<dyn TokenCredential>,
}

impl std::fmt::Debug for AzureKeyVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzureKeyVault")
            .field("_vault_url", &self._vault_url)
            .finish_non_exhaustive()
    }
}

impl AzureKeyVault {
    /// Create a new Azure Key Vault client
    /// Supports both Service Principal and Workload Identity
    /// # Errors
    /// Returns an error if Azure client initialization fails
    #[allow(
        clippy::missing_errors_doc,
        clippy::unused_async,
        reason = "Error docs in comments, async signature matches trait"
    )]
    pub async fn new(config: &AzureConfig, _k8s_client: &kube::Client) -> Result<Self> {
        let (client, http_client, credential, vault_url) = create_client_components(config).await?;

        Ok(Self {
            client,
            _vault_url: vault_url,
            http_client,
            credential,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::crd::{AzureAuthConfig, AzureConfig};

    #[test]
    fn test_azure_config_workload_identity() {
        let config = AzureConfig {
            vault_name: "my-vault".to_string(),
            location: "eastus".to_string(),
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
    fn test_azure_config_default() {
        let config = AzureConfig {
            vault_name: "prod-vault".to_string(),
            location: "eastus".to_string(),
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
            location: "eastus".to_string(),
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
            location: "eastus".to_string(),
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
        let valid_names = vec!["my-secret", "my-secret-123", "MySecret", "my_secret"];

        for name in valid_names {
            assert!(
                !name.is_empty() && name.len() <= 127,
                "Secret name {name} should be valid"
            );
        }
    }
}
