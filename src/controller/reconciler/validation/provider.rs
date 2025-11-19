//! # Provider Configuration Validation
//!
//! Validates provider-specific configuration (GCP, AWS, Azure).

use crate::crd::ProviderConfig;
use anyhow::Result;
use regex::Regex;

/// Validate provider configuration
/// Uses official provider API constraints from:
/// - GCP: https://cloud.google.com/resource-manager/docs/creating-managing-projects
/// - AWS: https://docs.aws.amazon.com/general/latest/gr/rande.html
/// - Azure: https://learn.microsoft.com/en-us/azure/key-vault/general/about-keys-secrets-certificates#vault-name
pub fn validate_provider_config(provider: &ProviderConfig) -> Result<()> {
    match provider {
        ProviderConfig::Gcp(gcp) => {
            if gcp.project_id.is_empty() {
                return Err(anyhow::anyhow!(
                    "provider.gcp.projectId is required but is empty"
                ));
            }
            // GCP project ID validation per official GCP API constraints:
            // - Length: 6-30 characters
            // - Must start with a lowercase letter
            // - Cannot end with a hyphen
            // - Allowed: lowercase letters, numbers, hyphens
            // Reference: https://cloud.google.com/resource-manager/docs/creating-managing-projects
            let project_id_regex = Regex::new(r"^[a-z][a-z0-9-]{4,28}[a-z0-9]$")
                .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

            if !project_id_regex.is_match(&gcp.project_id) {
                return Err(anyhow::anyhow!(
                    "provider.gcp.projectId '{}' must be a valid GCP project ID (6-30 characters, lowercase letters/numbers/hyphens, must start with letter, cannot end with hyphen). See: https://cloud.google.com/resource-manager/docs/creating-managing-projects",
                    gcp.project_id
                ));
            }
        }
        ProviderConfig::Aws(aws) => {
            if aws.region.is_empty() {
                return Err(anyhow::anyhow!(
                    "provider.aws.region is required but is empty"
                ));
            }
            // AWS region validation per official AWS API constraints:
            // - Format: [a-z]{2}-[a-z]+-[0-9]+ (e.g., us-east-1, eu-west-1)
            // - Some regions include -gov or -iso segments (e.g., us-gov-west-1)
            // - Must match valid AWS region codes
            // Reference: https://docs.aws.amazon.com/general/latest/gr/rande.html
            validate_aws_region(&aws.region)?;
        }
        ProviderConfig::Azure(azure) => {
            if azure.vault_name.is_empty() {
                return Err(anyhow::anyhow!(
                    "provider.azure.vaultName is required but is empty"
                ));
            }
            // Azure Key Vault name validation per official Azure API constraints:
            // - Length: 3-24 characters
            // - Must start with a letter
            // - Cannot end with a hyphen
            // - Allowed: alphanumeric characters and hyphens
            // - Hyphens cannot be consecutive
            // Reference: https://learn.microsoft.com/en-us/azure/key-vault/general/about-keys-secrets-certificates#vault-name
            let vault_name_regex = Regex::new(r"^[a-zA-Z][a-zA-Z0-9-]{1,22}[a-zA-Z0-9]$")
                .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

            if !vault_name_regex.is_match(&azure.vault_name) {
                return Err(anyhow::anyhow!(
                    "provider.azure.vaultName '{}' must be a valid Azure Key Vault name (3-24 characters, alphanumeric/hyphens, must start with letter, cannot end with hyphen). See: https://learn.microsoft.com/en-us/azure/key-vault/general/about-keys-secrets-certificates#vault-name",
                    azure.vault_name
                ));
            }

            // Check for consecutive hyphens
            if azure.vault_name.contains("--") {
                return Err(anyhow::anyhow!(
                    "provider.azure.vaultName '{}' cannot contain consecutive hyphens",
                    azure.vault_name
                ));
            }
        }
    }
    Ok(())
}

/// Validate AWS region against official AWS region format
/// Supports standard regions (us-east-1) and special regions (us-gov-west-1, cn-north-1)
/// Reference: https://docs.aws.amazon.com/general/latest/gr/rande.html
pub fn validate_aws_region(region: &str) -> Result<()> {
    let region_trimmed = region.trim().to_lowercase();

    if region_trimmed.is_empty() {
        return Err(anyhow::anyhow!("provider.aws.region cannot be empty"));
    }

    // AWS region format patterns:
    // Standard: [a-z]{2}-[a-z]+-[0-9]+ (e.g., us-east-1, eu-west-1)
    // Gov: [a-z]{2}-gov-[a-z]+-[0-9]+ (e.g., us-gov-west-1)
    // ISO: [a-z]{2}-iso-[a-z]+-[0-9]+ (e.g., us-iso-east-1)
    // China: cn-[a-z]+-[0-9]+ (e.g., cn-north-1)
    // Local: local (for localstack)

    // Standard region pattern: [a-z]{2}-[a-z]+-[0-9]+
    let standard_pattern = Regex::new(r"^[a-z]{2}-[a-z]+-\d+$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    // Gov region pattern: [a-z]{2}-gov-[a-z]+-[0-9]+
    let gov_pattern = Regex::new(r"^[a-z]{2}-gov-[a-z]+-\d+$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    // ISO region pattern: [a-z]{2}-iso-[a-z]+-[0-9]+
    let iso_pattern = Regex::new(r"^[a-z]{2}-iso-[a-z]+-\d+$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    // China region pattern: cn-[a-z]+-[0-9]+
    let china_pattern = Regex::new(r"^cn-[a-z]+-\d+$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    // Local pattern (for local development/testing with localstack)
    // Note: This allows "local" as a region for local development environments
    let local_pattern =
        Regex::new(r"^local$").map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if standard_pattern.is_match(&region_trimmed)
        || gov_pattern.is_match(&region_trimmed)
        || iso_pattern.is_match(&region_trimmed)
        || china_pattern.is_match(&region_trimmed)
        || local_pattern.is_match(&region_trimmed)
    {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "provider.aws.region '{region}' must be a valid AWS region code (e.g., 'us-east-1', 'eu-west-1', 'us-gov-west-1', 'cn-north-1'). See: https://docs.aws.amazon.com/general/latest/gr/rande.html"
        ))
    }
}
