//! # AWS Parameter Store Client
//!
//! Client for interacting with AWS Systems Manager Parameter Store API.
//!
//! This module provides functionality to:
//! - Create and update parameters in AWS Parameter Store
//! - Retrieve parameter values
//! - Support IRSA (IAM Roles for Service Accounts) authentication
//!
//! Parameter Store is used for storing configuration values (non-secrets)
//! and provides better integration with EKS via ASCP (AWS Secrets and Configuration Provider).

use crate::observability::metrics;
use crate::provider::ConfigStoreProvider;
use crate::{AwsAuthConfig, AwsConfig};
use anyhow::{Context, Result};
use async_trait::async_trait;
use aws_config::SdkConfig;
use aws_sdk_ssm::Client as SsmClient;
use tracing::{debug, info};

/// AWS Parameter Store provider implementation
pub struct AwsParameterStore {
    client: SsmClient,
    parameter_path_prefix: String,
    _region: String,
}

impl std::fmt::Debug for AwsParameterStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsParameterStore")
            .field("parameter_path_prefix", &self.parameter_path_prefix)
            .field("_region", &self._region)
            .finish_non_exhaustive()
    }
}

impl AwsParameterStore {
    /// Create a new AWS Parameter Store client
    /// Supports IRSA (IAM Roles for Service Accounts) authentication
    #[allow(
        clippy::missing_errors_doc,
        reason = "Error documentation is provided in doc comments"
    )]
    pub async fn new(
        config: &AwsConfig,
        parameter_path: Option<&str>,
        secret_prefix: &str,
        environment: &str,
        k8s_client: &kube::Client,
    ) -> Result<Self> {
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

        let client = SsmClient::new(&sdk_config);

        // Construct parameter path prefix
        // Default: /{prefix}/{environment}
        // Custom: use provided parameter_path
        let parameter_path_prefix = if let Some(path) = parameter_path {
            // Ensure path starts with /
            if path.starts_with('/') {
                path.to_string()
            } else {
                format!("/{path}")
            }
        } else {
            // Default: /{prefix}/{environment}
            format!("/{secret_prefix}/{environment}")
        };

        info!("AWS Parameter Store path prefix: {}", parameter_path_prefix);

        Ok(Self {
            client,
            parameter_path_prefix,
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

    /// Construct full parameter name from key
    /// Format: /{prefix}/{environment}/{key}
    /// Invalid characters are replaced with underscores
    fn construct_parameter_name(&self, key: &str) -> String {
        // Sanitize key (replace dots, slashes, etc. with underscores)
        let sanitized_key = key.replace(['.', '/'], "_");

        // Construct full path
        format!("{}/{}", self.parameter_path_prefix, sanitized_key)
    }
}

#[async_trait]
impl ConfigStoreProvider for AwsParameterStore {
    async fn create_or_update_config(&self, config_key: &str, config_value: &str) -> Result<bool> {
        let start = std::time::Instant::now();
        let parameter_name = self.construct_parameter_name(config_key);

        // Check if parameter exists
        let parameter_exists = self
            .client
            .get_parameter()
            .name(&parameter_name)
            .send()
            .await
            .is_ok();

        if !parameter_exists {
            // Create parameter
            info!("Creating AWS Parameter Store parameter: {}", parameter_name);
            self.client
                .put_parameter()
                .name(&parameter_name)
                .value(config_value)
                .r#type(aws_sdk_ssm::types::ParameterType::String)
                .overwrite(false)
                .send()
                .await
                .context("Failed to create AWS Parameter Store parameter")?;

            metrics::record_secret_operation(
                "aws_parameter_store",
                "create",
                start.elapsed().as_secs_f64(),
            );
            return Ok(true);
        }

        // Get current parameter value
        let current_value = self.get_config_value(config_key).await?;

        if let Some(current) = current_value {
            if current == config_value {
                debug!(
                    "AWS Parameter Store parameter {} unchanged, skipping update",
                    parameter_name
                );
                metrics::record_secret_operation(
                    "aws_parameter_store",
                    "no_change",
                    start.elapsed().as_secs_f64(),
                );
                return Ok(false);
            }
        }

        // Update parameter (overwrite existing)
        info!("Updating AWS Parameter Store parameter: {}", parameter_name);
        self.client
            .put_parameter()
            .name(&parameter_name)
            .value(config_value)
            .r#type(aws_sdk_ssm::types::ParameterType::String)
            .overwrite(true)
            .send()
            .await
            .context("Failed to update AWS Parameter Store parameter")?;

        metrics::record_secret_operation(
            "aws_parameter_store",
            "update",
            start.elapsed().as_secs_f64(),
        );
        Ok(true)
    }

    async fn get_config_value(&self, config_key: &str) -> Result<Option<String>> {
        let parameter_name = self.construct_parameter_name(config_key);

        match self
            .client
            .get_parameter()
            .name(&parameter_name)
            .with_decryption(true) // Decrypt SecureString parameters if needed
            .send()
            .await
        {
            Ok(response) => {
                if let Some(parameter) = response.parameter() {
                    Ok(parameter.value().map(|v| v.to_string()))
                } else {
                    Ok(None)
                }
            }
            Err(e) => {
                if e.to_string().contains("ParameterNotFound") {
                    Ok(None)
                } else {
                    Err(anyhow::anyhow!(
                        "Failed to get AWS Parameter Store parameter: {e}"
                    ))
                }
            }
        }
    }

    async fn delete_config(&self, config_key: &str) -> Result<()> {
        let parameter_name = self.construct_parameter_name(config_key);
        info!("Deleting AWS Parameter Store parameter: {}", parameter_name);
        self.client
            .delete_parameter()
            .name(&parameter_name)
            .send()
            .await
            .context("Failed to delete AWS Parameter Store parameter")?;
        Ok(())
    }
}
