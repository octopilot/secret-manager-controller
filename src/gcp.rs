//! # GCP Secret Manager Client
//!
//! Client for interacting with Google Cloud Secret Manager API.
//!
//! This module provides functionality to:
//! - Create and update secrets in GCP Secret Manager
//! - Retrieve secret values
//! - Manage secret versions
//!
//! Uses the official [`google-cloud-secretmanager-v1`] SDK for Rust.

use anyhow::{Context, Result};
use async_trait::async_trait;
use google_cloud_secretmanager_v1::client::SecretManagerService;
use tracing::{info, warn};
use crate::metrics;
use crate::provider::SecretManagerProvider;
use base64::{Engine as _, engine::general_purpose};

pub struct SecretManager {
    client: SecretManagerService,
    project_id: String,
}

impl SecretManager {
    /// Create a new SecretManager client with authentication
    /// Supports both JSON credentials and Workload Identity
    /// 
    /// Authentication is handled automatically by the Google Cloud SDK:
    /// - Workload Identity: Uses Application Default Credentials (ADC) when running in GKE
    ///   with Workload Identity enabled and service account annotation
    /// - JSON Credentials: Uses GOOGLE_APPLICATION_CREDENTIALS environment variable
    ///   pointing to mounted secret file
    /// 
    /// The auth_type parameter indicates which authentication method to use:
    /// - "WorkloadIdentity": Uses Workload Identity (DEFAULT, requires GKE with WI enabled)
    /// - "JsonCredentials": Uses JSON credentials from GOOGLE_APPLICATION_CREDENTIALS (DEPRECATED)
    /// - None: Defaults to Workload Identity
    pub async fn new(_project_id: String, auth_type: Option<&str>, service_account_email: Option<&str>) -> Result<Self> {
        match auth_type {
            Some("WorkloadIdentity") | None => {
                if let Some(email) = service_account_email {
                    info!("Using Workload Identity authentication with service account: {}", email);
                    info!("Ensure service account annotation is set: iam.gke.io/gcp-service-account={}", email);
                } else {
                    info!("Using Workload Identity authentication (service account from pod annotation)");
                }
            }
            Some("JsonCredentials") => {
                warn!("⚠️  DEPRECATED: JSON credentials are available but will be deprecated once GCP deprecates them. Please migrate to Workload Identity.");
                info!("Using JSON credentials from GOOGLE_APPLICATION_CREDENTIALS");
                info!("Ensure GOOGLE_APPLICATION_CREDENTIALS points to mounted secret file");
            }
            _ => {
                info!("No auth type specified, defaulting to Workload Identity");
            }
        }

        // Create client - SecretManagerService should have a constructor
        // For now, we'll use a placeholder that needs to be fixed with actual API
        // The client creation depends on the actual SDK API
        // The google-cloud-auth crate will automatically detect Workload Identity or JSON credentials
        // from the environment (GOOGLE_APPLICATION_CREDENTIALS or metadata server)
        // TODO: Implement actual client creation when SDK API is available
        return Err(anyhow::anyhow!(
            "SecretManagerService client creation needs to be implemented with correct SDK API. \
            Please check google-cloud-secretmanager-v1 documentation for client initialization."
        ));
        
        // Placeholder return (unreachable)
        // Ok(Self {
        //     client: SecretManagerService::new().await?,
        //     project_id,
        // })
    }

    /// Create or update secret, ensuring Git is source of truth
    /// If secret exists and value differs, creates new version and disables old versions
    async fn create_or_update_secret_impl(
        &self,
        _secret_name: &str,
        _secret_value: &str,
    ) -> Result<bool> {
        // Placeholder - needs proper SDK implementation
        // TODO: Implement when SDK API is available
        Err(anyhow::anyhow!("GCP Secret Manager not yet implemented - waiting for correct SDK API"))
    }

    /// Get the latest secret version value
    #[allow(dead_code)] // May be used in future implementations
    async fn get_latest_secret_value(&self, _secret_name: &str) -> Result<String> {
        Err(anyhow::anyhow!("Not implemented - waiting for correct SDK API"))
    }

