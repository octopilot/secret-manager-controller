//! # GCP Parameter Manager Client
//!
//! Client for interacting with Google Cloud Parameter Manager REST API.
//!
//! Parameter Manager is an extension to Secret Manager and provides centralized storage
//! for configuration parameters related to workload deployments.
//!
//! This module provides functionality to:
//! - Create and update parameters in GCP Parameter Manager
//! - Retrieve parameter values
//! - Support Workload Identity authentication
//!
//! References:
//! - [GCP Parameter Manager Overview](https://cloud.google.com/secret-manager/parameter-manager/docs/overview)
//! - [GCP Parameter Manager REST API Reference](https://docs.cloud.google.com/secret-manager/parameter-manager/docs/reference/rest)
//! - Base URL: `https://parametermanager.googleapis.com`
//! - API endpoints: `/v1/projects/{project}/locations/{location}/parameters`

mod requests;
mod responses;

use crate::provider::ConfigStoreProvider;
use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use reqwest::Client;
use tracing::{Instrument, info, info_span};

use requests::{
    CreateParameterRequest, CreateParameterVersionRequest, UpdateParameterRequest,
    UpdateParameterVersionRequest,
};
use responses::{
    AccessParameterVersionResponse, ListLocationsResponse, ListParameterVersionsResponse,
    ListParametersResponse, Location, Parameter, RenderParameterVersionResponse,
};

use crate::provider::gcp::client::common::{OperationTracker, determine_operation_type};
use smc_paths::prelude::{GcpOperation, PathBuilder};

/// GCP Parameter Manager REST client
pub struct ParameterManagerREST {
    http_client: Client,
    base_url: String,
    project_id: String,
    location: String, // Location (e.g., "global", "us-central1")
    access_token: String,
}

impl std::fmt::Debug for ParameterManagerREST {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParameterManagerREST")
            .field("project_id", &self.project_id)
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl ParameterManagerREST {
    /// Create a new GCP Parameter Manager REST client with authentication
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
        Self::new_with_location(project_id, None, _auth_type, service_account_email).await
    }

    /// Create a new GCP Parameter Manager REST client with location
    ///
    /// # Arguments
    /// - `project_id`: GCP project ID
    /// - `location`: Optional location (defaults to "global" if not specified)
    /// - `auth_type`: Authentication type (currently only WorkloadIdentity is supported)
    /// - `service_account_email`: Optional service account email for Workload Identity
    pub async fn new_with_location(
        project_id: String,
        location: Option<String>,
        _auth_type: Option<&str>,
        service_account_email: Option<&str>,
    ) -> Result<Self> {
        // Determine base URL - use Pact mock server if enabled
        // Parameter Manager uses its own service endpoint
        let base_url = if std::env::var("PACT_MODE").is_ok() {
            std::env::var("GCP_PARAMETER_MANAGER_ENDPOINT")
                .unwrap_or_else(|_| "https://parametermanager.googleapis.com".to_string())
        } else {
            "https://parametermanager.googleapis.com".to_string()
        };

        // Default location to "global" if not specified
        let location = location.unwrap_or_else(|| "global".to_string());

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

        info!(
            "Initializing GCP Parameter Manager REST client for project: {}",
            project_id
        );
        if std::env::var("PACT_MODE").is_ok() {
            info!("Pact mode enabled: using endpoint {}", base_url);
        }

        // Create HTTP client with rustls
        let http_client = Client::builder()
            .build()
            .context("Failed to create HTTP client")?;

        // Get OAuth2 access token (same method as Secret Manager)
        let access_token = Self::get_access_token().await?;

        Ok(Self {
            http_client,
            base_url,
            project_id,
            location,
            access_token,
        })
    }

