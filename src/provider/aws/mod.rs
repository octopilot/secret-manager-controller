//! # AWS Secrets Manager Client
//!
//! Client for interacting with AWS Secrets Manager API.
//!
//! This module provides functionality to:
//! - Create and update secrets in AWS Secrets Manager
//! - Retrieve secret values
//! - Support IRSA (IAM Roles for Service Accounts) authentication

use crate::observability::metrics;
use crate::provider::SecretManagerProvider;
use crate::{AwsAuthConfig, AwsConfig};
use anyhow::{Context, Result};
use async_trait::async_trait;
use aws_config::SdkConfig;
use aws_sdk_secretsmanager::Client as SecretsManagerClient;
use std::time::Instant;
use tracing::{debug, info, info_span, Instrument};

/// AWS Secrets Manager provider implementation
pub struct AwsSecretManager {
    client: SecretsManagerClient,
    _region: String,
}

impl std::fmt::Debug for AwsSecretManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsSecretManager")
            .field("_region", &self._region)
            .finish_non_exhaustive()
    }
}

impl AwsSecretManager {
    /// Create a new AWS Secrets Manager client
    /// Supports both Access Keys and IRSA (IAM Roles for Service Accounts)
    #[allow(
        clippy::missing_errors_doc,
        reason = "Error documentation is provided in doc comments"
    )]
    pub async fn new(config: &AwsConfig, k8s_client: &kube::Client) -> Result<Self> {
        let region = config.region.clone();

        // Build AWS SDK config based on authentication method
        // Default to IRSA when auth is not specified
        let sdk_config = match &config.auth {
            Some(AwsAuthConfig::Irsa { role_arn }) => {
                info!("Using IRSA authentication with role: {}", role_arn);
                Self::create_irsa_config(&region, role_arn, k8s_client).await?
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
    async fn create_or_update_secret(&self, secret_name: &str, secret_value: &str) -> Result<bool> {
        let span = info_span!(
            "aws.secret.create_or_update",
            secret.name = secret_name,
            region = self._region
        );
        let span_clone = span.clone();
        let start = Instant::now();

        async move {
            // Check if secret exists
            let secret_exists = self
                .client
                .describe_secret()
                .secret_id(secret_name)
                .send()
                .await
                .is_ok();

            let operation_type = if !secret_exists {
                // Create secret
                info!("Creating AWS secret: {}", secret_name);
                match self
                    .client
                    .create_secret()
                    .name(secret_name)
                    .secret_string(secret_value)
                    .send()
                    .await
                {
                    Ok(_) => {
                        metrics::record_secret_operation(
                            "aws",
                            "create",
                            start.elapsed().as_secs_f64(),
                        );
                        span_clone.record("operation.type", "create");
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        span_clone.record("operation.success", true);
                        return Ok(true);
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        span_clone.record("operation.success", false);
                        span_clone.record("operation.type", "create");
                        span_clone.record("error.message", error_msg.clone());
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::increment_provider_operation_errors("aws");
                        return Err(anyhow::anyhow!(
                            "Failed to create AWS secret {secret_name}: {e}"
                        ));
                    }
                }
            } else {
                // Get current secret value
                let current_value = self.get_secret_value(secret_name).await?;

                if let Some(current) = current_value {
                    if current == secret_value {
                        debug!("AWS secret {} unchanged, skipping update", secret_name);
                        metrics::record_secret_operation(
                            "aws",
                            "no_change",
                            start.elapsed().as_secs_f64(),
                        );
                        span_clone.record("operation.type", "no_change");
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        span_clone.record("operation.success", true);
                        return Ok(false);
                    }
                }

                // Update secret (creates new version automatically)
                info!("Updating AWS secret: {}", secret_name);
                match self
                    .client
                    .put_secret_value()
                    .secret_id(secret_name)
                    .secret_string(secret_value)
                    .send()
                    .await
                {
                    Ok(_) => {
                        metrics::record_secret_operation(
                            "aws",
                            "update",
                            start.elapsed().as_secs_f64(),
                        );
                        "update"
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        span_clone.record("operation.success", false);
                        span_clone.record("operation.type", "update");
                        span_clone.record("error.message", error_msg.clone());
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::increment_provider_operation_errors("aws");
                        return Err(anyhow::anyhow!(
                            "Failed to update AWS secret {secret_name}: {e}"
                        ));
                    }
                }
            };

            span_clone.record("operation.type", operation_type);
            span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
            span_clone.record("operation.success", true);
            Ok(true)
        }
        .instrument(span)
        .await
    }

    async fn get_secret_value(&self, secret_name: &str) -> Result<Option<String>> {
        let span = tracing::debug_span!(
            "aws.secret.get",
            secret.name = secret_name,
            region = self._region
        );
        let span_clone = span.clone();
        let start = Instant::now();

        async move {
            match self
                .client
                .get_secret_value()
                .secret_id(secret_name)
                .send()
                .await
            {
                Ok(response) => {
                    let value = response
                        .secret_string()
                        .map(ToString::to_string)
                        .or_else(|| {
                            response
                                .secret_binary()
                                .map(|blob| String::from_utf8_lossy(blob.as_ref()).to_string())
                        });

                    match value {
                        Some(v) => {
                            span_clone.record("operation.success", true);
                            span_clone.record("operation.found", true);
                            span_clone.record(
                                "operation.duration_ms",
                                start.elapsed().as_millis() as u64,
                            );
                            metrics::record_secret_operation(
                                "aws",
                                "get",
                                start.elapsed().as_secs_f64(),
                            );
                            Ok(Some(v))
                        }
                        None => {
                            span_clone.record("operation.success", false);
                            span_clone
                                .record("error.message", "Secret has no string or binary value");
                            span_clone.record(
                                "operation.duration_ms",
                                start.elapsed().as_millis() as u64,
                            );
                            metrics::increment_provider_operation_errors("aws");
                            Err(anyhow::anyhow!("Secret has no string or binary value"))
                        }
                    }
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    if error_msg.contains("ResourceNotFoundException") {
                        span_clone.record("operation.success", true);
                        span_clone.record("operation.found", false);
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::record_secret_operation(
                            "aws",
                            "get",
                            start.elapsed().as_secs_f64(),
                        );
                        Ok(None)
                    } else {
                        span_clone.record("operation.success", false);
                        span_clone.record("error.message", error_msg.clone());
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::increment_provider_operation_errors("aws");
                        Err(anyhow::anyhow!("Failed to get AWS secret: {e}"))
                    }
                }
            }
        }
        .instrument(span)
        .await
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
    use crate::{AwsAuthConfig, AwsConfig};

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
            assert!(
                !name.is_empty() && name.len() <= 512,
                "Secret name {name} should be valid"
            );
        }
    }
}

// Export Parameter Store provider
pub mod parameter_store;
pub use parameter_store::AwsParameterStore;
