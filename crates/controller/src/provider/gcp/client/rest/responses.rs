//! # Response Types
//!
//! GCP Secret Manager REST API response structures.
//!
//! These structs represent the JSON payloads returned by the GCP Secret Manager REST API v1.
//! They are designed to match the API schema as documented at:
//! https://cloud.google.com/secret-manager/docs/reference/rest

use serde::{Deserialize, Serialize};

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
#[derive(Debug, Deserialize)]
pub struct Secret {
    /// The resource name of the secret in the format `projects/*/secrets/*`
    pub name: String,
    /// Replication configuration for the secret
    pub replication: Replication,
}

/// Replication configuration for a secret
///
/// Defines how the secret is replicated across GCP regions.
/// Currently only supports automatic replication.
///
/// API Reference: https://cloud.google.com/secret-manager/docs/reference/rest/v1/Replication
#[derive(Debug, Serialize, Deserialize)]
pub struct Replication {
    /// Automatic replication configuration
    ///
    /// When set, the secret is automatically replicated to all regions.
    /// This is the default and recommended replication mode.
    #[serde(rename = "automatic")]
    pub automatic: Option<AutomaticReplication>,
}

/// Automatic replication configuration
///
/// Represents automatic replication where the secret is replicated
/// to all available regions automatically.
///
/// API Reference: https://cloud.google.com/secret-manager/docs/reference/rest/v1/Replication#Automatic
#[derive(Debug, Serialize, Deserialize)]
pub struct AutomaticReplication {}

/// Secret version representation
///
/// Represents a version of a secret with its payload.
/// Currently reserved for future use. The codebase uses `AccessSecretVersionResponse`
/// for accessing secret versions, which has a similar structure.
///
/// API Reference: https://cloud.google.com/secret-manager/docs/reference/rest/v1/projects.secrets.versions#SecretVersion
#[allow(dead_code)] // Reserved for future use
#[derive(Debug, Deserialize)]
pub struct SecretVersion {
    /// The resource name of the secret version
    pub name: String,
    /// The secret payload containing the actual secret data
    pub payload: SecretPayload,
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
pub struct SecretPayload {
    /// Base64-encoded secret data
    ///
    /// When serializing (sending to API): We encode the secret value to base64.
    /// When deserializing (receiving from API): We decode from base64 to get the original value.
    pub data: String,
}

/// Response from accessing a secret version
///
/// Returned by `GET /v1/projects/{project}/secrets/{secret}/versions/{version}:access`
/// when successfully retrieving a secret version's value.
///
/// API Reference: https://cloud.google.com/secret-manager/docs/reference/rest/v1/projects.secrets.versions/access
#[derive(Debug, Deserialize)]
pub struct AccessSecretVersionResponse {
    /// The resource name of the secret version
    ///
    /// Required for deserialization but not used in the implementation.
    /// We only need the payload data.
    #[allow(dead_code, reason = "Required for deserialization but not used")]
    pub name: String,
    /// The secret payload containing the base64-encoded secret value
    ///
    /// **Note**: The `data` field is base64-encoded and must be decoded
    /// to retrieve the original secret value.
    pub payload: SecretPayload,
}

/// GCP API error response wrapper
///
/// GCP REST API returns errors in a standard format with an `error` field
/// containing error details. This struct is used to deserialize error responses.
///
/// API Reference: https://cloud.google.com/apis/design/errors
#[derive(Debug, Deserialize)]
pub struct GcpErrorResponse {
    /// Error details
    pub error: GcpError,
}

/// Detailed error information from GCP API
///
/// Contains the error code, message, and status information returned
/// by the GCP Secret Manager API when an operation fails.
#[derive(Debug, Deserialize)]
pub struct GcpError {
    /// HTTP status code (e.g., 404, 403, 500)
    pub code: u16,
    /// Human-readable error message
    pub message: String,
    /// Error status string (e.g., "NOT_FOUND", "PERMISSION_DENIED")
    pub status: String,
}

/// OAuth2 access token response from GCP metadata server
///
/// Returned by the GCP metadata server when requesting an access token
/// for service account authentication (Workload Identity).
///
/// Endpoint: `http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token`
///
/// API Reference: https://cloud.google.com/compute/docs/metadata/querying-metadata
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    /// OAuth2 access token for authenticating with GCP APIs
    pub access_token: String,
    /// Token type (typically "Bearer")
    #[serde(rename = "token_type")]
    #[allow(dead_code)] // Field is required for deserialization but not used after parsing
    pub _token_type: String,
    /// Token expiration time in seconds
    #[allow(dead_code)] // Field is required for deserialization but not used after parsing
    pub expires_in: u64,
}
