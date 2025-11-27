//! Database-backed secret store implementation using SeaORM
//!
//! Provides persistent storage for secrets using PostgreSQL via SeaORM.
//! Each provider uses a separate schema to isolate data.
//! This replaces the in-memory store to persist data across pod restarts.

// Import entity types and their ActiveModel types directly
use super::entities::aws::secret::{ActiveModel as AwsSecretActiveModel, Entity as AwsSecret};
use super::entities::aws::version::{ActiveModel as AwsVersionActiveModel, Entity as AwsVersion};
use super::entities::azure::secret::{
    ActiveModel as AzureSecretActiveModel, Entity as AzureSecret,
};
use super::entities::azure::version::{
    ActiveModel as AzureVersionActiveModel, Entity as AzureVersion,
};
use super::entities::gcp::secret::{ActiveModel as GcpSecretActiveModel, Entity as GcpSecret};
use super::entities::gcp::version::{ActiveModel as GcpVersionActiveModel, Entity as GcpVersion};
use super::store::{SecretEntry, SecretVersion};
use anyhow::{Context, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::info;
use tracing::warn;

/// Database-backed secret store with versioning support
///
/// Stores secrets in PostgreSQL using SeaORM with provider-specific schemas:
/// - GCP: secrets, versions, parameters, parameter_versions
/// - AWS: secrets, versions, staging_labels
/// - Azure: secrets, versions, deleted_secrets
///
/// Each provider uses a separate schema (gcp, aws, azure) for isolation.
/// This is persistent - data survives pod restarts.
#[derive(Clone, Debug)]
pub struct DbSecretStore {
    db: DatabaseConnection,
    schema: String, // Schema name (gcp, aws, or azure)
}

impl DbSecretStore {
    /// Create a new database store, connecting to PostgreSQL and initializing schema
    ///
    /// # Arguments
    /// * `connection_string` - PostgreSQL connection string (e.g., "postgresql://user:pass@host/dbname")
    /// * `schema` - Schema name for this provider (e.g., "gcp", "aws", "azure")
    pub async fn new(connection_string: &str, schema: &str) -> Result<Self> {
        let db = Database::connect(connection_string)
            .await
            .context("Failed to connect to PostgreSQL")?;

        let store = Self {
            db,
            schema: schema.to_string(),
        };

        // Initialize schema
        store.init_schema().await?;

        info!("Initialized PostgreSQL store with schema: {}", schema);

        Ok(store)
    }

    /// Initialize the database schema for this provider
    async fn init_schema(&self) -> Result<()> {
        use sea_orm::ConnectionTrait;
        use sea_orm::Statement;

        let schema = self.schema.clone();

        // Create schema if it doesn't exist
        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!("CREATE SCHEMA IF NOT EXISTS {}", schema),
            ))
            .await
            .context("Failed to create schema")?;

        // Create tables based on provider
        match schema.as_str() {
            "gcp" => self.init_gcp_schema().await?,
            "aws" => self.init_aws_schema().await?,
            "azure" => self.init_azure_schema().await?,
            _ => anyhow::bail!("Unknown schema: {}", schema),
        }

        Ok(())
    }

    /// Initialize GCP schema tables
    async fn init_gcp_schema(&self) -> Result<()> {
        use sea_orm::ConnectionTrait;
        use sea_orm::Statement;

        let schema = self.schema.clone();

        // Create secrets table
        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "CREATE TABLE IF NOT EXISTS {}.secrets (
                        key TEXT PRIMARY KEY,
                        disabled BOOLEAN NOT NULL DEFAULT FALSE,
                        metadata JSONB NOT NULL DEFAULT '{{}}'::jsonb,
                        created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
                        updated_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT
                    )",
                    schema
                ),
            ))
            .await
            .context("Failed to create GCP secrets table")?;

        // Create versions table
        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "CREATE TABLE IF NOT EXISTS {}.versions (
                        secret_key TEXT NOT NULL,
                        version_id TEXT NOT NULL,
                        data JSONB NOT NULL,
                        enabled BOOLEAN NOT NULL DEFAULT TRUE,
                        created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
                        PRIMARY KEY (secret_key, version_id),
                        FOREIGN KEY (secret_key) REFERENCES {}.secrets(key) ON DELETE CASCADE
                    )",
                    schema, schema
                ),
            ))
            .await
            .context("Failed to create GCP versions table")?;

        // Create parameters table
        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "CREATE TABLE IF NOT EXISTS {}.parameters (
                        key TEXT PRIMARY KEY,
                        metadata JSONB NOT NULL DEFAULT '{{}}'::jsonb,
                        created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
                        updated_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT
                    )",
                    schema
                ),
            ))
            .await
            .context("Failed to create GCP parameters table")?;

        // Create parameter_versions table
        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "CREATE TABLE IF NOT EXISTS {}.parameter_versions (
                        parameter_key TEXT NOT NULL,
                        version_id TEXT NOT NULL,
                        data JSONB NOT NULL,
                        created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
                        PRIMARY KEY (parameter_key, version_id),
                        FOREIGN KEY (parameter_key) REFERENCES {}.parameters(key) ON DELETE CASCADE
                    )",
                    schema, schema
                ),
            ))
            .await
            .context("Failed to create GCP parameter_versions table")?;

        // Create indexes
        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "CREATE INDEX IF NOT EXISTS idx_{}_versions_secret_key ON {}.versions(secret_key)",
                    schema, schema
                ),
            ))
            .await
            .context("Failed to create index")?;

        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "CREATE INDEX IF NOT EXISTS idx_{}_versions_created_at ON {}.versions(secret_key, created_at)",
                    schema, schema
                ),
            ))
            .await
            .context("Failed to create index")?;

        Ok(())
    }

    /// Initialize AWS schema tables
    async fn init_aws_schema(&self) -> Result<()> {
        use sea_orm::ConnectionTrait;
        use sea_orm::Statement;

        let schema = self.schema.clone();

        // Create secrets table
        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "CREATE TABLE IF NOT EXISTS {}.secrets (
                        name TEXT PRIMARY KEY,
                        disabled BOOLEAN NOT NULL DEFAULT FALSE,
                        metadata JSONB NOT NULL DEFAULT '{{}}'::jsonb,
                        created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
                        updated_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT
                    )",
                    schema
                ),
            ))
            .await
            .context("Failed to create AWS secrets table")?;

        // Create versions table
        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "CREATE TABLE IF NOT EXISTS {}.versions (
                        secret_name TEXT NOT NULL,
                        version_id TEXT NOT NULL,
                        data JSONB NOT NULL,
                        enabled BOOLEAN NOT NULL DEFAULT TRUE,
                        created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
                        PRIMARY KEY (secret_name, version_id),
                        FOREIGN KEY (secret_name) REFERENCES {}.secrets(name) ON DELETE CASCADE
                    )",
                    schema, schema
                ),
            ))
            .await
            .context("Failed to create AWS versions table")?;

        // Create staging_labels table
        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "CREATE TABLE IF NOT EXISTS {}.staging_labels (
                        secret_name TEXT NOT NULL,
                        label TEXT NOT NULL,
                        version_id TEXT NOT NULL,
                        created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
                        updated_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
                        PRIMARY KEY (secret_name, label),
                        FOREIGN KEY (secret_name) REFERENCES {}.secrets(name) ON DELETE CASCADE
                    )",
                    schema, schema
                ),
            ))
            .await
            .context("Failed to create AWS staging_labels table")?;

        // Create indexes
        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "CREATE INDEX IF NOT EXISTS idx_{}_versions_secret_name ON {}.versions(secret_name)",
                    schema, schema
                ),
            ))
            .await
            .context("Failed to create index")?;

        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "CREATE INDEX IF NOT EXISTS idx_{}_staging_labels_secret_name ON {}.staging_labels(secret_name)",
                    schema, schema
                ),
            ))
            .await
            .context("Failed to create index")?;

        Ok(())
    }

    /// Initialize Azure schema tables
    async fn init_azure_schema(&self) -> Result<()> {
        use sea_orm::ConnectionTrait;
        use sea_orm::Statement;

        let schema = self.schema.clone();

        // Create secrets table
        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "CREATE TABLE IF NOT EXISTS {}.secrets (
                        name TEXT PRIMARY KEY,
                        disabled BOOLEAN NOT NULL DEFAULT FALSE,
                        metadata JSONB NOT NULL DEFAULT '{{}}'::jsonb,
                        created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
                        updated_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT
                    )",
                    schema
                ),
            ))
            .await
            .context("Failed to create Azure secrets table")?;

        // Create versions table
        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "CREATE TABLE IF NOT EXISTS {}.versions (
                        secret_name TEXT NOT NULL,
                        version_id TEXT NOT NULL,
                        data JSONB NOT NULL,
                        enabled BOOLEAN NOT NULL DEFAULT TRUE,
                        created_at BIGINT NOT NULL DEFAULT EXTRACT(EPOCH FROM NOW())::BIGINT,
                        PRIMARY KEY (secret_name, version_id),
                        FOREIGN KEY (secret_name) REFERENCES {}.secrets(name) ON DELETE CASCADE
                    )",
                    schema, schema
                ),
            ))
            .await
            .context("Failed to create Azure versions table")?;

        // Create deleted_secrets table
        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "CREATE TABLE IF NOT EXISTS {}.deleted_secrets (
                        secret_name TEXT PRIMARY KEY,
                        deleted_date BIGINT NOT NULL,
                        scheduled_purge_date BIGINT NOT NULL,
                        FOREIGN KEY (secret_name) REFERENCES {}.secrets(name) ON DELETE CASCADE
                    )",
                    schema, schema
                ),
            ))
            .await
            .context("Failed to create Azure deleted_secrets table")?;

        // Create indexes
        self.db
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!(
                    "CREATE INDEX IF NOT EXISTS idx_{}_versions_secret_name ON {}.versions(secret_name)",
                    schema, schema
                ),
            ))
            .await
            .context("Failed to create index")?;

        Ok(())
    }

    /// Add a new version to a secret (or create the secret if it doesn't exist)
    pub async fn add_version<F>(
        &self,
        key: String,
        version_data: Value,
        version_id: Option<String>,
        version_id_generator: F,
    ) -> Result<String>
    where
        F: FnOnce(&HashMap<String, SecretEntry>, &str) -> String + Send + 'static,
    {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Generate version ID if not provided
        let existing_versions = self.list_versions(&key).await.unwrap_or_default();
        let new_version_id = version_id.unwrap_or_else(|| {
            // Create a minimal snapshot for the generator
            let mut snapshot = HashMap::new();
            if !existing_versions.is_empty() {
                let entry = SecretEntry {
                    versions: existing_versions,
                    disabled: false,
                    metadata: json!({}),
                };
                snapshot.insert(key.clone(), entry);
            }
            version_id_generator(&snapshot, &key)
        });

        match self.schema.as_str() {
            "gcp" => {
                self.add_gcp_version(key, version_data, new_version_id, timestamp)
                    .await
            }
            "aws" => {
                self.add_aws_version(key, version_data, new_version_id, timestamp)
                    .await
            }
            "azure" => {
                self.add_azure_version(key, version_data, new_version_id, timestamp)
                    .await
            }
            _ => anyhow::bail!("Unknown schema: {}", self.schema),
        }
    }

    /// Extract environment from metadata labels
    fn extract_environment(metadata: &Value) -> Option<String> {
        metadata.get("labels").and_then(|labels| {
            labels
                .get("environment")
                .or_else(|| labels.get("Environment"))
                .or_else(|| labels.get("env"))
                .or_else(|| labels.get("Env"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
    }

    /// Extract location from metadata labels (for secrets) or key format (for parameters)
    fn extract_location_from_labels(metadata: &Value) -> Option<String> {
        metadata.get("labels").and_then(|labels| {
            labels
                .get("location")
                .or_else(|| labels.get("Location"))
                .or_else(|| labels.get("region"))
                .or_else(|| labels.get("Region"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
    }

    /// Extract environment from AWS Tags array format: [{"Key": "key", "Value": "value"}, ...]
    fn extract_environment_from_aws_tags(metadata: &Value) -> Option<String> {
        metadata
            .get("Tags")
            .and_then(|tags| tags.as_array())
            .and_then(|tags_array| {
                tags_array
                    .iter()
                    .find(|tag| {
                        tag.get("Key")
                            .and_then(|k| k.as_str())
                            .map(|k| {
                                k == "Environment" || k == "environment" || k == "Env" || k == "env"
                            })
                            .unwrap_or(false)
                    })
                    .and_then(|tag| tag.get("Value"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
    }

    /// Extract location from AWS Tags array or ARN
    fn extract_location_from_aws_tags(metadata: &Value) -> Option<String> {
        // First try Tags array
        if let Some(location) = metadata
            .get("Tags")
            .and_then(|tags| tags.as_array())
            .and_then(|tags_array| {
                tags_array
                    .iter()
                    .find(|tag| {
                        tag.get("Key")
                            .and_then(|k| k.as_str())
                            .map(|k| {
                                k == "Location" || k == "location" || k == "Region" || k == "region"
                            })
                            .unwrap_or(false)
                    })
                    .and_then(|tag| tag.get("Value"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
        {
            return Some(location);
        }

        // Fallback: extract from ARN if present
        metadata
            .get("ARN")
            .and_then(|arn| arn.as_str())
            .and_then(|arn_str| {
                // ARN format: arn:aws:service:region:account:resource
                // Extract region (4th segment)
                let parts: Vec<&str> = arn_str.split(':').collect();
                if parts.len() >= 5 {
                    Some(parts[4].to_string())
                } else {
                    None
                }
            })
    }

    /// Extract environment from Azure tags object format: { "key": "value", ... }
    fn extract_environment_from_azure_tags(metadata: &Value) -> Option<String> {
        metadata.get("tags").and_then(|tags| {
            tags.get("Environment")
                .or_else(|| tags.get("environment"))
                .or_else(|| tags.get("Env"))
                .or_else(|| tags.get("env"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
    }

    /// Extract location from Azure tags object format: { "key": "value", ... }
    fn extract_location_from_azure_tags(metadata: &Value) -> Option<String> {
        metadata.get("tags").and_then(|tags| {
            tags.get("Location")
                .or_else(|| tags.get("location"))
                .or_else(|| tags.get("Region"))
                .or_else(|| tags.get("region"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
    }

    async fn add_gcp_version(
        &self,
        key: String,
        version_data: Value,
        version_id: String,
        timestamp: u64,
    ) -> Result<String> {
        // Ensure secret exists - use existing metadata if available
        let secret = GcpSecret::find_by_id(&key).one(&self.db).await?;
        let (environment, location) = if let Some(existing_secret) = &secret {
            // Use existing environment/location from database
            let env = existing_secret.environment.clone();
            let loc = existing_secret.location.clone();

            // For backward compatibility: if secret exists but has NULL environment/location,
            // allow the version to be added but log a warning
            // This handles secrets created before validation was added
            if env.is_none() || env.as_ref().unwrap().is_empty() {
                warn!(
                    schema = "gcp",
                    secret_key = key,
                    operation = "add_version",
                    "Secret has NULL or empty environment. This secret was likely created before validation was added. Consider recreating the secret with proper labels.",
                );
            }
            if loc.is_none() || loc.as_ref().unwrap().is_empty() {
                warn!(
                    schema = "gcp",
                    secret_key = key,
                    operation = "add_version",
                    "Secret has NULL or empty location. This secret was likely created before validation was added. Consider recreating the secret with proper labels.",
                );
            }

            // Use existing values or default to None for backward compatibility
            (env, loc)
        } else {
            // Secret doesn't exist - reject because create_secret should have been called first
            anyhow::bail!(
                "Secret {} does not exist. Call create_secret with labels containing 'environment' and 'location' before adding versions.",
                key
            );
        };

        // Insert version
        let version_model = GcpVersionActiveModel {
            secret_key: Set(key.clone()),
            version_id: Set(version_id.clone()),
            data: Set(version_data),
            enabled: Set(true),
            created_at: Set(timestamp as i64),
        };
        version_model.insert(&self.db).await?;

        info!(
            schema = "gcp",
            secret_key = key,
            version_id = version_id,
            environment = environment.as_deref().unwrap_or("None"),
            location = location.as_deref().unwrap_or("None"),
            operation = "add_version",
            "Added version to database: key={}, version={}, environment={:?}, location={:?}",
            key,
            version_id,
            environment,
            location
        );
        Ok(version_id)
    }

    async fn add_aws_version(
        &self,
        name: String,
        version_data: Value,
        version_id: String,
        timestamp: u64,
    ) -> Result<String> {
        // Ensure secret exists - use existing metadata if available
        let secret = AwsSecret::find_by_id(&name).one(&self.db).await?;
        let (environment, location) = if let Some(existing_secret) = &secret {
            // Use existing environment/location from database
            (
                existing_secret.environment.clone(),
                existing_secret.location.clone(),
            )
        } else {
            // Secret doesn't exist - create with empty metadata
            // Metadata will be set later via update_metadata() call
            let metadata = json!({});
            let environment = Self::extract_environment_from_aws_tags(&metadata);
            let location = Self::extract_location_from_aws_tags(&metadata);

            info!(
                schema = "aws",
                secret_name = name,
                environment = environment.as_deref().unwrap_or("None"),
                location = location.as_deref().unwrap_or("None"),
                operation = "add_version",
                action = "create_secret",
                "Creating new secret in database: name={}, environment={:?}, location={:?}",
                name,
                environment,
                location
            );

            let secret_model = AwsSecretActiveModel {
                name: Set(name.clone()),
                disabled: Set(false),
                metadata: Set(metadata),
                environment: Set(environment.clone()),
                location: Set(location.clone()),
                created_at: Set(timestamp as i64),
                updated_at: Set(timestamp as i64),
            };
            secret_model.insert(&self.db).await?;
            (environment, location)
        };

        // Insert version
        let version_model = AwsVersionActiveModel {
            secret_name: Set(name.clone()),
            version_id: Set(version_id.clone()),
            data: Set(version_data),
            enabled: Set(true),
            created_at: Set(timestamp as i64),
        };
        version_model.insert(&self.db).await?;

        info!(
            schema = "aws",
            secret_name = name,
            version_id = version_id,
            environment = environment.as_deref().unwrap_or("None"),
            location = location.as_deref().unwrap_or("None"),
            operation = "add_version",
            "Added version to database: name={}, version={}, environment={:?}, location={:?}",
            name,
            version_id,
            environment,
            location
        );
        Ok(version_id)
    }

    async fn add_azure_version(
        &self,
        name: String,
        version_data: Value,
        version_id: String,
        timestamp: u64,
    ) -> Result<String> {
        // Ensure secret exists - use existing metadata if available
        let secret = AzureSecret::find_by_id(&name).one(&self.db).await?;
        let (environment, location) = if let Some(existing_secret) = &secret {
            // Use existing environment/location from database
            (
                existing_secret.environment.clone(),
                existing_secret.location.clone(),
            )
        } else {
            // Secret doesn't exist - create with empty metadata
            // Metadata will be set later via update_metadata() call
            let metadata = json!({});
            let environment = Self::extract_environment_from_azure_tags(&metadata);
            let location = Self::extract_location_from_azure_tags(&metadata);

            info!(
                schema = "azure",
                secret_name = name,
                environment = environment.as_deref().unwrap_or("None"),
                location = location.as_deref().unwrap_or("None"),
                operation = "add_version",
                action = "create_secret",
                "Creating new secret in database: name={}, environment={:?}, location={:?}",
                name,
                environment,
                location
            );

            let secret_model = AzureSecretActiveModel {
                name: Set(name.clone()),
                disabled: Set(false),
                metadata: Set(metadata),
                environment: Set(environment.clone()),
                location: Set(location.clone()),
                created_at: Set(timestamp as i64),
                updated_at: Set(timestamp as i64),
            };
            secret_model.insert(&self.db).await?;
            (environment, location)
        };

        // Insert version
        let version_model = AzureVersionActiveModel {
            secret_name: Set(name.clone()),
            version_id: Set(version_id.clone()),
            data: Set(version_data),
            enabled: Set(true),
            created_at: Set(timestamp as i64),
        };
        version_model.insert(&self.db).await?;

        info!(
            schema = "azure",
            secret_name = name,
            version_id = version_id,
            environment = environment.as_deref().unwrap_or("None"),
            location = location.as_deref().unwrap_or("None"),
            operation = "add_version",
            "Added version to database: name={}, version={}, environment={:?}, location={:?}",
            name,
            version_id,
            environment,
            location
        );
        Ok(version_id)
    }

    /// Update secret metadata
    /// Creates the secret if it doesn't exist (upsert pattern)
    pub async fn update_metadata(&self, key: String, metadata: Value) -> Result<()> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        match self.schema.as_str() {
            "gcp" => {
                let secret = GcpSecret::find_by_id(&key).one(&self.db).await?;
                // Extract environment and location from updated metadata
                let environment = Self::extract_environment(&metadata);
                // For GCP, location is extracted from labels, but for automatic replication
                // it should be NULL (not "automatic"). If location label is "automatic", set to None
                let location = Self::extract_location_from_labels(&metadata).and_then(|loc| {
                    if loc == "automatic" {
                        None
                    } else {
                        Some(loc)
                    }
                });

                if let Some(existing_secret) = secret {
                    // Update existing secret
                    info!(
                        schema = "gcp",
                        secret_key = key,
                        environment = environment.as_deref().unwrap_or("None"),
                        location = location.as_deref().unwrap_or("None"),
                        operation = "update_metadata",
                        action = "update",
                        "Updating secret metadata in database: key={}, environment={:?}, location={:?}",
                        key, environment, location
                    );
                    let mut secret_model: GcpSecretActiveModel = existing_secret.into();
                    secret_model.metadata = Set(metadata);
                    secret_model.environment = Set(environment.clone());
                    secret_model.location = Set(location.clone());
                    secret_model.updated_at = Set(timestamp as i64);
                    secret_model.update(&self.db).await?;
                } else {
                    // Create new secret with metadata
                    info!(
                        schema = "gcp",
                        secret_key = key,
                        environment = environment.as_deref().unwrap_or("None"),
                        location = location.as_deref().unwrap_or("None"),
                        operation = "update_metadata",
                        action = "create",
                        "Creating secret with metadata in database: key={}, environment={:?}, location={:?}",
                        key, environment, location
                    );
                    let secret_model = GcpSecretActiveModel {
                        key: Set(key),
                        disabled: Set(false),
                        metadata: Set(metadata),
                        environment: Set(environment),
                        location: Set(location),
                        created_at: Set(timestamp as i64),
                        updated_at: Set(timestamp as i64),
                    };
                    secret_model.insert(&self.db).await?;
                }
            }
            "aws" => {
                let secret = AwsSecret::find_by_id(&key).one(&self.db).await?;
                // Extract environment and location from updated metadata
                let environment = Self::extract_environment_from_aws_tags(&metadata);
                let location = Self::extract_location_from_aws_tags(&metadata);

                if let Some(existing_secret) = secret {
                    // Update existing secret
                    info!(
                        schema = "aws",
                        secret_name = key,
                        environment = environment.as_deref().unwrap_or("None"),
                        location = location.as_deref().unwrap_or("None"),
                        operation = "update_metadata",
                        action = "update",
                        "Updating secret metadata in database: name={}, environment={:?}, location={:?}",
                        key, environment, location
                    );
                    let mut secret_model: AwsSecretActiveModel = existing_secret.into();
                    secret_model.metadata = Set(metadata);
                    secret_model.environment = Set(environment.clone());
                    secret_model.location = Set(location.clone());
                    secret_model.updated_at = Set(timestamp as i64);
                    secret_model.update(&self.db).await?;
                } else {
                    // Create new secret with metadata
                    info!(
                        schema = "aws",
                        secret_name = key,
                        environment = environment.as_deref().unwrap_or("None"),
                        location = location.as_deref().unwrap_or("None"),
                        operation = "update_metadata",
                        action = "create",
                        "Creating secret with metadata in database: name={}, environment={:?}, location={:?}",
                        key, environment, location
                    );
                    let secret_model = AwsSecretActiveModel {
                        name: Set(key),
                        disabled: Set(false),
                        metadata: Set(metadata),
                        environment: Set(environment),
                        location: Set(location),
                        created_at: Set(timestamp as i64),
                        updated_at: Set(timestamp as i64),
                    };
                    secret_model.insert(&self.db).await?;
                }
            }
            "azure" => {
                let secret = AzureSecret::find_by_id(&key).one(&self.db).await?;
                // Extract environment and location from updated metadata
                let environment = Self::extract_environment_from_azure_tags(&metadata);
                let location = Self::extract_location_from_azure_tags(&metadata);

                if let Some(existing_secret) = secret {
                    // Update existing secret
                    info!(
                        schema = "azure",
                        secret_name = key,
                        environment = environment.as_deref().unwrap_or("None"),
                        location = location.as_deref().unwrap_or("None"),
                        operation = "update_metadata",
                        action = "update",
                        "Updating secret metadata in database: name={}, environment={:?}, location={:?}",
                        key, environment, location
                    );
                    let mut secret_model: AzureSecretActiveModel = existing_secret.into();
                    secret_model.metadata = Set(metadata);
                    secret_model.environment = Set(environment.clone());
                    secret_model.location = Set(location.clone());
                    secret_model.updated_at = Set(timestamp as i64);
                    secret_model.update(&self.db).await?;
                } else {
                    // Create new secret with metadata
                    info!(
                        schema = "azure",
                        secret_name = key,
                        environment = environment.as_deref().unwrap_or("None"),
                        location = location.as_deref().unwrap_or("None"),
                        operation = "update_metadata",
                        action = "create",
                        "Creating secret with metadata in database: name={}, environment={:?}, location={:?}",
                        key, environment, location
                    );
                    let secret_model = AzureSecretActiveModel {
                        name: Set(key),
                        disabled: Set(false),
                        metadata: Set(metadata),
                        environment: Set(environment),
                        location: Set(location),
                        created_at: Set(timestamp as i64),
                        updated_at: Set(timestamp as i64),
                    };
                    secret_model.insert(&self.db).await?;
                }
            }
            _ => anyhow::bail!("Unknown schema: {}", self.schema),
        }

        Ok(())
    }

    /// Get the latest version of a secret
    pub async fn get_latest(&self, key: &str) -> Option<SecretVersion> {
        match self.schema.as_str() {
            "gcp" => self.get_gcp_latest(key).await,
            "aws" => self.get_aws_latest(key).await,
            "azure" => self.get_azure_latest(key).await,
            _ => None,
        }
    }

    async fn get_gcp_latest(&self, key: &str) -> Option<SecretVersion> {
        let secret = GcpSecret::find_by_id(key).one(&self.db).await.ok()??;
        if secret.disabled {
            return None;
        }

        let version = GcpVersion::find()
            .filter(<GcpVersion as sea_orm::EntityTrait>::Column::SecretKey.eq(key))
            .filter(<GcpVersion as sea_orm::EntityTrait>::Column::Enabled.eq(true))
            .order_by_desc(<GcpVersion as sea_orm::EntityTrait>::Column::CreatedAt)
            .one(&self.db)
            .await
            .ok()??;

        Some(SecretVersion {
            version_id: version.version_id,
            data: version.data,
            enabled: version.enabled,
            created_at: version.created_at as u64,
        })
    }

    async fn get_aws_latest(&self, name: &str) -> Option<SecretVersion> {
        let secret = AwsSecret::find_by_id(name).one(&self.db).await.ok()??;
        if secret.disabled {
            return None;
        }

        let version = AwsVersion::find()
            .filter(<AwsVersion as sea_orm::EntityTrait>::Column::SecretName.eq(name))
            .filter(<AwsVersion as sea_orm::EntityTrait>::Column::Enabled.eq(true))
            .order_by_desc(<AwsVersion as sea_orm::EntityTrait>::Column::CreatedAt)
            .one(&self.db)
            .await
            .ok()??;

        Some(SecretVersion {
            version_id: version.version_id,
            data: version.data,
            enabled: version.enabled,
            created_at: version.created_at as u64,
        })
    }

    async fn get_azure_latest(&self, name: &str) -> Option<SecretVersion> {
        let secret = AzureSecret::find_by_id(name).one(&self.db).await.ok()??;
        if secret.disabled {
            return None;
        }

        let version = AzureVersion::find()
            .filter(<AzureVersion as sea_orm::EntityTrait>::Column::SecretName.eq(name))
            .filter(<AzureVersion as sea_orm::EntityTrait>::Column::Enabled.eq(true))
            .order_by_desc(<AzureVersion as sea_orm::EntityTrait>::Column::CreatedAt)
            .one(&self.db)
            .await
            .ok()??;

        Some(SecretVersion {
            version_id: version.version_id,
            data: version.data,
            enabled: version.enabled,
            created_at: version.created_at as u64,
        })
    }

    /// Get a specific version of a secret by version ID
    pub async fn get_version(&self, key: &str, version_id: &str) -> Option<SecretVersion> {
        match self.schema.as_str() {
            "gcp" => self.get_gcp_version(key, version_id).await,
            "aws" => self.get_aws_version(key, version_id).await,
            "azure" => self.get_azure_version(key, version_id).await,
            _ => None,
        }
    }

    async fn get_gcp_version(&self, key: &str, version_id: &str) -> Option<SecretVersion> {
        let secret = GcpSecret::find_by_id(key).one(&self.db).await.ok()??;
        if secret.disabled {
            return None;
        }

        let version = GcpVersion::find_by_id((key.to_string(), version_id.to_string()))
            .one(&self.db)
            .await
            .ok()??;

        if !version.enabled {
            return None;
        }

        Some(SecretVersion {
            version_id: version.version_id,
            data: version.data,
            enabled: version.enabled,
            created_at: version.created_at as u64,
        })
    }

    async fn get_aws_version(&self, name: &str, version_id: &str) -> Option<SecretVersion> {
        let secret = AwsSecret::find_by_id(name).one(&self.db).await.ok()??;
        if secret.disabled {
            return None;
        }

        let version = AwsVersion::find_by_id((name.to_string(), version_id.to_string()))
            .one(&self.db)
            .await
            .ok()??;

        if !version.enabled {
            return None;
        }

        Some(SecretVersion {
            version_id: version.version_id,
            data: version.data,
            enabled: version.enabled,
            created_at: version.created_at as u64,
        })
    }

    async fn get_azure_version(&self, name: &str, version_id: &str) -> Option<SecretVersion> {
        let secret = AzureSecret::find_by_id(name).one(&self.db).await.ok()??;
        if secret.disabled {
            return None;
        }

        let version = AzureVersion::find_by_id((name.to_string(), version_id.to_string()))
            .one(&self.db)
            .await
            .ok()??;

        if !version.enabled {
            return None;
        }

        Some(SecretVersion {
            version_id: version.version_id,
            data: version.data,
            enabled: version.enabled,
            created_at: version.created_at as u64,
        })
    }

    /// Get all versions of a secret
    pub async fn list_versions(&self, key: &str) -> Option<Vec<SecretVersion>> {
        match self.schema.as_str() {
            "gcp" => self.list_gcp_versions(key).await,
            "aws" => self.list_aws_versions(key).await,
            "azure" => self.list_azure_versions(key).await,
            _ => None,
        }
    }

    async fn list_gcp_versions(&self, key: &str) -> Option<Vec<SecretVersion>> {
        let versions = GcpVersion::find()
            .filter(<GcpVersion as sea_orm::EntityTrait>::Column::SecretKey.eq(key))
            .order_by_asc(<GcpVersion as sea_orm::EntityTrait>::Column::CreatedAt)
            .all(&self.db)
            .await
            .ok()?;

        Some(
            versions
                .into_iter()
                .map(|v| SecretVersion {
                    version_id: v.version_id,
                    data: v.data,
                    enabled: v.enabled,
                    created_at: v.created_at as u64,
                })
                .collect(),
        )
    }

    async fn list_aws_versions(&self, name: &str) -> Option<Vec<SecretVersion>> {
        let versions = AwsVersion::find()
            .filter(<AwsVersion as sea_orm::EntityTrait>::Column::SecretName.eq(name))
            .order_by_asc(<AwsVersion as sea_orm::EntityTrait>::Column::CreatedAt)
            .all(&self.db)
            .await
            .ok()?;

        Some(
            versions
                .into_iter()
                .map(|v| SecretVersion {
                    version_id: v.version_id,
                    data: v.data,
                    enabled: v.enabled,
                    created_at: v.created_at as u64,
                })
                .collect(),
        )
    }

    async fn list_azure_versions(&self, name: &str) -> Option<Vec<SecretVersion>> {
        let versions = AzureVersion::find()
            .filter(<AzureVersion as sea_orm::EntityTrait>::Column::SecretName.eq(name))
            .order_by_asc(<AzureVersion as sea_orm::EntityTrait>::Column::CreatedAt)
            .all(&self.db)
            .await
            .ok()?;

        Some(
            versions
                .into_iter()
                .map(|v| SecretVersion {
                    version_id: v.version_id,
                    data: v.data,
                    enabled: v.enabled,
                    created_at: v.created_at as u64,
                })
                .collect(),
        )
    }

    /// Get secret metadata
    pub async fn get_metadata(&self, key: &str) -> Option<Value> {
        match self.schema.as_str() {
            "gcp" => GcpSecret::find_by_id(key)
                .one(&self.db)
                .await
                .ok()?
                .map(|s| s.metadata),
            "aws" => AwsSecret::find_by_id(key)
                .one(&self.db)
                .await
                .ok()?
                .map(|s| s.metadata),
            "azure" => AzureSecret::find_by_id(key)
                .one(&self.db)
                .await
                .ok()?
                .map(|s| s.metadata),
            _ => None,
        }
    }

    /// Disable a secret
    pub async fn disable_secret(&self, key: &str) -> bool {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        match self.schema.as_str() {
            "gcp" => {
                if let Ok(Some(secret)) = GcpSecret::find_by_id(key).one(&self.db).await {
                    let mut active: GcpSecretActiveModel = secret.into();
                    active.disabled = Set(true);
                    active.updated_at = Set(timestamp as i64);
                    active.update(&self.db).await.is_ok()
                } else {
                    false
                }
            }
            "aws" => {
                if let Ok(Some(secret)) = AwsSecret::find_by_id(key).one(&self.db).await {
                    let mut active: AwsSecretActiveModel = secret.into();
                    active.disabled = Set(true);
                    active.updated_at = Set(timestamp as i64);
                    active.update(&self.db).await.is_ok()
                } else {
                    false
                }
            }
            "azure" => {
                if let Ok(Some(secret)) = AzureSecret::find_by_id(key).one(&self.db).await {
                    let mut active: AzureSecretActiveModel = secret.into();
                    active.disabled = Set(true);
                    active.updated_at = Set(timestamp as i64);
                    active.update(&self.db).await.is_ok()
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Enable a secret
    pub async fn enable_secret(&self, key: &str) -> bool {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        match self.schema.as_str() {
            "gcp" => {
                if let Ok(Some(secret)) = GcpSecret::find_by_id(key).one(&self.db).await {
                    let mut active: GcpSecretActiveModel = secret.into();
                    active.disabled = Set(false);
                    active.updated_at = Set(timestamp as i64);
                    active.update(&self.db).await.is_ok()
                } else {
                    false
                }
            }
            "aws" => {
                if let Ok(Some(secret)) = AwsSecret::find_by_id(key).one(&self.db).await {
                    let mut active: AwsSecretActiveModel = secret.into();
                    active.disabled = Set(false);
                    active.updated_at = Set(timestamp as i64);
                    active.update(&self.db).await.is_ok()
                } else {
                    false
                }
            }
            "azure" => {
                if let Ok(Some(secret)) = AzureSecret::find_by_id(key).one(&self.db).await {
                    let mut active: AzureSecretActiveModel = secret.into();
                    active.disabled = Set(false);
                    active.updated_at = Set(timestamp as i64);
                    active.update(&self.db).await.is_ok()
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Disable a specific version
    pub async fn disable_version(&self, key: &str, version_id: &str) -> bool {
        match self.schema.as_str() {
            "gcp" => {
                if let Ok(Some(version)) =
                    GcpVersion::find_by_id((key.to_string(), version_id.to_string()))
                        .one(&self.db)
                        .await
                {
                    let mut active: GcpVersionActiveModel = version.into();
                    active.enabled = Set(false);
                    active.update(&self.db).await.is_ok()
                } else {
                    false
                }
            }
            "aws" => {
                if let Ok(Some(version)) =
                    AwsVersion::find_by_id((key.to_string(), version_id.to_string()))
                        .one(&self.db)
                        .await
                {
                    let mut active: AwsVersionActiveModel = version.into();
                    active.enabled = Set(false);
                    active.update(&self.db).await.is_ok()
                } else {
                    false
                }
            }
            "azure" => {
                if let Ok(Some(version)) =
                    AzureVersion::find_by_id((key.to_string(), version_id.to_string()))
                        .one(&self.db)
                        .await
                {
                    let mut active: AzureVersionActiveModel = version.into();
                    active.enabled = Set(false);
                    active.update(&self.db).await.is_ok()
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Enable a specific version
    pub async fn enable_version(&self, key: &str, version_id: &str) -> bool {
        match self.schema.as_str() {
            "gcp" => {
                if let Ok(Some(version)) =
                    GcpVersion::find_by_id((key.to_string(), version_id.to_string()))
                        .one(&self.db)
                        .await
                {
                    let mut active: GcpVersionActiveModel = version.into();
                    active.enabled = Set(true);
                    active.update(&self.db).await.is_ok()
                } else {
                    false
                }
            }
            "aws" => {
                if let Ok(Some(version)) =
                    AwsVersion::find_by_id((key.to_string(), version_id.to_string()))
                        .one(&self.db)
                        .await
                {
                    let mut active: AwsVersionActiveModel = version.into();
                    active.enabled = Set(true);
                    active.update(&self.db).await.is_ok()
                } else {
                    false
                }
            }
            "azure" => {
                if let Ok(Some(version)) =
                    AzureVersion::find_by_id((key.to_string(), version_id.to_string()))
                        .one(&self.db)
                        .await
                {
                    let mut active: AzureVersionActiveModel = version.into();
                    active.enabled = Set(true);
                    active.update(&self.db).await.is_ok()
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Delete a secret entirely
    pub async fn delete_secret(&self, key: &str) -> bool {
        match self.schema.as_str() {
            "gcp" => GcpSecret::delete_by_id(key).exec(&self.db).await.is_ok(),
            "aws" => AwsSecret::delete_by_id(key).exec(&self.db).await.is_ok(),
            "azure" => AzureSecret::delete_by_id(key).exec(&self.db).await.is_ok(),
            _ => false,
        }
    }

    /// Delete a specific version
    pub async fn delete_version(&self, key: &str, version_id: &str) -> bool {
        match self.schema.as_str() {
            "gcp" => GcpVersion::delete_by_id((key.to_string(), version_id.to_string()))
                .exec(&self.db)
                .await
                .is_ok(),
            "aws" => AwsVersion::delete_by_id((key.to_string(), version_id.to_string()))
                .exec(&self.db)
                .await
                .is_ok(),
            "azure" => AzureVersion::delete_by_id((key.to_string(), version_id.to_string()))
                .exec(&self.db)
                .await
                .is_ok(),
            _ => false,
        }
    }

    /// Check if a secret exists
    pub async fn exists(&self, key: &str) -> bool {
        match self.schema.as_str() {
            "gcp" => GcpSecret::find_by_id(key)
                .one(&self.db)
                .await
                .unwrap_or(None)
                .is_some(),
            "aws" => AwsSecret::find_by_id(key)
                .one(&self.db)
                .await
                .unwrap_or(None)
                .is_some(),
            "azure" => AzureSecret::find_by_id(key)
                .one(&self.db)
                .await
                .unwrap_or(None)
                .is_some(),
            _ => false,
        }
    }

    /// Check if a secret is enabled
    pub async fn is_enabled(&self, key: &str) -> bool {
        match self.schema.as_str() {
            "gcp" => GcpSecret::find_by_id(key)
                .one(&self.db)
                .await
                .ok()
                .and_then(|s| s)
                .map(|s| !s.disabled)
                .unwrap_or(false),
            "aws" => AwsSecret::find_by_id(key)
                .one(&self.db)
                .await
                .ok()
                .and_then(|s| s)
                .map(|s| !s.disabled)
                .unwrap_or(false),
            "azure" => AzureSecret::find_by_id(key)
                .one(&self.db)
                .await
                .ok()
                .and_then(|s| s)
                .map(|s| !s.disabled)
                .unwrap_or(false),
            _ => false,
        }
    }

    /// List all secret keys
    pub async fn list_all_keys(&self) -> Vec<String> {
        match self.schema.as_str() {
            "gcp" => GcpSecret::find()
                .all(&self.db)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|s| s.key)
                .collect(),
            "aws" => AwsSecret::find()
                .all(&self.db)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|s| s.name)
                .collect(),
            "azure" => AzureSecret::find()
                .all(&self.db)
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|s| s.name)
                .collect(),
            _ => vec![],
        }
    }

    /// List all projects that have secrets (GCP only)
    /// Uses the existing database connection for performance
    pub async fn list_gcp_projects(&self) -> Vec<String> {
        if self.schema != "gcp" {
            return vec![];
        }

        use sea_orm::ConnectionTrait;
        let stmt = sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT * FROM gcp.get_secret_projects()".to_string(),
        );

        if let Ok(rows) = self.db.query_all(stmt).await {
            return rows
                .into_iter()
                .filter_map(|row| {
                    // Get the project_id column value by column name
                    row.try_get::<Option<String>>("", "project_id")
                        .or_else(|_| row.try_get::<String>("", "project_id").map(Some))
                        .ok()
                        .flatten()
                })
                .collect();
        }
        vec![]
    }

    /// Execute a database query using the store's connection
    /// This allows accessing the database connection for custom queries
    pub async fn query_all(&self, stmt: sea_orm::Statement) -> Result<Vec<sea_orm::QueryResult>> {
        use sea_orm::ConnectionTrait;
        self.db
            .query_all(stmt)
            .await
            .map_err(|e| anyhow::anyhow!("Database query failed: {}", e))
    }
}

#[async_trait::async_trait]
impl crate::secrets::common::store_trait::SecretStoreBackend for DbSecretStore {
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
        self.add_version(key, version_data, version_id, version_id_generator)
            .await
    }

    async fn update_metadata(&self, key: String, metadata: Value) -> Result<()> {
        self.update_metadata(key, metadata).await
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
