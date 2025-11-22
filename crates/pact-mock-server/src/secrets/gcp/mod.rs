//! GCP Secret Manager secret store implementation
//!
//! Wraps the common SecretStore with GCP-specific behavior:
//! - Sequential version IDs (1, 2, 3, ...)
//! - Secret key format: "projects/{project}/secrets/{secret}"

pub mod parameter_store;

pub use parameter_store::GcpParameterStore;

use super::common::{SecretStore, SecretVersion};
use serde_json::Value;

/// GCP-specific secret store wrapper
#[derive(Clone, Debug)]
pub struct GcpSecretStore {
    store: SecretStore,
}

impl GcpSecretStore {
    pub fn new() -> Self {
        Self {
            store: SecretStore::new(),
        }
    }

    /// Format GCP secret key
    pub fn format_key(project: &str, secret: &str) -> String {
        format!("projects/{project}/secrets/{secret}")
    }

    /// Add a new version to a secret
    /// GCP uses sequential version numbers (1, 2, 3, ...)
    pub async fn add_version(
        &self,
        project: &str,
        secret: &str,
        version_data: Value,
        version_id: Option<String>,
    ) -> String {
        let key = Self::format_key(project, secret);
        self.store
            .add_version(key, version_data, version_id, |store, key| {
                // GCP: use sequential version numbers
                let entry = store.get(key);
                let next_version = entry.map(|e| e.versions.len() + 1).unwrap_or(1);
                next_version.to_string()
            })
            .await
    }

    /// Update secret metadata (replication config)
    pub async fn update_metadata(&self, project: &str, secret: &str, metadata: Value) {
        let key = Self::format_key(project, secret);
        self.store.update_metadata(key, metadata).await;
    }

    /// Get the latest version of a secret
    pub async fn get_latest(&self, project: &str, secret: &str) -> Option<SecretVersion> {
        let key = Self::format_key(project, secret);
        self.store.get_latest(&key).await
    }

    /// Get a specific version by version ID
    pub async fn get_version(
        &self,
        project: &str,
        secret: &str,
        version_id: &str,
    ) -> Option<SecretVersion> {
        let key = Self::format_key(project, secret);
        self.store.get_version(&key, version_id).await
    }

    /// List all versions of a secret
    pub async fn list_versions(&self, project: &str, secret: &str) -> Option<Vec<SecretVersion>> {
        let key = Self::format_key(project, secret);
        self.store.list_versions(&key).await
    }

    /// Get secret metadata
    pub async fn get_metadata(&self, project: &str, secret: &str) -> Option<Value> {
        let key = Self::format_key(project, secret);
        self.store.get_metadata(&key).await
    }

    /// Delete a secret (all versions)
    pub async fn delete_secret(&self, project: &str, secret: &str) -> bool {
        let key = Self::format_key(project, secret);
        self.store.delete_secret(&key).await
    }

    /// Check if a secret exists
    pub async fn exists(&self, project: &str, secret: &str) -> bool {
        let key = Self::format_key(project, secret);
        self.store.exists(&key).await
    }

    /// Disable a secret (disables all versions, but keeps them for history)
    pub async fn disable_secret(&self, project: &str, secret: &str) -> bool {
        let key = Self::format_key(project, secret);
        self.store.disable_secret(&key).await
    }

    /// Enable a secret (re-enables the secret, versions remain in their current state)
    pub async fn enable_secret(&self, project: &str, secret: &str) -> bool {
        let key = Self::format_key(project, secret);
        self.store.enable_secret(&key).await
    }

    /// Disable a specific version
    pub async fn disable_version(&self, project: &str, secret: &str, version_id: &str) -> bool {
        let key = Self::format_key(project, secret);
        self.store.disable_version(&key, version_id).await
    }

    /// Enable a specific version
    pub async fn enable_version(&self, project: &str, secret: &str, version_id: &str) -> bool {
        let key = Self::format_key(project, secret);
        self.store.enable_version(&key, version_id).await
    }

    /// List all secrets for a project
    /// Returns a vector of secret names (without the "projects/{project}/secrets/" prefix)
    pub async fn list_all_secrets(&self, project: &str) -> Vec<String> {
        let prefix = format!("projects/{project}/secrets/");
        let all_keys = self.store.list_all_keys().await;

        all_keys
            .iter()
            .filter_map(|key| {
                if key.starts_with(&prefix) {
                    // Extract secret name from "projects/{project}/secrets/{secret}"
                    key.strip_prefix(&prefix).map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Default for GcpSecretStore {
    fn default() -> Self {
        Self::new()
    }
}
