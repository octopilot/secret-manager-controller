//! # GCP Secret Manager Client
//!
//! Client for interacting with Google Cloud Secret Manager API.
//!
//! This module provides functionality to:
//! - Create and update secrets in GCP Secret Manager
//! - Retrieve secret values
//! - Manage secret versions
//!
//! Supports two implementations:
//! - **REST Client** (recommended): Native REST implementation using reqwest
//!   - Works directly with Pact HTTP mock servers
//!   - Avoids gRPC/SSL issues with the official SDK
//!   - Easier to troubleshoot and maintain
//!   - Enabled via `GCP_USE_REST` environment variable or when `PACT_MODE=true`
//! - **gRPC Client**: Official [`google-cloud-secretmanager-v1`] SDK (legacy)
//!   - Used when REST client is not enabled
//!   - May have SSL/reqwest compatibility issues

mod client;
pub use client::{SecretManagerGRPC, SecretManagerREST};

use crate::provider::SecretManagerProvider;
use anyhow::Result;
use tracing::info;

/// Create a GCP Secret Manager provider
///
/// Automatically selects REST or gRPC client based on:
/// - `GCP_USE_REST` environment variable (explicit choice)
/// - `PACT_MODE` environment variable (automatically uses REST for Pact compatibility)
///
/// # Arguments
/// - `project_id`: GCP project ID
/// - `auth_type`: Authentication type (currently only WorkloadIdentity is supported)
/// - `service_account_email`: Optional service account email for Workload Identity
///
/// # Returns
/// A boxed `SecretManagerProvider` implementation
pub async fn create_gcp_provider(
    project_id: String,
    auth_type: Option<&str>,
    service_account_email: Option<&str>,
) -> Result<Box<dyn SecretManagerProvider>> {
    // Use REST client if explicitly requested or if Pact mode is enabled
    let use_rest = std::env::var("GCP_USE_REST").is_ok() || std::env::var("PACT_MODE").is_ok();

    if use_rest {
        info!("Using GCP REST client (Pact mode or GCP_USE_REST enabled)");
        Ok(Box::new(
            SecretManagerREST::new(project_id, auth_type, service_account_email).await?,
        ))
    } else {
        info!("Using GCP gRPC client (official SDK)");
        Ok(Box::new(
            SecretManagerGRPC::new(project_id, auth_type, service_account_email).await?,
        ))
    }
}