    #[allow(dead_code)] // May be used in future implementations
    async fn get_secret(&self, _secret_name: &str) -> Result<()> {
        Err(anyhow::anyhow!("Not implemented - waiting for correct SDK API"))
    }

    #[allow(dead_code)] // May be used in future implementations
    async fn create_secret(&self, _project_id: &str, _secret_name: &str) -> Result<()> {
        Err(anyhow::anyhow!("Not implemented - waiting for correct SDK API"))
    }

    #[allow(dead_code)] // May be used in future implementations
    async fn add_secret_version(
        &self,
        _secret_name: &str,
        _secret_value: &str,
    ) -> Result<()> {
        Err(anyhow::anyhow!("Not implemented - waiting for correct SDK API"))
    }
}

#[async_trait]
impl SecretManagerProvider for SecretManager {
    async fn create_or_update_secret(
        &self,
        secret_name: &str,
        secret_value: &str,
    ) -> Result<bool> {
        let start = std::time::Instant::now();
        
        // TODO: Implement actual GCP Secret Manager API calls when SDK is available
        // For now, return error indicating not implemented
        let result = self.create_or_update_secret_impl(secret_name, secret_value).await;
        
        if let Ok(was_updated) = &result {
            if *was_updated {
                metrics::record_secret_operation("gcp", "update", start.elapsed().as_secs_f64());
            } else {
                metrics::record_secret_operation("gcp", "no_change", start.elapsed().as_secs_f64());
            }
        }
        
        result
    }

    async fn get_secret_value(&self, secret_name: &str) -> Result<Option<String>> {
        use google_cloud_secretmanager_v1::model::AccessSecretVersionRequest;
        
        // Construct the secret version name: projects/{project}/secrets/{secret}/versions/latest
        let secret_version_name = format!("projects/{}/secrets/{}/versions/latest", self.project_id, secret_name);
        
        let request = AccessSecretVersionRequest::default();
        let request_for_send = request.clone().set_name(secret_version_name.clone()); // set_name returns Self
        
        match self.client
            .access_secret_version()
            .with_request(request_for_send)
            .send()
            .await
        {
            Ok(response) => {
                // Extract the secret value from the response
                // The payload.data field contains base64-encoded secret data (as bytes)
                if let Some(payload) = response.payload {
                    // payload.data is bytes::Bytes, decode from base64
                    let data = payload.data.as_ref();
                    if data.is_empty() {
                        return Err(anyhow::anyhow!("Secret version has no payload data"));
                    }
                    
                    // Decode base64 to get the actual secret value
                    let decoded = general_purpose::STANDARD
                        .decode(data)
                        .context("Failed to decode base64 secret data")?;
                    let secret_value = String::from_utf8(decoded)
                        .context("Secret value is not valid UTF-8")?;
                    Ok(Some(secret_value))
                } else {
                    Err(anyhow::anyhow!("Secret version response has no payload"))
                }
            }
            Err(e) => {
                // Check if it's a "not found" error (404)
                let error_msg = e.to_string();
                if error_msg.contains("404") || 
                   error_msg.contains("NOT_FOUND") || 
                   error_msg.contains("not found") ||
                   (error_msg.contains("Secret") && error_msg.contains("not found")) {
                    Ok(None)
                } else {
                    Err(anyhow::anyhow!("Failed to get GCP secret {}: {}", secret_name, e))
                }
            }
        }
    }

    async fn delete_secret(&self, secret_name: &str) -> Result<()> {
        use google_cloud_secretmanager_v1::model::DeleteSecretRequest;
        
        info!("Deleting GCP secret: {}", secret_name);
        
        // Construct the secret name: projects/{project}/secrets/{secret}
        let secret_name_full = format!("projects/{}/secrets/{}", self.project_id, secret_name);
        
        let request = DeleteSecretRequest::default();
        let request_for_send = request.clone().set_name(secret_name_full.clone()); // set_name returns Self
        
        self.client
            .delete_secret()
            .with_request(request_for_send)
            .send()
            .await
            .map(|_| ())
            .context(format!("Failed to delete GCP secret: {}", secret_name))?;
        
        Ok(())
    }
}

// Alias for consistency
pub type SecretManagerClient = SecretManager;
