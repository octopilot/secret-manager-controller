//! Azure Key Vault Mock Server
//!
//! A lightweight Axum-based HTTP server that serves as a mock for the Azure Key Vault Secrets API.
//! Uses RESTful paths with api-version query parameter.
//!
//! Environment Variables:
//! - PACT_BROKER_URL: URL of the Pact broker (default: http://pact-broker:9292)
//! - PACT_BROKER_USERNAME: Username for broker authentication (default: pact)
//! - PACT_BROKER_PASSWORD: Password for broker authentication (default: pact)
//! - PACT_PROVIDER: Provider name in contracts (default: Azure-Key-Vault)
//! - PACT_CONSUMER: Consumer name in contracts (default: Secret-Manager-Controller)
//! - PORT: Port to listen on (default: 1234)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{delete, get, patch, post, put},
    Router,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use pact_mock_server::prelude::*;
use pact_mock_server::secrets::common::errors::azure_error_codes;
use paths::azure::routes::key_vault as azure_routes;
use serde_json::json;
use std::env;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{info, warn, Level};

/// Azure-specific application state
#[derive(Clone)]
struct AzureAppState {
    #[allow(dead_code)] // Reserved for future contract-based responses
    contracts:
        std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, serde_json::Value>>>,
    #[allow(dead_code)] // Will be used when Azure handlers are fully implemented
    secrets: AzureSecretStore,
}

#[derive(serde::Deserialize)]
struct SetSecretRequest {
    value: String,
}

/// Format Unix timestamp to Azure API format (Unix timestamp as integer)
fn format_timestamp_azure(timestamp: u64) -> i64 {
    timestamp as i64
}

/// GET secret
/// Path: /secrets/{name}/ (with trailing slash)
/// Query: api-version=2025-07-01
async fn get_secret(State(app_state): State<AzureAppState>, Path(name): Path<String>) -> Response {
    info!("  GET secret: name={}", name);

    // Check if secret is disabled
    if !app_state.secrets.is_enabled(&name).await {
        return azure_error_response(
            StatusCode::BAD_REQUEST,
            azure_error_codes::BAD_PARAMETER,
            format!("Secret {} is disabled", name),
        );
    }

    // Get latest version with timestamp
    let latest_version = app_state.secrets.get_latest(&name).await;

    if latest_version.is_none() {
        return azure_error_response(
            StatusCode::NOT_FOUND,
            azure_error_codes::SECRET_NOT_FOUND,
            format!("Secret {} not found", name),
        );
    }

    let created = latest_version
        .as_ref()
        .map(|v| format_timestamp_azure(v.created_at))
        .unwrap_or_else(|| {
            format_timestamp_azure(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            )
        });
    let updated = created; // Azure uses same timestamp for created/updated in our mock

    let value = latest_version
        .as_ref()
        .and_then(|v| v.data.get("value"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("mock-value-for-{}", name));

    let version_id = latest_version
        .as_ref()
        .map(|v| v.version_id.clone())
        .unwrap_or_else(|| "abc123".to_string());

    Json(json!({
        "value": value,
        "id": format!("https://test-vault.vault.azure.net/secrets/{}/{}", name, version_id),
        "attributes": {
            "enabled": true,
            "created": created,
            "updated": updated,
            "recoveryLevel": "Recoverable+Purgeable"
        }
    }))
    .into_response()
}

/// GET secret specific version
/// Path: /secrets/{name}/{version}
/// Query: api-version=2025-07-01
async fn get_secret_version(
    State(app_state): State<AzureAppState>,
    Path((name, version_id)): Path<(String, String)>,
) -> Response {
    info!(
        "  GET secret version: name={}, version={}",
        name, version_id
    );

    // Check if secret is disabled
    if !app_state.secrets.is_enabled(&name).await {
        return azure_error_response(
            StatusCode::BAD_REQUEST,
            azure_error_codes::BAD_PARAMETER,
            format!("Secret {} is disabled", name),
        );
    }

    // Get specific version
    let version = app_state.secrets.get_version(&name, &version_id).await;

    if version.is_none() {
        return azure_error_response(
            StatusCode::NOT_FOUND,
            azure_error_codes::SECRET_NOT_FOUND,
            format!("Version {} not found for secret {}", version_id, name),
        );
    }

    let version = version.unwrap();

    // Check if version is enabled
    if !version.enabled {
        return azure_error_response(
            StatusCode::BAD_REQUEST,
            azure_error_codes::BAD_PARAMETER,
            format!("Version {} is disabled", version_id),
        );
    }

    let created = format_timestamp_azure(version.created_at);
    let updated = created; // Azure uses same timestamp for created/updated in our mock

    let value = version
        .data
        .get("value")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("mock-value-for-{}-{}", name, version_id));

    Json(json!({
        "value": value,
        "id": format!("https://test-vault.vault.azure.net/secrets/{}/{}", name, version_id),
        "attributes": {
            "enabled": true,
            "created": created,
            "updated": updated,
            "recoveryLevel": "Recoverable+Purgeable"
        }
    }))
    .into_response()
}

