//! GCP Secret Manager REST Client
//!
//! Native REST implementation for GCP Secret Manager API v1.
//! Uses reqwest for HTTP requests and OAuth2 for authentication.
//!
//! This implementation:
//! - Works directly with Pact HTTP mock servers
//! - Avoids gRPC/SSL issues with the official SDK
//! - Easier to troubleshoot and maintain
//! - Suitable for low-volume use cases
//!
//! References:
//! - [GCP Secret Manager REST API v1](https://docs.cloud.google.com/secret-manager/docs/reference/rest)

use crate::observability::metrics;
use crate::provider::SecretManagerProvider;
use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, info, info_span, warn, Instrument};

use super::common::{determine_operation_type, format_secret_path, OperationTracker};

/// GCP Secret Manager REST client
pub struct SecretManagerREST {
    http_client: Client,
    base_url: String,
    project_id: String,
    access_token: String,
}

// ============================================================================
// GCP Secret Manager REST API Request/Response Structures
// ============================================================================
// These structs represent the JSON payloads used for communication with the
// GCP Secret Manager REST API v1. They are designed to match the API schema
// as documented at:
// https://cloud.google.com/secret-manager/docs/reference/rest
// ============================================================================

/// Secret resource representation
///
/// Represents a secret in GCP Secret Manager. Used for both requests and responses.
/// Maps to the `Secret` resource in the GCP API.
///
/// **Note**: Currently this struct is defined but not actively used in the implementation.
/// The codebase uses `CreateSecretRequest` for creating secrets, which contains
/// the necessary fields. This struct is reserved for future use when we need to
/// deserialize full secret resource responses.
///
/// API Reference: https://cloud.google.com/secret-manager/docs/reference/rest/v1/projects.secrets#Secret
#[allow(dead_code)] // Reserved for future use
#[derive(Debug, Serialize, Deserialize)]
struct Secret {
    /// The resource name of the secret in the format `projects/*/secrets/*`
    name: String,
    /// Replication configuration for the secret
    replication: Replication,
}

/// Replication configuration for a secret
///
/// Defines how the secret is replicated across GCP regions.
/// Currently only supports automatic replication.
///
/// API Reference: https://cloud.google.com/secret-manager/docs/reference/rest/v1/Replication
#[derive(Debug, Serialize, Deserialize)]
struct Replication {
    /// Automatic replication configuration
    ///
    /// When set, the secret is automatically replicated to all regions.
    /// This is the default and recommended replication mode.
    #[serde(rename = "automatic")]
    automatic: Option<AutomaticReplication>,
}

/// Automatic replication configuration
///
/// Represents automatic replication where the secret is replicated
/// to all available regions automatically.
///
/// API Reference: https://cloud.google.com/secret-manager/docs/reference/rest/v1/Replication#Automatic
#[derive(Debug, Serialize, Deserialize)]
struct AutomaticReplication {}

/// Secret version representation
///
/// Represents a version of a secret with its payload.
/// Currently reserved for future use. The codebase uses `AccessSecretVersionResponse`
/// for accessing secret versions, which has a similar structure.
///
/// API Reference: https://cloud.google.com/secret-manager/docs/reference/rest/v1/projects.secrets.versions#SecretVersion
#[allow(dead_code)] // Reserved for future use
#[derive(Debug, Serialize, Deserialize)]
struct SecretVersion {
    /// The resource name of the secret version
    name: String,
    /// The secret payload containing the actual secret data
    payload: SecretPayload,
}

/// Secret payload containing the actual secret data
///
/// The payload contains the secret value, which is base64-encoded
/// when transmitted over the REST API.
///
/// **Important**: The `data` field is base64-encoded. When sending,
/// we encode the secret value to base64. When receiving, we decode
/// from base64 to get the original value.
///
/// API Reference: https://cloud.google.com/secret-manager/docs/reference/rest/v1/SecretPayload
#[derive(Debug, Serialize, Deserialize)]
struct SecretPayload {
    /// Base64-encoded secret data
    ///
    /// When serializing (sending to API): We encode the secret value to base64.
    /// When deserializing (receiving from API): We decode from base64 to get the original value.
    data: String,
}

// ============================================================================
// Request Structures
// ============================================================================

/// Request body for creating a new secret
///
/// Used in `POST /v1/projects/{project}/secrets` to create a new secret resource.
/// Note: This creates the secret metadata only, not the secret value.
/// To add a value, use `AddVersionRequest` after creating the secret.
///
/// API Reference: https://cloud.google.com/secret-manager/docs/reference/rest/v1/projects.secrets/create
#[derive(Debug, Serialize)]
struct CreateSecretRequest {
    /// The ID of the secret (not the full resource name)
    ///
    /// This will be combined with the project ID to form the full resource name:
    /// `projects/{project}/secrets/{secret_id}`
    ///
    /// Note: GCP API expects camelCase "secretId" in JSON
    #[serde(rename = "secretId")]
    secret_id: String,
    /// Replication configuration for the secret
    replication: Replication,
}

