//! GCP Secret Manager Mock Server
//!
//! A lightweight Axum-based HTTP server that serves as a mock for the GCP Secret Manager REST API.
//! Loads contracts from the Pact broker and serves them as mock responses.
//!
//! Environment Variables:
//! - PACT_BROKER_URL: URL of the Pact broker (default: http://pact-broker:9292)
//! - PACT_BROKER_USERNAME: Username for broker authentication (default: pact)
//! - PACT_BROKER_PASSWORD: Password for broker authentication (default: pact)
//! - PACT_PROVIDER: Provider name in contracts (default: GCP-Secret-Manager)
//! - PACT_CONSUMER: Consumer name in contracts (default: Secret-Manager-Controller)
//! - PORT: Port to listen on (default: 1234)

use axum::{
    extract::{Path, Query, Request, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post},
    Router,
};
// Use std::time for timestamp generation instead of chrono
// base64 encoding is handled by the secret store
use futures::future;
use pact_mock_server::prelude::*;
use paths::gcp::routes;
use paths::prelude::{GcpOperation, PathBuilder};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{info, warn, Level};

/// Format Unix timestamp (seconds) to RFC3339 format (GCP API format)
fn format_timestamp_rfc3339(timestamp: u64) -> String {
    // Format as RFC3339 (e.g., "2023-01-01T00:00:00Z")
    // Using a simple format since we don't have chrono in dependencies
    // GCP uses format like "2023-01-01T00:00:00.000000Z"
    let secs = timestamp;
    let days = secs / 86400;
    let secs_in_day = secs % 86400;
    let hours = secs_in_day / 3600;
    let minutes = (secs_in_day % 3600) / 60;
    let seconds = secs_in_day % 60;

    // Approximate year calculation (simplified, but sufficient for mock)
    let year = 1970 + (days / 365);
    let day_of_year = days % 365;
    let month = 1 + (day_of_year / 30);
    let day = 1 + (day_of_year % 30);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.000000Z",
        year, month, day, hours, minutes, seconds
    )
}

