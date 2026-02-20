//! # Request Types
//!
//! GCP Parameter Manager REST API request structures.
//!
//! These structs represent the JSON payloads used for communication with the
//! GCP Parameter Manager REST API v1. Parameter Manager is an extension to
//! Secret Manager and uses similar API patterns.
//!
//! References:
//! - [GCP Parameter Manager Overview](https://cloud.google.com/secret-manager/parameter-manager/docs/overview)
//! - [GCP Parameter Manager REST API Reference](https://docs.cloud.google.com/secret-manager/parameter-manager/docs/reference/rest)
//! - API endpoints: `/v1/projects/{project}/locations/{location}/parameters`

use serde::Serialize;

use super::responses::ParameterPayload;

/// Request body for creating a new parameter
///
/// Used in `POST /v1/projects/{project}/locations/{location}/parameters` to create a new parameter resource.
/// Note: This creates the parameter metadata only, not the parameter value.
/// To add a value, use `CreateParameterVersionRequest` after creating the parameter.
///
/// API Reference: https://docs.cloud.google.com/secret-manager/parameter-manager/docs/reference/rest/v1/projects.locations.parameters/create
#[derive(Debug, Serialize)]
pub struct CreateParameterRequest {
    /// The ID of the parameter (not the full resource name)
    ///
    /// This will be combined with the project ID and location to form the full resource name:
    /// `projects/{project}/locations/{location}/parameters/{parameter_id}`
    ///
    /// Note: GCP API expects camelCase "parameterId" in JSON
    #[serde(rename = "parameterId")]
    pub parameter_id: String,
    /// Parameter resource with format and labels
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter: Option<ParameterSpec>,
}

/// Parameter specification for create/update operations
#[derive(Debug, Serialize)]
pub struct ParameterSpec {
    /// Parameter format (e.g., "PLAIN_TEXT", "JSON", "YAML")
    /// Defaults to "PLAIN_TEXT" if not specified
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Labels for the parameter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<std::collections::HashMap<String, String>>,
}

impl CreateParameterRequest {
    /// Create a new request with default format (PLAIN_TEXT)
    pub fn new(parameter_id: String) -> Self {
        Self {
            parameter_id,
            parameter: Some(ParameterSpec {
                format: Some("PLAIN_TEXT".to_string()),
                labels: None,
            }),
        }
    }
}

/// Request body for creating a new parameter version
///
/// Used in `POST /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions` to add
/// a new version with parameter data to an existing parameter.
///
/// **Important**: The payload data must be base64-encoded before sending.
///
/// API Reference: https://docs.cloud.google.com/secret-manager/parameter-manager/docs/reference/rest/v1/projects.locations.parameters.versions/create
#[derive(Debug, Serialize)]
pub struct CreateParameterVersionRequest {
    /// The version ID (user-provided name for the version, optional)
    ///
    /// Unlike Secret Manager which uses auto-incrementing numbers,
    /// Parameter Manager allows user-provided version names.
    /// If not provided, GCP will auto-generate a version ID.
    #[serde(rename = "parameterVersionId", skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    /// The parameter version resource with payload
    pub parameter_version: ParameterVersionSpec,
}

/// Parameter version specification for create operations
#[derive(Debug, Serialize)]
pub struct ParameterVersionSpec {
    /// The parameter payload containing the base64-encoded parameter value
    pub payload: ParameterPayload,
}

impl CreateParameterVersionRequest {
    /// Create a new request with base64-encoded data
    /// If version_id is None, GCP will auto-generate one
    pub fn new(version_id: Option<String>, data: String) -> Self {
        // Base64 encode the data
        use base64::{Engine as _, engine::general_purpose};
        let encoded = general_purpose::STANDARD.encode(data.as_bytes());
        Self {
            version_id,
            parameter_version: ParameterVersionSpec {
                payload: ParameterPayload { data: encoded },
            },
        }
    }
}

/// Request body for updating a parameter (PATCH)
///
/// Used in `PATCH /v1/projects/{project}/locations/{location}/parameters/{parameter}`
/// to update parameter metadata (format, labels).
///
/// API Reference: https://docs.cloud.google.com/secret-manager/parameter-manager/docs/reference/rest/v1/projects.locations.parameters/patch
#[derive(Debug, Serialize)]
pub struct UpdateParameterRequest {
    /// Parameter resource with updated format and labels
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter: Option<ParameterSpec>,
    /// Field mask specifying which fields to update
    /// Format: comma-separated field names (e.g., "format,labels")
    #[serde(rename = "updateMask", skip_serializing_if = "Option::is_none")]
    pub update_mask: Option<String>,
}

impl UpdateParameterRequest {
    /// Create a new update request
    pub fn new(parameter: Option<ParameterSpec>, update_mask: Option<String>) -> Self {
        Self {
            parameter,
            update_mask,
        }
    }
}

/// Request body for updating a parameter version (PATCH)
///
/// Used in `PATCH /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}`
/// to update version state (enable/disable).
///
/// API Reference: https://docs.cloud.google.com/secret-manager/parameter-manager/docs/reference/rest/v1/projects.locations.parameters.versions/patch
#[derive(Debug, Serialize)]
pub struct UpdateParameterVersionRequest {
    /// Parameter version resource with updated state
    pub parameter_version: UpdateParameterVersionSpec,
    /// Field mask specifying which fields to update
    /// Format: comma-separated field names (e.g., "state")
    #[serde(rename = "updateMask", skip_serializing_if = "Option::is_none")]
    pub update_mask: Option<String>,
}

/// Parameter version specification for update operations
#[derive(Debug, Serialize)]
pub struct UpdateParameterVersionSpec {
    /// Version state: "ENABLED" or "DISABLED"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

impl UpdateParameterVersionRequest {
    /// Create a new update request
    pub fn new(state: Option<String>, update_mask: Option<String>) -> Self {
        Self {
            parameter_version: UpdateParameterVersionSpec { state },
            update_mask,
        }
    }
}