/// Request body for adding a new version to an existing secret
///
/// Used in `POST /v1/projects/{project}/secrets/{secret}:addVersion` to add
/// a new version with secret data to an existing secret.
///
/// **Important**: The payload data must be base64-encoded before sending.
///
/// API Reference: https://cloud.google.com/secret-manager/docs/reference/rest/v1/projects.secrets#addVersion
#[derive(Debug, Serialize)]
struct AddVersionRequest {
    /// The secret payload containing the base64-encoded secret value
    payload: SecretPayload,
}

// ============================================================================
// Response Structures
// ============================================================================

/// Response from accessing a secret version
///
/// Returned by `GET /v1/projects/{project}/secrets/{secret}/versions/{version}:access`
/// when successfully retrieving a secret version's value.
///
/// API Reference: https://cloud.google.com/secret-manager/docs/reference/rest/v1/projects.secrets.versions/access
#[derive(Debug, Deserialize)]
struct AccessSecretVersionResponse {
    /// The resource name of the secret version
    ///
    /// Required for deserialization but not used in the implementation.
    /// We only need the payload data.
    #[allow(dead_code)]
    name: String,
    /// The secret payload containing the base64-encoded secret value
    ///
    /// **Note**: The `data` field is base64-encoded and must be decoded
    /// to retrieve the original secret value.
    payload: SecretPayload,
}

// ============================================================================
// Error Response Structures
// ============================================================================

/// GCP API error response wrapper
///
/// GCP REST API returns errors in a standard format with an `error` field
/// containing error details. This struct is used to deserialize error responses.
///
/// API Reference: https://cloud.google.com/apis/design/errors
#[derive(Debug, Deserialize)]
struct GcpErrorResponse {
    /// Error details
    error: GcpError,
}

/// Detailed error information from GCP API
///
/// Contains the error code, message, and status information returned
/// by the GCP Secret Manager API when an operation fails.
#[derive(Debug, Deserialize)]
struct GcpError {
    /// HTTP status code (e.g., 404, 403, 500)
    code: u16,
    /// Human-readable error message
    message: String,
    /// Error status string (e.g., "NOT_FOUND", "PERMISSION_DENIED")
    status: String,
}

// ============================================================================
// OAuth2 Token Response Structure
// ============================================================================

