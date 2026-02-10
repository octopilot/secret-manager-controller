//! AWS provider entities

pub mod parameter;
pub mod parameter_version;
pub mod secret;
pub mod staging_label;
pub mod version;

// Re-export for convenience
pub use parameter::Entity as AwsParameter;
pub use parameter_version::Entity as AwsParameterVersion;
pub use secret::Entity as AwsSecret;
pub use staging_label::Entity as AwsStagingLabel;
pub use version::Entity as AwsVersion;
