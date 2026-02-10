//! AWS Secrets Manager secret store implementation
//!
//! Wraps the common SecretStore with AWS-specific behavior:
//! - UUID-like version IDs
//! - Staging labels (AWSCURRENT, AWSPREVIOUS)
//! - Secret key format: secret name (no path prefix)

use super::common::{
    db_store::DbSecretStore, SecretStore, SecretStoreBackend, SecretStoreEnum, SecretVersion,
};
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// AWS staging labels
pub const AWS_CURRENT: &str = "AWSCURRENT";
pub const AWS_PREVIOUS: &str = "AWSPREVIOUS";

/// AWS-specific secret store wrapper
#[derive(Clone, Debug)]
pub struct AwsSecretStore {
    store: Arc<SecretStoreEnum>,
    /// Maps staging labels to version IDs for each secret
    /// Key: secret name, Value: HashMap of label -> version_id
    /// Note: For database store, staging labels are stored in aws.staging_labels table
    staging_labels: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
}

impl AwsSecretStore {
    pub async fn new() -> Self {
        // Check if DATABASE_URL is set - if so, use database store
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            tracing::info!(
                provider = "aws",
                store_type = "database",
                "Initializing AWS secret store with database backend"
            );
            if let Ok(db_store) = DbSecretStore::new(&database_url, "aws").await {
                tracing::info!(
                    provider = "aws",
                    store_type = "database",
                    schema = "aws",
                    "✅ AWS secret store initialized with database backend"
                );
                return Self {
                    store: Arc::new(SecretStoreEnum::Database(db_store)),
                    staging_labels: Arc::new(RwLock::new(HashMap::new())),
                };
            }
            // If database connection fails, fall back to in-memory store
            tracing::warn!(
                provider = "aws",
                store_type = "in_memory",
                "Failed to connect to database, falling back to in-memory store"
            );
        } else {
            tracing::info!(
                provider = "aws",
                store_type = "in_memory",
                "DATABASE_URL not set, using in-memory store"
            );
        }

        // Fallback to in-memory store
        Self {
            store: Arc::new(SecretStoreEnum::InMemory(SecretStore::new())),
            staging_labels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate UUID-like version ID for AWS
    fn generate_version_id(secret_name: &str, timestamp: u64) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        secret_name.hash(&mut hasher);
        timestamp.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Add a new version to a secret
    /// AWS uses UUID-like version IDs and manages staging labels
    pub async fn add_version(
        &self,
        secret_name: &str,
        version_data: Value,
        version_id: Option<String>,
    ) -> Result<String> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let new_version_id =
            version_id.unwrap_or_else(|| Self::generate_version_id(secret_name, timestamp));

        // Get current version ID before adding new one (for AWSPREVIOUS label)
        let current_version_id = {
            let labels = self.staging_labels.read().await;
            labels
                .get(secret_name)
                .and_then(|labels| labels.get(AWS_CURRENT))
                .cloned()
        };

        // Add the version to the store
        let version_id = self
            .store
            .add_version(
                secret_name.to_string(),
                version_data,
                Some(new_version_id.clone()),
                move |_, _| new_version_id.clone(), // Not used since we provide version_id
            )
            .await?;

        // Update staging labels
        let mut labels = self.staging_labels.write().await;
        let secret_labels = labels
            .entry(secret_name.to_string())
            .or_insert_with(HashMap::new);

        // Move AWSCURRENT to AWSPREVIOUS if it exists
        if let Some(prev_current) = current_version_id {
            secret_labels.insert(AWS_PREVIOUS.to_string(), prev_current);
        }

        // Set new version as AWSCURRENT
        secret_labels.insert(AWS_CURRENT.to_string(), version_id.clone());

        Ok(version_id)
    }

    /// Get version by staging label
    pub async fn get_version_by_label(
        &self,
        secret_name: &str,
        label: &str,
    ) -> Option<SecretVersion> {
        let labels = self.staging_labels.read().await;
        if let Some(secret_labels) = labels.get(secret_name) {
            if let Some(version_id) = secret_labels.get(label) {
                return self.store.get_version(secret_name, version_id).await;
            }
        }
        None
    }

    /// Get the current version (AWSCURRENT)
    pub async fn get_current(&self, secret_name: &str) -> Option<SecretVersion> {
        self.get_version_by_label(secret_name, AWS_CURRENT).await
    }

