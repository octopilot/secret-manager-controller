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
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::post,
    Router,
};
use pact_mock_server::{health_check, load_contracts_from_broker, logging_middleware, AppState};
use serde_json::{json, Value};
use std::env;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{info, warn, Level};

/// Handler for all AWS Secrets Manager requests
/// All requests are POST to "/" with x-amz-target header
async fn handle_aws_request(request: Request) -> Response {
    let target = request
        .headers()
        .get("x-amz-target")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    info!("  AWS Request: x-amz-target={}", target);

    // For now, return a generic success response
    // In the future, we can parse the target and body to return specific responses
    match target {
        "secretsmanager.CreateSecret" => {
            info!("  CREATE secret");
            (
                StatusCode::OK,
                Json(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "VersionId": "test-version-id"
                })),
            )
                .into_response()
        }
        "secretsmanager.GetSecretValue" => {
            info!("  GET secret value");
            (
                StatusCode::OK,
                Json(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "SecretString": "mock-secret-value",
                    "VersionId": "current-version-id",
                    "VersionStages": ["AWSCURRENT"]
                })),
            )
                .into_response()
        }
        "secretsmanager.DescribeSecret" => {
            info!("  DESCRIBE secret");
            (
                StatusCode::OK,
                Json(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "VersionIdToStages": {
                        "current-version-id": ["AWSCURRENT"]
                    }
                })),
            )
                .into_response()
        }
        "secretsmanager.PutSecretValue" => {
            info!("  PUT secret value");
            (
                StatusCode::OK,
                Json(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "VersionId": "new-version-id",
                    "VersionStages": ["AWSCURRENT"]
                })),
            )
                .into_response()
        }
        "secretsmanager.DeleteSecret" => {
            info!("  DELETE secret");
            (
                StatusCode::OK,
                Json(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "DeletionDate": 1704067200.0
                })),
            )
                .into_response()
        }
        _ => {
            warn!("  ⚠️  Unknown x-amz-target: {}", target);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "__type": "InvalidRequestException",
                    "message": format!("Unknown target: {}", target)
                })),
            )
                .into_response()
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
    let broker_url = env::var("PACT_BROKER_URL")
        .unwrap_or_else(|_| "http://pact-broker:9292".to_string());
    let username = env::var("PACT_BROKER_USERNAME").unwrap_or_else(|_| "pact".to_string());
    let password = env::var("PACT_BROKER_PASSWORD").unwrap_or_else(|_| "pact".to_string());
    let provider = env::var("PACT_PROVIDER")
        .unwrap_or_else(|_| "AWS-Secrets-Manager".to_string());
    let consumer = env::var("PACT_CONSUMER")
        .unwrap_or_else(|_| "Secret-Manager-Controller".to_string());
    let port = env::var("PORT")
        .unwrap_or_else(|_| "1234".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid u16");

    info!("Starting AWS Secrets Manager Mock Server...");
    info!("Broker URL: {}", broker_url);
    info!("Provider: {}, Consumer: {}", provider, consumer);

    // Load contracts from broker
    let contracts =
        load_contracts_from_broker(&broker_url, &username, &password, &provider, &consumer).await;
    if contracts.is_empty() {
        warn!("⚠️  No contracts loaded, using default mock responses");
    }

    let app_state = AppState::new(contracts);

    // Build router - all AWS requests go to POST "/"
    let app = Router::new()
        // Health check endpoints
        .route("/", axum::routing::get(health_check))
        .route("/health", axum::routing::get(health_check))
        // AWS Secrets Manager API - all operations via POST "/" with x-amz-target header
        .route("/", post(handle_aws_request))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(axum::middleware::from_fn(logging_middleware)),
        )
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Listening on port {}", port);
    info!("✅ AWS Mock server ready at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

