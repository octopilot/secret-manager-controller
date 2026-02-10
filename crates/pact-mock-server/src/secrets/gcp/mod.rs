//! GCP Secret Manager secret store implementation
//!
//! Wraps the common SecretStore with GCP-specific behavior:
//! - Sequential version IDs (1, 2, 3, ...)
//! - Secret key format: "projects/{project}/secrets/{secret}"

pub mod parameter_store;

pub use parameter_store::GcpParameterStore;

use super::common::{
    db_store::DbSecretStore, SecretStore, SecretStoreBackend, SecretStoreEnum, SecretVersion,
};
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

/// GCP-specific secret store wrapper
#[derive(Clone, Debug)]
pub struct GcpSecretStore {
    store: Arc<SecretStoreEnum>,
}

impl GcpSecretStore {
    pub async fn new() -> Self {
        // Check if DATABASE_URL is set - if so, use database store
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            tracing::info!(
                provider = "gcp",
                store_type = "database",
                "Initializing GCP secret store with database backend"
            );
            if let Ok(db_store) = DbSecretStore::new(&database_url, "gcp").await {
                tracing::info!(
                    provider = "gcp",
                    store_type = "database",
                    schema = "gcp",
                    "✅ GCP secret store initialized with database backend"
                );
                return Self {
                    store: Arc::new(SecretStoreEnum::Database(db_store)),
                };
            }
            // If database connection fails, fall back to in-memory store
            tracing::warn!(
                provider = "gcp",
                store_type = "in_memory",
                "Failed to connect to database, falling back to in-memory store"
            );
        } else {
            tracing::info!(
                provider = "gcp",
                store_type = "in_memory",
                "DATABASE_URL not set, using in-memory store"
            );
        }

        // Fallback to in-memory store
        Self {
            store: Arc::new(SecretStoreEnum::InMemory(SecretStore::new())),
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
    ) -> Result<String> {
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
    pub async fn update_metadata(
        &self,
        project: &str,
        secret: &str,
        metadata: Value,
    ) -> Result<()> {
        let key = Self::format_key(project, secret);
        self.store.update_metadata(key, metadata).await
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

    /// List secrets for a project with database-level filtering by environment and location
    /// This queries the database directly with WHERE clauses, avoiding loading all secrets
    pub async fn list_secrets_filtered(
        &self,
        project: &str,
        environment: Option<&str>,
        location: Option<&str>,
    ) -> Vec<String> {
        // Try to use database if available
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            if let Ok(db) = sea_orm::Database::connect(&database_url).await {
                use crate::secrets::common::entities::gcp::secret::Column as GcpSecretColumn;
                use crate::secrets::common::entities::gcp::secret::Entity as GcpSecret;
                use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

                let prefix = format!("projects/{}/secrets/", project);

                let mut query = GcpSecret::find().filter(GcpSecretColumn::Key.starts_with(&prefix));

                // Apply environment filter if provided
                if let Some(env) = environment {
                    if !env.is_empty() {
                        query = query.filter(GcpSecretColumn::Environment.eq(env));
                    }
                }

                // Apply location filter if provided
                if let Some(loc) = location {
                    if !loc.is_empty() {
                        query = query.filter(GcpSecretColumn::Location.eq(loc));
                    }
                }

                if let Ok(secrets) = query.all(&db).await {
                    return secrets
                        .into_iter()
                        .filter_map(|s| s.key.strip_prefix(&prefix).map(|s| s.to_string()))
                        .collect();
                }
            }
        }

        // Fallback to in-memory store (no filtering at database level)
        self.list_all_secrets(project).await
    }

    /// List all projects that have secrets
    /// Returns a vector of unique project IDs extracted from secret keys
    /// Uses database function if available, otherwise falls back to in-memory store
    pub async fn list_all_projects(&self) -> Vec<String> {
        // Try to use existing database store connection first (much faster)
        if let SecretStoreEnum::Database(db_store) = self.store.as_ref() {
            let projects = db_store.list_gcp_projects().await;
            if !projects.is_empty() {
                return projects;
            }
        }

        // Fallback to in-memory store
        let all_keys = self.store.list_all_keys().await;
        let mut projects = std::collections::HashSet::new();

        for key in all_keys {
            // Extract project from "projects/{project}/secrets/{secret}"
            if let Some(project_start) = key.strip_prefix("projects/") {
                if let Some(project_end) = project_start.find("/secrets/") {
                    let project = &project_start[..project_end];
                    projects.insert(project.to_string());
                }
            }
        }

        let mut project_list: Vec<String> = projects.into_iter().collect();
        project_list.sort();
        project_list
    }

    /// List unique environments for secrets using database function
    /// Returns empty vec if database is not available
    /// CRITICAL: Requires both project AND location - different locations can have different environments
    pub async fn list_environments(
        &self,
        project: &str,
        location: Option<&str>,
        resource_type: &str,
    ) -> Vec<String> {
        // Try to use existing database store connection first (much faster)
        if let SecretStoreEnum::Database(db_store) = self.store.as_ref() {
            if resource_type != "secrets" {
                // For parameters, we need location too - but this method doesn't have it
                // This should be called from parameter_store.rs instead
                return vec![];
            }

            // Location is required for secrets too - different locations can have different environments
            let location_value = match location {
                Some(loc) => sea_orm::Value::String(Some(Box::new(loc.to_string()))),
                None => sea_orm::Value::String(None),
            };

            // Use parameterized query to prevent SQL injection
            let stmt = sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Postgres,
                "SELECT * FROM gcp.get_secret_environments($1, $2)",
                vec![
                    sea_orm::Value::String(Some(Box::new(project.to_string()))),
                    location_value,
                ],
            );

            if let Ok(rows) = db_store.query_all(stmt).await {
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
        vec![]
    }

    /// List unique locations for secrets using database function
    /// Returns empty vec if database is not available
    pub async fn list_locations(&self, project: &str, resource_type: &str) -> Vec<String> {
        // Try to use existing database store connection first (much faster)
        if let SecretStoreEnum::Database(db_store) = self.store.as_ref() {
            let stmt = if resource_type == "secrets" {
                // Use parameterized query to prevent SQL injection
                sea_orm::Statement::from_sql_and_values(
                    sea_orm::DatabaseBackend::Postgres,
                    "SELECT * FROM gcp.get_secret_locations($1)",
                    vec![sea_orm::Value::String(Some(Box::new(project.to_string())))],
                )
            } else {
                // For parameters, we need location too - but this method doesn't have it
                // This should be called from parameter_store.rs instead
                return vec![];
            };

            if let Ok(rows) = db_store.query_all(stmt).await {
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
        vec![]
    }
}

// Note: Cannot implement Default for async new() - use GcpSecretStore::new().await instead
