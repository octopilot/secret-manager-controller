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

            // GCP location validation per official GCP API constraints:
            // - Format: [continent]-[direction][number] (e.g., us-central1, europe-west1)
            // - Examples: us-central1, us-east1, europe-west1, asia-east1
            // Reference: https://cloud.google.com/about/locations
            validate_gcp_location(&gcp.location)?;
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

            // Azure location validation per official Azure API constraints:
            // - Format: [direction][region][number] (e.g., eastus, westus2, southeastasia)
            // - Examples: eastus, westus2, centralus, southeastasia
            // Reference: https://azure.microsoft.com/en-us/explore/global-infrastructure/geographies/
            validate_azure_location(&azure.location)?;
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

/// Validate GCP location against official GCP location format
/// Format: [continent]-[direction][number] (e.g., us-central1, europe-west1)
/// Reference: https://cloud.google.com/about/locations
pub fn validate_gcp_location(location: &str) -> Result<()> {
    let location_trimmed = location.trim().to_lowercase();

    if location_trimmed.is_empty() {
        return Err(anyhow::anyhow!("provider.gcp.location cannot be empty"));
    }

    // GCP location format: [continent]-[direction][number]
    // Examples: us-central1, us-east1, europe-west1, asia-east1
    // Pattern: [a-z]+-[a-z]+[0-9]+
    let location_pattern = Regex::new(r"^[a-z]+-[a-z]+[0-9]+$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if location_pattern.is_match(&location_trimmed) {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "provider.gcp.location '{}' must be a valid GCP location (format: [continent]-[direction][number], e.g., 'us-central1', 'europe-west1', 'asia-east1'). See: https://cloud.google.com/about/locations",
            location
        ))
    }
}

