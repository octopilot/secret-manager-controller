//! # AWS Secrets Manager Authentication
//!
//! Handles AWS SDK configuration and authentication setup.

use crate::crd::{AwsAuthConfig, AwsConfig};
use anyhow::{Context, Result};
use aws_config::SdkConfig;
use tracing::{info, warn};

/// Create AWS SDK config using IRSA (IAM Roles for Service Accounts)
pub async fn create_irsa_config(
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

    let mut builder = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new(region.to_string()));

    // Support Pact mock server integration via environment variable
    // When PACT_MODE=true, route requests to Pact mock server instead of real AWS
    // Read environment variables at the time of config creation to ensure they're current
    let pact_mode = std::env::var("PACT_MODE");
    if pact_mode.is_ok() {
        let endpoint = std::env::var("AWS_SECRETS_MANAGER_ENDPOINT")
            .context("PACT_MODE is enabled but AWS_SECRETS_MANAGER_ENDPOINT is not set. This is required for Pact testing.")?;

        // Validate endpoint URL is safe (not pointing to production AWS)
        // Allow localhost, Kubernetes service names, Docker hostnames, and other mock server hostnames
        let is_production_aws = endpoint.contains("secretsmanager.amazonaws.com")
            || endpoint.contains("amazonaws.com/secretsmanager");

        if is_production_aws {
            return Err(anyhow::anyhow!(
                "PACT_MODE is enabled but endpoint '{}' points to production AWS. \
                This is not allowed in Pact mode. Use a mock server endpoint instead.",
                endpoint
            ));
        }

        // Warn if endpoint doesn't look like a typical mock server (localhost, service name, etc.)
        let looks_like_mock = endpoint.starts_with("http://localhost")
            || endpoint.starts_with("http://127.0.0.1")
            || endpoint.starts_with("http://[::1]")
            || endpoint.contains("host.docker.internal")
            || endpoint.contains(".svc.cluster.local")
            || endpoint.contains("pact")
            || endpoint.contains("mock");

        if !looks_like_mock {
            warn!(
                "PACT_MODE is enabled but endpoint '{}' does not appear to be a mock server. \
                Verify this is correct and not pointing to production AWS.",
                endpoint
            );
        }

        info!(
            "Pact mode enabled: routing AWS Secrets Manager requests to {}",
            endpoint
        );
        // Set endpoint_url on the builder - this will be used by all services created from this config
        builder = builder.endpoint_url(&endpoint);
    }

    let sdk_config = builder.load().await;

    Ok(sdk_config)
}

/// Create AWS SDK config using default credential chain
pub async fn create_default_config(region: &str) -> Result<SdkConfig> {
    let mut builder = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new(region.to_string()));

    // Support Pact mock server integration via environment variable
    // When PACT_MODE=true, route requests to Pact mock server instead of real AWS
    // Read environment variables at the time of config creation to ensure they're current
    let pact_mode = std::env::var("PACT_MODE");
    if pact_mode.is_ok() {
        let endpoint = std::env::var("AWS_SECRETS_MANAGER_ENDPOINT")
            .context("PACT_MODE is enabled but AWS_SECRETS_MANAGER_ENDPOINT is not set. This is required for Pact testing.")?;

        // Validate endpoint URL is safe (not pointing to production AWS)
        // Allow localhost, Kubernetes service names, Docker hostnames, and other mock server hostnames
        let is_production_aws = endpoint.contains("secretsmanager.amazonaws.com")
            || endpoint.contains("amazonaws.com/secretsmanager");

        if is_production_aws {
            return Err(anyhow::anyhow!(
                "PACT_MODE is enabled but endpoint '{}' points to production AWS. \
                This is not allowed in Pact mode. Use a mock server endpoint instead.",
                endpoint
            ));
        }

        // Warn if endpoint doesn't look like a typical mock server (localhost, service name, etc.)
        let looks_like_mock = endpoint.starts_with("http://localhost")
            || endpoint.starts_with("http://127.0.0.1")
            || endpoint.starts_with("http://[::1]")
            || endpoint.contains("host.docker.internal")
            || endpoint.contains(".svc.cluster.local")
            || endpoint.contains("pact")
            || endpoint.contains("mock");

        if !looks_like_mock {
            warn!(
                "PACT_MODE is enabled but endpoint '{}' does not appear to be a mock server. \
                Verify this is correct and not pointing to production AWS.",
                endpoint
            );
        }

        info!(
            "Pact mode enabled: routing AWS Secrets Manager requests to {}",
            endpoint
        );
        // Set endpoint_url on the builder - this will be used by all services created from this config
        builder = builder.endpoint_url(&endpoint);
    }

    let sdk_config = builder.load().await;

    Ok(sdk_config)
}

/// Create AWS SDK config based on authentication method
pub async fn create_sdk_config(config: &AwsConfig, k8s_client: &kube::Client) -> Result<SdkConfig> {
    let region = config.region.clone();

    // Build AWS SDK config based on authentication method
    // Default to IRSA when auth is not specified
    match &config.auth {
        Some(AwsAuthConfig::Irsa { role_arn }) => {
            info!("Using IRSA authentication with role: {}", role_arn);
            create_irsa_config(&region, role_arn, k8s_client).await
        }
        None => {
            info!("No auth configuration specified, defaulting to IRSA (IAM Roles for Service Accounts)");
            info!(
                "Ensure pod service account has annotation: eks.amazonaws.com/role-arn=<role-arn>"
            );
            // Default to IRSA - the AWS SDK will automatically discover the role from the pod's service account
            create_default_config(&region).await
        }
    }
}
