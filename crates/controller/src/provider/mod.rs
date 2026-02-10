//! # Provider Modules
//!
//! Provider modules for different cloud secret managers and config stores.
//!
//! Each provider implements either:
//! - `SecretManagerProvider` trait for secret stores
//! - `ConfigStoreProvider` trait for config stores

use anyhow::Result;
use async_trait::async_trait;

/// Provider trait for cloud secret managers
#[async_trait]
pub trait SecretManagerProvider: Send + Sync {
    /// Create or update a secret, ensuring Git is source of truth
    /// Returns true if secret was created/updated, false if no change was needed
    ///
    /// # Arguments
    /// * `secret_name` - Name of the secret
    /// * `secret_value` - Value of the secret
    /// * `environment` - Environment name (e.g., "dev", "prod")
    /// * `location` - Location/region (e.g., "us-central1", "us-east-1", "eastus")
    async fn create_or_update_secret(
        &self,
        secret_name: &str,
        secret_value: &str,
        environment: &str,
        location: &str,
    ) -> Result<bool>;

    /// Get the latest secret value
    async fn get_secret_value(&self, secret_name: &str) -> Result<Option<String>>;

    /// Delete a secret (optional - may not be supported by all providers)
    async fn delete_secret(&self, secret_name: &str) -> Result<()>;

    /// Disable a secret (makes it inaccessible but doesn't delete it)
    /// Returns true if secret was disabled, false if it was already disabled or doesn't exist
    async fn disable_secret(&self, secret_name: &str) -> Result<bool>;

    /// Enable a secret (makes it accessible again)
    /// Returns true if secret was enabled, false if it was already enabled or doesn't exist
    async fn enable_secret(&self, secret_name: &str) -> Result<bool>;
}

/// Provider trait for cloud config stores
/// Used for storing application.properties and other configuration values
#[async_trait]
pub trait ConfigStoreProvider: Send + Sync {
    /// Create or update a config value, ensuring Git is source of truth
    /// Returns true if config was created/updated, false if no change was needed
    async fn create_or_update_config(&self, config_key: &str, config_value: &str) -> Result<bool>;

    /// Get a config value
    async fn get_config_value(&self, config_key: &str) -> Result<Option<String>>;

    /// Delete a config value (optional - may not be supported by all providers)
    async fn delete_config(&self, config_key: &str) -> Result<()>;
}

// Common utilities shared across providers
pub mod common;

// Provider implementations
pub mod aws;
pub mod azure;
pub mod gcp;
