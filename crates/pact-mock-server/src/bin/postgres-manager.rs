//! # PostgreSQL Migration Manager
//!
//! Sidecar container that manages PostgreSQL database migrations:
//! - Watches for PostgreSQL to start and be ready
//! - Watches ConfigMap for migration file changes
//! - Runs database migrations when ConfigMap changes
//! - Monitors migration status

use anyhow::{Context, Result};
use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use futures::{pin_mut, StreamExt};
use k8s_openapi::api::core::v1::ConfigMap;
use kube::{api::Api, Client};
use kube_runtime::watcher::{self, Config};
use sea_orm::{ConnectionTrait, Database, Statement};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Configuration for the postgres manager
#[derive(Debug, Clone)]
struct ManagerConfig {
    database_url: String,
    namespace: String,
    configmap_name: String,
    configmap_path: String,
    migrations_applied_flag_path: String,
    migrations_ready_sentinel_path: String,
    postgres_host: String,
    postgres_port: u16,
    check_interval: Duration,
    postgres_timeout: Duration,
}

impl ManagerConfig {
    fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgresql://pact:pact@localhost:5432/pact_mock_servers".to_string()
            }),
            namespace: std::env::var("NAMESPACE")
                .unwrap_or_else(|_| "secret-manager-controller-pact-broker".to_string()),
            configmap_name: std::env::var("CONFIGMAP_NAME")
                .unwrap_or_else(|_| "postgres-migrations".to_string()),
            configmap_path: std::env::var("CONFIGMAP_PATH")
                .unwrap_or_else(|_| "/migrations-configmap".to_string()),
            migrations_applied_flag_path: std::env::var("MIGRATIONS_APPLIED_FLAG_PATH")
                .unwrap_or_else(|_| "/tmp/migrations-applied.flag".to_string()),
            migrations_ready_sentinel_path: std::env::var("MIGRATIONS_READY_SENTINEL_PATH")
                .unwrap_or_else(|_| "/shared/migrations-ready.flag".to_string()),
            postgres_host: std::env::var("POSTGRES_HOST")
                .unwrap_or_else(|_| "localhost".to_string()),
            postgres_port: std::env::var("POSTGRES_PORT")
                .unwrap_or_else(|_| "5432".to_string())
                .parse()
                .context("Invalid POSTGRES_PORT")?,
            check_interval: Duration::from_secs(
                std::env::var("CHECK_INTERVAL_SECS")
                    .unwrap_or_else(|_| "2".to_string())
                    .parse()
                    .context("Invalid CHECK_INTERVAL_SECS")?,
            ),
            postgres_timeout: Duration::from_secs(
                std::env::var("POSTGRES_TIMEOUT_SECS")
                    .unwrap_or_else(|_| "90".to_string())
                    .parse()
                    .context("Invalid POSTGRES_TIMEOUT_SECS")?,
            ),
        })
    }
}

/// Check if PostgreSQL is ready by attempting a connection
/// Connects to 'postgres' database (always exists) instead of target database
async fn check_postgres_ready(config: &ManagerConfig) -> Result<bool> {
    // Connect to 'postgres' database (always exists) to check if PostgreSQL is ready
    // This avoids errors if the target database doesn't exist yet
    let postgres_url = if config.database_url.contains("/pact_mock_servers") {
        config
            .database_url
            .replace("/pact_mock_servers", "/postgres")
    } else {
        // Fallback: try to construct postgres URL
        let db_name = config
            .database_url
            .split('/')
            .last()
            .and_then(|s| s.split('?').next())
            .unwrap_or("postgres");
        config
            .database_url
            .replace(&format!("/{}", db_name), "/postgres")
    };

    match Database::connect(&postgres_url).await {
        Ok(_) => Ok(true),
        Err(e) => {
            debug!("PostgreSQL connection check failed: {}", e);
            Ok(false)
        }
    }
}

/// Check if a port is available (listening)
async fn check_port_available(host: &str, port: u16) -> Result<bool> {
    use tokio::net::TcpStream;
    use tokio::time::timeout;

    let addr = format!("{}:{}", host, port);
    match timeout(Duration::from_secs(2), TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => Ok(true),
        Ok(Err(_)) | Err(_) => Ok(false),
    }
}

