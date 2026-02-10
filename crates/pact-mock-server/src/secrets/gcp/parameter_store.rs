//! GCP Parameter Manager parameter store implementation
//!
//! Wraps the common SecretStore with GCP Parameter Manager-specific behavior:
//! - User-provided version IDs (e.g., "v1234567890")
//! - Parameter key format: "projects/{project}/parameters/{parameter}"

use super::super::common::{SecretStore, SecretVersion};
use serde_json::Value;

/// GCP Parameter Manager-specific parameter store wrapper
#[derive(Clone, Debug)]
pub struct GcpParameterStore {
    store: SecretStore,
}

impl GcpParameterStore {
    pub fn new() -> Self {
        Self {
            store: SecretStore::new(),
        }
    }

    /// Format GCP parameter key
    /// Format: projects/{project}/locations/{location}/parameters/{parameter}
    pub fn format_key(project: &str, location: &str, parameter: &str) -> String {
        format!(
            "projects/{}/locations/{}/parameters/{}",
            project, location, parameter
        )
    }

    /// Add a new version to a parameter
    /// Parameter Manager uses user-provided version IDs (e.g., "v1234567890")
    pub async fn add_version(
        &self,
        project: &str,
        location: &str,
        parameter: &str,
        version_data: Value,
        version_id: String, // User-provided version ID (required for Parameter Manager)
    ) -> String {
        let key = Self::format_key(project, location, parameter);
        self.store
            .add_version(
                key,
                version_data,
                Some(version_id.clone()),
                |_store, _key| {
                    // Parameter Manager: version ID is always user-provided
                    // This closure should never be called since version_id is always Some
                    unreachable!("Parameter Manager requires user-provided version IDs")
                },
            )
            .await;
        version_id
    }

    /// Update parameter metadata (format, labels, etc.)
    pub async fn update_metadata(
        &self,
        project: &str,
        location: &str,
        parameter: &str,
        metadata: Value,
    ) {
        let key = Self::format_key(project, location, parameter);
        self.store.update_metadata(key, metadata).await;
    }

    /// Get the latest version of a parameter
    pub async fn get_latest(
        &self,
        project: &str,
        location: &str,
        parameter: &str,
    ) -> Option<SecretVersion> {
        let key = Self::format_key(project, location, parameter);
        self.store.get_latest(&key).await
    }

    /// Get a specific version by version ID
    pub async fn get_version(
        &self,
        project: &str,
        location: &str,
        parameter: &str,
        version_id: &str,
    ) -> Option<SecretVersion> {
        let key = Self::format_key(project, location, parameter);
        self.store.get_version(&key, version_id).await
    }

    /// Check if a parameter exists
    pub async fn exists(&self, project: &str, location: &str, parameter: &str) -> bool {
        let key = Self::format_key(project, location, parameter);
        self.store.exists(&key).await
    }

    /// Delete a parameter
    pub async fn delete_parameter(&self, project: &str, location: &str, parameter: &str) -> bool {
        let key = Self::format_key(project, location, parameter);
        self.store.delete_secret(&key).await
    }

    /// List all versions of a parameter
    pub async fn list_versions(
        &self,
        project: &str,
        location: &str,
        parameter: &str,
    ) -> Option<Vec<SecretVersion>> {
        let key = Self::format_key(project, location, parameter);
        self.store.list_versions(&key).await
    }

    /// Get parameter metadata
    pub async fn get_metadata(
        &self,
        project: &str,
        location: &str,
        parameter: &str,
    ) -> Option<Value> {
        let key = Self::format_key(project, location, parameter);
        self.store.get_metadata(&key).await
    }

    /// Enable a parameter version
    pub async fn enable_version(
        &self,
        project: &str,
        location: &str,
        parameter: &str,
        version_id: &str,
    ) -> bool {
        let key = Self::format_key(project, location, parameter);
        self.store.enable_version(&key, version_id).await
    }