    /// Get OAuth2 access token for GCP API authentication
    ///
    /// Supports:
    /// - Workload Identity (via metadata server) - recommended for GKE
    /// - Service account JSON key file (via GOOGLE_APPLICATION_CREDENTIALS)
    /// - Pact mode (returns dummy token)
    async fn get_access_token() -> Result<String> {
        if std::env::var("PACT_MODE").is_ok() {
            return Ok("test-token".to_string());
        }

        // Use the same token retrieval as Secret Manager
        // This will use Workload Identity or service account JSON
        crate::provider::gcp::client::rest::SecretManagerREST::get_access_token().await
    }

    /// Build parameter path using PathBuilder (single source of truth)
    fn build_parameter_path(&self, operation: GcpOperation, parameter_id: &str) -> Result<String> {
        PathBuilder::new()
            .gcp_operation(operation)
            .project(&self.project_id)
            .location(&self.location)
            .parameter(parameter_id)
            .build_http_path()
            .map_err(|e| anyhow::anyhow!("Failed to build parameter path: {}", e))
    }

    /// Build parameter parent path using PathBuilder (single source of truth)
    fn build_parameter_parent_path(&self, operation: GcpOperation) -> Result<String> {
        PathBuilder::new()
            .gcp_operation(operation)
            .project(&self.project_id)
            .location(&self.location)
            .build_http_path()
            .map_err(|e| anyhow::anyhow!("Failed to build parameter parent path: {}", e))
    }

    /// Build parameter version path using PathBuilder (single source of truth)
    fn build_parameter_version_path(
        &self,
        operation: GcpOperation,
        parameter_id: &str,
        version_id: &str,
    ) -> Result<String> {
        PathBuilder::new()
            .gcp_operation(operation)
            .project(&self.project_id)
            .location(&self.location)
            .parameter(parameter_id)
            .version(version_id)
            .build_http_path()
            .map_err(|e| anyhow::anyhow!("Failed to build parameter version path: {}", e))
    }

    /// Build parameter versions parent path using PathBuilder (single source of truth)
    fn build_parameter_versions_parent_path(
        &self,
        operation: GcpOperation,
        parameter_id: &str,
    ) -> Result<String> {
        PathBuilder::new()
            .gcp_operation(operation)
            .project(&self.project_id)
            .location(&self.location)
            .parameter(parameter_id)
            .build_http_path()
            .map_err(|e| anyhow::anyhow!("Failed to build parameter versions parent path: {}", e))
    }