/// Wait for PostgreSQL to be ready
async fn wait_for_postgres(config: &ManagerConfig) -> Result<()> {
    use std::time::Instant;
    let start_time = Instant::now();

    info!(
        "👀 Waiting for PostgreSQL at {}:{}...",
        config.postgres_host, config.postgres_port
    );

    loop {
        if start_time.elapsed() > config.postgres_timeout {
            return Err(anyhow::anyhow!(
                "Timeout waiting for PostgreSQL after {} seconds",
                config.postgres_timeout.as_secs()
            ));
        }

        // First check if port is available
        if check_port_available(&config.postgres_host, config.postgres_port).await? {
            // Port is available, check if PostgreSQL is ready
            if check_postgres_ready(config).await? {
                info!("✅ PostgreSQL is ready!");
                return Ok(());
            }
        }

        sleep(config.check_interval).await;
    }
}

/// Process ConfigMap and prepare migration files
/// This function:
/// 1. Reads flattened migration files from ConfigMap
/// 2. Reconstructs the directory structure
/// 3. Creates a sentinel file to signal postgres can start
/// Returns the migration file paths organized by schema
fn prepare_migrations(
    configmap_path: &Path,
    sentinel_path: &Path,
) -> Result<HashMap<String, String>> {
    let mut migrations = HashMap::new();

    // ConfigMap volumes are read-only, so we need to copy files to a temp directory
    // and reconstruct the directory structure there
    // Use a fixed path in /tmp for container environments
    let temp_dir = Path::new("/tmp/postgres-migrations");
    std::fs::create_dir_all(&temp_dir).context("Failed to create temp directory for migrations")?;

    // Create schema subdirectories in temp directory
    for schema in ["gcp", "aws", "azure"] {
        let schema_dir = temp_dir.join(schema);
        std::fs::create_dir_all(&schema_dir).with_context(|| {
            format!(
                "Failed to create schema directory: {}",
                schema_dir.display()
            )
        })?;
    }

    // Read all files from the ConfigMap mount root
    // Keys are flattened like "gcp_001_create_schema.sql"
    if configmap_path.exists() {
        for entry in std::fs::read_dir(configmap_path)? {
            let entry = entry?;
            let path = entry.path();

            // Only process files (not directories) with .sql extension
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("sql") {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;

                // Parse key format: "schema_filename.sql"
                // Find the first underscore to split schema and filename
                if let Some(underscore_pos) = filename.find('_') {
                    let schema = &filename[..underscore_pos];
                    let actual_filename = &filename[underscore_pos + 1..];

                    // Validate schema
                    if ["gcp", "aws", "azure"].contains(&schema) {
                        // Reconstruct the path in the temp schema subdirectory
                        let schema_dir = temp_dir.join(schema);
                        let reconstructed_path = schema_dir.join(actual_filename);

                        // Copy file to the reconstructed location
                        std::fs::copy(&path, &reconstructed_path).with_context(|| {
                            format!(
                                "Failed to copy {} to {}",
                                path.display(),
                                reconstructed_path.display()
                            )
                        })?;

                        // Store with original key format for sorting
                        let key = format!("{}/{}", schema, actual_filename);
                        migrations.insert(key, reconstructed_path.to_string_lossy().to_string());
                    } else {
                        warn!(
                            "⚠️  Unknown schema '{}' in migration file: {}",
                            schema, filename
                        );
                    }
                } else {
                    warn!("⚠️  Migration file doesn't match expected format (schema_filename.sql): {}", filename);
                }
            }
        }
    }

    // Create sentinel file to signal that migrations are ready
    // This allows the postgres init container to proceed
    if !migrations.is_empty() {
        // Ensure parent directory exists
        if let Some(parent) = sentinel_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create sentinel directory")?;
        }

        // Write sentinel file with timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        std::fs::write(sentinel_path, format!("migrations-ready-{}\n", timestamp))
            .context("Failed to create sentinel file")?;

        info!(
            "✅ Created sentinel file: {} ({} migrations ready)",
            sentinel_path.display(),
            migrations.len()
        );
    } else {
        warn!("⚠️  No migrations found - sentinel file not created");
    }

    Ok(migrations)
}

