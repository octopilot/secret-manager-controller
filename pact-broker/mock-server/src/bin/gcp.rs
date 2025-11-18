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
    extract::Path,
    http::{Method, StatusCode, Uri},
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post},
    Router,
};
use base64::{engine::general_purpose, Engine as _};
use pact_mock_server::{health_check, load_contracts_from_broker, logging_middleware, AppState};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{info, warn, Level};

#[derive(Debug, Serialize, Deserialize)]
struct CreateSecretRequest {
    #[serde(rename = "secretId")]
    secret_id: String,
    replication: Replication,
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
}

/// GET secret value (access latest version)
/// Path: /v1/projects/{project}/secrets/{secret}/versions/latest:access
async fn get_secret_value_access(
    Path((project, secret)): Path<(String, String)>,
) -> Json<SecretResponse> {
    info!(
        "  GET secret value (access): project={}, secret={}",
        project, secret
    );

    // Return a mock secret value
    let test_value = format!("mock-value-for-{}", secret);
    let encoded = general_purpose::STANDARD.encode(test_value.as_bytes());

    let response = SecretResponse {
        name: format!("projects/{}/secrets/{}/versions/latest", project, secret),
        payload: Some(SecretPayload { data: encoded }),
        replication: None,
    };

    info!("  Returning mock secret value for {}", secret);
    Json(response)
}

/// Handler for routes with colons in the path (fallback)
/// Handles: POST /v1/projects/{project}/secrets/{secret}:addVersion
async fn handle_colon_routes(
    method: Method,
    uri: Uri,
    body: Option<axum::extract::Json<AddVersionRequest>>,
) -> Response {
    let path = uri.path();

    // Check if this is a POST request to a path ending with :addVersion
    if method == Method::POST && path.contains(":addVersion") {
        // Parse path: /v1/projects/{project}/secrets/{secret}:addVersion
        let parts: Vec<&str> = path.split('/').collect();
        let project = parts.get(3).unwrap_or(&"unknown").to_string();
        let secret_part = parts.get(5).unwrap_or(&"unknown");
        let secret = secret_part.split(':').next().unwrap_or("unknown").to_string();

        if let Some(Json(body)) = body {
            info!("  ADD VERSION: project={}, secret={}", project, secret);

            let response = SecretResponse {
                name: format!("projects/{}/secrets/{}/versions/1", project, secret),
                payload: Some(body.payload),
                replication: None,
            };

            info!("  Added version to mock secret: {}", secret);
            return Json(response).into_response();
        } else {
            return (StatusCode::BAD_REQUEST, Json(json!({"error": "Missing request body"})))
                .into_response();
        }
    }

    // Not a colon route, return 404
    warn!("  ⚠️  Unmatched route: {} {}", method, path);
    (StatusCode::NOT_FOUND, Json(json!({
        "error": "Not found",
        "path": path
    })))
    .into_response()
}

/// CREATE secret
async fn create_secret(
    Path(project): Path<String>,
    Json(body): Json<CreateSecretRequest>,
) -> Json<SecretResponse> {
    info!("  CREATE secret: project={}, secret_id={}", project, body.secret_id);

    let response = SecretResponse {
        name: format!("projects/{}/secrets/{}", project, body.secret_id),
        payload: None,
        replication: Some(body.replication),
    };

    info!("  Created mock secret: {}", body.secret_id);
    Json(response)
}

/// GET secret metadata
/// Path: /v1/projects/{project}/secrets/{secret}
async fn get_secret_metadata(
    Path((project, secret)): Path<(String, String)>,
) -> Json<SecretResponse> {
    info!("  GET secret metadata: project={}, secret={}", project, secret);

    let response = SecretResponse {
        name: format!("projects/{}/secrets/{}", project, secret),
        payload: None,
        replication: Some(Replication {
            automatic: Some(json!({})),
        }),
    };

    info!("  Returning mock secret metadata for {}", secret);
    Json(response)
}

/// DELETE secret
/// Path: /v1/projects/{project}/secrets/{secret}
async fn delete_secret(Path((project, secret)): Path<(String, String)>) -> StatusCode {
    info!("  DELETE secret: project={}, secret={}", project, secret);
    info!("  Deleted mock secret: {}", secret);
    StatusCode::OK
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    // Load configuration from environment
    let broker_url = env::var("PACT_BROKER_URL")
        .unwrap_or_else(|_| "http://pact-broker:9292".to_string());
    let username = env::var("PACT_BROKER_USERNAME").unwrap_or_else(|_| "pact".to_string());
    let password = env::var("PACT_BROKER_PASSWORD").unwrap_or_else(|_| "pact".to_string());
    let provider = env::var("PACT_PROVIDER")
        .unwrap_or_else(|_| "GCP-Secret-Manager".to_string());
    let consumer = env::var("PACT_CONSUMER")
        .unwrap_or_else(|_| "Secret-Manager-Controller".to_string());
    let port = env::var("PORT")
        .unwrap_or_else(|_| "1234".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid u16");

    info!("Starting GCP Secret Manager Mock Server...");
    info!("Broker URL: {}", broker_url);
    info!("Provider: {}, Consumer: {}", provider, consumer);

    // Load contracts from broker
    let contracts =
        load_contracts_from_broker(&broker_url, &username, &password, &provider, &consumer).await;
    if contracts.is_empty() {
        warn!("⚠️  No contracts loaded, using default mock responses");
    }

    let app_state = AppState::new(contracts);

    // Build router with explicit routes for all GCP Secret Manager API endpoints
    let app = Router::new()
        // Health check endpoints
        .route("/", get(health_check))
        .route("/health", get(health_check))
        // GCP Secret Manager API endpoints
        // POST /v1/projects/{project}/secrets - Create a new secret
        .route("/v1/projects/{project}/secrets", post(create_secret))
        // GET /v1/projects/{project}/secrets/{secret}/versions/latest:access - Get secret value (access latest)
        .route(
            "/v1/projects/{project}/secrets/{secret}/versions/latest",
            get(get_secret_value_access),
        )
        // DELETE /v1/projects/{project}/secrets/{secret} - Delete secret
        .route(
            "/v1/projects/{project}/secrets/{secret}",
            delete(delete_secret).get(get_secret_metadata),
        )
        // POST /v1/projects/{project}/secrets/{secret}:addVersion - Add a new version
        .fallback(handle_colon_routes)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(axum::middleware::from_fn(logging_middleware)),
        )
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Listening on port {}", port);
    info!("✅ GCP Mock server ready at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