/// GET list of secret versions
/// Path: /secrets/{name}/versions
/// Query: api-version=2025-07-01
async fn list_secret_versions(
    State(app_state): State<AzureAppState>,
    Path(name): Path<String>,
) -> Response {
    info!("  GET secret versions list: name={}", name);

    // Check if secret exists
    if !app_state.secrets.exists(&name).await {
        warn!("  Secret not found: {}", name);
        return azure_error_response(
            StatusCode::NOT_FOUND,
            azure_error_codes::SECRET_NOT_FOUND,
            format!("Secret {} not found", name),
        );
    }

    // Get all versions
    if let Some(versions) = app_state.secrets.list_versions(&name).await {
        let version_list: Vec<serde_json::Value> = versions
            .iter()
            .map(|v| {
                json!({
                    "id": format!("https://test-vault.vault.azure.net/secrets/{}/{}", name, v.version_id),
                    "attributes": {
                        "enabled": v.enabled,
                        "created": format_timestamp_azure(v.created_at),
                        "updated": format_timestamp_azure(v.created_at),
                        "recoveryLevel": "Recoverable+Purgeable"
                    }
                })
            })
            .collect();

        Json(json!({
            "value": version_list
        }))
        .into_response()
    } else {
        // No versions found, return empty list
        Json(json!({
            "value": []
        }))
        .into_response()
    }
}

/// GET list of all secrets
/// Path: /secrets
/// Query: api-version=2025-07-01
async fn list_all_secrets(State(app_state): State<AzureAppState>) -> Response {
    info!("  GET all secrets list");

    // Get all secret names
    let all_keys = app_state.secrets.list_all_secrets().await;

    let secret_list: Vec<serde_json::Value> = all_keys
        .iter()
        .filter_map(|secret_name| {
            // Get latest version for metadata
            let latest_version = app_state.secrets.get_latest(secret_name);
            // Use tokio::runtime::Handle to run async in sync context
            let rt = tokio::runtime::Handle::current();
            let version = rt.block_on(latest_version)?;

            Some(json!({
                "id": format!("https://test-vault.vault.azure.net/secrets/{}", secret_name),
                "attributes": {
                    "enabled": version.enabled,
                    "created": format_timestamp_azure(version.created_at),
                    "updated": format_timestamp_azure(version.created_at),
                    "recoveryLevel": "Recoverable+Purgeable"
                }
            }))
        })
        .collect();

    Json(json!({
        "value": secret_list,
        "nextLink": null
    }))
    .into_response()
}

