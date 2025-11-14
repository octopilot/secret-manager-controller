//! # Provider Trait
//!
//! Abstract interface for cloud provider secret managers.
//!
//! This trait allows the controller to work with multiple cloud providers
//! (GCP, AWS, Azure) through a unified interface.

use anyhow::Result;
use async_trait::async_trait;

/// Provider trait for cloud secret managers
#[async_trait]
pub trait SecretManagerProvider: Send + Sync {
    /// Create or update a secret, ensuring Git is source of truth
    /// Returns true if secret was created/updated, false if no change was needed
    async fn create_or_update_secret(
        &self,
        secret_name: &str,
        secret_value: &str,
    ) -> Result<bool>;

    /// Get the latest secret value
    async fn get_secret_value(&self, secret_name: &str) -> Result<Option<String>>;

    /// Delete a secret (optional - may not be supported by all providers)
    async fn delete_secret(&self, secret_name: &str) -> Result<()>;
}

