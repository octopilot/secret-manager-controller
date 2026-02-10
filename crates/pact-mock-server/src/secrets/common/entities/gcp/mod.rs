//! GCP provider entities

pub mod parameter;
pub mod parameter_version;
pub mod secret;
pub mod version;

// Re-export for convenience
pub use parameter::Entity as GcpParameter;
pub use parameter_version::Entity as GcpParameterVersion;
pub use secret::Entity as GcpSecret;
pub use version::Entity as GcpVersion;
