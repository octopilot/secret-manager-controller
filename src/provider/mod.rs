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
    async fn create_or_update_secret(&self, secret_name: &str, secret_value: &str) -> Result<bool>;

    /// Get the latest secret value
    async fn get_secret_value(&self, secret_name: &str) -> Result<Option<String>>;

    /// Delete a secret (optional - may not be supported by all providers)
    async fn delete_secret(&self, secret_name: &str) -> Result<()>;
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