/// Run a single migration file
/// Migration files may contain multiple SQL statements separated by semicolons
/// We need to split and execute them separately since PostgreSQL doesn't allow
/// multiple commands in a single prepared statement
///
/// This function properly handles:
/// - Dollar-quoted strings ($$ ... $$)
/// - Function definitions with semicolons inside
/// - Comments (-- and /* */)
async fn run_migration_file(db: &sea_orm::DatabaseConnection, file_path: &Path) -> Result<()> {
    let sql = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read migration file: {}", file_path.display()))?;

    info!("  Running migration: {}", file_path.display());

    // Split SQL into individual statements, handling dollar-quoted strings
    // Dollar-quoted strings can contain semicolons, so we need to track when we're inside one
    let statements = split_sql_statements(&sql);

    if statements.is_empty() {
        warn!("  No SQL statements found in migration file");
        return Ok(());
    }

    // Execute each statement separately
    for (idx, statement) in statements.iter().enumerate() {
        // Statement should already have proper syntax (semicolon at end if needed)
        db.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            statement.clone(),
        ))
        .await
        .with_context(|| {
            format!(
                "Failed to execute statement {} of {} in migration {}",
                idx + 1,
                statements.len(),
                file_path.display()
            )
        })?;
    }

    Ok(())
}

/// Split SQL into individual statements, properly handling dollar-quoted strings
/// Dollar-quoted strings use $$ or $tag$ ... $tag$ syntax and can contain semicolons
fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current_statement = String::new();
    let mut chars = sql.chars().peekable();
    let mut in_dollar_quote = false;
    let mut dollar_tag: Option<String> = None;
    let mut in_single_comment = false;
    let mut in_multi_comment = false;

    while let Some(ch) = chars.next() {
        // Handle comments
        if !in_dollar_quote {
            if ch == '-' && chars.peek() == Some(&'-') {
                // Single-line comment
                in_single_comment = true;
                current_statement.push(ch);
                if let Some(next) = chars.next() {
                    current_statement.push(next);
                }
                continue;
            }
            if in_single_comment {
                if ch == '\n' {
                    in_single_comment = false;
                }
                current_statement.push(ch);
                continue;
            }

            if ch == '/' && chars.peek() == Some(&'*') {
                // Multi-line comment start
                in_multi_comment = true;
                current_statement.push(ch);
                if let Some(next) = chars.next() {
                    current_statement.push(next);
                }
                continue;
            }
            if in_multi_comment {
                if ch == '*' && chars.peek() == Some(&'/') {
                    // Multi-line comment end
                    in_multi_comment = false;
                    current_statement.push(ch);
                    if let Some(next) = chars.next() {
                        current_statement.push(next);
                    }
                    continue;
                }
                current_statement.push(ch);
                continue;
            }
        }

        // Handle dollar-quoted strings
        if ch == '$' && !in_single_comment && !in_multi_comment {
            // Check if this is a dollar quote delimiter
            let mut peek_iter = chars.clone();
            if let Some(next) = peek_iter.next() {
                if next == '$' {
                    // $$ delimiter - simple toggle
                    in_dollar_quote = !in_dollar_quote;
                    dollar_tag = None;
                    current_statement.push(ch);
                    current_statement.push(next);
                    chars.next(); // consume the next $
                    continue;
                } else if (next.is_alphanumeric() || next == '_') && !in_dollar_quote {
                    // Potential $tag$ delimiter start - collect the tag
                    let mut tag = String::new();
                    tag.push(next);
                    let mut found_end = false;
                    let mut tag_chars = peek_iter;

                    while let Some(tag_ch) = tag_chars.next() {
                        if tag_ch == '$' {
                            found_end = true;
                            break;
                        } else if tag_ch.is_alphanumeric() || tag_ch == '_' {
                            tag.push(tag_ch);
                        } else {
                            break;
                        }
                    }

                    if found_end {
                        // Start of new dollar quote with tag
                        in_dollar_quote = true;
                        dollar_tag = Some(tag.clone());
                        current_statement.push(ch);
                        for tag_ch in tag.chars() {
                            current_statement.push(tag_ch);
                            chars.next();
                        }
                        current_statement.push('$');
                        chars.next(); // consume the closing $
                        continue;
                    }
                } else if in_dollar_quote {
                    // Check if this is the closing delimiter for current tag
                    let current_tag_opt = dollar_tag.clone();
                    if let Some(current_tag) = current_tag_opt {
                        let mut tag_match = String::new();
                        let mut peek_iter2 = chars.clone();

                        // Check if the following characters match our tag
                        for expected_ch in current_tag.chars() {
                            if let Some(actual_ch) = peek_iter2.next() {
                                if actual_ch == expected_ch {
                                    tag_match.push(actual_ch);
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }

                        // Check if tag is followed by $
                        if tag_match == current_tag {
                            if let Some(end_ch) = peek_iter2.next() {
                                if end_ch == '$' {
                                    // Found closing delimiter
                                    in_dollar_quote = false;
                                    dollar_tag = None;
                                    current_statement.push(ch);
                                    for tag_ch in current_tag.chars() {
                                        current_statement.push(tag_ch);
                                        chars.next();
                                    }
                                    current_statement.push('$');
                                    chars.next(); // consume the closing $
                                    continue;
                                }
                            }
                        }
                    } else {
                        // No tag, check for $$
                        if let Some(next) = chars.peek() {
                            if *next == '$' {
                                // $$ delimiter - end
                                in_dollar_quote = false;
                                current_statement.push(ch);
                                current_statement.push('$');
                                chars.next(); // consume the next $
                                continue;
                            }
                        }
                    }
                }
            }
        }

        // If we're inside a dollar quote, don't split on semicolons
        if ch == ';' && !in_dollar_quote {
            current_statement.push(ch);
            let trimmed = current_statement.trim();
            if !trimmed.is_empty() && trimmed != ";" {
                statements.push(trimmed.to_string());
            }
            current_statement.clear();
        } else {
            current_statement.push(ch);
        }
    }

    // Add any remaining statement
    let trimmed = current_statement.trim();
    if !trimmed.is_empty() {
        statements.push(trimmed.to_string());
    }

    statements
}

/// Ensure the target database exists, creating it if necessary
async fn ensure_database_exists(config: &ManagerConfig) -> Result<()> {
    // Parse database name from connection URL
    // Format: postgresql://user:pass@host:port/database
    let db_name = config
        .database_url
        .split('/')
        .last()
        .and_then(|s| s.split('?').next())
        .ok_or_else(|| anyhow::anyhow!("Failed to parse database name from URL"))?;

    // Validate database name (must be a valid PostgreSQL identifier)
    // Only allow alphanumeric and underscore characters
    if !db_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(anyhow::anyhow!("Invalid database name: {}", db_name));
    }

    // Connect to 'postgres' database (always exists) to check/create target database
    let postgres_url = config
        .database_url
        .replace(&format!("/{}", db_name), "/postgres");
    let db = Database::connect(&postgres_url)
        .await
        .context("Failed to connect to PostgreSQL")?;

    // Check if database exists using parameterized query
    let result = db
        .query_one(Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT 1 FROM pg_database WHERE datname = $1",
            vec![sea_orm::Value::String(Some(Box::new(db_name.to_string())))],
        ))
        .await
        .context("Failed to check if database exists")?;

    if result.is_none() {
        info!("📦 Database '{}' does not exist, creating it...", db_name);
        // CREATE DATABASE doesn't support parameters, but we've validated the name
        db.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!("CREATE DATABASE {}", db_name),
        ))
        .await
        .context("Failed to create database")?;
        info!("✅ Database '{}' created successfully", db_name);
    } else {
        debug!("Database '{}' already exists", db_name);
    }

    Ok(())
}

