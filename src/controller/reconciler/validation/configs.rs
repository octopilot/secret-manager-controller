//! # Configs Configuration Validation
//!
//! Validates configs configuration (Parameter Store, App Configuration).

use crate::crd::ConfigsConfig;
use anyhow::Result;

use super::paths::{validate_aws_parameter_path, validate_url};

/// Validate configs configuration
pub fn validate_configs_config(configs: &ConfigsConfig) -> Result<()> {
    // Validate store type if present
    // ConfigStoreType is an enum, so it's already validated by serde
    // No additional validation needed - enum variants are: SecretManager, ParameterManager
    if let Some(ref _store) = configs.store {
        // Enum is already validated by serde deserialization
        // ConfigStoreType::SecretManager or ConfigStoreType::ParameterManager are the only valid values
    }

    // Validate appConfigEndpoint if present
    if let Some(endpoint) = &configs.app_config_endpoint {
        if !endpoint.is_empty() {
            if let Err(e) = validate_url(endpoint, "configs.appConfigEndpoint") {
                return Err(anyhow::anyhow!(
                    "Invalid configs.appConfigEndpoint '{}': {}",
                    endpoint,
                    e
                ));
            }
        }
    }

    // Validate parameterPath if present
    if let Some(path) = &configs.parameter_path {
        if !path.is_empty() {
            if let Err(e) = validate_aws_parameter_path(path, "configs.parameterPath") {
                return Err(anyhow::anyhow!(
                    "Invalid configs.parameterPath '{}': {}",
                    path,
                    e
                ));
            }
        }
    }

    Ok(())
}