    /// Get the previous version (AWSPREVIOUS)
    pub async fn get_previous(&self, secret_name: &str) -> Option<SecretVersion> {
        self.get_version_by_label(secret_name, AWS_PREVIOUS).await
    }

    /// Get the latest version (same as current for AWS)
    pub async fn get_latest(&self, secret_name: &str) -> Option<SecretVersion> {
        self.get_current(secret_name).await
    }

    /// Get a specific version by version ID
    pub async fn get_version(&self, secret_name: &str, version_id: &str) -> Option<SecretVersion> {
        self.store.get_version(secret_name, version_id).await
    }

    /// List all versions of a secret
    pub async fn list_versions(&self, secret_name: &str) -> Option<Vec<SecretVersion>> {
        self.store.list_versions(secret_name).await
    }

    /// Get staging labels for a secret
    pub async fn get_staging_labels(&self, secret_name: &str) -> Option<HashMap<String, String>> {
        let labels = self.staging_labels.read().await;
        labels.get(secret_name).cloned()
    }

    /// Update staging labels for a secret
    /// Moves a label from one version to another
    pub async fn update_staging_label(
        &self,
        secret_name: &str,
        label: &str,
        remove_from_version_id: Option<&str>,
        move_to_version_id: &str,
    ) -> bool {
        // Verify both versions exist
        if let Some(remove_vid) = remove_from_version_id {
            if self
                .store
                .get_version(secret_name, remove_vid)
                .await
                .is_none()
            {
                return false;
            }
        }
        if self
            .store
            .get_version(secret_name, move_to_version_id)
            .await
            .is_none()
        {
            return false;
        }

        let mut labels = self.staging_labels.write().await;
        let secret_labels = labels
            .entry(secret_name.to_string())
            .or_insert_with(HashMap::new);

        // Remove label from old version if specified
        if let Some(remove_vid) = remove_from_version_id {
            // Find and remove any label pointing to the old version
            let labels_to_remove: Vec<String> = secret_labels
                .iter()
                .filter(|(_, vid)| **vid == remove_vid)
                .map(|(l, _)| l.clone())
                .collect();
            for label_to_remove in labels_to_remove {
                secret_labels.remove(&label_to_remove);
            }
        }

        // Add label to new version
        secret_labels.insert(label.to_string(), move_to_version_id.to_string());

        // If moving AWSCURRENT, update AWSPREVIOUS
        if label == AWS_CURRENT {
            if let Some(remove_vid) = remove_from_version_id {
                secret_labels.insert(AWS_PREVIOUS.to_string(), remove_vid.to_string());
            }
        }

        true
    }

    /// Get secret metadata
    pub async fn get_metadata(&self, secret_name: &str) -> Option<Value> {
        self.store.get_metadata(secret_name).await
    }

    /// Delete a secret (all versions and labels)
    pub async fn delete_secret(&self, secret_name: &str) -> bool {
        let deleted = self.store.delete_secret(secret_name).await;
        if deleted {
            let mut labels = self.staging_labels.write().await;
            labels.remove(secret_name);
        }
        deleted
    }

    /// Check if a secret exists
    pub async fn exists(&self, secret_name: &str) -> bool {
        self.store.exists(secret_name).await
    }

    /// List all secret names
    pub async fn list_all_secrets(&self) -> Vec<String> {
        self.store.list_all_keys().await
    }

    /// Delete a secret (marks for deletion, can be restored)
    /// AWS uses deletion with recovery window instead of explicit disable
    /// Note: recovery_window_days is stored for future use (automatic cleanup after window)
    pub async fn delete_secret_with_recovery(
        &self,
        secret_name: &str,
        _recovery_window_days: Option<u32>,
    ) -> bool {
        // Mark secret as disabled (deleted) but keep it for recovery
        // TODO: Store recovery_window_days and implement automatic cleanup
        self.store.disable_secret(secret_name).await
    }

    /// Restore a deleted secret
    pub async fn restore_secret(&self, secret_name: &str) -> bool {
        self.store.enable_secret(secret_name).await
    }

    /// Check if a secret is deleted (disabled)
    pub async fn is_deleted(&self, secret_name: &str) -> bool {
        !self.store.is_enabled(secret_name).await
    }