/// OAuth2 access token response from GCP metadata server
///
/// Returned by the GCP metadata server when requesting an access token
/// for service account authentication (Workload Identity).
///
/// Endpoint: `http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token`
///
/// API Reference: https://cloud.google.com/compute/docs/metadata/querying-metadata
#[derive(Debug, Deserialize)]
struct TokenResponse {
    /// OAuth2 access token for authenticating with GCP APIs
    access_token: String,
    /// Token type (typically "Bearer")
    #[serde(rename = "token_type")]
    #[allow(dead_code)] // Field is required for deserialization but not used after parsing
    _token_type: String,
    /// Token expiration time in seconds
    #[allow(dead_code)] // Field is required for deserialization but not used after parsing
    expires_in: u64,
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
    async fn get_access_token() -> Result<String> {
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
                let token_response: TokenResponse = response
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
    fn make_request(
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
    fn handle_error_response(&self, status: reqwest::StatusCode, error_text: String) -> Result<()> {
        // Try to parse GCP error response
        if let Ok(error_response) = serde_json::from_str::<GcpErrorResponse>(&error_text) {
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
}

#[async_trait]
impl SecretManagerProvider for SecretManagerREST {
    async fn create_or_update_secret(&self, secret_name: &str, secret_value: &str) -> Result<bool> {
        let span = info_span!(
            "gcp.secret.create_or_update",
            secret.name = secret_name,
            project.id = self.project_id
        );
        let span_clone = span.clone();
        let project_id = self.project_id.clone();
        let http_client = self.http_client.clone();
        let base_url = self.base_url.clone();
        let access_token = self.access_token.clone();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());
            let self_ref = Self {
                http_client,
                base_url,
                project_id: project_id.clone(),
                access_token,
            };

            // Check if secret exists
            let existing_secret = self_ref.get_secret_value(secret_name).await?;

            // Create secret if it doesn't exist
            if existing_secret.is_none() {
                info!("Creating new GCP secret: {}", secret_name);

                let create_request = CreateSecretRequest {
                    secret_id: secret_name.to_string(),
                    replication: Replication {
                        automatic: Some(AutomaticReplication {}),
                    },
                };

                let response = self_ref
                    .make_request(
                        "POST",
                        &format!("projects/{}/secrets", self_ref.project_id),
                        Some(serde_json::to_value(&create_request)?),
                    )
                    .send()
                    .await
                    .context("Failed to create secret")?;

                if !response.status().is_success() {
                    let status = response.status();
                    let error_text = response.text().await.unwrap_or_default();
                    tracker.record_error(None, &error_text);
                    return Err(self_ref
                        .handle_error_response(status, error_text)
                        .context(format!("Failed to create GCP secret: {}", secret_name))
                        .unwrap_err());
                }
            }

            // Check if value changed
            let operation_type = match determine_operation_type(&existing_secret, secret_value) {
                Some("no_change") => {
                    tracker.record_no_change();
                    return Ok(false);
                }
                Some(op_type) => {
                    if op_type == "update" {
                        info!("Secret value changed, updating GCP secret: {}", secret_name);
                    }
                    op_type
                }
                None => {
                    return Err(anyhow::anyhow!(
                        "Unexpected error determining operation type for secret: {secret_name}"
                    ));
                }
            };

            // Add new version with the value
            // GCP Secret Manager expects base64-encoded data
            let secret_bytes = secret_value.as_bytes();
            let encoded = general_purpose::STANDARD.encode(secret_bytes);

            let add_version_request = AddVersionRequest {
                payload: SecretPayload { data: encoded },
            };

            let response = self_ref
                .make_request(
                    "POST",
                    &format!(
                        "projects/{}/secrets/{}:addVersion",
                        self_ref.project_id, secret_name
                    ),
                    Some(serde_json::to_value(&add_version_request)?),
                )
                .send()
                .await
                .context("Failed to add secret version")?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracker.record_error(Some(operation_type), &error_text);
                self_ref
                    .handle_error_response(status, error_text)
                    .context(format!(
                        "Failed to add version to GCP secret: {}",
                        secret_name
                    ))?;
                unreachable!();
            }

            tracker.record_success(operation_type);
            Ok(true)
        }
        .instrument(span)
        .await
    }

    async fn get_secret_value(&self, secret_name: &str) -> Result<Option<String>> {
        let span = tracing::debug_span!(
            "gcp.secret.get",
            secret.name = secret_name,
            project.id = self.project_id
        );
        let span_clone = span.clone();
        let project_id = self.project_id.clone();
        let http_client = &self.http_client;
        let base_url = self.base_url.clone();
        let access_token = self.access_token.clone();

        async move {
            let start = Instant::now();
            let self_ref = Self {
                http_client: http_client.clone(),
                base_url,
                project_id: project_id.clone(),
                access_token,
            };

            let version_path = format!(
                "projects/{}/secrets/{}/versions/latest:access",
                project_id, secret_name
            );

            let response = self_ref
                .make_request("GET", &version_path, None)
                .send()
                .await
                .context("Failed to access secret version")?;

            match response.status() {
                status if status.is_success() => {
                    let access_response: AccessSecretVersionResponse = response
                        .json()
                        .await
                        .context("Failed to parse secret version response")?;

                    // Decode base64
                    let decoded = general_purpose::STANDARD
                        .decode(access_response.payload.data.as_bytes())
                        .context("Failed to decode base64 secret data")?;
                    let secret_value =
                        String::from_utf8(decoded).context("Secret value is not valid UTF-8")?;

                    span_clone.record("operation.success", true);
                    span_clone.record("operation.found", true);
                    span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                    metrics::record_secret_operation("gcp", "get", start.elapsed().as_secs_f64());
                    Ok(Some(secret_value))
                }
                status if status == 404 => {
                    // Secret not found
                    span_clone.record("operation.success", true);
                    span_clone.record("operation.found", false);
                    span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                    metrics::record_secret_operation("gcp", "get", start.elapsed().as_secs_f64());
                    Ok(None)
                }
                status => {
                    let error_text = response.text().await.unwrap_or_default();
                    span_clone.record("operation.success", false);
                    span_clone.record("error.message", error_text.clone());
                    span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
                    metrics::increment_provider_operation_errors("gcp");
                    self_ref
                        .handle_error_response(status, error_text)
                        .context(format!("Failed to get GCP secret: {}", secret_name))?;
                    unreachable!()
                }
            }
        }
        .instrument(span)
        .await
    }

    async fn delete_secret(&self, secret_name: &str) -> Result<()> {
        info!("Deleting GCP secret: {}", secret_name);

        let secret_path = format_secret_path(&self.project_id, secret_name);

        let response = self
            .make_request("DELETE", &secret_path, None)
            .send()
            .await
            .context("Failed to delete secret")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return self
                .handle_error_response(status, error_text)
                .context(format!("Failed to delete GCP secret: {}", secret_name));
        }

        Ok(())
    }
}
