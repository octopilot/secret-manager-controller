//! Shared library for Pact Mock Servers
//!
//! Provides common functionality for GCP, AWS, and Azure mock servers:
//! - Contract loading from Pact broker
//! - Request logging middleware
//! - Health check endpoints
//! - App state management

use axum::{
    extract::Request,
    http::Method,
    middleware::Next,
    response::Json,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Application state shared across all mock servers
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
            if content_type.to_str().unwrap_or("").contains("application/json") {
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
        Ok(response) if response.status().is_success() => {
            match response.json::<Value>().await {
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
            }
        }
        Ok(response) => {
            warn!("Could not load contracts from broker: {}", response.status());
            HashMap::new()
        }
        Err(e) => {
            warn!("Could not connect to broker: {}", e);
            HashMap::new()
        }
    }
}