    /// List unique environments for secrets using database function
    /// Returns empty vec if database is not available
    pub async fn list_environments(&self, resource_type: &str) -> Vec<String> {
        // Try to get DATABASE_URL from environment
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            if let Ok(db) = sea_orm::Database::connect(&database_url).await {
                use sea_orm::ConnectionTrait;
                let function_name = if resource_type == "secrets" {
                    "aws.get_secret_environments"
                } else {
                    "aws.get_parameter_environments"
                };

                let stmt = sea_orm::Statement::from_string(
                    sea_orm::DatabaseBackend::Postgres,
                    format!("SELECT * FROM {}()", function_name),
                );

                if let Ok(rows) = db.query_all(stmt).await {
                    return rows
                        .into_iter()
                        .filter_map(|row| {
                            // Get the environment column value by column name
                            row.try_get::<Option<String>>("", "environment")
                                .or_else(|_| row.try_get::<String>("", "environment").map(Some))
                                .ok()
                                .flatten()
                        })
                        .collect();
                }
            }
        }
        vec![]
    }

    /// List unique locations for secrets using database function
    /// Returns empty vec if database is not available
    pub async fn list_locations(&self, resource_type: &str) -> Vec<String> {
        // Try to get DATABASE_URL from environment
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            if let Ok(db) = sea_orm::Database::connect(&database_url).await {
                use sea_orm::ConnectionTrait;
                let function_name = if resource_type == "secrets" {
                    "aws.get_secret_locations"
                } else {
                    "aws.get_parameter_locations"
                };

                let stmt = sea_orm::Statement::from_string(
                    sea_orm::DatabaseBackend::Postgres,
                    format!("SELECT * FROM {}()", function_name),
                );

                if let Ok(rows) = db.query_all(stmt).await {
                    return rows
                        .into_iter()
                        .filter_map(|row| {
                            // Get the location column value by column name
                            row.try_get::<Option<String>>("", "location")
                                .or_else(|_| row.try_get::<String>("", "location").map(Some))
                                .ok()
                                .flatten()
                        })
                        .collect();
                }
            }
        }
        vec![]
    }

    /// List all projects/accounts that have secrets
    /// AWS doesn't have projects like GCP, but we can extract account IDs from ARNs
    /// Returns empty vec if database is not available
    pub async fn list_all_projects(&self) -> Vec<String> {
        // Try to use database function first
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            if let Ok(db) = sea_orm::Database::connect(&database_url).await {
                use sea_orm::ConnectionTrait;
                let stmt = sea_orm::Statement::from_string(
                    sea_orm::DatabaseBackend::Postgres,
                    "SELECT DISTINCT (metadata->>'ARN')::text as arn FROM aws.secrets WHERE metadata->>'ARN' IS NOT NULL".to_string(),
                );

                if let Ok(rows) = db.query_all(stmt).await {
                    let mut accounts = std::collections::HashSet::new();
                    for row in rows {
                        if let Ok(Some(arn)) = row.try_get::<Option<String>>("", "arn") {
                            // Extract account ID from ARN: arn:aws:secretsmanager:region:account-id:secret:name
                            let parts: Vec<&str> = arn.split(':').collect();
                            if parts.len() >= 5 {
                                accounts.insert(parts[4].to_string());
                            }
                        }
                    }
                    let mut account_list: Vec<String> = accounts.into_iter().collect();
                    account_list.sort();
                    if !account_list.is_empty() {
                        return account_list;
                    }
                }
            }
        }

        // Fallback to in-memory store - extract from metadata
        let all_keys = self.store.list_all_keys().await;
        let mut accounts = std::collections::HashSet::new();
        for key in all_keys {
            if let Some(metadata) = self.store.get_metadata(&key).await {
                if let Some(arn) = metadata.get("ARN").and_then(|v| v.as_str()) {
                    // Extract account ID from ARN
                    let parts: Vec<&str> = arn.split(':').collect();
                    if parts.len() >= 5 {
                        accounts.insert(parts[4].to_string());
                    }
                }
            }
        }
        let mut account_list: Vec<String> = accounts.into_iter().collect();
        account_list.sort();
        account_list
    }
}

// Note: Cannot implement Default for async new() - use AwsSecretStore::new().await instead