    /// Make HTTP request to GCP Parameter Manager API
    fn make_request(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> reqwest::RequestBuilder {
        // Path from build_http_path() is without /v1/ prefix, add it here
        let url = format!(
            "{}/v1/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        );
        let mut request = match method {
            "GET" => self.http_client.get(&url),
            "POST" => self.http_client.post(&url),
            "PATCH" => self.http_client.patch(&url),
            "PUT" => self.http_client.put(&url),
            "DELETE" => self.http_client.delete(&url),
            _ => panic!("Unsupported HTTP method: {}", method),
        };

        request = request
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Content-Type", "application/json");

        if let Some(body_value) = body {
            request = request.json(&body_value);
        }

        request
    }

    /// Handle error response from GCP API
    fn handle_error_response(&self, status: reqwest::StatusCode, error_text: String) -> Result<()> {
        match status.as_u16() {
            404 => Err(anyhow::anyhow!("Parameter not found: {}", error_text)),
            403 => Err(anyhow::anyhow!("Permission denied: {}", error_text)),
            401 => Err(anyhow::anyhow!("Unauthorized: {}", error_text)),
            400 => Err(anyhow::anyhow!("Bad request: {}", error_text)),
            _ => Err(anyhow::anyhow!("API error ({}): {}", status, error_text)),
        }
    }

    /// Get project ID
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// Get HTTP client (for testing)
    pub fn http_client(&self) -> &Client {
        &self.http_client
    }

    /// Get base URL (for testing)
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Get access token (for testing)
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    /// Get location (for testing)
    pub fn location(&self) -> &str {
        &self.location
    }

    /// Get a parameter by name
    ///
    /// Returns the parameter metadata (format, labels) without the value.
    pub async fn get_parameter(&self, parameter_name: &str) -> Result<Option<Parameter>> {
        let span = info_span!(
            "gcp.parameter.get_metadata",
            parameter.name = parameter_name,
            project.id = self.project_id(),
            location.id = self.location()
        );
        let span_clone = span.clone();
        let project_id = self.project_id().to_string();
        let location = self.location.clone();
        let http_client = self.http_client().clone();
        let base_url = self.base_url().to_string();
        let access_token = self.access_token().to_string();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());
            let self_ref = ParameterManagerREST {
                http_client,
                base_url,
                project_id: project_id.clone(),
                location: location.clone(),
                access_token,
            };

            let parameter_path = self_ref
                .build_parameter_path(GcpOperation::GetParameter, parameter_name)
                .context("Failed to build parameter path")?;
            let response = self_ref
                .make_request("GET", &parameter_path, None)
                .send()
                .await
                .context("Failed to get parameter")?;

            if response.status().as_u16() == 404 {
                tracker.record_success("get_parameter");
                return Ok(None);
            }

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracker.record_error(Some("get_parameter"), &error_text);
                return Err(self_ref
                    .handle_error_response(status, error_text)
                    .context(format!("Failed to get GCP parameter: {}", parameter_name))
                    .unwrap_err());
            }

            let parameter: Parameter = response
                .json()
                .await
                .context("Failed to deserialize parameter response")?;

            tracker.record_success("get_parameter");
            Ok(Some(parameter))
        }
        .instrument(span)
        .await
    }

    /// List all parameters in the project and location
    pub async fn list_parameters(&self) -> Result<ListParametersResponse> {
        let span = info_span!(
            "gcp.parameter.list",
            project.id = self.project_id(),
            location.id = self.location()
        );
        let span_clone = span.clone();
        let project_id = self.project_id().to_string();
        let location = self.location.clone();
        let http_client = self.http_client().clone();
        let base_url = self.base_url().to_string();
        let access_token = self.access_token().to_string();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());
            let self_ref = ParameterManagerREST {
                http_client,
                base_url,
                project_id: project_id.clone(),
                location: location.clone(),
                access_token,
            };

            let parameters_path = self_ref
                .build_parameter_parent_path(GcpOperation::ListParameters)
                .context("Failed to build parameter parent path")?;
            let response = self_ref
                .make_request("GET", &parameters_path, None)
                .send()
                .await
                .context("Failed to list parameters")?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracker.record_error(Some("list_parameters"), &error_text);
                return Err(self_ref
                    .handle_error_response(status, error_text)
                    .context("Failed to list GCP parameters")
                    .unwrap_err());
            }

            let list_response: ListParametersResponse = response
                .json()
                .await
                .context("Failed to deserialize parameters list response")?;

            tracker.record_success("list_parameters");
            Ok(list_response)
        }
        .instrument(span)
        .await
    }

    /// Update a parameter's metadata (format, labels)
    pub async fn update_parameter(
        &self,
        parameter_name: &str,
        update_request: UpdateParameterRequest,
    ) -> Result<Parameter> {
        let span = info_span!(
            "gcp.parameter.update",
            parameter.name = parameter_name,
            project.id = self.project_id(),
            location.id = self.location()
        );
        let span_clone = span.clone();
        let project_id = self.project_id().to_string();
        let location = self.location.clone();
        let http_client = self.http_client().clone();
        let base_url = self.base_url().to_string();
        let access_token = self.access_token().to_string();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());
            let self_ref = ParameterManagerREST {
                http_client,
                base_url,
                project_id: project_id.clone(),
                location: location.clone(),
                access_token,
            };

            let parameter_path = self_ref
                .build_parameter_path(GcpOperation::UpdateParameter, parameter_name)
                .context("Failed to build parameter path")?;
            let response = self_ref
                .make_request(
                    "PATCH",
                    &parameter_path,
                    Some(serde_json::to_value(&update_request)?),
                )
                .send()
                .await
                .context("Failed to update parameter")?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracker.record_error(Some("update_parameter"), &error_text);
                return Err(self_ref
                    .handle_error_response(status, error_text)
                    .context(format!(
                        "Failed to update GCP parameter: {}",
                        parameter_name
                    ))
                    .unwrap_err());
            }

            let parameter: Parameter = response
                .json()
                .await
                .context("Failed to deserialize parameter response")?;

            tracker.record_success("update_parameter");
            Ok(parameter)
        }
        .instrument(span)
        .await
    }

    /// Get a specific parameter version by version ID
    pub async fn get_version(
        &self,
        parameter_name: &str,
        version_id: &str,
    ) -> Result<Option<AccessParameterVersionResponse>> {
        let span = info_span!(
            "gcp.parameter.version.get",
            parameter.name = parameter_name,
            version.id = version_id,
            project.id = self.project_id(),
            location.id = self.location()
        );
        let span_clone = span.clone();
        let project_id = self.project_id().to_string();
        let location = self.location.clone();
        let http_client = self.http_client().clone();
        let base_url = self.base_url().to_string();
        let access_token = self.access_token().to_string();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());
            let self_ref = ParameterManagerREST {
                http_client,
                base_url,
                project_id: project_id.clone(),
                location: location.clone(),
                access_token,
            };

            let version_path = self_ref
                .build_parameter_version_path(
                    GcpOperation::GetParameterVersion,
                    parameter_name,
                    version_id,
                )
                .context("Failed to build parameter version path")?;
            let response = self_ref
                .make_request("GET", &version_path, None)
                .send()
                .await
                .context("Failed to get parameter version")?;

            if response.status().as_u16() == 404 {
                tracker.record_success("get_version");
                return Ok(None);
            }

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracker.record_error(Some("get_version"), &error_text);
                return Err(self_ref
                    .handle_error_response(status, error_text)
                    .context(format!(
                        "Failed to get GCP parameter version: {}:{}",
                        parameter_name, version_id
                    ))
                    .unwrap_err());
            }

            let version: AccessParameterVersionResponse = response
                .json()
                .await
                .context("Failed to deserialize parameter version response")?;

            tracker.record_success("get_version");
            Ok(Some(version))
        }
        .instrument(span)
        .await
    }

    /// List all versions of a parameter
    pub async fn list_versions(
        &self,
        parameter_name: &str,
    ) -> Result<ListParameterVersionsResponse> {
        let span = info_span!(
            "gcp.parameter.versions.list",
            parameter.name = parameter_name,
            project.id = self.project_id(),
            location.id = self.location()
        );
        let span_clone = span.clone();
        let project_id = self.project_id().to_string();
        let location = self.location.clone();
        let http_client = self.http_client().clone();
        let base_url = self.base_url().to_string();
        let access_token = self.access_token().to_string();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());
            let self_ref = ParameterManagerREST {
                http_client,
                base_url,
                project_id: project_id.clone(),
                location: location.clone(),
                access_token,
            };

            let versions_path = self_ref
                .build_parameter_versions_parent_path(
                    GcpOperation::ListParameterVersions,
                    parameter_name,
                )
                .context("Failed to build parameter versions parent path")?;
            let response = self_ref
                .make_request("GET", &versions_path, None)
                .send()
                .await
                .context("Failed to list parameter versions")?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracker.record_error(Some("list_versions"), &error_text);
                return Err(self_ref
                    .handle_error_response(status, error_text)
                    .context(format!(
                        "Failed to list versions for GCP parameter: {}",
                        parameter_name
                    ))
                    .unwrap_err());
            }

            let list_response: ListParameterVersionsResponse = response
                .json()
                .await
                .context("Failed to deserialize versions list response")?;

            tracker.record_success("list_versions");
            Ok(list_response)
        }
        .instrument(span)
        .await
    }

    /// Update a parameter version's state (enable/disable)
    pub async fn update_version(
        &self,
        parameter_name: &str,
        version_id: &str,
        update_request: UpdateParameterVersionRequest,
    ) -> Result<AccessParameterVersionResponse> {
        let span = info_span!(
            "gcp.parameter.version.update",
            parameter.name = parameter_name,
            version.id = version_id,
            project.id = self.project_id(),
            location.id = self.location()
        );
        let span_clone = span.clone();
        let project_id = self.project_id().to_string();
        let location = self.location.clone();
        let http_client = self.http_client().clone();
        let base_url = self.base_url().to_string();
        let access_token = self.access_token().to_string();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());
            let self_ref = ParameterManagerREST {
                http_client,
                base_url,
                project_id: project_id.clone(),
                location: location.clone(),
                access_token,
            };

            let version_path = self_ref
                .build_parameter_version_path(
                    GcpOperation::UpdateParameterVersion,
                    parameter_name,
                    version_id,
                )
                .context("Failed to build parameter version path")?;
            let response = self_ref
                .make_request(
                    "PATCH",
                    &version_path,
                    Some(serde_json::to_value(&update_request)?),
                )
                .send()
                .await
                .context("Failed to update parameter version")?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracker.record_error(Some("update_version"), &error_text);
                return Err(self_ref
                    .handle_error_response(status, error_text)
                    .context(format!(
                        "Failed to update GCP parameter version: {}:{}",
                        parameter_name, version_id
                    ))
                    .unwrap_err());
            }

            let version: AccessParameterVersionResponse = response
                .json()
                .await
                .context("Failed to deserialize parameter version response")?;

            tracker.record_success("update_version");
            Ok(version)
        }
        .instrument(span)
        .await
    }

    /// Delete a parameter version
    pub async fn delete_version(&self, parameter_name: &str, version_id: &str) -> Result<bool> {
        let span = info_span!(
            "gcp.parameter.version.delete",
            parameter.name = parameter_name,
            version.id = version_id,
            project.id = self.project_id(),
            location.id = self.location()
        );
        let span_clone = span.clone();
        let project_id = self.project_id().to_string();
        let location = self.location.clone();
        let http_client = self.http_client().clone();
        let base_url = self.base_url().to_string();
        let access_token = self.access_token().to_string();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());
            let self_ref = ParameterManagerREST {
                http_client,
                base_url,
                project_id: project_id.clone(),
                location: location.clone(),
                access_token,
            };

            let version_path = self_ref
                .build_parameter_version_path(
                    GcpOperation::DeleteParameterVersion,
                    parameter_name,
                    version_id,
                )
                .context("Failed to build parameter version path")?;
            let response = self_ref
                .make_request("DELETE", &version_path, None)
                .send()
                .await
                .context("Failed to delete parameter version")?;

            if response.status().as_u16() == 404 {
                tracker.record_success("delete_version");
                return Ok(false);
            }

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracker.record_error(Some("delete_version"), &error_text);
                return Err(self_ref
                    .handle_error_response(status, error_text)
                    .context(format!(
                        "Failed to delete GCP parameter version: {}:{}",
                        parameter_name, version_id
                    ))
                    .unwrap_err());
            }

            tracker.record_success("delete_version");
            Ok(true)
        }
        .instrument(span)
        .await
    }

    /// Render a parameter version (get decoded value)
    pub async fn render_version(
        &self,
        parameter_name: &str,
        version_id: &str,
    ) -> Result<Option<String>> {
        let span = info_span!(
            "gcp.parameter.version.render",
            parameter.name = parameter_name,
            version.id = version_id,
            project.id = self.project_id(),
            location.id = self.location()
        );
        let span_clone = span.clone();
        let project_id = self.project_id().to_string();
        let location = self.location.clone();
        let http_client = self.http_client().clone();
        let base_url = self.base_url().to_string();
        let access_token = self.access_token().to_string();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());
            let self_ref = ParameterManagerREST {
                http_client,
                base_url,
                project_id: project_id.clone(),
                location: location.clone(),
                access_token,
            };

            // Render endpoint uses PathBuilder
            let render_path = PathBuilder::new()
                .gcp_operation(GcpOperation::RenderParameterVersion)
                .project(&self_ref.project_id)
                .location(&self_ref.location)
                .parameter(parameter_name)
                .version(version_id)
                .build_http_path()
                .context("Failed to build render parameter version path")?;
            let response = self_ref
                .make_request("GET", &render_path, None)
                .send()
                .await
                .context("Failed to render parameter version")?;

            if response.status().as_u16() == 404 {
                tracker.record_success("render_version");
                return Ok(None);
            }

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracker.record_error(Some("render_version"), &error_text);
                return Err(self_ref
                    .handle_error_response(status, error_text)
                    .context(format!(
                        "Failed to render GCP parameter version: {}:{}",
                        parameter_name, version_id
                    ))
                    .unwrap_err());
            }

            let render_response: RenderParameterVersionResponse = response
                .json()
                .await
                .context("Failed to deserialize render response")?;

            tracker.record_success("render_version");
            Ok(Some(render_response.rendered_value))
        }
        .instrument(span)
        .await
    }

    /// Get a location by name
    pub async fn get_location(&self, location_name: &str) -> Result<Option<Location>> {
        let span = info_span!(
            "gcp.location.get",
            location.name = location_name,
            project.id = self.project_id()
        );
        let span_clone = span.clone();
        let project_id = self.project_id().to_string();
        let http_client = self.http_client().clone();
        let base_url = self.base_url().to_string();
        let access_token = self.access_token().to_string();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());
            let self_ref = ParameterManagerREST {
                http_client,
                base_url,
                project_id: project_id.clone(),
                location: "global".to_string(), // Not used for location endpoints
                access_token,
            };

            let location_path = PathBuilder::new()
                .gcp_operation(GcpOperation::GetLocation)
                .project(&project_id)
                .location(location_name)
                .build_http_path()
                .context("Failed to build get location path")?;
            let response = self_ref
                .make_request("GET", &location_path, None)
                .send()
                .await
                .context("Failed to get location")?;

            if response.status().as_u16() == 404 {
                tracker.record_success("get_location");
                return Ok(None);
            }

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracker.record_error(Some("get_location"), &error_text);
                return Err(self_ref
                    .handle_error_response(status, error_text)
                    .context(format!("Failed to get GCP location: {}", location_name))
                    .unwrap_err());
            }

            let location: Location = response
                .json()
                .await
                .context("Failed to deserialize location response")?;

            tracker.record_success("get_location");
            Ok(Some(location))
        }
        .instrument(span)
        .await
    }

    /// List all available locations
    pub async fn list_locations(&self) -> Result<ListLocationsResponse> {
        let span = info_span!("gcp.location.list", project.id = self.project_id());
        let span_clone = span.clone();
        let project_id = self.project_id().to_string();
        let http_client = self.http_client().clone();
        let base_url = self.base_url().to_string();
        let access_token = self.access_token().to_string();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());
            let self_ref = ParameterManagerREST {
                http_client,
                base_url,
                project_id: project_id.clone(),
                location: "global".to_string(), // Not used for location endpoints
                access_token,
            };

            let locations_path = PathBuilder::new()
                .gcp_operation(GcpOperation::ListLocations)
                .project(&project_id)
                .build_http_path()
                .context("Failed to build list locations path")?;
            let response = self_ref
                .make_request("GET", &locations_path, None)
                .send()
                .await
                .context("Failed to list locations")?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracker.record_error(Some("list_locations"), &error_text);
                return Err(self_ref
                    .handle_error_response(status, error_text)
                    .context("Failed to list GCP locations")
                    .unwrap_err());
            }

            let list_response: ListLocationsResponse = response
                .json()
                .await
                .context("Failed to deserialize locations list response")?;

            tracker.record_success("list_locations");
            Ok(list_response)
        }
        .instrument(span)
        .await
    }
}

