//! Common secret store implementation
//!
//! Provides the core in-memory secret store with versioning support.
//! This is shared across all provider-specific implementations.

use crate::secrets::common::store_trait::SecretStoreBackend;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Secret version metadata
/// Represents a single version of a secret
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretVersion {
    /// Version identifier (e.g., "1", "2", or UUID for AWS/Azure)
    pub version_id: String,
    /// Version data (payload, metadata, etc.)
    pub data: Value,
    /// Whether this version is enabled (can be disabled without deleting)
    pub enabled: bool,
    /// Creation timestamp (for ordering)
    pub created_at: u64, // Unix timestamp
}

/// Secret entry containing all versions
/// Versions are stored in order (oldest first)
#[derive(Debug, Clone)]
pub struct SecretEntry {
    /// Ordered list of versions (oldest first)
    pub versions: Vec<SecretVersion>,
    /// Whether the secret itself is disabled (all versions disabled)
    pub disabled: bool,
    /// Secret metadata (replication config, etc.)
    pub metadata: Value,
}

/// In-memory secret store with versioning support
///
/// Stores secrets with version history:
/// - Key: Secret identifier (provider-specific format)
/// - Value: SecretEntry containing ordered list of versions
///
/// Version behavior:
/// - Versions are ordered (oldest first)
/// - Each version can be enabled/disabled independently
/// - Secrets can be disabled (all versions disabled) or deleted (removed entirely)
///
/// This is ephemeral - data does not persist across restarts.
/// Thread-safe using Arc<RwLock> for concurrent access.
#[derive(Clone, Debug)]
pub struct SecretStore {
    store: Arc<RwLock<HashMap<String, SecretEntry>>>,
}

/// Get a reference to the internal store for version ID generation
impl SecretStore {
    /// Get a read-only snapshot of the store for version ID generation
    pub async fn snapshot(&self) -> HashMap<String, SecretEntry> {
        self.store.read().await.clone()
    }
}

impl SecretStore {
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a new version to a secret (or create the secret if it doesn't exist)
    /// Returns the version ID of the newly created version
    ///
    /// version_id_generator: Function to generate version ID if not provided
    pub async fn add_version<F>(
        &self,
        key: String,
        version_data: Value,
        version_id: Option<String>,
        version_id_generator: F,
    ) -> String
    where
        F: FnOnce(&HashMap<String, SecretEntry>, &str) -> String,
    {
        let mut store = self.store.write().await;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Generate version ID if not provided
        let new_version_id = version_id.unwrap_or_else(|| version_id_generator(&store, &key));

        let version = SecretVersion {
            version_id: new_version_id.clone(),
            data: version_data,
            enabled: true,
            created_at: timestamp,
        };

        let entry = store.entry(key.clone()).or_insert_with(|| SecretEntry {
            versions: Vec::new(),
            disabled: false,
            metadata: json!({}),
        });

        entry.versions.push(version);
        info!("  Added version {} to secret: {}", new_version_id, key);
        new_version_id
    }

    /// Update secret metadata (replication config, etc.)
    pub async fn update_metadata(&self, key: String, metadata: Value) {
        let mut store = self.store.write().await;
        let entry = store.entry(key.clone()).or_insert_with(|| SecretEntry {
            versions: Vec::new(),
            disabled: false,
            metadata: json!({}),
        });
        entry.metadata = metadata;
    }

    /// Get the latest version of a secret
    /// Returns None if the secret doesn't exist or has no enabled versions
    ///
    /// Versions are sorted by timestamp to ensure correct ordering even if
    /// insertion order is wrong (defensive programming).
    pub async fn get_latest(&self, key: &str) -> Option<SecretVersion> {
        let store = self.store.read().await;
        let entry = store.get(key)?;

        if entry.disabled {
            return None;
        }

        // Filter enabled versions and sort by timestamp (oldest first)
        let mut versions: Vec<_> = entry.versions.iter().filter(|v| v.enabled).collect();
        versions.sort_by_key(|v| v.created_at);

        // Return the last version (newest by timestamp)
        versions.last().cloned().cloned()
    }

    /// Get a specific version of a secret by version ID
    pub async fn get_version(&self, key: &str, version_id: &str) -> Option<SecretVersion> {
        let store = self.store.read().await;
        let entry = store.get(key)?;

        if entry.disabled {
            return None;
        }

        entry
            .versions
            .iter()
            .find(|v| v.version_id == version_id && v.enabled)
            .cloned()
    }

    /// Get all versions of a secret (for listing)
    /// Returns versions sorted by creation timestamp (oldest first)
    pub async fn list_versions(&self, key: &str) -> Option<Vec<SecretVersion>> {
        let store = self.store.read().await;
        let entry = store.get(key)?;

        // Sort by timestamp to ensure correct ordering
        let mut versions = entry.versions.clone();
        versions.sort_by_key(|v| v.created_at);
        Some(versions)
    }

    /// Get secret metadata
    pub async fn get_metadata(&self, key: &str) -> Option<Value> {
        let store = self.store.read().await;
        store.get(key).map(|e| e.metadata.clone())
    }

    /// Disable a secret (disables all versions, but keeps them for history)
    /// If the secret doesn't exist, creates it as disabled (idempotent operation)
    pub async fn disable_secret(&self, key: &str) -> bool {
        let mut store = self.store.write().await;
        let entry = store.entry(key.to_string()).or_insert_with(|| {
            info!("  Secret not found, creating as disabled: {}", key);
            SecretEntry {
                versions: Vec::new(),
                disabled: true,
                metadata: json!({}),
            }
        });
        entry.disabled = true;
        info!("  Disabled secret: {}", key);
        true
    }

    /// Enable a secret (re-enables the secret, versions remain in their current state)
    /// If the secret doesn't exist, creates it as enabled (idempotent operation)
    pub async fn enable_secret(&self, key: &str) -> bool {
        let mut store = self.store.write().await;
        let entry = store.entry(key.to_string()).or_insert_with(|| {
            info!("  Secret not found, creating as enabled: {}", key);
            SecretEntry {
                versions: Vec::new(),
                disabled: false,
                metadata: json!({}),
            }
        });
        entry.disabled = false;
        info!("  Enabled secret: {}", key);
        true
    }

    /// Disable a specific version
    pub async fn disable_version(&self, key: &str, version_id: &str) -> bool {
        let mut store = self.store.write().await;
        if let Some(entry) = store.get_mut(key) {
            if let Some(version) = entry
                .versions
                .iter_mut()
                .find(|v| v.version_id == version_id)
            {
                version.enabled = false;
                info!("  Disabled version {} of secret: {}", version_id, key);
                return true;
            }
        }
        false
    }

    /// Enable a specific version
    pub async fn enable_version(&self, key: &str, version_id: &str) -> bool {
        let mut store = self.store.write().await;
        if let Some(entry) = store.get_mut(key) {
            if let Some(version) = entry
                .versions
                .iter_mut()
                .find(|v| v.version_id == version_id)
            {
                version.enabled = true;
                info!("  Enabled version {} of secret: {}", version_id, key);
                return true;
            }
        }
        false
    }

    /// Delete a secret entirely (removes all versions)
    /// Returns true if the secret was deleted, false if it didn't exist
    pub async fn delete_secret(&self, key: &str) -> bool {
        let mut store = self.store.write().await;
        if store.remove(key).is_some() {
            info!("  Deleted secret: {}", key);
            true
        } else {
            false
        }
    }

    /// Delete a specific version
    /// Returns true if the version was deleted
    pub async fn delete_version(&self, key: &str, version_id: &str) -> bool {
        let mut store = self.store.write().await;
        if let Some(entry) = store.get_mut(key) {
            let initial_len = entry.versions.len();
            entry.versions.retain(|v| v.version_id != version_id);
            if entry.versions.len() < initial_len {
                info!("  Deleted version {} of secret: {}", version_id, key);
                return true;
            }
        }
        false
    }

    /// Check if a secret exists
    pub async fn exists(&self, key: &str) -> bool {
        let store = self.store.read().await;
        store.contains_key(key)
    }

    /// Check if a secret is enabled (has at least one enabled version)
    pub async fn is_enabled(&self, key: &str) -> bool {
        let store = self.store.read().await;
        if let Some(entry) = store.get(key) {
            !entry.disabled && entry.versions.iter().any(|v| v.enabled)
        } else {
            false
        }
    }

    /// List all secret keys (for listing all secrets)
    pub async fn list_all_keys(&self) -> Vec<String> {
        let store = self.store.read().await;
        store.keys().cloned().collect()
    }
}

#[async_trait::async_trait]
impl SecretStoreBackend for SecretStore {
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
        // Clone the store snapshot to avoid holding the guard across await
        let snapshot = self.snapshot().await;
        let new_version_id = version_id.unwrap_or_else(|| version_id_generator(&snapshot, &key));

