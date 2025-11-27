//! # Request Types
//!
//! GCP Secret Manager REST API request structures.
//!
//! These structs represent the JSON payloads used for communication with the
//! GCP Secret Manager REST API v1. They are designed to match the API schema
//! as documented at:
//! https://cloud.google.com/secret-manager/docs/reference/rest

use serde::Serialize;

use super::responses::{AutomaticReplication, Replication, SecretPayload};

/// Request body for creating a new secret
///
/// Used in `POST /v1/projects/{project}/secrets` to create a new secret resource.
/// Note: This creates the secret metadata only, not the secret value.
/// To add a value, use `AddVersionRequest` after creating the secret.
///
/// API Reference: https://cloud.google.com/secret-manager/docs/reference/rest/v1/projects.secrets/create
#[derive(Debug, Serialize)]
pub struct CreateSecretRequest {
    /// The ID of the secret (not the full resource name)
    ///
    /// This will be combined with the project ID to form the full resource name:
    /// `projects/{project}/secrets/{secret_id}`
    ///
    /// Note: GCP API expects camelCase "secretId" in JSON
    #[serde(rename = "secretId")]
    pub secret_id: String,
    /// Replication configuration for the secret
    pub replication: Replication,
    /// Labels for the secret (required: must include "environment" and "location")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<std::collections::HashMap<String, String>>,
}

impl CreateSecretRequest {
    /// Create a new request with automatic replication and labels
    pub fn new(secret_id: String, environment: String, location: String) -> Self {
        let mut labels = std::collections::HashMap::new();
        labels.insert("environment".to_string(), environment);
        // For GCP automatic replication, location should not be added to labels
        // "automatic" is not a valid GCP location - it means no specific location (NULL)
        if !location.is_empty() && location != "automatic" {
            labels.insert("location".to_string(), location);
        }

        Self {
            secret_id,
            replication: Replication {
                automatic: Some(AutomaticReplication {}),
            },
            labels: Some(labels),
        }
    }
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
pub struct AddVersionRequest {
    /// The secret payload containing the base64-encoded secret value
    pub payload: SecretPayload,
}

impl AddVersionRequest {
    /// Create a new request with base64-encoded data
    pub fn new(data: String) -> Self {
        Self {
            payload: SecretPayload { data },
        }
    }
}
