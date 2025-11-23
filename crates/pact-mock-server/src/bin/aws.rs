//! AWS Secrets Manager Mock Server
//!
//! A lightweight Axum-based HTTP server that serves as a mock for the AWS Secrets Manager API.
//! All requests go to POST "/" with x-amz-target header specifying the operation.
//!
//! Environment Variables:
//! - PACT_BROKER_URL: URL of the Pact broker (default: http://pact-broker:9292)
//! - PACT_BROKER_USERNAME: Username for broker authentication (default: pact)
//! - PACT_BROKER_PASSWORD: Password for broker authentication (default: pact)
//! - PACT_PROVIDER: Provider name in contracts (default: AWS-Secrets-Manager)
//! - PACT_CONSUMER: Consumer name in contracts (default: Secret-Manager-Controller)
//! - PORT: Port to listen on (default: 1234)

use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::post,
    Router,
};
use pact_mock_server::prelude::*;
use pact_mock_server::secrets::common::errors::aws_error_types;
use paths::aws::routes::secrets_manager as aws_routes;
use paths::aws::secrets_manager;
use serde_json::json;
use std::env;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{info, warn, Level};

/// AWS-specific application state
#[derive(Clone)]
struct AwsAppState {
    #[allow(dead_code)] // Reserved for future contract-based responses
    contracts:
        std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, serde_json::Value>>>,
    #[allow(dead_code)] // Will be used when AWS handlers are fully implemented
    secrets: AwsSecretStore,
}

/// Format Unix timestamp to AWS API format (seconds since epoch as float)
fn format_timestamp_aws(timestamp: u64) -> f64 {
    timestamp as f64
}

