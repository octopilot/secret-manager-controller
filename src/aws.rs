//! # AWS Secrets Manager Client
//!
//! Client for interacting with AWS Secrets Manager API.
//!
//! This module provides functionality to:
//! - Create and update secrets in AWS Secrets Manager
//! - Retrieve secret values
//! - Support IRSA (IAM Roles for Service Accounts) authentication

use anyhow::{Context, Result};
use async_trait::async_trait;
use aws_config::SdkConfig;
use aws_sdk_secretsmanager::Client as SecretsManagerClient;
use tracing::{debug, info, warn};
use crate::metrics;
use crate::provider::SecretManagerProvider;
use crate::{AwsConfig, AwsAuthConfig};

/// AWS Secrets Manager provider implementation
pub struct AwsSecretManager {
    client: SecretsManagerClient,
    _region: String,
}

impl AwsSecretManager {
    /// Create a new AWS Secrets Manager client
    /// Supports both Access Keys and IRSA (IAM Roles for Service Accounts)
    pub async fn new(config: &AwsConfig, k8s_client: &kube::Client) -> Result<Self> {
        let region = config.region.clone();
        
        // Build AWS SDK config based on authentication method
        // Default to IRSA when auth is not specified
        let sdk_config = match &config.auth {
            Some(AwsAuthConfig::Irsa { role_arn }) => {
                info!("Using IRSA authentication with role: {}", role_arn);
                Self::create_irsa_config(&region, role_arn, k8s_client).await?
            }
            Some(AwsAuthConfig::AccessKeys { secret_name, secret_namespace, access_key_id_key, secret_access_key_key }) => {
                warn!("⚠️  DEPRECATED: Access Keys are available but will be deprecated once AWS deprecates them. Please migrate to IRSA.");
                info!("Using Access Keys authentication from secret: {}/{}", 
                    secret_namespace.as_ref().unwrap_or(&"default".to_string()), secret_name);
                Self::create_access_keys_config(&region, secret_name, secret_namespace, access_key_id_key, secret_access_key_key, k8s_client).await?
            }
            None => {
                info!("No auth configuration specified, defaulting to IRSA (IAM Roles for Service Accounts)");
                info!("Ensure pod service account has annotation: eks.amazonaws.com/role-arn=<role-arn>");
                // Default to IRSA - the AWS SDK will automatically discover the role from the pod's service account
                Self::create_default_config(&region).await?
            }
        };

        let client = SecretsManagerClient::new(&sdk_config);

        Ok(Self {
            client,
            _region: region,
        })
    }

    /// Create AWS SDK config using IRSA (IAM Roles for Service Accounts)
    async fn create_irsa_config(
        region: &str,
        role_arn: &str,
        _k8s_client: &kube::Client,
    ) -> Result<SdkConfig> {
        // IRSA works by:
        // 1. Pod has service account annotation: eks.amazonaws.com/role-arn
        // 2. AWS SDK automatically discovers the role ARN from the pod's service account
        // 3. SDK uses the pod's identity token to assume the role
        
        // For now, we'll use the AWS SDK's default credential chain which supports IRSA
        // The role ARN from the config is informational - the actual role comes from the pod annotation
        info!("IRSA authentication: Ensure pod service account has annotation: eks.amazonaws.com/role-arn={}", role_arn);
        
        let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(region.to_string()))
            .load()
            .await;
        
        Ok(sdk_config)
    }

    /// Create AWS SDK config using Access Keys from Kubernetes secret
    /// Note: This approach sets environment variables temporarily
    /// 
    /// ⚠️ DEPRECATED: Access Keys are available but will be deprecated once AWS deprecates them.
    /// IRSA is the recommended and default authentication method.
    async fn create_access_keys_config(
        region: &str,
        secret_name: &str,
        secret_namespace: &Option<String>,
        access_key_id_key: &str,
        secret_access_key_key: &str,
        k8s_client: &kube::Client,
    ) -> Result<SdkConfig> {
        use kube::api::Api;
        use k8s_openapi::api::core::v1::Secret;
        
        let namespace = secret_namespace.as_deref().unwrap_or("default");
        let secrets_api: Api<Secret> = Api::namespaced(k8s_client.clone(), namespace);
        
        let secret = secrets_api.get(secret_name).await
            .context(format!("Failed to get secret {}/{}", namespace, secret_name))?;
        
        let data = secret.data.as_ref()
            .context("Secret data is empty")?;
        
        let access_key_id = data.get(access_key_id_key)
            .and_then(|v| String::from_utf8(v.0.clone()).ok())
            .context(format!("Failed to read {} from secret", access_key_id_key))?;
        
        let secret_access_key = data.get(secret_access_key_key)
            .and_then(|v| String::from_utf8(v.0.clone()).ok())
            .context(format!("Failed to read {} from secret", secret_access_key_key))?;
        
        // Set environment variables for AWS SDK to pick up
        // The SDK will automatically use these from the environment
        std::env::set_var("AWS_ACCESS_KEY_ID", &access_key_id);
        std::env::set_var("AWS_SECRET_ACCESS_KEY", &secret_access_key);
        
        // Load config from environment (will use the env vars we just set)
        let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(region.to_string()))
            .load()
            .await;
        
        Ok(sdk_config)
    }

    /// Create AWS SDK config using default credential chain
    async fn create_default_config(region: &str) -> Result<SdkConfig> {
        let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(region.to_string()))
            .load()
            .await;
        
        Ok(sdk_config)
    }
}