/// PUT secret (set/update)
/// Path: /secrets/{name} (without trailing slash)
/// Query: api-version=2025-07-01
async fn set_secret(
    State(app_state): State<AzureAppState>,
    Path(name): Path<String>,
    Json(body): Json<SetSecretRequest>,
) -> Response {
    info!(
        "  PUT secret: name={}, value_length={}",
        name,
        body.value.len()
    );

    // Validate secret size (Azure limit: 25KB)
    if let Err(size_error) = validate_azure_secret_size(&body.value) {
        warn!("  Secret size validation failed: {}", size_error);
        return azure_error_response(
            StatusCode::BAD_REQUEST,
            azure_error_codes::BAD_PARAMETER,
            size_error,
        );
    }

    // Create new version
    let version_id = app_state
        .secrets
        .set_secret(&name, body.value.clone())
        .await;

    // Get the version to include timestamp
    let version = app_state.secrets.get_version(&name, &version_id).await;
    let created = version
        .as_ref()
        .map(|v| format_timestamp_azure(v.created_at))
        .unwrap_or_else(|| {
            format_timestamp_azure(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            )
        });
    let updated = created; // Azure uses same timestamp for created/updated in our mock

    Json(json!({
        "value": body.value,
        "id": format!("https://test-vault.vault.azure.net/secrets/{}/{}", name, version_id),
        "attributes": {
            "enabled": true,
            "created": created,
            "updated": updated,
            "recoveryLevel": "Recoverable+Purgeable"
        }
    }))
    .into_response()
}

#[derive(serde::Deserialize)]
struct UpdateSecretRequest {
    attributes: Option<SecretAttributes>,
}

#[derive(serde::Deserialize)]
struct SecretAttributes {
    enabled: Option<bool>,
}

/// PATCH secret (update attributes like enabled/disabled)
/// Path: /secrets/{name} (without trailing slash)
/// Query: api-version=2025-07-01
async fn update_secret(
    State(app_state): State<AzureAppState>,
    Path(name): Path<String>,
    Json(body): Json<UpdateSecretRequest>,
) -> Response {
    info!("  PATCH secret: name={}", name);

    // Check if secret exists
    if !app_state.secrets.exists(&name).await {
        return azure_error_response(
            StatusCode::NOT_FOUND,
            azure_error_codes::SECRET_NOT_FOUND,
            format!("Secret {} not found", name),
        );
    }

    // Update enabled state if provided
    if let Some(attributes) = body.attributes {
        if let Some(enabled) = attributes.enabled {
            if enabled {
                app_state.secrets.enable_secret(&name).await;
                info!("  Enabled secret: {}", name);
            } else {
                app_state.secrets.disable_secret(&name).await;
                info!("  Disabled secret: {}", name);
            }
        }
    }

    // Get latest version for response
    let latest_version = app_state.secrets.get_latest(&name).await;
    let created = latest_version
        .as_ref()
        .map(|v| format_timestamp_azure(v.created_at))
        .unwrap_or_else(|| {
            format_timestamp_azure(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            )
        });
    let updated = format_timestamp_azure(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );

    let version_id = latest_version
        .as_ref()
        .map(|v| v.version_id.clone())
        .unwrap_or_else(|| "abc123".to_string());

    let is_enabled = app_state.secrets.is_enabled(&name).await;

    Json(json!({
        "id": format!("https://test-vault.vault.azure.net/secrets/{}/{}", name, version_id),
        "attributes": {
            "enabled": is_enabled,
            "created": created,
            "updated": updated,
            "recoveryLevel": "Recoverable+Purgeable"
        }
    }))
    .into_response()
}

/// DELETE secret
/// Path: /secrets/{name}
/// Query: api-version=2025-07-01
///
/// Azure Key Vault uses soft-delete by default, but for simplicity in the mock server,
/// we implement immediate deletion (no soft-delete recovery period).
/// In production, Azure Key Vault would soft-delete the secret and allow recovery
/// within the retention period (7-90 days).
async fn delete_secret(
    State(app_state): State<AzureAppState>,
    Path(name): Path<String>,
) -> Response {
    info!("  DELETE secret: name={}", name);

    // Check if secret exists
    if !app_state.secrets.exists(&name).await {
        return azure_error_response(
            StatusCode::NOT_FOUND,
            azure_error_codes::SECRET_NOT_FOUND,
            format!("Secret {} not found", name),
        );
    }

    // Delete the secret (all versions)
    if app_state.secrets.delete_secret(&name).await {
        // Azure Key Vault returns 200 OK with the deleted secret's attributes
        // For simplicity, we return a minimal response matching Azure's soft-delete format
        Json(json!({
            "id": format!("https://test-vault.vault.azure.net/secrets/{}", name),
            "recoveryId": format!("https://test-vault.vault.azure.net/deletedsecrets/{}", name),
            "deletedDate": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            "scheduledPurgeDate": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() + (90 * 24 * 60 * 60), // 90 days from now (default retention)
        }))
        .into_response()
    } else {
        // Should not happen since we checked existence, but handle gracefully
        azure_error_response(
            StatusCode::NOT_FOUND,
            azure_error_codes::SECRET_NOT_FOUND,
            format!("Secret {} not found", name),
        )
    }
}

