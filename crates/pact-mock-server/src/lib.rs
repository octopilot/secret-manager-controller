//! Shared library for Pact Mock Servers
//!
//! Provides common functionality for GCP, AWS, and Azure mock servers:
//! - Contract loading from Pact broker
//! - Request logging middleware
//! - Health check endpoints
//! - App state management
//! - API path definitions (source of truth for GCP API paths)
//!
//! ## Quick Start
//!
//! ```rust
//! use pact_mock_server::prelude::*;
//! ```
//!
//! This brings commonly used types and functions into scope.

pub mod prelude;
pub mod secrets;

use axum::{
    extract::Request,
    http::{HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Application state shared across all mock servers
///
/// Note: Each provider binary should use its provider-specific store type
/// (GcpSecretStore, AwsSecretStore, AzureSecretStore) instead of this generic AppState.
/// This is kept for backward compatibility and shared functionality.
#[derive(Clone, Debug)]
pub struct AppState {
    pub contracts: Arc<RwLock<HashMap<String, Value>>>,
}

impl AppState {
    pub fn new(contracts: HashMap<String, Value>) -> Self {
        Self {
            contracts: Arc::new(RwLock::new(contracts)),
        }
    }
}

/// Request logging middleware
/// Logs all incoming requests with method, path, client IP, response status, and duration
pub async fn logging_middleware(request: Request, next: Next) -> axum::response::Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let start = std::time::Instant::now();

    // Log request
    info!(
        "→ {} {} [client: {}]",
        method,
        path,
        request
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
    );

    // Log request body for POST/PUT/PATCH
    if matches!(method, Method::POST | Method::PUT | Method::PATCH) {
        if let Some(content_type) = request.headers().get("content-type") {
            if content_type
                .to_str()
                .unwrap_or("")
                .contains("application/json")
            {
                // Note: Body is consumed by the handler, so we can't log it here
                // The handler will log it if needed
            }
        }
    }

    let response = next.run(request).await;
    let duration = start.elapsed();
    let status = response.status();

    info!(
        "← {} {} [{}] [{:.3}s]",
        method,
        path,
        status.as_u16(),
        duration.as_secs_f64()
    );

    response
}

/// Health check endpoint
/// Returns a simple JSON response indicating the service is healthy
pub async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "pact-mock-server"
    }))
}

/// Rate limiting middleware
/// Returns 429 Too Many Requests if request includes X-Rate-Limit header
///
/// This allows testing individual requests without affecting the entire server.
/// To trigger a 429 response, include the header: X-Rate-Limit: true
///
/// Optional header: X-Rate-Limit-Retry-After: <seconds> (default: 60)
/// This sets the Retry-After header value in the response.
pub async fn rate_limit_middleware(request: Request, next: Next) -> Response {
    // Check for X-Rate-Limit header
    if let Some(header_value) = request.headers().get("x-rate-limit") {
        if let Ok(header_str) = header_value.to_str() {
            if header_str.to_lowercase() == "true" {
                // Get optional retry-after value from header
                let retry_after = request
                    .headers()
                    .get("x-rate-limit-retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60);

                warn!(
                    "Rate limit header detected - returning 429 with retry-after: {}s",
                    retry_after
                );

                let mut response = (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({
                        "error": {
                            "code": 429,
                            "message": "Rate limit exceeded",
                            "retry_after": retry_after
                        }
                    })),
                )
                    .into_response();

                // Add Retry-After header
                if let Ok(header_value) = HeaderValue::from_str(&retry_after.to_string()) {
                    response.headers_mut().insert("retry-after", header_value);
                }

                return response;
            }
        }
    }

    next.run(request).await
}

/// Service unavailable middleware
/// Returns 503 Service Unavailable if request includes X-Service-Unavailable header
///
/// This allows testing individual requests without affecting the entire server.
/// To trigger a 503 response, include the header: X-Service-Unavailable: true
pub async fn service_unavailable_middleware(request: Request, next: Next) -> Response {
    // Skip health checks
    if request.uri().path() == "/health" || request.uri().path() == "/" {
        return next.run(request).await;
    }

    // Check for X-Service-Unavailable header
    if let Some(header_value) = request.headers().get("x-service-unavailable") {
        if let Ok(header_str) = header_value.to_str() {
            if header_str.to_lowercase() == "true" {
                warn!("Service unavailable header detected - returning 503");
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({
                        "error": {
                            "code": 503,
                            "message": "Service temporarily unavailable"
                        }
                    })),
                )
                    .into_response();
            }
        }
    }

    next.run(request).await
}

/// Authentication failure middleware
/// Returns 401 Unauthorized or 403 Forbidden if request includes authentication failure headers
///
/// This allows testing individual requests without affecting the entire server.
/// To trigger a 401 response, include the header: X-Auth-Failure: 401
/// To trigger a 403 response, include the header: X-Auth-Failure: 403
pub async fn auth_failure_middleware(request: Request, next: Next) -> Response {
    // Skip health checks
    if request.uri().path() == "/health" || request.uri().path() == "/" {
        return next.run(request).await;
    }

    // Check for X-Auth-Failure header
    if let Some(header_value) = request.headers().get("x-auth-failure") {
        if let Ok(header_str) = header_value.to_str() {
            let status = match header_str {
                "401" | "unauthorized" => Some(StatusCode::UNAUTHORIZED),
                "403" | "forbidden" => Some(StatusCode::FORBIDDEN),
                _ => None,
            };

            if let Some(status_code) = status {
                warn!("Auth failure header detected - returning {}", status_code);
                return (
                    status_code,
                    Json(json!({
                        "error": {
                            "code": status_code.as_u16(),
                            "message": if status_code == StatusCode::UNAUTHORIZED {
                                "Unauthorized: Invalid or missing authentication credentials"
                            } else {
                                "Forbidden: Insufficient permissions"
                            }
                        }
                    })),
                )
                    .into_response();
            }
        }
    }

    next.run(request).await
}

/// Load contracts from Pact broker
/// Fetches the latest contracts for a given provider and consumer
pub async fn load_contracts_from_broker(
    broker_url: &str,
    username: &str,
    password: &str,
    provider: &str,
    consumer: &str,
) -> HashMap<String, Value> {
    let url = format!(
        "{}/pacts/provider/{}/consumer/{}/latest",
        broker_url, provider, consumer
    );

    let client = reqwest::Client::new();
    match client
        .get(&url)
        .basic_auth(username, Some(password))
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => match response.json::<Value>().await {
            Ok(contracts) => {
                info!("✅ Loaded contracts from broker");
                let mut map = HashMap::new();
                map.insert("contracts".to_string(), contracts);
                map
            }
            Err(e) => {
                warn!("Failed to parse contracts: {}", e);
                HashMap::new()
            }
        },
        Ok(response) => {
            warn!(
                "Could not load contracts from broker: {}",
                response.status()
            );
            HashMap::new()
        }
        Err(e) => {
            warn!("Could not connect to broker: {}", e);
            HashMap::new()
        }
    }
}