/// Run all migrations from prepared migration files
async fn run_migrations(config: &ManagerConfig) -> Result<(usize, usize)> {
    // Read migrations from the prepared directory (processed by init container)
    // The init container prepares migrations in /shared/postgres-migrations
    // Fallback to /tmp/postgres-migrations if shared doesn't exist (for backwards compatibility)
    let migrations_dir = if Path::new("/shared/postgres-migrations").exists() {
        Path::new("/shared/postgres-migrations")
    } else {
        Path::new("/tmp/postgres-migrations")
    };

    let mut migrations = HashMap::new();

    // Read from the prepared directory structure
    for schema in ["gcp", "aws", "azure"] {
        let schema_dir = migrations_dir.join(schema);
        if schema_dir.exists() {
            for entry in std::fs::read_dir(&schema_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("sql") {
                    let filename = path.file_name().unwrap().to_string_lossy().to_string();
                    let key = format!("{}/{}", schema, filename);
                    migrations.insert(key, path.to_string_lossy().to_string());
                }
            }
        }
    }

    if migrations.is_empty() {
        warn!(
            "⚠️  No migration files found in ConfigMap at {}",
            config.configmap_path
        );
        return Ok((0, 0));
    }

    info!("📋 Found {} migration file(s) to run", migrations.len());

    // Ensure the target database exists before connecting
    ensure_database_exists(&config).await?;

    // Connect to database
    let db = Database::connect(&config.database_url)
        .await
        .context("Failed to connect to PostgreSQL")?;

    let mut applied = 0;
    let mut failed = 0;

    // Sort migrations by key (schema/filename) to ensure consistent order
    let mut sorted_migrations: Vec<_> = migrations.iter().collect();
    sorted_migrations.sort_by_key(|(k, _)| *k);

    for (key, file_path) in sorted_migrations {
        let path = Path::new(file_path);
        match run_migration_file(&db, path).await {
            Ok(_) => {
                applied += 1;
                info!(
                    "✅ [{}/{}] Successfully applied: {}",
                    applied + failed,
                    migrations.len(),
                    key
                );
            }
            Err(e) => {
                failed += 1;
                error!(
                    "❌ [{}/{}] Failed to apply {}: {}",
                    applied + failed,
                    migrations.len(),
                    key,
                    e
                );
            }
        }
    }

    info!("📊 Migration Summary:");
    info!("   Total migrations: {}", migrations.len());
    info!("   ✅ Successfully applied: {}", applied);
    info!("   ❌ Failed: {}", failed);

    Ok((applied, failed))
}