/// POST backup secret
/// Path: /secrets/{name}/backup
/// Query: api-version=7.4
async fn backup_secret(
    State(app_state): State<AzureAppState>,
    Path(name): Path<String>,
) -> Response {
    info!("  BACKUP secret: name={}", name);

    // Check if secret exists
    if !app_state.secrets.exists(&name).await {
        return azure_error_response(
            StatusCode::NOT_FOUND,
            azure_error_codes::SECRET_NOT_FOUND,
            format!("Secret {} not found", name),
        );
    }

    // Get all versions of the secret
    let versions = app_state.secrets.list_versions(&name).await;

    // Create a simple backup blob (base64-encoded JSON of secret data)
    // In a real implementation, this would be a more complex backup format
    let backup_data = if let Some(versions) = versions {
        // Serialize all versions to JSON, then base64 encode
        let backup_json = json!({
            "name": name,
            "versions": versions.iter().map(|v| &v.data).collect::<Vec<_>>()
        });
        let json_str = serde_json::to_string(&backup_json).unwrap_or_else(|_| "{}".to_string());
        STANDARD.encode(json_str.as_bytes())
    } else {
        // Empty backup if no versions
        STANDARD.encode("{}".as_bytes())
    };

    Json(json!({
        "value": backup_data
    }))
    .into_response()
}

#[derive(serde::Deserialize)]
struct RestoreSecretRequest {
    value: String,
}

/// POST restore secret from backup
/// Path: /secrets/restore
/// Query: api-version=7.4
async fn restore_secret(
    State(app_state): State<AzureAppState>,
    Json(body): Json<RestoreSecretRequest>,
) -> Response {
    info!("  RESTORE secret from backup");

    // Decode base64 backup blob
    let backup_bytes = match STANDARD.decode(&body.value) {
        Ok(bytes) => bytes,
        Err(_) => {
            return azure_error_response(
                StatusCode::BAD_REQUEST,
                azure_error_codes::BAD_PARAMETER,
                "Invalid backup blob format".to_string(),
            );
        }
    };

    // Parse JSON backup data
    let backup_str = match String::from_utf8(backup_bytes) {
        Ok(s) => s,
        Err(_) => {
            return azure_error_response(
                StatusCode::BAD_REQUEST,
                azure_error_codes::BAD_PARAMETER,
                "Invalid backup blob format".to_string(),
            );
        }
    };

    let backup_json: serde_json::Value = match serde_json::from_str(&backup_str) {
        Ok(json) => json,
        Err(_) => {
            return azure_error_response(
                StatusCode::BAD_REQUEST,
                azure_error_codes::BAD_PARAMETER,
                "Invalid backup blob format".to_string(),
            );
        }
    };

    // Extract secret name and restore
    let secret_name = backup_json
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("restored-secret");

    // Restore versions (simplified - just restore the latest value)
    if let Some(versions) = backup_json.get("versions").and_then(|v| v.as_array()) {
        if let Some(last_version) = versions.last() {
            if let Some(value) = last_version.get("value").and_then(|v| v.as_str()) {
                let version_id = app_state
                    .secrets
                    .set_secret(secret_name, value.to_string())
                    .await;
                let version = app_state
                    .secrets
                    .get_version(secret_name, &version_id)
                    .await;
                let created = version
                    .as_ref()
                    .map(|v| format_timestamp_azure(v.created_at))
                    .unwrap_or_else(|| {
                        format_timestamp_azure(
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        )
                    });

                return Json(json!({
                    "id": format!("https://test-vault.vault.azure.net/secrets/{}/{}", secret_name, version_id),
                    "attributes": {
                        "enabled": true,
                        "created": created,
                        "updated": created,
                        "recoveryLevel": "Recoverable+Purgeable"
                    }
                }))
                .into_response();
            }
        }
    }

    azure_error_response(
        StatusCode::BAD_REQUEST,
        azure_error_codes::BAD_PARAMETER,
        "Invalid backup data".to_string(),
    )
}

