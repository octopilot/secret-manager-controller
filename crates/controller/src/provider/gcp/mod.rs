//! # GCP Providers
//!
//! Providers for Google Cloud Platform services:
//! - Secret Manager: For storing secrets
//! - Parameter Manager: For storing configuration parameters
//!
//! Uses native REST implementations that:
//! - Work directly with Pact HTTP mock servers
//! - Use reqwest with rustls (no OpenSSL dependencies)
//! - Easier to troubleshoot and maintain

mod client;
mod parameter_manager;

pub use client::SecretManagerREST;
pub use parameter_manager::ParameterManagerREST;

use crate::provider::{ConfigStoreProvider, SecretManagerProvider};
use anyhow::Result;
use tracing::{debug, info};

/// Create a GCP Secret Manager provider
///
/// Always uses the REST client implementation to avoid SSL/OpenSSL issues.
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
    debug!("Using GCP REST client (native implementation)");
    Ok(Box::new(
        SecretManagerREST::new(project_id, auth_type, service_account_email).await?,
    ))
}

/// Create a GCP Parameter Manager provider
///
/// Uses the REST client implementation to interact with GCP Parameter Manager API.
///
/// # Arguments
/// - `project_id`: GCP project ID
/// - `auth_type`: Authentication type (currently only WorkloadIdentity is supported)
/// - `service_account_email`: Optional service account email for Workload Identity
///
/// # Returns
/// A boxed `ConfigStoreProvider` implementation
pub async fn create_gcp_parameter_manager_provider(
    project_id: String,
    auth_type: Option<&str>,
    service_account_email: Option<&str>,
) -> Result<Box<dyn ConfigStoreProvider>> {
    info!("Using GCP Parameter Manager REST client (native implementation)");
    Ok(Box::new(
        ParameterManagerREST::new(project_id, auth_type, service_account_email).await?,
    ))
}