#[async_trait]
impl SecretManagerProvider for AwsSecretManager {
    async fn create_or_update_secret(
        &self,
        secret_name: &str,
        secret_value: &str,
    ) -> Result<bool> {
        let start = std::time::Instant::now();
        
        // Check if secret exists
        let secret_exists = self.client
            .describe_secret()
            .secret_id(secret_name)
            .send()
            .await
            .is_ok();
        
        if !secret_exists {
            // Create secret
            info!("Creating AWS secret: {}", secret_name);
            self.client
                .create_secret()
                .name(secret_name)
                .secret_string(secret_value)
                .send()
                .await
                .context("Failed to create AWS secret")?;
            
            metrics::record_secret_operation("aws", "create", start.elapsed().as_secs_f64());
            return Ok(true);
        }
        
        // Get current secret value
        let current_value = self.get_secret_value(secret_name).await?;
        
        if let Some(current) = current_value {
            if current == secret_value {
                debug!("AWS secret {} unchanged, skipping update", secret_name);
                metrics::record_secret_operation("aws", "no_change", start.elapsed().as_secs_f64());
                return Ok(false);
            }
        }
        
        // Update secret (creates new version automatically)
        info!("Updating AWS secret: {}", secret_name);
        self.client
            .put_secret_value()
            .secret_id(secret_name)
            .secret_string(secret_value)
            .send()
            .await
            .context("Failed to update AWS secret")?;
        
        metrics::record_secret_operation("aws", "update", start.elapsed().as_secs_f64());
        Ok(true)
    }

    async fn get_secret_value(&self, secret_name: &str) -> Result<Option<String>> {
        match self.client
            .get_secret_value()
            .secret_id(secret_name)
            .send()
            .await
        {
            Ok(response) => {
                let value = if let Some(s) = response.secret_string() {
                    Some(s.to_string())
                } else if let Some(blob) = response.secret_binary() {
                    Some(String::from_utf8_lossy(blob.as_ref()).to_string())
                } else {
                    None
                };
                
                match value {
                    Some(v) => Ok(Some(v)),
                    None => Err(anyhow::anyhow!("Secret has no string or binary value")),
                }
            }
            Err(e) => {
                if e.to_string().contains("ResourceNotFoundException") {
                    Ok(None)
                } else {
                    Err(anyhow::anyhow!("Failed to get AWS secret: {}", e))
                }
            }
        }
    }

    async fn delete_secret(&self, secret_name: &str) -> Result<()> {
        info!("Deleting AWS secret: {}", secret_name);
        self.client
            .delete_secret()
            .secret_id(secret_name)
            .force_delete_without_recovery(true)
            .send()
            .await
            .context("Failed to delete AWS secret")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AwsConfig, AwsAuthConfig};

    #[test]
    fn test_aws_config_irsa() {
        let config = AwsConfig {
            region: "us-east-1".to_string(),
            auth: Some(AwsAuthConfig::Irsa {
                role_arn: "arn:aws:iam::123456789012:role/test-role".to_string(),
            }),
        };
        
        assert_eq!(config.region, "us-east-1");
        match config.auth {
            Some(AwsAuthConfig::Irsa { role_arn }) => {
                assert_eq!(role_arn, "arn:aws:iam::123456789012:role/test-role");
            }
            _ => panic!("Expected IRSA auth config"),
        }
    }

    #[test]
    fn test_aws_config_access_keys() {
        let config = AwsConfig {
            region: "us-west-2".to_string(),
            auth: Some(AwsAuthConfig::AccessKeys {
                secret_name: "aws-credentials".to_string(),
                secret_namespace: Some("default".to_string()),
                access_key_id_key: "access-key-id".to_string(),
                secret_access_key_key: "secret-access-key".to_string(),
            }),
        };
        
        assert_eq!(config.region, "us-west-2");
        match config.auth {
            Some(AwsAuthConfig::AccessKeys { secret_name, secret_namespace, access_key_id_key, secret_access_key_key }) => {
                assert_eq!(secret_name, "aws-credentials");
                assert_eq!(secret_namespace, Some("default".to_string()));
                assert_eq!(access_key_id_key, "access-key-id");
                assert_eq!(secret_access_key_key, "secret-access-key");
            }
            _ => panic!("Expected AccessKeys auth config"),
        }
    }

    #[test]
    fn test_aws_config_default() {
        let config = AwsConfig {
            region: "eu-west-1".to_string(),
            auth: None,
        };
        
        assert_eq!(config.region, "eu-west-1");
        assert!(config.auth.is_none());
    }

    #[test]
    fn test_aws_secret_name_validation() {
        // AWS Secrets Manager secret names must be 1-512 characters
        // Can contain letters, numbers, / _ + = . @ -
        let valid_names = vec![
            "my-secret",
            "my/secret/path",
            "my_secret_123",
            "my+secret=test",
            "my.secret@test",
        ];
        
        for name in valid_names {
            assert!(name.len() >= 1 && name.len() <= 512, "Secret name {} should be valid", name);
        }
    }
}