/// GET deleted secret
/// Path: /deletedsecrets/{name}
/// Query: api-version=7.4
async fn get_deleted_secret(
    State(app_state): State<AzureAppState>,
    Path(name): Path<String>,
) -> Response {
    info!("  GET deleted secret: name={}", name);

    if let Some((deleted_date, scheduled_purge_date)) =
        app_state.secrets.get_deleted_secret(&name).await
    {
        Json(json!({
            "recoveryId": format!("https://test-vault.vault.azure.net/deletedsecrets/{}", name),
            "deletedDate": deleted_date,
            "scheduledPurgeDate": scheduled_purge_date,
            "recoveryLevel": "Recoverable+Purgeable"
        }))
        .into_response()
    } else {
        azure_error_response(
            StatusCode::NOT_FOUND,
            azure_error_codes::SECRET_NOT_FOUND,
            format!("Deleted secret {} not found", name),
        )
    }
}

/// GET list deleted secrets
/// Path: /deletedsecrets
/// Query: api-version=7.4
async fn list_deleted_secrets(State(app_state): State<AzureAppState>) -> Response {
    info!("  GET deleted secrets list");

    let deleted_names = app_state.secrets.list_deleted_secrets().await;
    let deleted_list: Vec<serde_json::Value> = deleted_names
        .iter()
        .filter_map(|name| {
            let (deleted_date, scheduled_purge_date) = {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(app_state.secrets.get_deleted_secret(name))?
            };

            Some(json!({
                "recoveryId": format!("https://test-vault.vault.azure.net/deletedsecrets/{}", name),
                "deletedDate": deleted_date,
                "scheduledPurgeDate": scheduled_purge_date
            }))
        })
        .collect();

    Json(json!({
        "value": deleted_list,
        "nextLink": null
    }))
    .into_response()
}

/// POST recover deleted secret
/// Path: /deletedsecrets/{name}/recover
/// Query: api-version=7.4
async fn recover_deleted_secret(
    State(app_state): State<AzureAppState>,
    Path(name): Path<String>,
) -> Response {
    info!("  RECOVER deleted secret: name={}", name);

    if app_state.secrets.recover_secret(&name).await {
        // Get the restored secret's latest version
        let latest_version = app_state.secrets.get_latest(&name).await;
        let version_id = latest_version
            .as_ref()
            .map(|v| v.version_id.clone())
            .unwrap_or_else(|| "abc123".to_string());
        let created = latest_version
            .as_ref()
            .map(|v| format_timestamp_azure(v.created_at))
            .unwrap_or_else(|| {
                format_timestamp_azure(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                )
            });

        Json(json!({
            "id": format!("https://test-vault.vault.azure.net/secrets/{}/{}", name, version_id),
            "attributes": {
                "enabled": true,
                "created": created,
                "updated": created,
                "recoveryLevel": "Recoverable+Purgeable"
            }
        }))
        .into_response()
    } else {
        azure_error_response(
            StatusCode::NOT_FOUND,
            azure_error_codes::SECRET_NOT_FOUND,
            format!("Deleted secret {} not found", name),
        )
    }
}