/// Check if ConfigMap exists and has migration files
async fn check_configmap_has_migrations(
    client: &Client,
    namespace: &str,
    configmap_name: &str,
) -> Result<bool> {
    let configmaps: Api<ConfigMap> = Api::namespaced(client.clone(), namespace);
    match configmaps.get(configmap_name).await {
        Ok(cm) => {
            if let Some(data) = &cm.data {
                let has_migrations = data.keys().any(|key| key.ends_with(".sql"));
                Ok(has_migrations)
            } else {
                Ok(false)
            }
        }
        Err(kube::Error::Api(e)) if e.code == 404 => Ok(false),
        Err(kube::Error::Api(e)) if e.code == 401 || e.code == 403 => Err(anyhow::anyhow!(
            "Unauthorized ({}): RBAC may not be ready yet - will retry",
            e.code
        )),
        Err(e) => Err(anyhow::anyhow!(
            "Error checking ConfigMap {}/{}: {}",
            namespace,
            configmap_name,
            e
        )),
    }
}

/// Wait for RBAC to be ready
async fn wait_for_rbac_ready(
    client: &Client,
    namespace: &str,
    configmap_name: &str,
    max_attempts: u32,
) -> Result<()> {
    let mut attempt = 0;
    let mut delay = Duration::from_secs(2);

    while attempt < max_attempts {
        match check_configmap_has_migrations(client, namespace, configmap_name).await {
            Ok(_) => {
                info!("✅ RBAC is ready - can access ConfigMap");
                return Ok(());
            }
            Err(e)
                if e.to_string().contains("Unauthorized")
                    || e.to_string().contains("401")
                    || e.to_string().contains("403") =>
            {
                attempt += 1;
                if attempt % 5 == 0 {
                    info!(
                        "⏳ Waiting for RBAC to be ready... (attempt {}/{})",
                        attempt, max_attempts
                    );
                }
                sleep(delay).await;
                delay = std::cmp::min(delay * 2, Duration::from_secs(30));
            }
            Err(e) => {
                attempt += 1;
                if attempt < 3 {
                    warn!("⚠️  Error checking ConfigMap (will retry): {}", e);
                    sleep(delay).await;
                } else {
                    warn!("⚠️  Error checking ConfigMap (continuing anyway): {}", e);
                    return Ok(());
                }
            }
        }
    }

    warn!(
        "⚠️  RBAC check timed out after {} attempts, but continuing - watcher will retry",
        max_attempts
    );
    Ok(())
}

