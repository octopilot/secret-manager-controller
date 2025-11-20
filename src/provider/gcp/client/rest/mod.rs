//! GCP Secret Manager REST Client
//!
//! Native REST implementation for GCP Secret Manager API v1.
//! Uses reqwest for HTTP requests and OAuth2 for authentication.
//!
//! This implementation:
//! - Works directly with Pact HTTP mock servers
//! - Uses reqwest with rustls (no OpenSSL dependencies)
//! - Easier to troubleshoot and maintain
//! - Suitable for low-volume use cases
//!
//! References:
//! - [GCP Secret Manager REST API v1](https://docs.cloud.google.com/secret-manager/docs/reference/rest)

mod operations;
mod requests;
mod responses;

// Re-export types
pub use requests::*;
pub use responses::*;

use anyhow::{Context, Result};
use reqwest::Client;
use tracing::{debug, info, warn};

/// GCP Secret Manager REST client
pub struct SecretManagerREST {
    http_client: Client,
    base_url: String,
    project_id: String,
    access_token: String,
}

impl std::fmt::Debug for SecretManagerREST {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretManagerREST")
            .field("project_id", &self.project_id)
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl SecretManagerREST {
    /// Create a new GCP REST client with authentication
    ///
    /// Supports both Workload Identity (via metadata server) and service account JSON.
    /// When `PACT_MODE=true`, uses a dummy token and routes to Pact mock server.
    ///
    /// # Errors
    /// Returns an error if client initialization or token retrieval fails
    #[allow(
        clippy::missing_errors_doc,
        reason = "Error documentation is provided in doc comments"
    )]
    pub async fn new(
        project_id: String,
        _auth_type: Option<&str>,
        service_account_email: Option<&str>,
    ) -> Result<Self> {
        // Determine base URL - use Pact mock server if enabled
        let base_url = if std::env::var("PACT_MODE").is_ok() {
            std::env::var("GCP_SECRET_MANAGER_ENDPOINT")
                .unwrap_or_else(|_| "https://secretmanager.googleapis.com".to_string())
        } else {
            "https://secretmanager.googleapis.com".to_string()
        };

        if let Some(email) = service_account_email {
            info!(
                "Using Workload Identity authentication with service account: {}",
                email
            );
            info!(
                "Ensure service account annotation is set: iam.gke.io/gcp-service-account={}",
                email
            );
        } else {
            info!("Using Workload Identity authentication (service account from pod annotation)");
        }

        info!("Initializing GCP REST client for project: {}", project_id);
        if std::env::var("PACT_MODE").is_ok() {
            info!("Pact mode enabled: using endpoint {}", base_url);
        }

        // Create HTTP client with rustls (already configured in Cargo.toml)
        let http_client = Client::builder()
            .build()
            .context("Failed to create HTTP client")?;

        // Get OAuth2 access token
        let access_token = Self::get_access_token().await?;

        Ok(Self {
            http_client,
            base_url,
            project_id,
            access_token,
        })
    }

    /// Get OAuth2 access token for GCP API authentication
    ///
    /// Supports:
    /// - Metadata server (Workload Identity) - for GKE/GCE
    /// - Service account JSON (GOOGLE_APPLICATION_CREDENTIALS) - for local/dev
    /// - Pact mode - returns dummy token
    pub(crate) async fn get_access_token() -> Result<String> {
        // In Pact mode, use dummy token
        if std::env::var("PACT_MODE").is_ok() {
            debug!("Pact mode: using dummy access token");
            return Ok("test-token".to_string());
        }

        // Try metadata server first (Workload Identity)
        let metadata_url = "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";
        let metadata_client = Client::builder().build()?;

        match metadata_client
            .get(metadata_url)
            .header("Metadata-Flavor", "Google")
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                let token_response: responses::TokenResponse = response
                    .json()
                    .await
                    .context("Failed to parse token response from metadata server")?;
                info!("Retrieved access token from metadata server (Workload Identity)");
                return Ok(format!("Bearer {}", token_response.access_token));
            }
            Ok(response) => {
                debug!(
                    "Metadata server returned status {}: {}",
                    response.status(),
                    response.text().await.unwrap_or_default()
                );
            }
            Err(e) => {
                debug!("Metadata server not available: {}", e);
            }
        }

        // Fall back to service account JSON if GOOGLE_APPLICATION_CREDENTIALS is set
        if let Ok(credentials_path) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
            warn!(
                "Service account JSON authentication not yet implemented. \
                GOOGLE_APPLICATION_CREDENTIALS={} is set but will be ignored. \
                Please use Workload Identity or implement JWT-based authentication.",
                credentials_path
            );
        }

        Err(anyhow::anyhow!(
            "Failed to get access token. Ensure:\n\
            1. Running in GKE/GCE with Workload Identity enabled, OR\n\
            2. GOOGLE_APPLICATION_CREDENTIALS is set (not yet implemented)"
        ))
    }

    /// Build HTTP request with authentication headers
    pub(crate) fn make_request(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> reqwest::RequestBuilder {
        let url = if path.starts_with("http") {
            path.to_string()
        } else {
            format!("{}/v1/{}", self.base_url, path)
        };

        let mut request = match method {
            "GET" => self.http_client.get(&url),
            "POST" => self.http_client.post(&url),
            "PATCH" => self.http_client.patch(&url),
            "DELETE" => self.http_client.delete(&url),
            _ => panic!("Unsupported HTTP method: {}", method),
        };

        // Format authorization header: add "Bearer " prefix if not already present
        let auth_header = if self.access_token.starts_with("Bearer ") {
            self.access_token.clone()
        } else {
            format!("Bearer {}", self.access_token)
        };

        request = request
            .header("Authorization", &auth_header)
            .header("Content-Type", "application/json");

        if let Some(body) = body {
            request = request.json(&body);
        }

        request
    }

    /// Handle GCP API error responses
    pub(crate) fn handle_error_response(
        &self,
        status: reqwest::StatusCode,
        error_text: String,
    ) -> Result<()> {
        // Try to parse GCP error response
        if let Ok(error_response) = serde_json::from_str::<responses::GcpErrorResponse>(&error_text)
        {
            Err(anyhow::anyhow!(
                "GCP API error: {} (code: {}, status: {})",
                error_response.error.message,
                error_response.error.code,
                error_response.error.status
            ))
        } else {
            // Include status code in error message for easier matching in tests
            Err(anyhow::anyhow!(
                "HTTP {} (status: {}): {}",
                status.as_u16(),
                status,
                error_text
            ))
        }
    }

    /// Get the project ID
    pub(crate) fn project_id(&self) -> &str {
        &self.project_id
    }

    /// Get the HTTP client
    pub(crate) fn http_client(&self) -> &Client {
        &self.http_client
    }

    /// Get the base URL
    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Get the access token
    pub(crate) fn access_token(&self) -> &str {
        &self.access_token
    }
}