/// GCP-specific application state
#[derive(Clone)]
struct GcpAppState {
    #[allow(dead_code)] // Reserved for future contract-based responses
    contracts:
        std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, serde_json::Value>>>,
    secrets: GcpSecretStore,
    parameters: GcpParameterStore,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateSecretRequest {
    #[serde(rename = "secretId")]
    secret_id: String,
    replication: Replication,
    #[serde(default)]
    labels: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Replication {
    automatic: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AddVersionRequest {
    payload: SecretPayload,
}

#[derive(Debug, Serialize, Deserialize)]
struct SecretPayload {
    data: String,
}

#[derive(Debug, Serialize)]
struct SecretResponse {
    name: String,
    payload: Option<SecretPayload>,
    replication: Option<Replication>,
    /// Creation timestamp (Unix timestamp in seconds)
    /// GCP includes this in version responses
    #[serde(skip_serializing_if = "Option::is_none")]
    create_time: Option<String>, // RFC3339 format
    /// Labels for the secret (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ListSecretsResponse {
    secrets: Vec<SecretResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_size: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct UpdateSecretRequest {
    /// The secret resource with updated fields
    secret: UpdateSecretSpec,
    /// A comma-separated list of the names of fields to update.
    /// E.g., "labels", "replication"
    #[serde(rename = "updateMask")]
    update_mask: String,
}

#[derive(Debug, Deserialize)]
struct UpdateSecretSpec {
    /// The resource name of the secret
    name: String,
    /// Labels to update (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<serde_json::Value>,
    /// Replication configuration (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    replication: Option<Replication>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateParameterRequest {
    #[serde(rename = "parameterId")]
    parameter_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameter: Option<ParameterSpec>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ParameterSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateParameterVersionRequest {
    #[serde(rename = "parameterVersionId", skip_serializing_if = "Option::is_none")]
    version_id: Option<String>,
    parameter_version: ParameterVersionSpec,
}

#[derive(Debug, Serialize, Deserialize)]
struct ParameterVersionSpec {
    payload: ParameterPayload,
}

#[derive(Debug, Serialize, Deserialize)]
struct ParameterPayload {
    data: String,
}

#[derive(Debug, Serialize)]
struct ParameterResponse {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<ParameterPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    create_time: Option<String>,
}

/// GET secret value (access latest version)
/// Path: /v1/projects/{project}/secrets/{secret}/versions/latest:access
async fn get_secret_value_access(
    State(app_state): State<GcpAppState>,
    Path((project, secret)): Path<(String, String)>,
) -> Response {
    info!(
        "  GET secret value (access): project={}, secret={}",
        project, secret
    );
    info!(
        "  📍 Request path: GET /v1/projects/{}/secrets/{}/versions/latest:access",
        project, secret
    );

    // Try to retrieve latest version from in-memory store
    if let Some(version) = app_state.secrets.get_latest(&project, &secret).await {
        info!(
            "  Found secret version {} in store: projects/{}/secrets/{}",
            version.version_id, project, secret
        );

        // Extract the payload from version data
        if let Some(payload_obj) = version.data.get("payload") {
            if let Some(data) = payload_obj.get("data").and_then(|v| v.as_str()) {
                // Convert Unix timestamp to RFC3339 format (GCP API format)
                let create_time = format_timestamp_rfc3339(version.created_at);

                let response = SecretResponse {
                    name: PathBuilder::new()
                        .gcp_operation(GcpOperation::GetVersion)
                        .project(&project)
                        .secret(&secret)
                        .version(&version.version_id)
                        .build_response_name()
                        .unwrap_or_else(|_| {
                            format!(
                                "projects/{}/secrets/{}/versions/{}",
                                project, secret, version.version_id
                            )
                        }),
                    payload: Some(SecretPayload {
                        data: data.to_string(),
                    }),
                    replication: None,
                    create_time: Some(create_time),
                    labels: None,
                };
                return Json(response).into_response();
            }
        }
    }

    // Secret not found in store or no enabled versions, return 404
    warn!(
        "  Secret not found or disabled in store: projects/{}/secrets/{}",
        project, secret
    );
    gcp_error_response(
        StatusCode::NOT_FOUND,
        format!(
            "Secret not found: {}",
            PathBuilder::new()
                .gcp_operation(GcpOperation::GetSecret)
                .project(&project)
                .secret(&secret)
                .build_response_name()
                .unwrap_or_else(|_| format!("projects/{}/secrets/{}", project, secret))
        ),
        Some("NOT_FOUND"),
    )
}

/// Handler for routes with colons in the path (fallback)
/// Handles:
/// - GET /v1/projects/{project}/secrets/{secret}/versions/latest:access
/// - GET /v1/projects/{project}/secrets/{secret}/versions/{version}:access
/// - GET /v1/projects/{project}/secrets/{secret}/versions (list versions)
/// - GET /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}:render
/// - POST /v1/projects/{project}/secrets/{secret}:addVersion
/// - POST /v1/projects/{project}/secrets/{secret}:disable
/// - POST /v1/projects/{project}/secrets/{secret}:enable
/// - POST /v1/projects/{project}/secrets/{secret}/versions/{version}:disable
/// - POST /v1/projects/{project}/secrets/{secret}/versions/{version}:enable
async fn handle_colon_routes(State(app_state): State<GcpAppState>, request: Request) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path();

    // Extract body for POST/PATCH/PUT requests
    let body_value = if method == Method::POST || method == Method::PATCH || method == Method::PUT {
        let (_parts, body) = request.into_parts();
        match axum::body::to_bytes(body, usize::MAX).await {
            Ok(bytes) if !bytes.is_empty() => {
                serde_json::from_slice::<serde_json::Value>(&bytes).ok()
            }
            _ => None,
        }
    } else {
        None
    };

    // Log the exact request path for debugging
    // Enable with: RUST_LOG=pact_mock_server=debug
    info!(
        method = %method,
        path = path,
        "🟢 GCP Mock Server received: {} {}",
        method,
        path
    );

    // Handle GET request to path ending with :access
    if method == Method::GET && path.contains(":access") {
        // Parse path: /v1/projects/{project}/secrets/{secret}/versions/latest:access
        // or: /v1/projects/{project}/secrets/{secret}/versions/{version}:access
        let parts: Vec<&str> = path.split('/').collect();
        let project = parts.get(3).unwrap_or(&"unknown").to_string();
        let secret = parts.get(5).unwrap_or(&"unknown").to_string();

        // Check if this is a specific version or latest
        if path.contains("/versions/latest:access") {
            return get_secret_value_access(State(app_state.clone()), Path((project, secret)))
                .await;
        } else if path.contains("/versions/") && path.contains(":access") {
            // Specific version: /v1/projects/{project}/secrets/{secret}/versions/{version}:access
            let version_part = parts.get(7).unwrap_or(&"unknown");
            let version_id = version_part
                .split(':')
                .next()
                .unwrap_or("unknown")
                .to_string();

            return get_secret_version_access(
                State(app_state.clone()),
                Path((project, secret, version_id)),
            )
            .await;
        }
    }

    // Handle POST request to path ending with :addVersion (Secret Manager only)
    // Parameter Manager now uses POST /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions
    // This handler only processes Secret Manager requests
    if method == Method::POST && path.contains(":addVersion") && !path.contains("/parameters/") {
        let parts: Vec<&str> = path.split('/').collect();
        let project = parts.get(3).unwrap_or(&"unknown").to_string();
        // Secret Manager route: /v1/projects/{project}/secrets/{secret}:addVersion
        let secret_part = parts.get(5).unwrap_or(&"unknown");
        let secret = secret_part
            .split(':')
            .next()
            .unwrap_or("unknown")
            .to_string();

        // Log request details for debugging
        info!(
            provider = "gcp",
            project = project,
            secret = secret,
            operation = "add_version",
            path = path,
            method = "POST",
            "📥 addVersion request: project={}, secret={}, path={}",
            project,
            secret,
            path
        );
        if let Some(ref body_json) = body_value {
            // Log that body is present but NOT the actual payload data
            let has_payload = body_json.get("payload").is_some();
            info!(
                provider = "gcp",
                project = project,
                secret = secret,
                operation = "add_version",
                has_payload = has_payload,
                "📦 Request body present: has_payload={}",
                has_payload
            );
        } else {
            warn!(
                provider = "gcp",
                project = project,
                secret = secret,
                operation = "add_version",
                "⚠️  No request body provided for addVersion"
            );
        }

        if let Some(body_json) = body_value {
            match serde_json::from_value::<AddVersionRequest>(body_json.clone()) {
                Ok(body) => {
                    info!(
                        provider = "gcp",
                        project = project,
                        secret = secret,
                        operation = "add_version",
                        "ADD VERSION: project={}, secret={}",
                        project,
                        secret
                    );
                    info!(
                        provider = "gcp",
                        project = project,
                        secret = secret,
                        operation = "add_version",
                        method = "POST",
                        "📍 Request path: POST /v1/projects/{}/secrets/{}:addVersion",
                        project,
                        secret
                    );

                    // Validate secret size (GCP limit: 64KB)
                    if let Err(size_error) = validate_gcp_secret_size(&body.payload.data) {
                        warn!("  Secret size validation failed: {}", size_error);
                        return gcp_error_response(
                            StatusCode::BAD_REQUEST,
                            size_error,
                            Some("INVALID_ARGUMENT"),
                        );
                    }

                    // Add a new version with the payload data
                    let version_data = json!({
                        "payload": {
                            "data": body.payload.data
                        }
                    });

                    let version_id = match app_state
                        .secrets
                        .add_version(
                            &project,
                            &secret,
                            version_data,
                            None, // Auto-generate version ID (sequential for GCP)
                        )
                        .await
                    {
                        Ok(id) => id,
                        Err(e) => {
                            warn!(
                                provider = "gcp",
                                project = project,
                                secret = secret,
                                operation = "add_version",
                                error = %e,
                                "Failed to add version: {}",
                                e
                            );
                            // Check if error is about missing environment/location
                            let error_msg = e.to_string();
                            let status_code = if error_msg.contains("NULL")
                                || error_msg.contains("empty")
                                || error_msg.contains("does not exist")
                            {
                                StatusCode::BAD_REQUEST
                            } else {
                                StatusCode::INTERNAL_SERVER_ERROR
                            };
                            return gcp_error_response(
                                status_code,
                                format!("Failed to add version: {}", e),
                                Some(if status_code == StatusCode::BAD_REQUEST {
                                    "INVALID_ARGUMENT"
                                } else {
                                    "INTERNAL"
                                }),
                            )
                            .into_response();
                        }
                    };

                    // Get the version to include timestamp
                    let version = app_state
                        .secrets
                        .get_version(&project, &secret, &version_id)
                        .await;
                    let create_time = version
                        .as_ref()
                        .map(|v| format_timestamp_rfc3339(v.created_at));

                    let response = SecretResponse {
                        name: PathBuilder::new()
                            .gcp_operation(GcpOperation::GetVersion)
                            .project(&project)
                            .secret(&secret)
                            .version(&version_id)
                            .build_response_name()
                            .unwrap_or_else(|_| {
                                format!(
                                    "projects/{}/secrets/{}/versions/{}",
                                    project, secret, version_id
                                )
                            }),
                        payload: Some(body.payload),
                        replication: None,
                        create_time,
                        labels: None,
                    };

                    info!("  Added version {} to mock secret: {}", version_id, secret);
                    return Json(response).into_response();
                }
                Err(e) => {
                    warn!("  ⚠️  Failed to parse AddVersionRequest: {}", e);
                    warn!(
                        "  📦 Body was: {}",
                        serde_json::to_string(&body_json)
                            .unwrap_or_else(|_| "invalid json".to_string())
                    );
                    return gcp_error_response(
                        StatusCode::BAD_REQUEST,
                        format!("Invalid request body for secret version: {}", e),
                        Some("INVALID_ARGUMENT"),
                    );
                }
            }
        } else {
            warn!("  ⚠️  No request body provided for addVersion operation");
            return gcp_error_response(
                StatusCode::BAD_REQUEST,
                "Missing request body for secret version".to_string(),
                Some("INVALID_ARGUMENT"),
            );
        }
    }

    // Handle POST request to path ending with :disable (secret or version)
    if method == Method::POST && path.contains(":disable") {
        // Parse path: /v1/projects/{project}/secrets/{secret}:disable
        // or: /v1/projects/{project}/secrets/{secret}/versions/{version}:disable
        let parts: Vec<&str> = path.split('/').collect();
        let project = parts.get(3).unwrap_or(&"unknown").to_string();

        if path.contains("/versions/") {
            // Version disable: /v1/projects/{project}/secrets/{secret}/versions/{version}:disable
            let secret = parts.get(5).unwrap_or(&"unknown").to_string();
            let version_part = parts.get(7).unwrap_or(&"unknown");
            let version_id = version_part
                .split(':')
                .next()
                .unwrap_or("unknown")
                .to_string();

            info!(
                "  DISABLE VERSION: project={}, secret={}, version={}",
                project, secret, version_id
            );

            if app_state
                .secrets
                .disable_version(&project, &secret, &version_id)
                .await
            {
                let response = SecretResponse {
                    name: PathBuilder::new()
                        .gcp_operation(GcpOperation::GetVersion)
                        .project(&project)
                        .secret(&secret)
                        .version(&version_id)
                        .build_response_name()
                        .unwrap_or_else(|_| {
                            format!(
                                "projects/{}/secrets/{}/versions/{}",
                                project, secret, version_id
                            )
                        }),
                    payload: None,
                    replication: None,
                    create_time: None,
                    labels: None,
                };
                return Json(response).into_response();
            } else {
                return gcp_error_response(
                    StatusCode::NOT_FOUND,
                    format!(
                        "Version not found: projects/{}/secrets/{}/versions/{}",
                        project, secret, version_id
                    ),
                    Some("NOT_FOUND"),
                );
            }
        } else {
            // Secret disable: /v1/projects/{project}/secrets/{secret}:disable
            let secret_part = parts.get(5).unwrap_or(&"unknown");
            let secret = secret_part
                .split(':')
                .next()
                .unwrap_or("unknown")
                .to_string();

            info!("  DISABLE SECRET: project={}, secret={}", project, secret);
            info!(
                "  📍 Request path: POST /v1/projects/{}/secrets/{}:disable",
                project, secret
            );

            if app_state.secrets.disable_secret(&project, &secret).await {
                let response = SecretResponse {
                    name: PathBuilder::new()
                        .gcp_operation(GcpOperation::GetSecret)
                        .project(&project)
                        .secret(&secret)
                        .build_response_name()
                        .unwrap_or_else(|_| format!("projects/{}/secrets/{}", project, secret)),
                    payload: None,
                    replication: None,
                    create_time: None,
                    labels: None,
                };
                return Json(response).into_response();
            } else {
                return gcp_error_response(
                    StatusCode::NOT_FOUND,
                    format!(
                        "Secret not found: {}",
                        PathBuilder::new()
                            .gcp_operation(GcpOperation::GetSecret)
                            .project(&project)
                            .secret(&secret)
                            .build_response_name()
                            .unwrap_or_else(|_| format!("projects/{}/secrets/{}", project, secret))
                    ),
                    Some("NOT_FOUND"),
                );
            }
        }
    }

    // Handle POST request to path ending with :enable (secret or version)
    if method == Method::POST && path.contains(":enable") {
        // Parse path: /v1/projects/{project}/secrets/{secret}:enable
        // or: /v1/projects/{project}/secrets/{secret}/versions/{version}:enable
        let parts: Vec<&str> = path.split('/').collect();
        let project = parts.get(3).unwrap_or(&"unknown").to_string();

        if path.contains("/versions/") {
            // Version enable: /v1/projects/{project}/secrets/{secret}/versions/{version}:enable
            let secret = parts.get(5).unwrap_or(&"unknown").to_string();
            let version_part = parts.get(7).unwrap_or(&"unknown");
            let version_id = version_part
                .split(':')
                .next()
                .unwrap_or("unknown")
                .to_string();

            info!(
                "  ENABLE VERSION: project={}, secret={}, version={}",
                project, secret, version_id
            );

            if app_state
                .secrets
                .enable_version(&project, &secret, &version_id)
                .await
            {
                let response = SecretResponse {
                    name: PathBuilder::new()
                        .gcp_operation(GcpOperation::GetVersion)
                        .project(&project)
                        .secret(&secret)
                        .version(&version_id)
                        .build_response_name()
                        .unwrap_or_else(|_| {
                            format!(
                                "projects/{}/secrets/{}/versions/{}",
                                project, secret, version_id
                            )
                        }),
                    payload: None,
                    replication: None,
                    create_time: None,
                    labels: None,
                };
                return Json(response).into_response();
            } else {
                return gcp_error_response(
                    StatusCode::NOT_FOUND,
                    format!(
                        "Version not found: projects/{}/secrets/{}/versions/{}",
                        project, secret, version_id
                    ),
                    Some("NOT_FOUND"),
                );
            }
        } else {
            // Secret enable: /v1/projects/{project}/secrets/{secret}:enable
            let secret_part = parts.get(5).unwrap_or(&"unknown");
            let secret = secret_part
                .split(':')
                .next()
                .unwrap_or("unknown")
                .to_string();

            info!("  ENABLE SECRET: project={}, secret={}", project, secret);
            info!(
                "  📍 Request path: POST /v1/projects/{}/secrets/{}:enable",
                project, secret
            );

            if app_state.secrets.enable_secret(&project, &secret).await {
                let response = SecretResponse {
                    name: PathBuilder::new()
                        .gcp_operation(GcpOperation::GetSecret)
                        .project(&project)
                        .secret(&secret)
                        .build_response_name()
                        .unwrap_or_else(|_| format!("projects/{}/secrets/{}", project, secret)),
                    payload: None,
                    replication: None,
                    create_time: None,
                    labels: None,
                };
                return Json(response).into_response();
            } else {
                return gcp_error_response(
                    StatusCode::NOT_FOUND,
                    format!(
                        "Secret not found: {}",
                        PathBuilder::new()
                            .gcp_operation(GcpOperation::GetSecret)
                            .project(&project)
                            .secret(&secret)
                            .build_response_name()
                            .unwrap_or_else(|_| format!("projects/{}/secrets/{}", project, secret))
                    ),
                    Some("NOT_FOUND"),
                );
            }
        }
    }

    // Handle GET request to list versions
    if method == Method::GET && path.ends_with("/versions") && !path.contains(":") {
        // Parse path: /v1/projects/{project}/secrets/{secret}/versions
        let parts: Vec<&str> = path.split('/').collect();
        let project = parts.get(3).unwrap_or(&"unknown").to_string();
        let secret = parts.get(5).unwrap_or(&"unknown").to_string();

        return list_secret_versions(State(app_state.clone()), Path((project, secret))).await;
    }

    // Not a colon route, return 404
    warn!("  ⚠️  Unmatched route: {} {}", method, path);
    gcp_error_response(
        StatusCode::NOT_FOUND,
        format!("Route not found: {} {}", method, path),
        Some("NOT_FOUND"),
    )
}

/// GET secret value (access specific version)
/// Path: /v1/projects/{project}/secrets/{secret}/versions/{version}:access
async fn get_secret_version_access(
    State(app_state): State<GcpAppState>,
    Path((project, secret, version_id)): Path<(String, String, String)>,
) -> Response {
    info!(
        "  GET secret version (access): project={}, secret={}, version={}",
        project, secret, version_id
    );

    // Try to retrieve specific version from in-memory store
    if let Some(version) = app_state
        .secrets
        .get_version(&project, &secret, &version_id)
        .await
    {
        // Check if version is enabled
        if !version.enabled {
            warn!(
                "  Version {} is disabled: projects/{}/secrets/{}/versions/{}",
                version_id, project, secret, version_id
            );
            return gcp_error_response(
                StatusCode::NOT_FOUND,
                format!(
                    "Version not found or disabled: projects/{}/secrets/{}/versions/{}",
                    project, secret, version_id
                ),
                Some("NOT_FOUND"),
            );
        }

        info!(
            "  Found secret version {} in store: projects/{}/secrets/{}/versions/{}",
            version_id, project, secret, version_id
        );

        // Extract the payload from version data
        if let Some(payload_obj) = version.data.get("payload") {
            if let Some(data) = payload_obj.get("data").and_then(|v| v.as_str()) {
                // Convert Unix timestamp to RFC3339 format (GCP API format)
                let create_time = format_timestamp_rfc3339(version.created_at);

                let response = SecretResponse {
                    name: PathBuilder::new()
                        .gcp_operation(GcpOperation::GetVersion)
                        .project(&project)
                        .secret(&secret)
                        .version(&version_id)
                        .build_response_name()
                        .unwrap_or_else(|_| {
                            format!(
                                "projects/{}/secrets/{}/versions/{}",
                                project, secret, version_id
                            )
                        }),
                    payload: Some(SecretPayload {
                        data: data.to_string(),
                    }),
                    replication: None,
                    create_time: Some(create_time),
                    labels: None,
                };
                return Json(response).into_response();
            }
        }
    }

    // Version not found, return 404
    warn!(
        "  Version not found in store: projects/{}/secrets/{}/versions/{}",
        project, secret, version_id
    );
    gcp_error_response(
        StatusCode::NOT_FOUND,
        format!(
            "Version not found: projects/{}/secrets/{}/versions/{}",
            project, secret, version_id
        ),
        Some("NOT_FOUND"),
    )
}

/// GET list of secret versions
/// Path: /v1/projects/{project}/secrets/{secret}/versions
async fn list_secret_versions(
    State(app_state): State<GcpAppState>,
    Path((project, secret)): Path<(String, String)>,
) -> Response {
    info!(
        "  GET secret versions list: project={}, secret={}",
        project, secret
    );

    // Check if secret exists
    if !app_state.secrets.exists(&project, &secret).await {
        warn!(
            "  Secret not found: projects/{}/secrets/{}",
            project, secret
        );
        return gcp_error_response(
            StatusCode::NOT_FOUND,
            format!(
                "Secret not found: {}",
                PathBuilder::new()
                    .gcp_operation(GcpOperation::GetSecret)
                    .project(&project)
                    .secret(&secret)
                    .build_response_name()
                    .unwrap_or_else(|_| format!("projects/{}/secrets/{}", project, secret))
            ),
            Some("NOT_FOUND"),
        );
    }

    // Get all versions
    if let Some(versions) = app_state.secrets.list_versions(&project, &secret).await {
        let version_list: Vec<serde_json::Value> = versions
            .iter()
            .map(|v| {
                json!({
                    "name": PathBuilder::new()
                        .gcp_operation(GcpOperation::GetVersion)
                        .project(&project)
                        .secret(&secret)
                        .version(&v.version_id)
                        .build_response_name()
                        .unwrap_or_else(|_| format!("projects/{}/secrets/{}/versions/{}", project, secret, v.version_id))
                        .strip_prefix("/v1/")
                        .unwrap_or(&format!("projects/{}/secrets/{}/versions/{}", project, secret, v.version_id))
                        .to_string(),
                    "createTime": format_timestamp_rfc3339(v.created_at),
                    "state": if v.enabled { "ENABLED" } else { "DISABLED" }
                })
            })
            .collect();

        Json(json!({
            "versions": version_list
        }))
        .into_response()
    } else {
        // No versions found, return empty list
        Json(json!({
            "versions": []
        }))
        .into_response()
    }
}

/// CREATE secret
async fn create_secret(
    State(app_state): State<GcpAppState>,
    Path(project): Path<String>,
    Json(body): Json<CreateSecretRequest>,
) -> Response {
    info!(
        provider = "gcp",
        project = project,
        secret_id = body.secret_id,
        operation = "create_secret",
        "CREATE secret: project={}, secret_id={}",
        project,
        body.secret_id
    );

    // Validate that labels are provided with environment and location
    let labels = body.labels.as_ref();
    let environment = labels
        .and_then(|l| l.get("environment"))
        .map(|s| s.as_str());
    let location = labels.and_then(|l| l.get("location")).map(|s| s.as_str());

    if environment.is_none() || environment.unwrap_or("").is_empty() {
        warn!(
            provider = "gcp",
            project = project,
            secret_id = body.secret_id,
            operation = "create_secret",
            "Validation failed: missing or empty environment label"
        );
        return gcp_error_response(
            StatusCode::BAD_REQUEST,
            "Missing required label: 'environment'. Secrets must include labels with 'environment' and 'location' keys.".to_string(),
            Some("INVALID_ARGUMENT"),
        )
        .into_response();
    }

    if location.is_none() || location.unwrap_or("").is_empty() {
        warn!(
            provider = "gcp",
            project = project,
            secret_id = body.secret_id,
            operation = "create_secret",
            "Validation failed: missing or empty location label"
        );
        return gcp_error_response(
            StatusCode::BAD_REQUEST,
            "Missing required label: 'location'. Secrets must include labels with 'environment' and 'location' keys.".to_string(),
            Some("INVALID_ARGUMENT"),
        )
        .into_response();
    }

    // Store the secret metadata (replication config and labels)
    // The secret will be created when the first version is added
    let metadata = json!({
        "replication": body.replication,
        "labels": labels
    });

    // Extract location from replication config if available
    // GCP automatic replication doesn't specify a location (replicated to all regions)
    // For user-managed replication, location would be in replicas, but we only support automatic
    // For automatic replication, location should be NULL (not "automatic")
    // We'll store NULL in the database for automatic replication
    let location: Option<String> = if body.replication.automatic.is_some() {
        None // Automatic replication = no specific location
    } else {
        Some("unknown".to_string())
    };

    info!(
        provider = "gcp",
        project = project,
        secret_id = body.secret_id,
        operation = "create_secret",
        location = location.as_ref().map(|s| s.as_str()).unwrap_or("NULL"),
        "Creating secret with metadata: project={}, secret={}, location={:?}",
        project,
        body.secret_id,
        location
    );

    if let Err(e) = app_state
        .secrets
        .update_metadata(&project, &body.secret_id, metadata)
        .await
    {
        warn!(
            provider = "gcp",
            project = project,
            secret_id = body.secret_id,
            operation = "create_secret",
            error = %e,
            "Failed to update metadata: {}",
            e
        );
        return gcp_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to update metadata: {}", e),
            Some("INTERNAL"),
        )
        .into_response();
    }

    let response = SecretResponse {
        name: PathBuilder::new()
            .gcp_operation(GcpOperation::CreateSecret)
            .project(&project)
            .secret(&body.secret_id)
            .build_response_name()
            .unwrap_or_else(|_| format!("projects/{}/secrets/{}", project, body.secret_id)),
        payload: None,
        replication: Some(body.replication),
        create_time: None, // Secret metadata doesn't include version timestamps
        labels: None,
    };

    info!("  Created mock secret and stored: {}", body.secret_id);
    Json(response).into_response()
}

/// GET secret metadata
/// Path: /v1/projects/{project}/secrets/{secret}
async fn get_secret_metadata(
    State(app_state): State<GcpAppState>,
    Path((project, secret)): Path<(String, String)>,
) -> Response {
    info!(
        "  GET secret metadata: project={}, secret={}",
        project, secret
    );
    info!(
        "  📍 Request path: GET /v1/projects/{}/secrets/{}",
        project, secret
    );

    // Try to retrieve metadata from in-memory store
    if let Some(metadata) = app_state.secrets.get_metadata(&project, &secret).await {
        info!(
            "  Found secret metadata in store: projects/{}/secrets/{}",
            project, secret
        );

        // Extract replication from metadata
        let replication = metadata
            .get("replication")
            .and_then(|r| serde_json::from_value(r.clone()).ok())
            .unwrap_or_else(|| Replication {
                automatic: Some(json!({})),
            });

        let response = SecretResponse {
            name: PathBuilder::new()
                .gcp_operation(GcpOperation::GetSecret)
                .project(&project)
                .secret(&secret)
                .build_response_name()
                .unwrap_or_else(|_| format!("projects/{}/secrets/{}", project, secret)),
            payload: None,
            replication: Some(replication),
            create_time: None, // Secret metadata doesn't include version timestamps
            labels: None,
        };

        return Json(response).into_response();
    }

    // Secret not found in store, return 404
    warn!(
        "  Secret not found in store: projects/{}/secrets/{}",
        project, secret
    );
    gcp_error_response(
        StatusCode::NOT_FOUND,
        format!(
            "Secret not found: {}",
            PathBuilder::new()
                .gcp_operation(GcpOperation::GetSecret)
                .project(&project)
                .secret(&secret)
                .build_response_name()
                .unwrap_or_else(|_| format!("projects/{}/secrets/{}", project, secret))
        ),
        Some("NOT_FOUND"),
    )
}

/// DELETE secret
/// Path: /v1/projects/{project}/secrets/{secret}
async fn delete_secret(
    State(app_state): State<GcpAppState>,
    Path((project, secret)): Path<(String, String)>,
) -> StatusCode {
    info!("  DELETE secret: project={}, secret={}", project, secret);

    if app_state.secrets.delete_secret(&project, &secret).await {
        info!("  Deleted secret from store: {}", secret);
        StatusCode::OK
    } else {
        warn!(
            "  Secret not found in store: projects/{}/secrets/{}",
            project, secret
        );
        StatusCode::NOT_FOUND
    }
}

/// Query parameters for filtering secrets
#[derive(Debug, serde::Deserialize)]
pub struct ListSecretsQuery {
    /// Filter by environment (e.g., "dev", "prod", "pact")
    pub environment: Option<String>,
    /// Filter by location (e.g., "us-central1", "global")
    pub location: Option<String>,
}

/// GET list of secrets
/// Path: /v1/projects/{project}/secrets
/// Query parameters: ?environment=dev&location=us-central1
async fn list_secrets(
    State(app_state): State<GcpAppState>,
    Path(project): Path<String>,
    Query(params): Query<ListSecretsQuery>,
) -> Response {
    info!(
        "  GET secrets list: project={}, environment={:?}, location={:?}",
        project, params.environment, params.location
    );

    // Get secrets for this project with database-level filtering
    // This queries the database directly with WHERE clauses instead of loading all secrets
    let secret_names = app_state
        .secrets
        .list_secrets_filtered(
            &project,
            params.environment.as_deref(),
            params.location.as_deref(),
        )
        .await;

    // Process filtered secrets concurrently using futures
    // Note: Filtering is already done at database level, so we just need to build responses
    let secret_futures: Vec<_> = secret_names
        .iter()
        .map(|secret_name| {
            let project = project.clone();
            let secret_name = secret_name.clone();
            let secrets = app_state.secrets.clone();
            async move {
                // Get metadata for each secret
                let metadata = match secrets.get_metadata(&project, &secret_name).await {
                    Some(m) => m,
                    None => return None, // Skip secrets without metadata
                };

                // Extract replication from metadata
                let replication = metadata
                    .get("replication")
                    .and_then(|r| serde_json::from_value(r.clone()).ok())
                    .unwrap_or_else(|| Replication {
                        automatic: Some(json!({})),
                    });

                // Get create time from first version (if exists)
                let create_time = secrets
                    .list_versions(&project, &secret_name)
                    .await
                    .and_then(|v| v.first().cloned())
                    .map(|v| format_timestamp_rfc3339(v.created_at));

                // Extract labels from metadata if present
                let labels = metadata.get("labels").cloned();

                Some(SecretResponse {
                    name: PathBuilder::new()
                        .gcp_operation(GcpOperation::GetSecret)
                        .project(&project)
                        .secret(&secret_name)
                        .build_response_name()
                        .unwrap_or_else(|_| {
                            format!("projects/{}/secrets/{}", project, secret_name)
                        }),
                    payload: None,
                    replication: Some(replication),
                    create_time,
                    labels,
                })
            }
        })
        .collect();

    // Await all futures concurrently
    let secret_results: Vec<_> = future::join_all(secret_futures).await;

    // Filter out None values and collect successful results
    let secret_list: Vec<SecretResponse> = secret_results
        .into_iter()
        .filter_map(|result| result)
        .collect();

    Json(ListSecretsResponse {
        secrets: secret_list,
        total_size: None, // GCP API doesn't always include this
    })
    .into_response()
}

/// GET list of all projects that have secrets
/// Custom endpoint: /v1/projects
/// Returns a list of project IDs that have secrets stored
async fn list_projects(State(app_state): State<GcpAppState>) -> Response {
    info!("  GET projects list");

    // Get all projects that have secrets
    let projects = app_state.secrets.list_all_projects().await;

    Json(json!({
        "projects": projects
    }))
    .into_response()
}

/// GET unique environments for secrets
/// Custom endpoint: /v1/projects/{project}/secrets/environments?location={location}
/// CRITICAL: Location is required - different locations can have different environments
async fn list_secret_environments(
    State(app_state): State<GcpAppState>,
    Path(project): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let location = params.get("location").map(|s| s.as_str());
    info!(
        "  GET secret environments: project={}, location={:?}",
        project, location
    );

    let environments = app_state
        .secrets
        .list_environments(&project, location, "secrets")
        .await;

    Json(json!({
        "environments": environments
    }))
    .into_response()
}

/// GET unique locations for secrets
/// Custom endpoint: /v1/projects/{project}/secrets/locations
async fn list_secret_locations(
    State(app_state): State<GcpAppState>,
    Path(project): Path<String>,
) -> Response {
    info!("  GET secret locations: project={}", project);

    let locations = app_state.secrets.list_locations(&project, "secrets").await;

    Json(json!({
        "locations": locations
    }))
    .into_response()
}

/// GET unique environments for parameters
/// Custom endpoint: /v1/projects/{project}/locations/{location}/parameters/environments
async fn list_parameter_environments(
    State(app_state): State<GcpAppState>,
    Path((project, location)): Path<(String, String)>,
) -> Response {
    info!(
        "  GET parameter environments: project={}, location={}",
        project, location
    );

    let environments = app_state
        .parameters
        .list_environments(&project, &location)
        .await;

    Json(json!({
        "environments": environments
    }))
    .into_response()
}

/// GET unique locations for parameters
/// Custom endpoint: /v1/projects/{project}/parameters/locations
async fn list_parameter_locations(
    State(app_state): State<GcpAppState>,
    Path(project): Path<String>,
) -> Response {
    info!("  GET parameter locations: project={}", project);

    let locations = app_state.parameters.list_locations(&project).await;

    Json(json!({
        "locations": locations
    }))
    .into_response()
}

/// PATCH secret (update metadata)
/// Path: /v1/projects/{project}/secrets/{secret}
async fn patch_secret(
    State(app_state): State<GcpAppState>,
    Path((project, secret)): Path<(String, String)>,
    Json(body): Json<UpdateSecretRequest>,
) -> Response {
    info!("  PATCH secret: project={}, secret={}", project, secret);

    // Check if secret exists
    if !app_state.secrets.exists(&project, &secret).await {
        warn!(
            "  Secret not found: projects/{}/secrets/{}",
            project, secret
        );
        return gcp_error_response(
            StatusCode::NOT_FOUND,
            format!(
                "Secret not found: {}",
                PathBuilder::new()
                    .gcp_operation(GcpOperation::GetSecret)
                    .project(&project)
                    .secret(&secret)
                    .build_response_name()
                    .unwrap_or_else(|_| format!("projects/{}/secrets/{}", project, secret))
            ),
            Some("NOT_FOUND"),
        );
    }

    // Get existing metadata
    let existing_metadata = app_state
        .secrets
        .get_metadata(&project, &secret)
        .await
        .unwrap_or_else(|| json!({}));

    // Parse update mask to determine which fields to update
    let update_mask: Vec<&str> = body.update_mask.split(',').map(|s| s.trim()).collect();

    // Build updated metadata
    let mut updated_metadata = existing_metadata.clone();

    // Update labels if in mask
    if update_mask.contains(&"labels") {
        if let Some(labels) = body.secret.labels {
            updated_metadata["labels"] = labels;
        }
    }

    // Update replication if in mask
    if update_mask.contains(&"replication") {
        if let Some(replication) = body.secret.replication {
            updated_metadata["replication"] =
                serde_json::to_value(&replication).unwrap_or(json!({}));
        }
    }

    // Save updated metadata
    if let Err(e) = app_state
        .secrets
        .update_metadata(&project, &secret, updated_metadata.clone())
        .await
    {
        warn!("Failed to update metadata: {}", e);
        return gcp_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to update metadata: {}", e),
            Some("INTERNAL"),
        )
        .into_response();
    }

    // Get replication for response
    let replication = updated_metadata
        .get("replication")
        .and_then(|r| serde_json::from_value(r.clone()).ok())
        .unwrap_or_else(|| Replication {
            automatic: Some(json!({})),
        });

    // Get labels for response
    let labels = updated_metadata.get("labels").cloned();

    // Get create time from first version (if exists)
    let versions = app_state.secrets.list_versions(&project, &secret).await;
    let create_time = versions
        .and_then(|v| v.first().cloned())
        .map(|v| format_timestamp_rfc3339(v.created_at));

    let response = SecretResponse {
        name: PathBuilder::new()
            .gcp_operation(GcpOperation::GetSecret)
            .project(&project)
            .secret(&secret)
            .build_response_name()
            .unwrap_or_else(|_| format!("projects/{}/secrets/{}", project, secret)),
        payload: None,
        replication: Some(replication),
        create_time,
        labels,
    };

    info!(
        "  Updated secret metadata: projects/{}/secrets/{}",
        project, secret
    );
    Json(response).into_response()
}

// ============================================================================
// GCP Parameter Manager API Handlers
// ============================================================================

/// CREATE parameter
/// Path: /v1/projects/{project}/locations/{location}/parameters
async fn create_parameter(
    State(app_state): State<GcpAppState>,
    Path((project, location)): Path<(String, String)>,
    Json(body): Json<CreateParameterRequest>,
) -> Json<ParameterResponse> {
    info!(
        "  CREATE parameter: project={}, location={}, parameter_id={}",
        project, location, body.parameter_id
    );

    // Store the parameter metadata (format, labels, etc.)
    let format_str = body
        .parameter
        .as_ref()
        .and_then(|p| p.format.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("PLAIN_TEXT");
    let metadata = json!({
        "format": format_str
    });
    app_state
        .parameters
        .update_metadata(&project, &location, &body.parameter_id, metadata)
        .await;

    let response = ParameterResponse {
        name: format!(
            "projects/{}/locations/{}/parameters/{}",
            project, location, body.parameter_id
        ),
        format: body.parameter.as_ref().and_then(|p| p.format.clone()),
        payload: None,
        create_time: None, // Parameter metadata doesn't include version timestamps
    };

    info!("  Created mock parameter and stored: {}", body.parameter_id);
    Json(response)
}

/// CREATE parameter version
/// Path: /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions
async fn create_parameter_version(
    State(app_state): State<GcpAppState>,
    Path((project, location, parameter)): Path<(String, String, String)>,
    Json(body): Json<CreateParameterVersionRequest>,
) -> Response {
    info!(
        "  CREATE parameter version: project={}, location={}, parameter={}",
        project, location, parameter
    );

    // Extract version ID from request or generate one
    let version_id = body.version_id.unwrap_or_else(|| {
        format!(
            "v{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        )
    });

    // Validate parameter size (GCP Parameter Manager limit: 1 MiB)
    let payload_size = body.parameter_version.payload.data.len();
    if payload_size > 1_048_576 {
        warn!(
            "  Parameter size validation failed: {} bytes exceeds 1 MiB limit",
            payload_size
        );
        return gcp_error_response(
            StatusCode::BAD_REQUEST,
            format!("Parameter size {} bytes exceeds 1 MiB limit", payload_size),
            Some("INVALID_ARGUMENT"),
        );
    }

    // Add a new version with the payload data
    let version_data = json!({
        "payload": {
            "data": body.parameter_version.payload.data
        }
    });

    let created_version_id = app_state
        .parameters
        .add_version(
            &project,
            &location,
            &parameter,
            version_data,
            version_id.clone(),
        )
        .await;

    // Get the version to include timestamp
    let version = app_state
        .parameters
        .get_version(&project, &location, &parameter, &created_version_id)
        .await;
    let create_time = version
        .as_ref()
        .map(|v| format_timestamp_rfc3339(v.created_at));

    let response = ParameterResponse {
        name: PathBuilder::new()
            .gcp_operation(GcpOperation::GetParameterVersion)
            .project(&project)
            .location(&location)
            .parameter(&parameter)
            .version(&created_version_id)
            .build_response_name()
            .unwrap_or_else(|_| {
                format!(
                    "projects/{}/locations/{}/parameters/{}/versions/{}",
                    project, location, parameter, created_version_id
                )
            })
            .strip_prefix("/v1/")
            .unwrap_or(&format!(
                "projects/{}/locations/{}/parameters/{}/versions/{}",
                project, location, parameter, created_version_id
            ))
            .to_string(),
        format: None,
        payload: None,
        create_time,
    };

    info!(
        "  Created version {} for parameter: {}",
        created_version_id, parameter
    );
    Json(response).into_response()
}

/// GET parameter versions list
/// Path: /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions
async fn list_parameter_versions(
    State(app_state): State<GcpAppState>,
    Path((project, location, parameter)): Path<(String, String, String)>,
) -> Response {
    info!(
        "  GET parameter versions list: project={}, location={}, parameter={}",
        project, location, parameter
    );

    // Check if parameter exists
    if !app_state
        .parameters
        .exists(&project, &location, &parameter)
        .await
    {
        warn!(
            "  Parameter not found: projects/{}/locations/{}/parameters/{}",
            project, location, parameter
        );
        return gcp_error_response(
            StatusCode::NOT_FOUND,
            format!(
                "Parameter not found: projects/{}/locations/{}/parameters/{}",
                project, location, parameter
            ),
            Some("NOT_FOUND"),
        );
    }

    // Get all versions
    if let Some(versions) = app_state
        .parameters
        .list_versions(&project, &location, &parameter)
        .await
    {
        let version_list: Vec<serde_json::Value> = versions
            .iter()
            .map(|v| {
                json!({
                    "name": PathBuilder::new()
                        .gcp_operation(GcpOperation::GetParameterVersion)
                        .project(&project)
                        .location(&location)
                        .parameter(&parameter)
                        .version(&v.version_id)
                        .build_response_name()
                        .unwrap_or_else(|_| format!("projects/{}/locations/{}/parameters/{}/versions/{}", project, location, parameter, v.version_id))
                        .strip_prefix("/v1/")
                        .unwrap_or(&format!("projects/{}/locations/{}/parameters/{}/versions/{}", project, location, parameter, v.version_id))
                        .to_string(),
                    "createTime": format_timestamp_rfc3339(v.created_at),
                    "state": if v.enabled { "ENABLED" } else { "DISABLED" }
                })
            })
            .collect();

        Json(json!({
            "versions": version_list
        }))
        .into_response()
    } else {
        // No versions found, return empty list
        Json(json!({
            "versions": []
        }))
        .into_response()
    }
}

/// GET parameter version (specific version)
/// Path: /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}
async fn get_parameter_version(
    State(app_state): State<GcpAppState>,
    Path((project, location, parameter, version)): Path<(String, String, String, String)>,
) -> Response {
    info!(
        "  GET parameter version: project={}, location={}, parameter={}, version={}",
        project, location, parameter, version
    );

    // Try to retrieve version from in-memory store
    if let Some(version_data) = app_state
        .parameters
        .get_version(&project, &location, &parameter, &version)
        .await
    {
        info!(
            "  Found parameter version {} in store: projects/{}/locations/{}/parameters/{}",
            version_data.version_id, project, location, parameter
        );

        // Extract the payload from version data
        if let Some(payload_obj) = version_data.data.get("payload") {
            if let Some(data) = payload_obj.get("data").and_then(|v| v.as_str()) {
                // Convert Unix timestamp to RFC3339 format (GCP API format)
                let create_time = format_timestamp_rfc3339(version_data.created_at);

                let response = ParameterResponse {
                    name: format!(
                        "projects/{}/locations/{}/parameters/{}/versions/{}",
                        project, location, parameter, version_data.version_id
                    ),
                    format: None,
                    payload: Some(ParameterPayload {
                        data: data.to_string(),
                    }),
                    create_time: Some(create_time),
                };
                return Json(response).into_response();
            }
        }
    }

    // Version not found, return 404
    warn!(
        "  Parameter version not found: projects/{}/locations/{}/parameters/{}/versions/{}",
        project, location, parameter, version
    );
    gcp_error_response(
        StatusCode::NOT_FOUND,
        format!(
            "Parameter version not found: projects/{}/locations/{}/parameters/{}/versions/{}",
            project, location, parameter, version
        ),
        Some("NOT_FOUND"),
    )
}

/// GET parameter metadata
/// Path: /v1/projects/{project}/locations/{location}/parameters/{parameter}
async fn get_parameter(
    State(app_state): State<GcpAppState>,
    Path((project, location, parameter)): Path<(String, String, String)>,
) -> Response {
    info!(
        "  GET parameter: project={}, location={}, parameter={}",
        project, location, parameter
    );

    if !app_state
        .parameters
        .exists(&project, &location, &parameter)
        .await
    {
        warn!(
            "  Parameter not found: projects/{}/locations/{}/parameters/{}",
            project, location, parameter
        );
        return gcp_error_response(
            StatusCode::NOT_FOUND,
            format!(
                "Parameter not found: projects/{}/locations/{}/parameters/{}",
                project, location, parameter
            ),
            Some("NOT_FOUND"),
        );
    }

    // Get parameter metadata
    if let Some(metadata) = app_state
        .parameters
        .get_metadata(&project, &location, &parameter)
        .await
    {
        let format = metadata
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("PLAIN_TEXT");

        let response = ParameterResponse {
            name: format!(
                "projects/{}/locations/{}/parameters/{}",
                project, location, parameter
            ),
            format: Some(format.to_string()),
            payload: None,
            create_time: None,
        };

        Json(response).into_response()
    } else {
        gcp_error_response(
            StatusCode::NOT_FOUND,
            format!(
                "Parameter not found: projects/{}/locations/{}/parameters/{}",
                project, location, parameter
            ),
            Some("NOT_FOUND"),
        )
    }
}

/// Query parameters for filtering parameters
#[derive(Debug, serde::Deserialize, Clone)]
pub struct ListParametersQuery {
    /// Filter by environment (e.g., "dev", "prod", "pact")
    pub environment: Option<String>,
    /// Filter by location (e.g., "us-central1", "global")
    pub location: Option<String>,
}

/// LIST parameters
/// Path: /v1/projects/{project}/locations/{location}/parameters
/// Query parameters: ?environment=dev&location=us-central1
async fn list_parameters(
    State(app_state): State<GcpAppState>,
    Path((project, location)): Path<(String, String)>,
    Query(params): Query<ListParametersQuery>,
) -> Response {
    info!(
        "  LIST parameters: project={}, location={}, environment={:?}",
        project, location, params.environment
    );

    // Get parameters for this project and location with database-level filtering
    // This queries the database directly with WHERE clauses instead of loading all parameters
    let parameter_names = app_state
        .parameters
        .list_parameters_filtered(&project, &location, params.environment.as_deref())
        .await;

    // Process filtered parameters concurrently using futures
    // Note: Filtering is already done at database level, so we just need to build responses
    let parameter_futures: Vec<_> = parameter_names
        .iter()
        .map(|parameter_name| {
            let project = project.clone();
            let location = location.clone();
            let parameter_name = parameter_name.clone();
            let parameters = app_state.parameters.clone();
            async move {
                // Get metadata for each parameter
                let metadata = match parameters
                    .get_metadata(&project, &location, &parameter_name)
                    .await
                {
                    Some(m) => m,
                    None => return None, // Skip parameters without metadata
                };

                // Get create time from first version (if exists)
                let create_time = parameters
                    .list_versions(&project, &location, &parameter_name)
                    .await
                    .and_then(|v| v.first().cloned())
                    .map(|v| format_timestamp_rfc3339(v.created_at));

                Some(ParameterResponse {
                    name: format!(
                        "projects/{}/locations/{}/parameters/{}",
                        project, location, parameter_name
                    ),
                    format: metadata
                        .get("format")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    payload: None,
                    create_time,
                })
            }
        })
        .collect();

    // Await all futures concurrently
    let parameter_results: Vec<_> = future::join_all(parameter_futures).await;

    // Filter out None values and collect successful results
    let parameter_list: Vec<ParameterResponse> = parameter_results
        .into_iter()
        .filter_map(|result| result)
        .collect();

    Json(json!({
        "parameters": parameter_list,
        "nextPageToken": serde_json::Value::Null
    }))
    .into_response()
}

/// PATCH parameter
/// Path: /v1/projects/{project}/locations/{location}/parameters/{parameter}
async fn update_parameter(
    State(app_state): State<GcpAppState>,
    Path((project, location, parameter)): Path<(String, String, String)>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    info!(
        "  PATCH parameter: project={}, location={}, parameter={}",
        project, location, parameter
    );

    if !app_state
        .parameters
        .exists(&project, &location, &parameter)
        .await
    {
        warn!(
            "  Parameter not found: projects/{}/locations/{}/parameters/{}",
            project, location, parameter
        );
        return gcp_error_response(
            StatusCode::NOT_FOUND,
            format!(
                "Parameter not found: projects/{}/locations/{}/parameters/{}",
                project, location, parameter
            ),
            Some("NOT_FOUND"),
        );
    }

    // Extract parameter spec from request
    if let Some(param_spec) = body.get("parameter").and_then(|p| p.as_object()) {
        let metadata = json!({
            "format": param_spec.get("format").and_then(|v| v.as_str()).unwrap_or("PLAIN_TEXT"),
            "labels": param_spec.get("labels")
        });
        app_state
            .parameters
            .update_metadata(&project, &location, &parameter, metadata)
            .await;

        let response = ParameterResponse {
            name: format!(
                "projects/{}/locations/{}/parameters/{}",
                project, location, parameter
            ),
            format: param_spec
                .get("format")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            payload: None,
            create_time: None,
        };

        Json(response).into_response()
    } else {
        gcp_error_response(
            StatusCode::BAD_REQUEST,
            "Invalid request body: missing 'parameter' field".to_string(),
            Some("INVALID_ARGUMENT"),
        )
    }
}

/// DELETE parameter
/// Path: /v1/projects/{project}/locations/{location}/parameters/{parameter}
async fn delete_parameter(
    State(app_state): State<GcpAppState>,
    Path((project, location, parameter)): Path<(String, String, String)>,
) -> StatusCode {
    info!(
        "  DELETE parameter: project={}, location={}, parameter={}",
        project, location, parameter
    );

    if app_state
        .parameters
        .delete_parameter(&project, &location, &parameter)
        .await
    {
        info!("  Deleted parameter from store: {}", parameter);
        StatusCode::OK
    } else {
        warn!(
            "  Parameter not found in store: projects/{}/locations/{}/parameters/{}",
            project, location, parameter
        );
        StatusCode::NOT_FOUND
    }
}

/// PATCH parameter version
/// Path: /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}
async fn update_parameter_version(
    State(app_state): State<GcpAppState>,
    Path((project, location, parameter, version)): Path<(String, String, String, String)>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    info!(
        "  PATCH parameter version: project={}, location={}, parameter={}, version={}",
        project, location, parameter, version
    );

    // Extract state from request
    if let Some(param_version) = body.get("parameterVersion").and_then(|pv| pv.as_object()) {
        if let Some(state) = param_version.get("state").and_then(|s| s.as_str()) {
            let enabled = state == "ENABLED";

            // Update version state in store
            if let Some(_version_data) = app_state
                .parameters
                .get_version(&project, &location, &parameter, &version)
                .await
            {
                // Disable/enable the version
                if enabled {
                    app_state
                        .parameters
                        .enable_version(&project, &location, &parameter, &version)
                        .await;
                } else {
                    app_state
                        .parameters
                        .disable_version(&project, &location, &parameter, &version)
                        .await;
                }

                // Get updated version
                if let Some(updated_version) = app_state
                    .parameters
                    .get_version(&project, &location, &parameter, &version)
                    .await
                {
                    let response = ParameterResponse {
                        name: format!(
                            "projects/{}/locations/{}/parameters/{}/versions/{}",
                            project, location, parameter, version
                        ),
                        format: None,
                        payload: Some(ParameterPayload {
                            data: updated_version
                                .data
                                .get("payload")
                                .and_then(|p| p.get("data"))
                                .and_then(|d| d.as_str())
                                .unwrap_or("")
                                .to_string(),
                        }),
                        create_time: Some(format_timestamp_rfc3339(updated_version.created_at)),
                    };

                    return Json(response).into_response();
                }
            }
        }
    }

    gcp_error_response(
        StatusCode::NOT_FOUND,
        format!(
            "Parameter version not found: projects/{}/locations/{}/parameters/{}/versions/{}",
            project, location, parameter, version
        ),
        Some("NOT_FOUND"),
    )
}

/// DELETE parameter version
/// Path: /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}
async fn delete_parameter_version(
    State(app_state): State<GcpAppState>,
    Path((project, location, parameter, version)): Path<(String, String, String, String)>,
) -> StatusCode {
    info!(
        "  DELETE parameter version: project={}, location={}, parameter={}, version={}",
        project, location, parameter, version
    );

    // Delete version from store using the parameter store method
    let deleted = app_state
        .parameters
        .delete_version(&project, &location, &parameter, &version)
        .await;

    if deleted {
        info!("  Deleted parameter version {} from store", version);
        StatusCode::OK
    } else {
        warn!(
            "  Parameter version not found: projects/{}/locations/{}/parameters/{}/versions/{}",
            project, location, parameter, version
        );
        StatusCode::NOT_FOUND
    }
}

/// RENDER parameter version
/// Path: /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}:render
async fn render_parameter_version(
    State(app_state): State<GcpAppState>,
    Path((project, location, parameter, version)): Path<(String, String, String, String)>,
) -> Response {
    info!(
        "  RENDER parameter version: project={}, location={}, parameter={}, version={}",
        project, location, parameter, version
    );

    if let Some(version_data) = app_state
        .parameters
        .get_version(&project, &location, &parameter, &version)
        .await
    {
        // Extract and decode the payload
        if let Some(payload_obj) = version_data.data.get("payload") {
            if let Some(data) = payload_obj.get("data").and_then(|v| v.as_str()) {
                // Decode base64
                use base64::{engine::general_purpose, Engine as _};
                if let Ok(decoded) = general_purpose::STANDARD.decode(data) {
                    if let Ok(rendered_value) = String::from_utf8(decoded) {
                        return Json(json!({
                            "renderedValue": rendered_value
                        }))
                        .into_response();
                    }
                }
            }
        }
    }

    gcp_error_response(
        StatusCode::NOT_FOUND,
        format!(
            "Parameter version not found: projects/{}/locations/{}/parameters/{}/versions/{}",
            project, location, parameter, version
        ),
        Some("NOT_FOUND"),
    )
}

// ============================================================================
// GCP Location API Handlers
// ============================================================================

/// GET location
/// Path: /v1/projects/{project}/locations/{location}
async fn get_location(
    State(_app_state): State<GcpAppState>,
    Path((project, location)): Path<(String, String)>,
) -> Response {
    info!("  GET location: project={}, location={}", project, location);

    // Return location information
    // Common locations: "global", "us-central1", "us-east1", "europe-west1", etc.
    let response = json!({
        "name": format!("projects/{}/locations/{}", project, location),
        "locationId": location,
        "displayName": format!("{} ({})", location, if location == "global" { "Global" } else { "Regional" })
    });

    Json(response).into_response()
}

/// LIST locations
/// Path: /v1/projects/{project}/locations
async fn list_locations(
    State(_app_state): State<GcpAppState>,
    Path(project): Path<String>,
) -> Response {
    info!("  LIST locations: project={}", project);

    // Return list of common GCP locations
    // In a real implementation, this would query GCP for available locations
    let locations = vec![
        json!({
            "name": format!("projects/{}/locations/global", project),
            "locationId": "global",
            "displayName": "Global"
        }),
        json!({
            "name": format!("projects/{}/locations/us-central1", project),
            "locationId": "us-central1",
            "displayName": "Iowa (Regional)"
        }),
        json!({
            "name": format!("projects/{}/locations/us-east1", project),
            "locationId": "us-east1",
            "displayName": "South Carolina (Regional)"
        }),
        json!({
            "name": format!("projects/{}/locations/europe-west1", project),
            "locationId": "europe-west1",
            "displayName": "Belgium (Regional)"
        }),
    ];

    Json(json!({
        "locations": locations,
        "nextPageToken": serde_json::Value::Null
    }))
    .into_response()
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
    let provider = env::var("PACT_PROVIDER").unwrap_or_else(|_| "GCP-Secret-Manager".to_string());
    let consumer =
        env::var("PACT_CONSUMER").unwrap_or_else(|_| "Secret-Manager-Controller".to_string());
    let port = env::var("PORT")
        .unwrap_or_else(|_| "1234".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid u16");

    info!("Starting GCP Secret Manager Mock Server...");
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
    let app_state = GcpAppState {
        contracts: contracts_state.contracts,
        secrets: GcpSecretStore::new().await,
        parameters: GcpParameterStore::new(),
    };

    // Build router with explicit routes for all GCP Secret Manager and Parameter Manager API endpoints
    let app = Router::new()
        // Health check endpoints
        .route("/", get(health_check))
        .route("/health", get(health_check))
        // Custom endpoint to list all projects (for UI)
        .route("/v1/projects", get(list_projects))
        // Custom endpoints for filter values (must come before parameterized routes to avoid conflicts)
        // These use explicit string paths since they're custom endpoints not in the GCP API spec
        // Note: Using {project} format to match existing route patterns
        .route(
            "/v1/projects/{project}/secrets/environments",
            get(list_secret_environments),
        )
        .route(
            "/v1/projects/{project}/secrets/locations",
            get(list_secret_locations),
        )
        .route(
            "/v1/projects/{project}/locations/{location}/parameters/environments",
            get(list_parameter_environments),
        )
        .route(
            "/v1/projects/{project}/parameters/locations",
            get(list_parameter_locations),
        )
        // GCP Secret Manager API endpoints
        // Using route constants from paths::gcp::routes for type safety
        // POST /v1/projects/{project}/secrets - Create a new secret
        // GET /v1/projects/{project}/secrets - List secrets
        .route(
            routes::secret_manager::CREATE_SECRET,
            post(create_secret).get(list_secrets),
        )
        // GET /v1/projects/{project}/secrets/{secret}/versions/latest:access - Get secret value (access latest)
        // Note: The colon in the path requires using fallback handler
        // This route is handled by the fallback handler which parses the path manually
        // GET /v1/projects/{project}/secrets/{secret} - Get secret metadata
        // PATCH /v1/projects/{project}/secrets/{secret} - Update secret metadata
        // DELETE /v1/projects/{project}/secrets/{secret} - Delete secret
        // POST /v1/projects/{project}/secrets/{secret}:addVersion - Add version (must come before SECRET route)
        // POST /v1/projects/{project}/secrets/{secret}:enable - Enable secret (must come before SECRET route)
        // POST /v1/projects/{project}/secrets/{secret}:disable - Disable secret (must come before SECRET route)
        // These routes with colons must be handled by the fallback handler, but we need to ensure
        // they're not matched by the SECRET route first. The fallback will catch them.
        // GET /v1/projects/{project}/secrets/{secret} - Get secret metadata
        // PATCH /v1/projects/{project}/secrets/{secret} - Update secret metadata
        // DELETE /v1/projects/{project}/secrets/{secret} - Delete secret
        // Note: POST is allowed here so colon routes (:addVersion, :enable, :disable)
        // don't get 405 Method Not Allowed. We check for colons and delegate to colon handler.
        .route(
            routes::secret_manager::SECRET,
            delete(delete_secret)
                .get(get_secret_metadata)
                .patch(patch_secret)
                .post(
                    |State(app_state): State<GcpAppState>, request: Request| async move {
                        // Check if this is a colon route (addVersion, enable, disable)
                        // Extract path before moving request
                        let path = request.uri().path().to_string();
                        if path.contains(':') {
                            // Delegate to colon route handler
                            handle_colon_routes(State(app_state), request).await
                        } else {
                            // Not a colon route, return 405 for POST on non-colon paths
                            gcp_error_response(
                                StatusCode::METHOD_NOT_ALLOWED,
                                format!("POST method not allowed for path: {}", path),
                                Some("METHOD_NOT_ALLOWED"),
                            )
                        }
                    },
                ),
        )
        // GCP Parameter Manager API endpoints
        // Using route constants from paths::gcp::routes for type safety
        // POST /v1/projects/{project}/locations/{location}/parameters - Create a new parameter
        // GET /v1/projects/{project}/locations/{location}/parameters - List parameters
        .route(
            routes::parameter_manager::CREATE_PARAMETER,
            post(create_parameter).get(list_parameters),
        )
        // GET /v1/projects/{project}/locations/{location}/parameters/{parameter} - Get parameter
        // PATCH /v1/projects/{project}/locations/{location}/parameters/{parameter} - Update parameter
        // DELETE /v1/projects/{project}/locations/{location}/parameters/{parameter} - Delete parameter
        .route(
            routes::parameter_manager::PARAMETER,
            get(get_parameter)
                .patch(update_parameter)
                .delete(delete_parameter),
        )
        // GCP Location API endpoints
        // Using route constants from paths::gcp::routes for type safety
        // GET /v1/projects/{project}/locations/{location} - Get location
        .route(routes::locations::LOCATION, get(get_location))
        // GET /v1/projects/{project}/locations - List locations
        .route(routes::locations::LIST_LOCATIONS, get(list_locations))
        // POST /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions - Create parameter version
        // GET /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions - List parameter versions
        .route(
            routes::parameter_manager::PARAMETER_VERSIONS,
            post(create_parameter_version).get(list_parameter_versions),
        )
        // GET /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version} - Get specific parameter version
        // PATCH /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version} - Update parameter version
        // DELETE /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version} - Delete parameter version
        .route(
            routes::parameter_manager::PARAMETER_VERSION,
            get(get_parameter_version)
                .patch(update_parameter_version)
                .delete(delete_parameter_version),
        )
        // POST /v1/projects/{project}/secrets/{secret}:addVersion - Add a new version (Secret Manager)
        // POST /v1/projects/{project}/parameters/{parameter}:addVersion - Add a new version (Parameter Manager)
        .fallback(handle_colon_routes)
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
    info!("✅ GCP Mock server ready at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