/// DELETE purge deleted secret
/// Path: /deletedsecrets/{name}
/// Query: api-version=7.4
async fn purge_deleted_secret(
    State(app_state): State<AzureAppState>,
    Path(name): Path<String>,
) -> StatusCode {
    info!("  PURGE deleted secret: name={}", name);

    if app_state.secrets.purge_deleted_secret(&name).await {
        StatusCode::NO_CONTENT // Azure returns 204 for purge
    } else {
        StatusCode::NOT_FOUND
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    // Load configuration from environment
    let broker_url =
        env::var("PACT_BROKER_URL").unwrap_or_else(|_| "http://pact-broker:9292".to_string());
    let username = env::var("PACT_BROKER_USERNAME").unwrap_or_else(|_| "pact".to_string());
    let password = env::var("PACT_BROKER_PASSWORD").unwrap_or_else(|_| "pact".to_string());
    let provider = env::var("PACT_PROVIDER").unwrap_or_else(|_| "Azure-Key-Vault".to_string());
    let consumer =
        env::var("PACT_CONSUMER").unwrap_or_else(|_| "Secret-Manager-Controller".to_string());
    let port = env::var("PORT")
        .unwrap_or_else(|_| "1234".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid u16");

    info!("Starting Azure Key Vault Mock Server...");
    info!("Broker URL: {}", broker_url);
    info!("Provider: {}, Consumer: {}", provider, consumer);

    // Load contracts from broker
    let contracts =
        load_contracts_from_broker(&broker_url, &username, &password, &provider, &consumer).await;
    if contracts.is_empty() {
        warn!("⚠️  No contracts loaded, using default mock responses");
    }

    let contracts_state = AppState::new(contracts);
    let app_state = AzureAppState {
        contracts: contracts_state.contracts,
        secrets: AzureSecretStore::new(),
    };

    // Build router with Azure Key Vault API endpoints
    // Note: GET uses trailing slash, PUT does not
    // All paths are defined in paths::azure::key_vault
    let app = Router::new()
        // Health check endpoints
        .route("/", get(health_check))
        .route("/health", get(health_check))
        // Azure Key Vault Secrets API endpoints
        // Using route constants from paths::azure::routes for type safety
        // GET /secrets - List all secrets
        .route(azure_routes::LIST_SECRETS, get(list_all_secrets))
        // GET /secrets/{name}/ - Get secret (with trailing slash)
        .route(azure_routes::GET_SECRET, get(get_secret))
        // GET /secrets/{name}/{version} - Get specific version
        .route(azure_routes::GET_SECRET_VERSION, get(get_secret_version))
        // GET /secrets/{name}/versions - List all versions
        .route(
            azure_routes::LIST_SECRET_VERSIONS,
            get(list_secret_versions),
        )
        // PUT /secrets/{name} - Set secret (without trailing slash)
        .route(azure_routes::SET_SECRET, put(set_secret))
        // DELETE /secrets/{name} - Delete secret (all versions)
        .route(azure_routes::DELETE_SECRET, delete(delete_secret))
        // PATCH /secrets/{name} - Update secret attributes (enabled/disabled)
        .route(azure_routes::UPDATE_SECRET, patch(update_secret))
        // POST /secrets/{name}/backup - Backup secret
        .route(azure_routes::BACKUP_SECRET, post(backup_secret))
        // POST /secrets/restore - Restore secret from backup
        .route(azure_routes::RESTORE_SECRET, post(restore_secret))
        // GET /deletedsecrets/{name} - Get deleted secret
        .route(azure_routes::GET_DELETED_SECRET, get(get_deleted_secret))
        // GET /deletedsecrets - List deleted secrets
        .route(
            azure_routes::LIST_DELETED_SECRETS,
            get(list_deleted_secrets),
        )
        // POST /deletedsecrets/{name}/recover - Recover deleted secret
        .route(
            azure_routes::RECOVER_DELETED_SECRET,
            post(recover_deleted_secret),
        )
        // DELETE /deletedsecrets/{name} - Purge deleted secret
        .route(
            azure_routes::PURGE_DELETED_SECRET,
            delete(purge_deleted_secret),
        )
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(axum::middleware::from_fn(auth_failure_middleware))
                .layer(axum::middleware::from_fn(service_unavailable_middleware))
                .layer(axum::middleware::from_fn(rate_limit_middleware))
                .layer(axum::middleware::from_fn(logging_middleware)),
        )
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Listening on port {}", port);
    info!("✅ Azure Mock server ready at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
