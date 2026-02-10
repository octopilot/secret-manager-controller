//! Trait for secret store backends
//!
//! Allows provider stores to use either in-memory or database backends

use super::store::{SecretEntry, SecretVersion};
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

/// Trait for secret store backends
#[async_trait::async_trait]
pub trait SecretStoreBackend: Send + Sync {
    async fn add_version<F>(
        &self,
        key: String,
        version_data: Value,
        version_id: Option<String>,
        version_id_generator: F,
    ) -> Result<String>
    where
        F: FnOnce(&HashMap<String, SecretEntry>, &str) -> String + Send + 'static;

    async fn update_metadata(&self, key: String, metadata: Value) -> Result<()>;
    async fn get_latest(&self, key: &str) -> Option<SecretVersion>;
    async fn get_version(&self, key: &str, version_id: &str) -> Option<SecretVersion>;
    async fn list_versions(&self, key: &str) -> Option<Vec<SecretVersion>>;
    async fn get_metadata(&self, key: &str) -> Option<Value>;
    async fn disable_secret(&self, key: &str) -> bool;
    async fn enable_secret(&self, key: &str) -> bool;
    async fn disable_version(&self, key: &str, version_id: &str) -> bool;
    async fn enable_version(&self, key: &str, version_id: &str) -> bool;
    async fn delete_secret(&self, key: &str) -> bool;
    async fn delete_version(&self, key: &str, version_id: &str) -> bool;
    async fn exists(&self, key: &str) -> bool;
    async fn is_enabled(&self, key: &str) -> bool;
    async fn list_all_keys(&self) -> Vec<String>;
}