    /// Disable a parameter version
    pub async fn disable_version(
        &self,
        project: &str,
        location: &str,
        parameter: &str,
        version_id: &str,
    ) -> bool {
        let key = Self::format_key(project, location, parameter);
        self.store.disable_version(&key, version_id).await
    }

    /// Delete a parameter version
    pub async fn delete_version(
        &self,
        project: &str,
        location: &str,
        parameter: &str,
        version_id: &str,
    ) -> bool {
        let key = Self::format_key(project, location, parameter);
        self.store.delete_version(&key, version_id).await
    }

    /// List all parameters for a project and location
    /// Returns a vector of parameter names (without the "projects/{project}/locations/{location}/parameters/" prefix)
    pub async fn list_all_parameters(&self, project: &str, location: &str) -> Vec<String> {
        let prefix = format!("projects/{}/locations/{}/parameters/", project, location);
        let all_keys = self.store.list_all_keys().await;

        all_keys
            .iter()
            .filter_map(|key| {
                if key.starts_with(&prefix) {
                    // Extract parameter name from "projects/{project}/locations/{location}/parameters/{parameter}"
                    key.strip_prefix(&prefix).map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    /// List parameters for a project and location with database-level filtering by environment
    /// This queries the database directly with WHERE clauses, avoiding loading all parameters
    pub async fn list_parameters_filtered(
        &self,
        project: &str,
        location: &str,
        environment: Option<&str>,
    ) -> Vec<String> {
        // Try to use database if available
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            if let Ok(db) = sea_orm::Database::connect(&database_url).await {
                use crate::secrets::common::entities::gcp::parameter::Column as GcpParameterColumn;
                use crate::secrets::common::entities::gcp::parameter::Entity as GcpParameter;
                use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

                let prefix = format!("projects/{}/locations/{}/parameters/", project, location);

                let mut query =
                    GcpParameter::find().filter(GcpParameterColumn::Key.starts_with(&prefix));

                // Apply environment filter if provided
                if let Some(env) = environment {
                    if !env.is_empty() {
                        query = query.filter(GcpParameterColumn::Environment.eq(env));
                    }
                }

                // Apply location filter if provided (location is already in the prefix, but we can filter by column too)
                // Note: location is already filtered by the prefix, but we can add explicit filter if needed

                if let Ok(parameters) = query.all(&db).await {
                    return parameters
                        .into_iter()
                        .filter_map(|p| p.key.strip_prefix(&prefix).map(|s| s.to_string()))
                        .collect();
                }
            }
        }

        // Fallback to in-memory store (no filtering at database level)
        self.list_all_parameters(project, location).await
    }

    /// List unique environments for parameters using database function
    pub async fn list_environments(&self, project: &str, location: &str) -> Vec<String> {
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            if let Ok(db) = sea_orm::Database::connect(&database_url).await {
                use sea_orm::ConnectionTrait;
                // Use parameterized query to prevent SQL injection
                let stmt = sea_orm::Statement::from_sql_and_values(
                    sea_orm::DatabaseBackend::Postgres,
                    "SELECT * FROM gcp.get_parameter_environments($1, $2)",
                    vec![
                        sea_orm::Value::String(Some(Box::new(project.to_string()))),
                        sea_orm::Value::String(Some(Box::new(location.to_string()))),
                    ],
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

    /// List unique locations for parameters using database function
    pub async fn list_locations(&self, project: &str) -> Vec<String> {
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            if let Ok(db) = sea_orm::Database::connect(&database_url).await {
                use sea_orm::ConnectionTrait;
                // Use parameterized query to prevent SQL injection
                let stmt = sea_orm::Statement::from_sql_and_values(
                    sea_orm::DatabaseBackend::Postgres,
                    "SELECT * FROM gcp.get_parameter_locations($1)",
                    vec![sea_orm::Value::String(Some(Box::new(project.to_string())))],
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
}
