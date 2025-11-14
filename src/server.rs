//! # HTTP Server
//!
//! HTTP server for metrics, health checks, and Kubernetes probes.
//!
//! Provides endpoints:
//! - `/metrics` - Prometheus metrics in text format
//! - `/healthz` - Liveness probe (always returns 200)
//! - `/readyz` - Readiness probe (returns 200 when controller is ready)
//!
//! The server runs on port 5000 by default (configurable via `METRICS_PORT` environment variable).

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use prometheus::{Encoder, TextEncoder};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

pub struct ServerState {
    pub is_ready: Arc<std::sync::atomic::AtomicBool>,
}

pub async fn start_server(port: u16, state: Arc<ServerState>) -> Result<(), anyhow::Error> {
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/healthz", get(healthz_handler))
        .route("/readyz", get(readyz_handler))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    
    info!("HTTP server listening on {}", addr);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

fn gather() -> Vec<prometheus::proto::MetricFamily> {
    use crate::metrics::REGISTRY;
    REGISTRY.gather()
}

async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = gather();
    
    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        error!("Failed to encode metrics: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            [("content-type", "text/plain")],
            format!("Failed to encode metrics: {}", e).into_bytes(),
        );
    }
    
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        buffer,
    )
}

async fn healthz_handler() -> impl IntoResponse {
    StatusCode::OK
}

async fn readyz_handler(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    if state.is_ready.load(std::sync::atomic::Ordering::Relaxed) {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