#[async_trait]
impl ConfigStoreProvider for ParameterManagerREST {
    async fn create_or_update_config(&self, config_key: &str, config_value: &str) -> Result<bool> {
        let span = info_span!(
            "gcp.parameter.create_or_update",
            parameter.name = config_key,
            project.id = self.project_id()
        );
        let span_clone = span.clone();
        let project_id = self.project_id().to_string();
        let location = self.location.clone();
        let http_client = self.http_client().clone();
        let base_url = self.base_url().to_string();
        let access_token = self.access_token().to_string();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());
            let self_ref = ParameterManagerREST {
                http_client,
                base_url,
                project_id: project_id.clone(),
                location: location.clone(),
                access_token,
            };

            // Check if parameter exists
            let existing_parameter = self_ref.get_config_value(config_key).await?;

            // Create parameter if it doesn't exist
            if existing_parameter.is_none() {
                info!("Creating new GCP parameter: {}", config_key);

                let create_request = CreateParameterRequest::new(config_key.to_string());

                let response = self_ref
                    .make_request(
                        "POST",
                        &self_ref
                            .build_parameter_parent_path(GcpOperation::CreateParameter)
                            .context("Failed to build parameter parent path")?,
                        Some(serde_json::to_value(&create_request)?),
                    )
                    .send()
                    .await
                    .context("Failed to create parameter")?;

                if !response.status().is_success() {
                    let status = response.status();
                    let error_text = response.text().await.unwrap_or_default();
                    tracker.record_error(None, &error_text);
                    return Err(self_ref
                        .handle_error_response(status, error_text)
                        .context(format!("Failed to create GCP parameter: {}", config_key))
                        .unwrap_err());
                }
            }

