//! # Secret Manager Controller
//!
//! A Kubernetes controller that syncs secrets from GitOps repositories (FluxCD/ArgoCD) to Google Cloud Secret Manager.
//!
//! ## Overview
//!
//! This controller provides GitOps-style secret management by:
//!
//! 1. **Watching GitOps sources** - Monitors FluxCD GitRepository or ArgoCD Application resources
//! 2. **Reading secret files** - Parses `application.secrets.env`, `application.secrets.yaml`, and `application.properties`
//! 3. **SOPS decryption** - Automatically decrypts SOPS-encrypted files using GPG keys from Kubernetes secrets
//! 4. **Kustomize support** - Runs `kustomize build` to extract secrets from generated Kubernetes Secret resources
//! 5. **GCP Secret Manager sync** - Stores secrets in Google Cloud Secret Manager for CloudRun consumption
//!
//! ## Features
//!
//! - **GitOps-agnostic**: Works with FluxCD GitRepository and ArgoCD Application via `sourceRef` pattern
//! - **Kustomize Build Mode**: Supports overlays, patches, and generators by running `kustomize build`
//! - **Raw File Mode**: Direct parsing of application secret files
//! - **SOPS encryption**: Automatic decryption of SOPS-encrypted files
//! - **Multi-namespace**: Watches `SecretManagerConfig` resources across all namespaces
//! - **Prometheus metrics**: Exposes metrics for monitoring and observability
//! - **Health probes**: HTTP endpoints for liveness and readiness checks
//!
//! ## Usage
//!
//! See the [README.md](../README.md) for detailed usage instructions and examples.

use anyhow::Result;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

mod config;
mod constants;
pub mod controller;
pub mod crd;
pub mod observability;
pub mod provider;
pub mod runtime;

use runtime::initialization::initialize;
use runtime::watch_loop::run_watch_loop;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the controller runtime
    let init_result = initialize().await?;

    // Run the watch loop
    run_watch_loop(
        init_result.configs,
        init_result.reconciler,
        init_result.server_state,
        init_result.controller_config,
    )
    .await?;

    // Shutdown OpenTelemetry tracer provider if it was initialized
    observability::otel::shutdown_otel(init_result.otel_tracer_provider);

    Ok(())
}