/// Watch ConfigMap for changes and rerun migrations
async fn watch_configmap(
    client: Client,
    namespace: String,
    configmap_name: String,
    config: Arc<ManagerConfig>,
    migrations_applied: Arc<AtomicBool>,
) -> Result<()> {
    info!("🔐 Waiting for RBAC to be ready before starting ConfigMap watch...");
    if let Err(e) = wait_for_rbac_ready(&client, &namespace, &configmap_name, 30).await {
        warn!(
            "⚠️  RBAC readiness check failed: {} (will continue and retry)",
            e
        );
    }

    let configmaps: Api<ConfigMap> = Api::namespaced(client.clone(), &namespace);
    let watcher_config = Config::default().fields(&format!("metadata.name={}", configmap_name));
    let watcher = watcher::watcher(configmaps, watcher_config);
    pin_mut!(watcher);

    info!(
        "👀 Watching ConfigMap {}/{} for changes...",
        namespace, configmap_name
    );

    while let Some(event_result) = watcher.next().await {
        match event_result {
            Ok(event) => {
                match event {
                    kube::runtime::watcher::Event::Apply(cm) => {
                        if cm.metadata.name.as_deref() == Some(&configmap_name) {
                            info!(
                                "📝 ConfigMap {} changed, checking for migration files...",
                                configmap_name
                            );

                            // Small delay to ensure mounted volume is updated
                            sleep(Duration::from_millis(500)).await;

                            // Re-process ConfigMap and update sentinel
                            let configmap_path = Path::new(&config.configmap_path);
                            let sentinel_path = Path::new(&config.migrations_ready_sentinel_path);
                            if let Err(e) = prepare_migrations(configmap_path, sentinel_path) {
                                warn!("⚠️  Failed to re-process migrations: {}", e);
                            }

                            // Check if postgres is ready
                            if let Ok(true) = check_postgres_ready(&config).await {
                                // Remove applied flag to force re-running migrations
                                let _ = std::fs::remove_file(&config.migrations_applied_flag_path);

                                match run_migrations(&config).await {
                                    Ok((applied, failed)) => {
                                        if applied > 0 {
                                            let _ = std::fs::write(
                                                &config.migrations_applied_flag_path,
                                                "",
                                            );
                                            migrations_applied.store(true, Ordering::Relaxed);
                                            info!(
                                                "✅ Applied {} migration(s) from ConfigMap",
                                                applied
                                            );
                                        } else {
                                            migrations_applied.store(false, Ordering::Relaxed);
                                            warn!("⚠️  No migrations were applied (no migration files found in ConfigMap)");
                                        }
                                        if failed > 0 {
                                            warn!("⚠️  {} migration(s) failed", failed);
                                        }
                                    }
                                    Err(e) => {
                                        migrations_applied.store(false, Ordering::Relaxed);
                                        error!("Error running migrations: {}", e);
                                    }
                                }
                            } else {
                                warn!("PostgreSQL not ready, skipping migrations (will retry on next change)");
                            }
                        }
                    }
                    kube::runtime::watcher::Event::Delete(cm) => {
                        if cm.metadata.name.as_deref() == Some(&configmap_name) {
                            warn!("⚠️  ConfigMap {} was deleted", configmap_name);
                            migrations_applied.store(false, Ordering::Relaxed);
                            let _ = std::fs::remove_file(&config.migrations_applied_flag_path);
                        }
                    }
                    kube::runtime::watcher::Event::Init
                    | kube::runtime::watcher::Event::InitApply(_)
                    | kube::runtime::watcher::Event::InitDone => {
                        debug!("Initial watch event received");
                    }
                }
            }
            Err(e) => {
                let error_str = e.to_string();
                if error_str.contains("Unauthorized")
                    || error_str.contains("401")
                    || error_str.contains("403")
                {
                    warn!(
                        "⚠️  RBAC error watching ConfigMap (will retry with backoff): {}",
                        e
                    );
                    sleep(Duration::from_secs(10)).await;
                } else {
                    error!("Error watching ConfigMap: {}", e);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    warn!("ConfigMap watch stream ended - this should not happen");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("🚀 Starting PostgreSQL migration manager sidecar...");
    info!("📋 Manager responsibilities:");
    info!("   - Process ConfigMap and prepare migration files");
    info!("   - Create sentinel file to signal postgres can start");
    info!("   - Wait for PostgreSQL to start and be ready");
    info!("   - Run database migrations once PostgreSQL is ready");
    info!("   - Watch ConfigMap for changes and re-run migrations");
    info!("");

    let config = ManagerConfig::from_env().context("Failed to load configuration")?;
    let config = Arc::new(config);

    // STEP 1: Check if migrations are already prepared by init container
    // The init container processes ConfigMap and creates sentinel file before we start
    info!("📦 Checking for prepared migrations...");
    let sentinel_path = Path::new(&config.migrations_ready_sentinel_path);

    // Check if sentinel exists (created by init container)
    if sentinel_path.exists() {
        if let Ok(content) = std::fs::read_to_string(sentinel_path) {
            info!(
                "✅ Found sentinel file (created by init container): {}",
                content.trim()
            );
        }
    } else {
        warn!("⚠️  Sentinel file not found - init container may not have run yet");
    }

    // Shared state for health checks (created early so health server can start immediately)
    let postgres_healthy = Arc::new(AtomicBool::new(false));
    let migrations_applied = Arc::new(AtomicBool::new(false));

    // Start HTTP health server EARLY (before migrations) so liveness probes work immediately
    let health_state = HealthState {
        postgres_healthy: postgres_healthy.clone(),
        migrations_applied: migrations_applied.clone(),
    };

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/healthz", get(health_handler))
        .route("/liveness", get(liveness_handler))
        .route("/readiness", get(readiness_handler))
        .with_state(health_state);

    let health_port = std::env::var("HEALTH_PORT")
        .unwrap_or_else(|_| "1239".to_string())
        .parse()
        .unwrap_or(1239);

    info!("🏥 Starting health server on port {}...", health_port);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", health_port))
        .await
        .context("Failed to bind health server")?;

    info!("   Health endpoint: http://0.0.0.0:{}/healthz", health_port);
    info!(
        "   Liveness endpoint: http://0.0.0.0:{}/liveness",
        health_port
    );
    info!(
        "   Readiness endpoint: http://0.0.0.0:{}/readiness",
        health_port
    );

    // Start health server in background task so it's immediately available for probes
    let server_handle = tokio::spawn(async move {
        info!("✅ Health server is now listening and ready to accept connections");
        axum::serve(listener, app)
            .await
            .context("Health server error")?;
        Ok::<(), anyhow::Error>(())
    });

    // Give the server a moment to start accepting connections
    // This ensures the startup probe doesn't fail immediately
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // STEP 2: Wait for PostgreSQL to be ready (after it starts)
    info!("👀 Waiting for PostgreSQL to be ready...");
    if let Err(e) = wait_for_postgres(&config).await {
        error!("Failed to wait for PostgreSQL: {}", e);
        return Err(e);
    }

    // Run migrations if not already applied
    let applied_flag = Path::new(&config.migrations_applied_flag_path);
    if !applied_flag.exists() {
        info!("📦 Running database migrations...");
        match run_migrations(&config).await {
            Ok((applied, failed)) => {
                if applied > 0 {
                    let _ = std::fs::write(&config.migrations_applied_flag_path, "");
                    migrations_applied.store(true, Ordering::Relaxed);
                    info!("✅ All migrations applied! Manager will continue running.");
                } else if failed > 0 {
                    migrations_applied.store(false, Ordering::Relaxed);
                    warn!("⚠️  No migrations were applied (all failed)");
                } else {
                    migrations_applied.store(false, Ordering::Relaxed);
                    warn!("⚠️  No migrations were applied (no migration files found)");
                }
            }
            Err(e) => {
                migrations_applied.store(false, Ordering::Relaxed);
                error!("Error running migrations: {}", e);
                return Err(e);
            }
        }
    } else {
        migrations_applied.store(true, Ordering::Relaxed);
        info!("ℹ️  Migrations already applied (flag exists), skipping initial run");
    }

    rustls::crypto::CryptoProvider::install_default(rustls::crypto::ring::default_provider())
        .expect("Failed to install rustls crypto provider");

    let client = Client::try_default()
        .await
        .context("Failed to create Kubernetes client")?;

    // Start watching ConfigMap in background
    let config_clone = config.clone();
    let namespace = config.namespace.clone();
    let configmap_name = config.configmap_name.clone();
    let migrations_applied_clone = migrations_applied.clone();
    tokio::spawn(async move {
        if let Err(e) = watch_configmap(
            client,
            namespace,
            configmap_name,
            config_clone,
            migrations_applied_clone,
        )
        .await
        {
            error!("ConfigMap watcher error: {}", e);
        }
    });

    // Start health check background task
    let postgres_healthy_clone = postgres_healthy.clone();
    let config_for_health = config.clone();
    tokio::spawn(async move {
        loop {
            match check_postgres_ready(&config_for_health).await {
                Ok(true) => {
                    postgres_healthy_clone.store(true, Ordering::Relaxed);
                }
                Ok(false) => {
                    postgres_healthy_clone.store(false, Ordering::Relaxed);
                    warn!("⚠️  PostgreSQL health check failed");
                }
                Err(e) => {
                    postgres_healthy_clone.store(false, Ordering::Relaxed);
                    warn!("⚠️  PostgreSQL health check error: {}", e);
                }
            }
            sleep(Duration::from_secs(30)).await;
        }
    });

    info!("✅ Manager is running. Monitoring for ConfigMap changes...");

    // Wait for health server (it runs forever)
    if let Err(e) = server_handle.await {
        error!("Health server error: {:?}", e);
        return Err(anyhow::anyhow!("Health server failed: {:?}", e));
    }

    Ok(())
}

#[derive(Clone)]
struct HealthState {
    postgres_healthy: Arc<AtomicBool>,
    migrations_applied: Arc<AtomicBool>,
}

async fn health_handler(State(state): State<HealthState>) -> (StatusCode, Json<Value>) {
    let response = json!({
        "status": "healthy",
        "postgres_healthy": state.postgres_healthy.load(Ordering::Relaxed),
        "migrations_applied": state.migrations_applied.load(Ordering::Relaxed),
    });
    (StatusCode::OK, Json(response))
}

async fn liveness_handler(State(_state): State<HealthState>) -> (StatusCode, Json<Value>) {
    let response = json!({
        "status": "alive",
    });
    (StatusCode::OK, Json(response))
}

async fn readiness_handler(State(state): State<HealthState>) -> (StatusCode, Json<Value>) {
    let postgres_healthy = state.postgres_healthy.load(Ordering::Relaxed);
    let migrations_applied = state.migrations_applied.load(Ordering::Relaxed);
    let response = json!({
        "status": if postgres_healthy && migrations_applied { "ready" } else { "not_ready" },
        "postgres_healthy": postgres_healthy,
        "migrations_applied": migrations_applied,
    });

    if postgres_healthy && migrations_applied {
        (StatusCode::OK, Json(response))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(response))
    }
}