/// Handler for all AWS Secrets Manager requests
/// All requests are POST to "/" with x-amz-target header
async fn handle_aws_request(State(app_state): State<AwsAppState>, request: Request) -> Response {
    // Extract target header before consuming request
    let target = request
        .headers()
        .get("x-amz-target")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    info!("  AWS Request: x-amz-target={}", target);

    // Parse request body once and extract secret name
    let (secret_name, body_json) = match axum::body::to_bytes(request.into_body(), usize::MAX).await
    {
        Ok(bytes) => {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                let name = json
                    .get("Name")
                    .or_else(|| json.get("SecretId"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("test-secret-name")
                    .to_string();
                (name, Some(json))
            } else {
                ("test-secret-name".to_string(), None)
            }
        }
        Err(_) => {
            return aws_error_response(
                StatusCode::BAD_REQUEST,
                aws_error_types::INVALID_PARAMETER,
                "Invalid request body".to_string(),
            );
        }
    };

    // Use PathBuilder to validate and get operation constants
    // Match against constants from paths::aws::secrets_manager
    match target.as_str() {
        secrets_manager::CREATE_SECRET => {
            info!("  CREATE secret: {}", secret_name);

            // Validate secret size if SecretString is provided (AWS limit: 64KB)
            if let Some(json) = &body_json {
                if let Some(secret_string) = json.get("SecretString").and_then(|v| v.as_str()) {
                    if let Err(size_error) = validate_aws_secret_size(secret_string) {
                        warn!("  Secret size validation failed: {}", size_error);
                        return aws_error_response(
                            StatusCode::BAD_REQUEST,
                            aws_error_types::INVALID_PARAMETER,
                            size_error,
                        );
                    }
                }
            }

            // Try to get the created version to include timestamp
            let current_version = app_state.secrets.get_current(&secret_name).await;
            let created_date = current_version
                .as_ref()
                .map(|v| format_timestamp_aws(v.created_at))
                .unwrap_or_else(|| {
                    format_timestamp_aws(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    )
                });

            (
                StatusCode::OK,
                Json(json!({
                    "ARN": format!("arn:aws:secretsmanager:us-east-1:123456789012:secret:{}", secret_name),
                    "Name": secret_name,
                    "VersionId": current_version.as_ref().map(|v| v.version_id.clone()).unwrap_or_else(|| "test-version-id".to_string()),
                    "CreatedDate": created_date
                })),
            )
                .into_response()
        }
        secrets_manager::GET_SECRET_VALUE => {
            info!("  GET secret value: {}", secret_name);

            // Check if secret is deleted (disabled)
            if app_state.secrets.is_deleted(&secret_name).await {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "__type": "InvalidRequestException",
                        "message": format!("You tried to access a secret that is scheduled for deletion. Use RestoreSecret to restore it.")
                    })),
                )
                    .into_response();
            }

            // Check if VersionId is specified in request body
            let version_id = body_json
                .as_ref()
                .and_then(|json| json.get("VersionId"))
                .and_then(|v| v.as_str());

            // Get version (specific or current)
            let version = if let Some(vid) = version_id {
                info!("  Getting specific version: {}", vid);
                app_state.secrets.get_version(&secret_name, vid).await
            } else {
                info!("  Getting current version (AWSCURRENT)");
                app_state.secrets.get_current(&secret_name).await
            };

            if version.is_none() {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "__type": "ResourceNotFoundException",
                        "message": format!("Secrets Manager can't find the specified secret{}",
                            version_id.map(|v| format!(" version {}", v)).unwrap_or_default())
                    })),
                )
                    .into_response();
            }

            let created_date = version
                .as_ref()
                .map(|v| format_timestamp_aws(v.created_at))
                .unwrap_or_else(|| {
                    format_timestamp_aws(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    )
                });

            let secret_value = version
                .as_ref()
                .and_then(|v| v.data.get("SecretString"))
                .and_then(|v| v.as_str())
                .unwrap_or("mock-secret-value");

            let version_id_used = version
                .as_ref()
                .map(|v| v.version_id.clone())
                .unwrap_or_else(|| "unknown".to_string());

            // Get staging labels to determine VersionStages
            let labels = app_state
                .secrets
                .get_staging_labels(&secret_name)
                .await
                .unwrap_or_default();
            let version_stages: Vec<String> = labels
                .iter()
                .filter(|(_, vid)| **vid == version_id_used)
                .map(|(label, _)| label.clone())
                .collect();

            (
                StatusCode::OK,
                Json(json!({
                    "ARN": format!("arn:aws:secretsmanager:us-east-1:123456789012:secret:{}", secret_name),
                    "Name": secret_name,
                    "SecretString": secret_value,
                    "VersionId": version_id_used,
                    "VersionStages": if version_stages.is_empty() {
                        vec!["AWSCURRENT".to_string()] // Default if no labels
                    } else {
                        version_stages
                    },
                    "CreatedDate": created_date
                })),
            )
                .into_response()
        }
        secrets_manager::DESCRIBE_SECRET => {
            info!("  DESCRIBE secret: {}", secret_name);
            // Get current version for timestamp
            let current_version = app_state.secrets.get_current(&secret_name).await;
            let created_date = current_version
                .as_ref()
                .map(|v| format_timestamp_aws(v.created_at))
                .unwrap_or_else(|| {
                    format_timestamp_aws(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    )
                });

            (
                StatusCode::OK,
                Json(json!({
                    "ARN": format!("arn:aws:secretsmanager:us-east-1:123456789012:secret:{}", secret_name),
                    "Name": secret_name,
                    "VersionIdToStages": {
                        current_version.as_ref().map(|v| v.version_id.clone()).unwrap_or_else(|| "current-version-id".to_string()): ["AWSCURRENT"]
                    },
                    "CreatedDate": created_date
                })),
            )
                .into_response()
        }
        secrets_manager::PUT_SECRET_VALUE => {
            info!("  PUT secret value: {}", secret_name);

            // Validate secret size if SecretString is provided (AWS limit: 64KB)
            if let Some(json) = &body_json {
                if let Some(secret_string) = json.get("SecretString").and_then(|v| v.as_str()) {
                    if let Err(size_error) = validate_aws_secret_size(secret_string) {
                        warn!("  Secret size validation failed: {}", size_error);
                        return aws_error_response(
                            StatusCode::BAD_REQUEST,
                            aws_error_types::INVALID_PARAMETER,
                            size_error,
                        );
                    }
                }
            }

            // Get current version for timestamp
            let current_version = app_state.secrets.get_current(&secret_name).await;
            let created_date = current_version
                .as_ref()
                .map(|v| format_timestamp_aws(v.created_at))
                .unwrap_or_else(|| {
                    format_timestamp_aws(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    )
                });

            (
                StatusCode::OK,
                Json(json!({
                    "ARN": format!("arn:aws:secretsmanager:us-east-1:123456789012:secret:{}", secret_name),
                    "Name": secret_name,
                    "VersionId": current_version.as_ref().map(|v| v.version_id.clone()).unwrap_or_else(|| "new-version-id".to_string()),
                    "VersionStages": ["AWSCURRENT"],
                    "CreatedDate": created_date
                })),
            )
                .into_response()
        }
        secrets_manager::DELETE_SECRET => {
            info!("  DELETE secret: {}", secret_name);

            // Parse recovery window from request body (optional)
            let recovery_window_days = body_json
                .as_ref()
                .and_then(|json| json.get("RecoveryWindowInDays"))
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);

            if app_state
                .secrets
                .delete_secret_with_recovery(&secret_name, recovery_window_days)
                .await
            {
                let deletion_date = format_timestamp_aws(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                );
                (
                    StatusCode::OK,
                    Json(json!({
                        "ARN": format!("arn:aws:secretsmanager:us-east-1:123456789012:secret:{}", secret_name),
                        "Name": secret_name,
                        "DeletionDate": deletion_date
                    })),
                )
                    .into_response()
            } else {
                aws_error_response(
                    StatusCode::NOT_FOUND,
                    aws_error_types::RESOURCE_NOT_FOUND,
                    format!("Secret {} not found", secret_name),
                )
            }
        }
        secrets_manager::RESTORE_SECRET => {
            info!("  RESTORE secret: {}", secret_name);

            if app_state.secrets.restore_secret(&secret_name).await {
                (
                    StatusCode::OK,
                    Json(json!({
                        "ARN": format!("arn:aws:secretsmanager:us-east-1:123456789012:secret:{}", secret_name),
                        "Name": secret_name
                    })),
                )
                    .into_response()
            } else {
                aws_error_response(
                    StatusCode::NOT_FOUND,
                    aws_error_types::RESOURCE_NOT_FOUND,
                    format!("Secret {} not found", secret_name),
                )
            }
        }
        secrets_manager::LIST_SECRET_VERSIONS => {
            info!("  LIST secret versions: {}", secret_name);

            // Check if secret exists
            if !app_state.secrets.exists(&secret_name).await {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "__type": "ResourceNotFoundException",
                        "message": format!("Secrets Manager can't find the specified secret.")
                    })),
                )
                    .into_response();
            }

            // Get all versions
            if let Some(versions) = app_state.secrets.list_versions(&secret_name).await {
                // Get staging labels for this secret
                let labels = app_state
                    .secrets
                    .get_staging_labels(&secret_name)
                    .await
                    .unwrap_or_default();

                let version_list: Vec<serde_json::Value> = versions
                    .iter()
                    .map(|v| {
                        // Find which labels point to this version
                        let version_stages: Vec<String> = labels
                            .iter()
                            .filter(|(_, vid)| **vid == v.version_id)
                            .map(|(label, _)| label.clone())
                            .collect();

                        json!({
                            "VersionId": v.version_id,
                            "VersionStages": if version_stages.is_empty() {
                                vec!["AWSCURRENT".to_string()] // Default if no labels
                            } else {
                                version_stages
                            },
                            "CreatedDate": format_timestamp_aws(v.created_at)
                        })
                    })
                    .collect();

                (
                    StatusCode::OK,
                    Json(json!({
                        "ARN": format!("arn:aws:secretsmanager:us-east-1:123456789012:secret:{}", secret_name),
                        "Name": secret_name,
                        "Versions": version_list
                    })),
                )
                    .into_response()
            } else {
                // No versions found, return empty list
                (
                    StatusCode::OK,
                    Json(json!({
                        "ARN": format!("arn:aws:secretsmanager:us-east-1:123456789012:secret:{}", secret_name),
                        "Name": secret_name,
                        "Versions": []
                    })),
                )
                    .into_response()
            }
        }
        secrets_manager::UPDATE_SECRET => {
            info!("  UPDATE secret: {}", secret_name);
            // UpdateSecret can update description, KMS key, etc.
            // For now, just return success
            let current_version = app_state.secrets.get_current(&secret_name).await;
            let created_date = current_version
                .as_ref()
                .map(|v| format_timestamp_aws(v.created_at))
                .unwrap_or_else(|| {
                    format_timestamp_aws(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    )
                });

            (
                StatusCode::OK,
                Json(json!({
                    "ARN": format!("arn:aws:secretsmanager:us-east-1:123456789012:secret:{}", secret_name),
                    "Name": secret_name,
                    "VersionId": current_version.as_ref().map(|v| v.version_id.clone()).unwrap_or_else(|| "current-version-id".to_string()),
                    "CreatedDate": created_date
                })),
            )
                .into_response()
        }
        secrets_manager::LIST_SECRETS => {
            info!("  LIST all secrets");

            // Get all secret names
            let all_keys = app_state.secrets.list_all_secrets().await;

            let secret_list: Vec<serde_json::Value> = all_keys
                .iter()
                .filter_map(|secret_name| {
                    // Get current version for metadata
                    let current_version = app_state.secrets.get_current(secret_name);
                    // Use tokio::runtime::Handle to run async in sync context
                    let rt = tokio::runtime::Handle::current();
                    let version = rt.block_on(current_version)?;

                    let labels = rt.block_on(app_state.secrets.get_staging_labels(secret_name)).unwrap_or_default();
                    let version_stages: Vec<String> = labels.iter()
                        .filter(|(_, vid)| **vid == version.version_id)
                        .map(|(label, _)| label.clone())
                        .collect();

                    Some(json!({
                        "ARN": format!("arn:aws:secretsmanager:us-east-1:123456789012:secret:{}", secret_name),
                        "Name": secret_name,
                        "Description": "",
                        "LastChangedDate": format_timestamp_aws(version.created_at),
                        "LastRotatedDate": format_timestamp_aws(version.created_at),
                        "VersionIdToStages": {
                            version.version_id: if version_stages.is_empty() {
                                vec!["AWSCURRENT".to_string()]
                            } else {
                                version_stages
                            }
                        }
                    }))
                })
                .collect();

            (
                StatusCode::OK,
                Json(json!({
                    "SecretList": secret_list
                })),
            )
                .into_response()
        }
        secrets_manager::UPDATE_SECRET_VERSION_STAGE => {
            info!("  UPDATE secret version stage: {}", secret_name);

            // Parse VersionId and VersionStage from request body
            let version_id = body_json
                .as_ref()
                .and_then(|json| json.get("VersionId"))
                .and_then(|v| v.as_str());
            let remove_from_version_id = body_json
                .as_ref()
                .and_then(|json| json.get("RemoveFromVersionId"))
                .and_then(|v| v.as_str());
            let move_to_version_id = body_json
                .as_ref()
                .and_then(|json| json.get("MoveToVersionId"))
                .and_then(|v| v.as_str());

            if version_id.is_none()
                || (remove_from_version_id.is_none() && move_to_version_id.is_none())
            {
                return aws_error_response(
                    StatusCode::BAD_REQUEST,
                    aws_error_types::INVALID_PARAMETER,
                    "VersionId and either RemoveFromVersionId or MoveToVersionId are required"
                        .to_string(),
                );
            }

            // Update staging labels
            // AWS UpdateSecretVersionStage moves a staging label from one version to another
            if let (Some(remove_vid), Some(move_vid)) = (remove_from_version_id, move_to_version_id)
            {
                // Default to AWSCURRENT if no specific label is provided
                let label = "AWSCURRENT";

                if !app_state
                    .secrets
                    .update_staging_label(&secret_name, label, Some(remove_vid), move_vid)
                    .await
                {
                    return aws_error_response(
                        StatusCode::NOT_FOUND,
                        aws_error_types::RESOURCE_NOT_FOUND,
                        format!(
                            "One or more versions not found: {} or {}",
                            remove_vid, move_vid
                        ),
                    );
                }
            } else if let Some(move_vid) = move_to_version_id {
                // If only MoveToVersionId is provided, just add the label (default to AWSCURRENT)
                let label = "AWSCURRENT";
                if !app_state
                    .secrets
                    .update_staging_label(&secret_name, label, None, move_vid)
                    .await
                {
                    return aws_error_response(
                        StatusCode::NOT_FOUND,
                        aws_error_types::RESOURCE_NOT_FOUND,
                        format!("Version not found: {}", move_vid),
                    );
                }
            }

            (
                StatusCode::OK,
                Json(json!({
                    "ARN": format!("arn:aws:secretsmanager:us-east-1:123456789012:secret:{}", secret_name),
                    "Name": secret_name
                })),
            )
                .into_response()
        }
        secrets_manager::TAG_RESOURCE => {
            info!("  TAG secret: {}", secret_name);

            // Check if secret exists
            if !app_state.secrets.exists(&secret_name).await {
                return aws_error_response(
                    StatusCode::NOT_FOUND,
                    aws_error_types::RESOURCE_NOT_FOUND,
                    format!("Secret {} not found", secret_name),
                );
            }

            // Parse tags from request body
            if let Some(json) = &body_json {
                if let Some(tags) = json.get("Tags").and_then(|t| t.as_array()) {
                    info!("  Adding {} tags to secret", tags.len());
                    // In a real implementation, we would store tags in the secret metadata
                    // For the mock server, we just return success
                }
            }

            // TagResource returns empty response on success
            (StatusCode::OK, Json(json!({}))).into_response()
        }
        secrets_manager::UNTAG_RESOURCE => {
            info!("  UNTAG secret: {}", secret_name);

            // Check if secret exists
            if !app_state.secrets.exists(&secret_name).await {
                return aws_error_response(
                    StatusCode::NOT_FOUND,
                    aws_error_types::RESOURCE_NOT_FOUND,
                    format!("Secret {} not found", secret_name),
                );
            }

            // Parse tag keys from request body
            if let Some(json) = &body_json {
                if let Some(tag_keys) = json.get("TagKeys").and_then(|t| t.as_array()) {
                    info!("  Removing {} tags from secret", tag_keys.len());
                    // In a real implementation, we would remove tags from secret metadata
                    // For the mock server, we just return success
                }
            }

            // UntagResource returns empty response on success
            (StatusCode::OK, Json(json!({}))).into_response()
        }
        secrets_manager::GET_RESOURCE_POLICY => {
            info!("  GET resource policy: {}", secret_name);

            // Check if secret exists
            if !app_state.secrets.exists(&secret_name).await {
                return aws_error_response(
                    StatusCode::NOT_FOUND,
                    aws_error_types::RESOURCE_NOT_FOUND,
                    format!("Secret {} not found", secret_name),
                );
            }

            // Return a default resource policy (empty policy allows all)
            // In a real implementation, this would be stored with the secret
            let default_policy = json!({
                "Version": "2012-10-17",
                "Statement": [{
                    "Effect": "Allow",
                    "Principal": {
                        "AWS": "arn:aws:iam::123456789012:root"
                    },
                    "Action": "secretsmanager:GetSecretValue",
                    "Resource": "*"
                }]
            });

            (
                StatusCode::OK,
                Json(json!({
                    "ARN": format!("arn:aws:secretsmanager:us-east-1:123456789012:secret:{}", secret_name),
                    "Name": secret_name,
                    "ResourcePolicy": serde_json::to_string(&default_policy).unwrap_or_else(|_| "{}".to_string())
                })),
            )
                .into_response()
        }
        _ => {
            warn!("  ⚠️  Unknown x-amz-target: {}", target);
            aws_error_response(
                StatusCode::BAD_REQUEST,
                aws_error_types::INVALID_REQUEST,
                format!("Unknown target: {}", target),
            )
        }
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
    let provider = env::var("PACT_PROVIDER").unwrap_or_else(|_| "AWS-Secrets-Manager".to_string());
    let consumer =
        env::var("PACT_CONSUMER").unwrap_or_else(|_| "Secret-Manager-Controller".to_string());
    let port = env::var("PORT")
        .unwrap_or_else(|_| "1234".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid u16");

    info!("Starting AWS Secrets Manager Mock Server...");
    info!("Broker URL: {}", broker_url);
    info!("Provider: {}, Consumer: {}", provider, consumer);

    // Wait for manager to be ready and our provider's pact to be published
    // The manager tracks which pacts have been successfully published
    let manager_url =
        env::var("MANAGER_URL").unwrap_or_else(|_| "http://localhost:8081".to_string());
    info!("Manager URL: {}", manager_url);

    if let Err(e) = wait_for_manager_ready(
        &manager_url,
        &provider,
        90, // 90 seconds max wait - should be enough with manager sidecar
    )
    .await
    {
        eprintln!("❌ Failed to wait for manager and pact: {}", e);
        eprintln!("⚠️  Starting server anyway with default mock responses");
    }

    // Load contracts from broker
    let contracts =
        load_contracts_from_broker(&broker_url, &username, &password, &provider, &consumer).await;
    if contracts.is_empty() {
        warn!("⚠️  No contracts loaded, using default mock responses");
    }

    let contracts_state = AppState::new(contracts);
    let app_state = AwsAppState {
        contracts: contracts_state.contracts,
        secrets: AwsSecretStore::new(),
    };

    // Build router - all AWS requests go to POST "/"
    // Build router with AWS Secrets Manager API endpoints
    // Note: AWS uses a single POST endpoint "/" with x-amz-target header
    // All operation names are defined in paths::aws::secrets_manager
    let app = Router::new()
        // Health check endpoints
        .route("/", axum::routing::get(health_check))
        .route("/health", axum::routing::get(health_check))
        // AWS Secrets Manager API - all operations via POST "/" with x-amz-target header
        // Using route constant from paths::aws::routes for type safety
        .route(aws_routes::BASE, post(handle_aws_request))
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
    info!("✅ AWS Mock server ready at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
