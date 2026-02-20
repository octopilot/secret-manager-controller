//! # Provider Creation
//!
//! Handles creation of cloud provider clients (GCP, AWS, Azure).

use crate::controller::reconciler::types::{Reconciler, ReconcilerError};
use crate::crd::{ProviderConfig, SecretManagerConfig};
use crate::provider::SecretManagerProvider;
use crate::provider::aws::AwsSecretManager;
use crate::provider::azure::AzureKeyVault;
use crate::provider::gcp::create_gcp_provider;
use anyhow::Context;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Create provider client based on provider configuration
///
/// Each provider has different authentication methods:
/// - GCP: Workload Identity (default)
/// - AWS: IRSA - IAM Roles for Service Accounts (default)
/// - Azure: Workload Identity or Managed Identity (default)
/// Provider is created per-reconciliation to support per-resource auth config
pub async fn create_provider(
    config: &Arc<SecretManagerConfig>,
    ctx: &Arc<Reconciler>,
) -> Result<Box<dyn SecretManagerProvider>, ReconcilerError> {
    let name = config.metadata.name.as_deref().unwrap_or("unknown");

    let provider: Box<dyn SecretManagerProvider> = match &config.spec.provider {
        ProviderConfig::Gcp(gcp_config) => {
            // GCP Secret Manager provider
            // Validate required GCP configuration
            if gcp_config.project_id.is_empty() {
                let err = anyhow::anyhow!("GCP projectId is required but is empty");
                error!("Validation error for {}: {}", name, err);
                return Err(ReconcilerError::ReconciliationFailed(err));
            }

            // Determine authentication method from config
            // Default to Workload Identity when auth is not specified
            // Workload Identity requires GKE with WI enabled and service account annotation
            let (auth_type, service_account_email_owned) = if let Some(ref auth_config) =
                gcp_config.auth
            {
                match serde_json::to_value(auth_config)
                    .context("Failed to serialize gcpAuth config")
                {
                    Ok(auth_json) => {
                        let auth_type_str = auth_json.get("authType").and_then(|t| t.as_str());
                        if let Some("WorkloadIdentity") = auth_type_str {
                            match auth_json
                                .get("serviceAccountEmail")
                                .and_then(|e| e.as_str())
                            {
                                Some(email) => (Some("WorkloadIdentity"), Some(email.to_string())),
                                None => {
                                    warn!(
                                        "WorkloadIdentity specified but serviceAccountEmail is missing, using default"
                                    );
                                    (Some("WorkloadIdentity"), None)
                                }
                            }
                        } else {
                            // Default to Workload Identity
                            info!("No auth type specified, defaulting to Workload Identity");
                            (Some("WorkloadIdentity"), None)
                        }
                    }
                    Err(e) => {
                        error!("Failed to serialize GCP auth config: {}", e);
                        return Err(ReconcilerError::ReconciliationFailed(e));
                    }
                }
            } else {
                // Default to Workload Identity when auth is not specified
                info!("No auth configuration specified, defaulting to Workload Identity");
                (Some("WorkloadIdentity"), None)
            };

            let service_account_email = service_account_email_owned.as_deref();
            match create_gcp_provider(
                gcp_config.project_id.clone(),
                auth_type,
                service_account_email,
            )
            .await
            {
                Ok(gcp_client) => gcp_client,
                Err(e) => {
                    error!("Failed to create GCP Secret Manager client: {}", e);
                    return Err(ReconcilerError::ReconciliationFailed(e));
                }
            }
        }
        ProviderConfig::Aws(aws_config) => {
            match AwsSecretManager::new(aws_config, &ctx.client).await {
                Ok(aws_provider) => Box::new(aws_provider),
                Err(e) => {
                    error!("Failed to create AWS Secrets Manager client: {}", e);
                    return Err(ReconcilerError::ReconciliationFailed(
                        e.context("Failed to create AWS Secrets Manager client"),
                    ));
                }
            }
        }
        ProviderConfig::Azure(azure_config) => {
            match AzureKeyVault::new(azure_config, &ctx.client).await {
                Ok(azure_provider) => Box::new(azure_provider),
                Err(e) => {
                    error!("Failed to create Azure Key Vault client: {}", e);
                    return Err(ReconcilerError::ReconciliationFailed(
                        e.context("Failed to create Azure Key Vault client"),
                    ));
                }
            }
        }
    };

    Ok(provider)
}