        // Now add the version directly
        let mut store = self.store.write().await;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let version = SecretVersion {
            version_id: new_version_id.clone(),
            data: version_data,
            enabled: true,
            created_at: timestamp,
        };

        let entry = store.entry(key.clone()).or_insert_with(|| SecretEntry {
            versions: Vec::new(),
            disabled: false,
            metadata: json!({}),
        });

        entry.versions.push(version);
        info!("  Added version {} to secret: {}", new_version_id, key);
        Ok(new_version_id)
    }

    async fn update_metadata(&self, key: String, metadata: Value) -> Result<()> {
        self.update_metadata(key, metadata).await;
        Ok(())
    }

    async fn get_latest(&self, key: &str) -> Option<SecretVersion> {
        self.get_latest(key).await
    }

    async fn get_version(&self, key: &str, version_id: &str) -> Option<SecretVersion> {
        self.get_version(key, version_id).await
    }

    async fn list_versions(&self, key: &str) -> Option<Vec<SecretVersion>> {
        self.list_versions(key).await
    }

    async fn get_metadata(&self, key: &str) -> Option<Value> {
        self.get_metadata(key).await
    }

    async fn disable_secret(&self, key: &str) -> bool {
        self.disable_secret(key).await
    }

    async fn enable_secret(&self, key: &str) -> bool {
        self.enable_secret(key).await
    }

    async fn disable_version(&self, key: &str, version_id: &str) -> bool {
        self.disable_version(key, version_id).await
    }

    async fn enable_version(&self, key: &str, version_id: &str) -> bool {
        self.enable_version(key, version_id).await
    }

    async fn delete_secret(&self, key: &str) -> bool {
        self.delete_secret(key).await
    }

    async fn delete_version(&self, key: &str, version_id: &str) -> bool {
        self.delete_version(key, version_id).await
    }

    async fn exists(&self, key: &str) -> bool {
        self.exists(key).await
    }

    async fn is_enabled(&self, key: &str) -> bool {
        self.is_enabled(key).await
    }

    async fn list_all_keys(&self) -> Vec<String> {
        self.list_all_keys().await
    }
}

impl Default for SecretStore {
    fn default() -> Self {
        Self::new()
    }
}
