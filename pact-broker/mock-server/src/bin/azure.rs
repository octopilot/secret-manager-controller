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
    extract::Path,
    http::StatusCode,
    response::Json,
    routing::{get, put},
    Router,
};
use pact_mock_server::{health_check, load_contracts_from_broker, logging_middleware, AppState};
use serde_json::json;
use std::env;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{info, warn, Level};

#[derive(serde::Deserialize)]
struct SetSecretRequest {
    value: String,
}

/// GET secret
/// Path: /secrets/{name}/ (with trailing slash)
/// Query: api-version=2025-07-01
async fn get_secret(Path(name): Path<String>) -> Json<serde_json::Value> {
    info!("  GET secret: name={}", name);

    Json(json!({
        "value": format!("mock-value-for-{}", name),
        "id": format!("https://test-vault.vault.azure.net/secrets/{}/abc123", name),
        "attributes": {
            "enabled": true,
            "created": 1704067200,
            "updated": 1704067200,
            "recoveryLevel": "Recoverable+Purgeable"
        }
    }))
}

/// PUT secret (set/update)
/// Path: /secrets/{name} (without trailing slash)
/// Query: api-version=2025-07-01
async fn set_secret(
    Path(name): Path<String>,
    Json(body): Json<SetSecretRequest>,
) -> Json<serde_json::Value> {
    info!("  PUT secret: name={}, value_length={}", name, body.value.len());

    Json(json!({
        "value": body.value,
        "id": format!("https://test-vault.vault.azure.net/secrets/{}/abc123", name),
        "attributes": {
            "enabled": true,
            "created": 1704067200,
            "updated": 1704067200,
            "recoveryLevel": "Recoverable+Purgeable"
        }
    }))
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
        .unwrap_or_else(|_| "Azure-Key-Vault".to_string());
    let consumer = env::var("PACT_CONSUMER")
        .unwrap_or_else(|_| "Secret-Manager-Controller".to_string());
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

    let app_state = AppState::new(contracts);

    // Build router with Azure Key Vault API endpoints
    // Note: GET uses trailing slash, PUT does not
    let app = Router::new()
        // Health check endpoints
        .route("/", get(health_check))
        .route("/health", get(health_check))
        // Azure Key Vault Secrets API endpoints
        // GET /secrets/{name}/ - Get secret (with trailing slash)
        .route("/secrets/{name}/", get(get_secret))
        // PUT /secrets/{name} - Set secret (without trailing slash)
        .route("/secrets/{name}", put(set_secret))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(axum::middleware::from_fn(logging_middleware)),
        )
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Listening on port {}", port);
    info!("✅ Azure Mock server ready at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

