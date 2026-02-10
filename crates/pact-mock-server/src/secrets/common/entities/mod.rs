//! SeaORM entities for all provider schemas
//!
//! Each provider has its own schema with provider-specific tables:
//! - GCP: secrets, versions, parameters, parameter_versions
//! - AWS: secrets, versions, staging_labels, parameters, parameter_versions
//! - Azure: secrets, versions, deleted_secrets, app_config, app_config_versions
//!
//! Entities are organized by provider in subdirectories for better organization.

pub mod aws;
pub mod azure;
pub mod gcp;

// Re-export for convenience (explicit to avoid ambiguous glob re-exports)
pub use aws::{AwsParameter, AwsParameterVersion, AwsSecret, AwsStagingLabel, AwsVersion};
pub use azure::{
    AzureAppConfig, AzureAppConfigVersion, AzureDeletedSecret, AzureSecret, AzureVersion,
};
pub use gcp::{GcpParameter, GcpParameterVersion, GcpSecret, GcpVersion};
