//! Enum wrapper for secret store backends
//!
//! This enum allows us to use either in-memory or database backends
//! without requiring `dyn Trait` (which doesn't work with generic methods)

use super::store::{SecretEntry, SecretVersion};
use super::{db_store::DbSecretStore, store::SecretStore, store_trait::SecretStoreBackend};
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

/// Enum wrapper for secret store backends
/// This allows us to use either in-memory or database stores
#[derive(Clone)]
pub enum SecretStoreEnum {
    InMemory(SecretStore),
    Database(DbSecretStore),
}

#[async_trait::async_trait]
impl SecretStoreBackend for SecretStoreEnum {
    async fn add_version<F>(
        &self,
        key: String,
        version_data: Value,
        version_id: Option<String>,
        version_id_generator: F,
    ) -> Result<String>
    where
        F: FnOnce(&HashMap<String, SecretEntry>, &str) -> String + Send + 'static,
    {
        match self {
            SecretStoreEnum::InMemory(store) => {
                // Call through the trait to ensure we get Result<String>
                SecretStoreBackend::add_version(
                    store,
                    key,
                    version_data,
                    version_id,
                    version_id_generator,
                )
                .await
            }
            SecretStoreEnum::Database(store) => {
                SecretStoreBackend::add_version(
                    store,
                    key,
                    version_data,
                    version_id,
                    version_id_generator,
                )
                .await
            }
        }
    }

    async fn update_metadata(&self, key: String, metadata: Value) -> Result<()> {
        match self {
            SecretStoreEnum::InMemory(store) => {
                SecretStoreBackend::update_metadata(store, key, metadata).await
            }
            SecretStoreEnum::Database(store) => {
                SecretStoreBackend::update_metadata(store, key, metadata).await
            }
        }
    }

    async fn get_latest(&self, key: &str) -> Option<SecretVersion> {
        match self {
            SecretStoreEnum::InMemory(store) => store.get_latest(key).await,
            SecretStoreEnum::Database(store) => store.get_latest(key).await,
        }
    }

    async fn get_version(&self, key: &str, version_id: &str) -> Option<SecretVersion> {
        match self {
            SecretStoreEnum::InMemory(store) => store.get_version(key, version_id).await,
            SecretStoreEnum::Database(store) => store.get_version(key, version_id).await,
        }
    }

    async fn list_versions(&self, key: &str) -> Option<Vec<SecretVersion>> {
        match self {
            SecretStoreEnum::InMemory(store) => store.list_versions(key).await,
            SecretStoreEnum::Database(store) => store.list_versions(key).await,
        }
    }

    async fn get_metadata(&self, key: &str) -> Option<Value> {
        match self {
            SecretStoreEnum::InMemory(store) => store.get_metadata(key).await,
            SecretStoreEnum::Database(store) => store.get_metadata(key).await,
        }
    }

    async fn disable_secret(&self, key: &str) -> bool {
        match self {
            SecretStoreEnum::InMemory(store) => store.disable_secret(key).await,
            SecretStoreEnum::Database(store) => store.disable_secret(key).await,
        }
    }

    async fn enable_secret(&self, key: &str) -> bool {
        match self {
            SecretStoreEnum::InMemory(store) => store.enable_secret(key).await,
            SecretStoreEnum::Database(store) => store.enable_secret(key).await,
        }
    }

    async fn disable_version(&self, key: &str, version_id: &str) -> bool {
        match self {
            SecretStoreEnum::InMemory(store) => store.disable_version(key, version_id).await,
            SecretStoreEnum::Database(store) => store.disable_version(key, version_id).await,
        }
    }

    async fn enable_version(&self, key: &str, version_id: &str) -> bool {
        match self {
            SecretStoreEnum::InMemory(store) => store.enable_version(key, version_id).await,
            SecretStoreEnum::Database(store) => store.enable_version(key, version_id).await,
        }
    }

    async fn delete_secret(&self, key: &str) -> bool {
        match self {
            SecretStoreEnum::InMemory(store) => store.delete_secret(key).await,
            SecretStoreEnum::Database(store) => store.delete_secret(key).await,
        }
    }

    async fn delete_version(&self, key: &str, version_id: &str) -> bool {
        match self {
            SecretStoreEnum::InMemory(store) => store.delete_version(key, version_id).await,
            SecretStoreEnum::Database(store) => store.delete_version(key, version_id).await,
        }
    }

    async fn exists(&self, key: &str) -> bool {
        match self {
            SecretStoreEnum::InMemory(store) => store.exists(key).await,
            SecretStoreEnum::Database(store) => store.exists(key).await,
        }
    }

    async fn is_enabled(&self, key: &str) -> bool {
        match self {
            SecretStoreEnum::InMemory(store) => store.is_enabled(key).await,
            SecretStoreEnum::Database(store) => store.is_enabled(key).await,
        }
    }

    async fn list_all_keys(&self) -> Vec<String> {
        match self {
            SecretStoreEnum::InMemory(store) => store.list_all_keys().await,
            SecretStoreEnum::Database(store) => store.list_all_keys().await,
        }
    }
}

impl std::fmt::Debug for SecretStoreEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecretStoreEnum::InMemory(_) => write!(f, "SecretStoreEnum::InMemory"),
            SecretStoreEnum::Database(_) => write!(f, "SecretStoreEnum::Database"),
        }
    }
}