            // Check if value changed
            let operation_type = match determine_operation_type(&existing_parameter, config_value) {
                Some("no_change") => {
                    tracker.record_no_change();
                    return Ok(false);
                }
                Some(op) => op,
                None => "update",
            };

            // Add new version with the value
            info!("Adding new version to GCP parameter: {}", config_key);
            // Parameter Manager allows user-provided version names
            // Use timestamp-based version ID for uniqueness
            let version_id = format!("v{}", chrono::Utc::now().timestamp());
            let add_version_request = CreateParameterVersionRequest::new(
                Some(version_id.clone()),
                config_value.to_string(),
            );

            // Create version using POST to /versions endpoint (not :addVersion)
            let response = self_ref
                .make_request(
                    "POST",
                    &self_ref
                        .build_parameter_versions_parent_path(
                            GcpOperation::CreateParameterVersion,
                            config_key,
                        )
                        .context("Failed to build parameter versions parent path")?,
                    Some(serde_json::to_value(&add_version_request)?),
                )
                .send()
                .await
                .context("Failed to add parameter version")?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracker.record_error(Some(operation_type), &error_text);
                return Err(self_ref
                    .handle_error_response(status, error_text)
                    .context(format!(
                        "Failed to add version to GCP parameter: {}",
                        config_key
                    ))
                    .unwrap_err());
            }

            // Parse response to get the created version ID (if we need it)
            let _create_response: responses::CreateParameterVersionResponse = response
                .json()
                .await
                .context("Failed to deserialize create version response")?;

            tracker.record_success(operation_type);
            Ok(true)
        }
        .instrument(span)
        .await
    }

    async fn get_config_value(&self, config_key: &str) -> Result<Option<String>> {
        let span = info_span!(
            "gcp.parameter.get",
            parameter.name = config_key,
            project.id = self.project_id()
        );
        let span_clone = span.clone();
        let project_id = self.project_id().to_string();
        let http_client = self.http_client().clone();
        let base_url = self.base_url().to_string();
        let access_token = self.access_token().to_string();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());
            let location = self.location.clone();
            let self_ref = ParameterManagerREST {
                http_client,
                base_url,
                project_id: project_id.clone(),
                location: location.clone(),
                access_token,
            };

            // List versions and get the latest one
            // First, list versions to find the latest
            let versions_path = self_ref
                .build_parameter_versions_parent_path(
                    GcpOperation::ListParameterVersions,
                    config_key,
                )
                .context("Failed to build parameter versions parent path")?;
            let response = self_ref
                .make_request("GET", &versions_path, None)
                .send()
                .await
                .context("Failed to list parameter versions")?;

            if response.status().as_u16() == 404 {
                tracker.record_success("get");
                return Ok(None);
            }

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracker.record_error(None, &error_text);
                return Err(self_ref
                    .handle_error_response(status, error_text)
                    .context(format!(
                        "Failed to list versions for GCP parameter: {}",
                        config_key
                    ))
                    .unwrap_err());
            }

            // Parse versions list and get the latest
            let versions_response: ListParameterVersionsResponse = response
                .json()
                .await
                .context("Failed to deserialize versions list")?;

            if versions_response.versions.is_empty() {
                return Ok(None);
            }

            // Find the latest version (highest createTime or last in list)
            let latest_version = versions_response
                .versions
                .iter()
                .max_by_key(|v| v.create_time.as_deref().unwrap_or(""))
                .ok_or_else(|| anyhow::anyhow!("No versions found"))?;

            // Extract version ID from name: projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}
            let version_id = latest_version
                .name
                .split('/')
                .last()
                .ok_or_else(|| anyhow::anyhow!("Invalid version name format"))?;

            // Get the version details
            let version_path = self_ref
                .build_parameter_version_path(
                    GcpOperation::GetParameterVersion,
                    config_key,
                    version_id,
                )
                .context("Failed to build parameter version path")?;
            let response = self_ref
                .make_request("GET", &version_path, None)
                .send()
                .await
                .context("Failed to get parameter version")?;

            if response.status().as_u16() == 404 {
                tracker.record_success("get");
                return Ok(None);
            }

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracker.record_error(None, &error_text);
                return Err(self_ref
                    .handle_error_response(status, error_text)
                    .context(format!(
                        "Failed to get GCP parameter version: {}",
                        config_key
                    ))
                    .unwrap_err());
            }

            let version_response: AccessParameterVersionResponse = response
                .json()
                .await
                .context("Failed to deserialize parameter version response")?;

            // Decode the base64-encoded payload
            let payload_data = general_purpose::STANDARD
                .decode(&version_response.payload.data)
                .context("Failed to decode parameter payload")?;
            let value = String::from_utf8(payload_data)
                .context("Failed to convert parameter payload to string")?;

            tracker.record_success("get");
            Ok(Some(value))
        }
        .instrument(span)
        .await
    }

    async fn delete_config(&self, config_key: &str) -> Result<()> {
        let span = info_span!(
            "gcp.parameter.delete",
            parameter.name = config_key,
            project.id = self.project_id()
        );
        let span_clone = span.clone();
        let project_id = self.project_id().to_string();
        let location = self.location.clone();
        let http_client = self.http_client().clone();
        let base_url = self.base_url().to_string();
        let access_token = self.access_token().to_string();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());
            let self_ref = ParameterManagerREST {
                http_client,
                base_url,
                project_id: project_id.clone(),
                location: location.clone(),
                access_token,
            };

            info!("Deleting GCP parameter: {}", config_key);
            let path = self_ref
                .build_parameter_path(GcpOperation::DeleteParameter, config_key)
                .context("Failed to build parameter path")?;

            let response = self_ref
                .make_request("DELETE", &path, None)
                .send()
                .await
                .context("Failed to delete parameter")?;

            if response.status().as_u16() == 404 {
                // Parameter doesn't exist - treat as success
                tracker.record_success("delete");
                return Ok(());
            }

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                tracker.record_error(Some("delete"), &error_text);
                return Err(self_ref
                    .handle_error_response(status, error_text)
                    .context(format!("Failed to delete GCP parameter: {}", config_key))
                    .unwrap_err());
            }

            tracker.record_success("delete");
            Ok(())
        }
        .instrument(span)
        .await
    }
}
