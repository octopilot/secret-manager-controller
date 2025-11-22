//! # CRUD Operations
//!
//! Implementation of SecretManagerProvider trait for GCP Secret Manager REST API.
//!
//! This module contains all the CRUD operations: create, read, update, delete,
//! enable, and disable secrets.

use crate::observability::metrics;
use crate::provider::SecretManagerProvider;
use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use std::time::Instant;
use tracing::{debug, info, info_span, Instrument};

use super::requests::{AddVersionRequest, CreateSecretRequest};
use super::responses::AccessSecretVersionResponse;
use crate::provider::gcp::client::common::{determine_operation_type, OperationTracker};
use crate::provider::gcp::client::rest::SecretManagerREST;
use paths::prelude::{GcpOperation, PathBuilder};

#[async_trait]
impl SecretManagerProvider for SecretManagerREST {
    async fn create_or_update_secret(&self, secret_name: &str, secret_value: &str) -> Result<bool> {
        let span = info_span!(
            "gcp.secret.create_or_update",
            secret.name = secret_name,
            project.id = self.project_id()
        );
        let span_clone = span.clone();
        let project_id = self.project_id().to_string();
        let http_client = self.http_client().clone();
        let base_url = self.base_url().to_string();
        let access_token = self.access_token().to_string();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());
            let self_ref = SecretManagerREST {
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

                let create_request = CreateSecretRequest::new(secret_name.to_string());

                let path = PathBuilder::new()
                    .gcp_operation(GcpOperation::CreateSecret)
                    .project(&self_ref.project_id)
                    .build_http_path()
                    .context("Failed to build create secret path")?;

                let response = self_ref
                    .make_request("POST", &path, Some(serde_json::to_value(&create_request)?))
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

            let add_version_request = AddVersionRequest::new(encoded);

            let path = PathBuilder::new()
                .gcp_operation(GcpOperation::AddVersion)
                .project(&self_ref.project_id)
                .secret(secret_name)
                .build_http_path()
                .context("Failed to build add version path")?;

            let response = self_ref
                .make_request(
                    "POST",
                    &path,
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
            project.id = self.project_id()
        );
        let span_clone = span.clone();
        let project_id = self.project_id().to_string();
        let http_client = self.http_client().clone();
        let base_url = self.base_url().to_string();
        let access_token = self.access_token().to_string();

        async move {
            let start = Instant::now();
            let self_ref = SecretManagerREST {
                http_client,
                base_url,
                project_id: project_id.clone(),
                access_token,
            };

            let version_path = PathBuilder::new()
                .gcp_operation(GcpOperation::AccessVersion)
                .project(&project_id)
                .secret(secret_name)
                .version("latest")
                .build_http_path()
                .context("Failed to build access version path")?;

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

        let secret_path = PathBuilder::new()
            .gcp_operation(GcpOperation::DeleteSecret)
            .project(self.project_id())
            .secret(secret_name)
            .build_http_path()
            .context("Failed to build delete secret path")?;

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

    async fn disable_secret(&self, secret_name: &str) -> Result<bool> {
        info!("Disabling GCP secret: {}", secret_name);

        // Use PathBuilder for type-safe path construction
        let path = PathBuilder::new()
            .gcp_operation(GcpOperation::DisableSecret)
            .project(self.project_id())
            .secret(secret_name)
            .build_http_path()
            .context("Failed to build disable secret path")?;
        let response = self
            .make_request("POST", &path, None)
            .send()
            .await
            .context("Failed to disable secret")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            // If secret doesn't exist, return false (not an error)
            if status == 404 {
                debug!("Secret {} does not exist, cannot disable", secret_name);
                return Ok(false);
            }

            self.handle_error_response(status, error_text)
                .context(format!("Failed to disable GCP secret: {}", secret_name))?;
            unreachable!()
        }

        Ok(true)
    }

    async fn enable_secret(&self, secret_name: &str) -> Result<bool> {
        info!("Enabling GCP secret: {}", secret_name);

        // Use PathBuilder for type-safe path construction
        let path = PathBuilder::new()
            .gcp_operation(GcpOperation::EnableSecret)
            .project(self.project_id())
            .secret(secret_name)
            .build_http_path()
            .context("Failed to build enable secret path")?;
        let response = self
            .make_request("POST", &path, None)
            .send()
            .await
            .context("Failed to enable secret")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            // If secret doesn't exist, return false (not an error)
            if status == 404 {
                debug!("Secret {} does not exist, cannot enable", secret_name);
                return Ok(false);
            }

            self.handle_error_response(status, error_text)
                .context(format!("Failed to enable GCP secret: {}", secret_name))?;
            unreachable!()
        }

        Ok(true)
    }
}