/// Validate Azure location against official Azure location format
/// Format: [direction][region][number] (e.g., eastus, westus2, southeastasia)
/// Reference: https://azure.microsoft.com/en-us/explore/global-infrastructure/geographies/
pub fn validate_azure_location(location: &str) -> Result<()> {
    let location_trimmed = location.trim().to_lowercase();

    if location_trimmed.is_empty() {
        return Err(anyhow::anyhow!("provider.azure.location cannot be empty"));
    }

    // Azure location format: [direction][region][number]
    // Examples: eastus, westus2, centralus, southeastasia
    // Pattern: [a-z]+[0-9]*
    let location_pattern = Regex::new(r"^[a-z]+[0-9]*$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    if location_pattern.is_match(&location_trimmed) {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "provider.azure.location '{}' must be a valid Azure location (format: [direction][region][number], e.g., 'eastus', 'westus2', 'southeastasia'). See: https://azure.microsoft.com/en-us/explore/global-infrastructure/geographies/",
            location
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crd::{AwsConfig, AzureConfig, GcpConfig};

    #[test]
    fn test_validate_gcp_location_valid() {
        let valid_locations = vec![
            "us-central1",
            "us-east1",
            "us-west1",
            "europe-west1",
            "europe-west4",
            "asia-east1",
            "asia-southeast1",
        ];

        for location in valid_locations {
            assert!(
                validate_gcp_location(location).is_ok(),
                "GCP location '{}' should be valid",
                location
            );
        }
    }

    #[test]
    fn test_validate_gcp_location_invalid() {
        let invalid_locations = vec![
            "invalid",
            "us-central",
            "us-central-1",
            "us_central1",
            "",
            "us-central-1a", // AWS format
        ];

        for location in invalid_locations {
            assert!(
                validate_gcp_location(location).is_err(),
                "GCP location '{}' should be invalid",
                location
            );
        }
    }

    #[test]
    fn test_validate_gcp_location_case_insensitive() {
        // Validation normalizes to lowercase, so uppercase should be accepted
        assert!(validate_gcp_location("US-CENTRAL1").is_ok());
        assert!(validate_gcp_location("Europe-West1").is_ok());
    }

    #[test]
    fn test_validate_aws_region_valid() {
        let valid_regions = vec![
            "us-east-1",
            "eu-west-1",
            "ap-southeast-1",
            "us-gov-west-1",
            "us-iso-east-1",
            "cn-north-1",
            "local", // For localstack
        ];

        for region in valid_regions {
            assert!(
                validate_aws_region(region).is_ok(),
                "AWS region '{}' should be valid",
                region
            );
        }
    }

    #[test]
    fn test_validate_aws_region_invalid() {
        let invalid_regions = vec![
            "invalid",
            "us-east",
            "us-east-1a", // Availability zone, not region
            "us_east_1",
            "",
        ];

        for region in invalid_regions {
            assert!(
                validate_aws_region(region).is_err(),
                "AWS region '{}' should be invalid",
                region
            );
        }
    }

    #[test]
    fn test_validate_aws_region_case_insensitive() {
        // Validation normalizes to lowercase, so uppercase should be accepted
        assert!(validate_aws_region("US-EAST-1").is_ok());
        assert!(validate_aws_region("Eu-West-1").is_ok());
    }

    #[test]
    fn test_validate_azure_location_valid() {
        let valid_locations = vec![
            "eastus",
            "westus",
            "westus2",
            "centralus",
            "southeastasia",
            "northeurope",
        ];

        for location in valid_locations {
            assert!(
                validate_azure_location(location).is_ok(),
                "Azure location '{}' should be valid",
                location
            );
        }
    }

    #[test]
    fn test_validate_azure_location_invalid() {
        let invalid_locations = vec![
            "east-us",
            "east_us",
            "",
            "east-us-2", // Wrong format
        ];

        for location in invalid_locations {
            assert!(
                validate_azure_location(location).is_err(),
                "Azure location '{}' should be invalid",
                location
            );
        }
    }

    #[test]
    fn test_validate_azure_location_case_insensitive() {
        // Validation normalizes to lowercase, so uppercase should be accepted
        // Note: "invalid" actually matches the pattern (all lowercase letters), so it's technically valid
        assert!(validate_azure_location("EASTUS").is_ok());
        assert!(validate_azure_location("WestUs2").is_ok());
    }

    #[test]
    fn test_validate_provider_config_gcp_with_valid_location() {
        let config = ProviderConfig::Gcp(GcpConfig {
            project_id: "test-project".to_string(),
            location: "us-central1".to_string(),
            auth: None,
        });

        assert!(validate_provider_config(&config).is_ok());
    }

    #[test]
    fn test_validate_provider_config_gcp_with_invalid_location() {
        let config = ProviderConfig::Gcp(GcpConfig {
            project_id: "test-project".to_string(),
            location: "invalid-location".to_string(),
            auth: None,
        });

        assert!(validate_provider_config(&config).is_err());
    }

    #[test]
    fn test_validate_provider_config_aws_with_valid_region() {
        let config = ProviderConfig::Aws(AwsConfig {
            region: "us-east-1".to_string(),
            auth: None,
        });

        assert!(validate_provider_config(&config).is_ok());
    }

    #[test]
    fn test_validate_provider_config_aws_with_invalid_region() {
        let config = ProviderConfig::Aws(AwsConfig {
            region: "invalid-region".to_string(),
            auth: None,
        });

        assert!(validate_provider_config(&config).is_err());
    }

    #[test]
    fn test_validate_provider_config_azure_with_valid_location() {
        let config = ProviderConfig::Azure(AzureConfig {
            vault_name: "test-vault".to_string(),
            location: "eastus".to_string(),
            auth: None,
        });

        assert!(validate_provider_config(&config).is_ok());
    }

    #[test]
    fn test_validate_provider_config_azure_with_invalid_location() {
        let config = ProviderConfig::Azure(AzureConfig {
            vault_name: "test-vault".to_string(),
            location: "invalid-location".to_string(),
            auth: None,
        });

        assert!(validate_provider_config(&config).is_err());
    }
}
