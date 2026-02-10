//! Azure provider entities

pub mod app_config;
pub mod app_config_version;
pub mod deleted_secret;
pub mod secret;
pub mod version;

// Re-export for convenience
pub use app_config::Entity as AzureAppConfig;
pub use app_config_version::Entity as AzureAppConfigVersion;
pub use deleted_secret::Entity as AzureDeletedSecret;
pub use secret::Entity as AzureSecret;
pub use version::Entity as AzureVersion;
