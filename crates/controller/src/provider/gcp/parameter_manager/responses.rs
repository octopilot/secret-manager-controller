//! # Response Types
//!
//! GCP Parameter Manager REST API response structures.
//!
//! These structs represent the JSON payloads returned by the GCP Parameter Manager REST API v1.
//! Parameter Manager is an extension to Secret Manager and uses similar response structures.
//!
//! References:
//! - [GCP Parameter Manager Overview](https://cloud.google.com/secret-manager/parameter-manager/docs/overview)

use serde::{Deserialize, Serialize};

/// Parameter resource representation
///
/// Represents a parameter in GCP Parameter Manager.
/// Maps to the `Parameter` resource in the GCP API.
///
/// API Reference: Similar to Secret Manager Secret resource
#[derive(Debug, Deserialize)]
pub struct Parameter {
    /// The resource name of the parameter in the format `projects/*/parameters/*`
    pub name: String,
    /// Parameter format (e.g., "PLAIN_TEXT", "JSON", "YAML")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Labels for the parameter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<std::collections::HashMap<String, String>>,
}

/// Parameter version representation
///
/// Represents a version of a parameter with its payload.
///
/// API Reference: Similar to Secret Manager SecretVersion
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields are part of API contract, may be used in future
pub struct ParameterVersion {
    /// The resource name of the parameter version
    pub name: String,
    /// The parameter payload containing the actual parameter data
    pub payload: ParameterPayload,
    /// Creation time (RFC3339 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_time: Option<String>,
    /// Whether this version is disabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

/// Parameter payload containing the actual parameter data
///
/// The payload contains the parameter value, which is base64-encoded
/// when transmitted over the REST API.
///
/// **Important**: The `data` field is base64-encoded. When sending,
/// we encode the parameter value to base64. When receiving, we decode
/// from base64 to get the original value.
///
/// API Reference: Similar to Secret Manager SecretPayload
#[derive(Debug, Serialize, Deserialize)]
pub struct ParameterPayload {
    /// Base64-encoded parameter data
    ///
    /// When serializing (sending to API): We encode the parameter value to base64.
    /// When deserializing (receiving from API): We decode from base64 to get the original value.
    pub data: String,
}

/// Response from accessing a parameter version
///
/// Returned by `GET /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}`
/// when successfully retrieving a parameter version's value.
///
/// API Reference: https://docs.cloud.google.com/secret-manager/parameter-manager/docs/reference/rest/v1/projects.locations.parameters.versions/get
#[derive(Debug, Deserialize)]
pub struct AccessParameterVersionResponse {
    /// The resource name of the parameter version
    ///
    /// Format: `projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}`
    /// Can be used to extract the version ID.
    pub name: String,
    /// The parameter payload containing the base64-encoded parameter value
    ///
    /// **Note**: The `data` field is base64-encoded and must be decoded
    /// to retrieve the original parameter value.
    pub payload: ParameterPayload,
    /// Creation time (RFC3339 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(dead_code)]
    pub create_time: Option<String>,
    /// Whether this version is disabled
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(dead_code)]
    pub state: Option<String>,
}

/// Response from creating a parameter version
///
/// Returned by `POST /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions`
/// when successfully creating a new parameter version.
///
/// API Reference: https://docs.cloud.google.com/secret-manager/parameter-manager/docs/reference/rest/v1/projects.locations.parameters.versions/create
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields are part of API contract, may be used in future
pub struct CreateParameterVersionResponse {
    /// The resource name of the created parameter version
    ///
    /// Format: `projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}`
    pub name: String,
    /// Creation time (RFC3339 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_time: Option<String>,
}

/// Response from listing parameter versions
///
/// Returned by `GET /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions`
/// when successfully listing parameter versions.
///
/// API Reference: https://docs.cloud.google.com/secret-manager/parameter-manager/docs/reference/rest/v1/projects.locations.parameters.versions/list
#[derive(Debug, Deserialize)]
pub struct ListParameterVersionsResponse {
    /// List of parameter versions
    pub versions: Vec<ParameterVersionListItem>,
    /// Token to retrieve the next page of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Parameter version list item
#[derive(Debug, Deserialize)]
pub struct ParameterVersionListItem {
    /// The resource name of the parameter version
    pub name: String,
    /// Creation time (RFC3339 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_time: Option<String>,
    /// Whether this version is disabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

/// Response from listing parameters
///
/// Returned by `GET /v1/projects/{project}/locations/{location}/parameters`
/// when successfully listing parameters.
///
/// API Reference: https://docs.cloud.google.com/secret-manager/parameter-manager/docs/reference/rest/v1/projects.locations.parameters/list
#[derive(Debug, Deserialize)]
pub struct ListParametersResponse {
    /// List of parameters
    pub parameters: Vec<Parameter>,
    /// Token to retrieve the next page of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Response from rendering a parameter version
///
/// Returned by `GET /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}:render`
/// when successfully rendering a parameter version.
///
/// API Reference: https://docs.cloud.google.com/secret-manager/parameter-manager/docs/reference/rest/v1/projects.locations.parameters.versions/render
#[derive(Debug, Deserialize)]
pub struct RenderParameterVersionResponse {
    /// The rendered parameter value
    ///
    /// This is the decoded parameter value, ready for use.
    pub rendered_value: String,
}

/// Location resource representation
///
/// Represents a GCP location where parameters can be stored.
///
/// API Reference: https://cloud.google.com/secret-manager/parameter-manager/docs/reference/rest/v1/projects.locations#Location
#[derive(Debug, Deserialize)]
pub struct Location {
    /// The resource name of the location
    pub name: String,
    /// Location ID (e.g., "global", "us-central1")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location_id: Option<String>,
    /// Display name of the location
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

/// Response from listing locations
///
/// Returned by `GET /v1/projects/{project}/locations`
/// when successfully listing available locations.
///
/// API Reference: https://docs.cloud.google.com/secret-manager/parameter-manager/docs/reference/rest/v1/projects.locations/list
#[derive(Debug, Deserialize)]
pub struct ListLocationsResponse {
    /// List of locations
    pub locations: Vec<Location>,
    /// Token to retrieve the next page of results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}
