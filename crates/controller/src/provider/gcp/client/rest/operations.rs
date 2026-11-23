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
use base64::{Engine as _, engine::general_purpose};
use std::time::Instant;
use tracing::{Instrument, debug, info, info_span, warn};

use super::requests::{AddVersionRequest, CreateSecretRequest};
use super::responses::{
    AccessSecretVersionResponse, ListSecretVersionsResponse, SecretVersionMetadata,
};
use crate::provider::gcp::client::common::{OperationTracker, determine_operation_type};
use crate::provider::gcp::client::rest::SecretManagerREST;
use smc_paths::prelude::{GcpOperation, PathBuilder};

#[async_trait]
impl SecretManagerProvider for SecretManagerREST {
    async fn create_or_update_secret(
        &self,
        secret_name: &str,
        secret_value: &str,
        environment: &str,
        location: &str,
    ) -> Result<bool> {
        let span = info_span!(
            "gcp.secret.create_or_update",
            secret.name = secret_name,
            project.id = self.project_id(),
            provider = "gcp",
            operation = "create_or_update_secret"
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
                info!(
                    provider = "gcp",
                    project = self_ref.project_id,
                    secret_name = secret_name,
                    operation = "create_secret",
                    "Creating new GCP secret: project={}, secret={}",
                    self_ref.project_id,
                    secret_name
                );

                let create_request = CreateSecretRequest::new(
                    secret_name.to_string(),
                    environment.to_string(),
                    location.to_string(),
                );

                let path = PathBuilder::new()
                    .gcp_operation(GcpOperation::CreateSecret)
                    .project(&self_ref.project_id)
                    .build_http_path()
                    .context("Failed to build create secret path")?;

                info!(
                    provider = "gcp",
                    project = self_ref.project_id,
                    secret_name = secret_name,
                    operation = "create_secret",
                    path = path,
                    "Building create_secret path: project={}, secret={}, path={}",
                    self_ref.project_id,
                    secret_name,
                    path
                );

                // secretId is a query parameter of projects.secrets.create; the body is the Secret.
                let path = format!("{path}?secretId={}", create_request.secret_id);
                let response = self_ref
                    .make_request("POST", &path, Some(serde_json::to_value(&create_request)?))
                    .send()
                    .await
                    .context("Failed to create secret")?;

                // 409: the secret exists but has no readable latest version (none yet, or the
                // latest was destroyed). Adding a version below is what is wanted.
                if response.status() == reqwest::StatusCode::CONFLICT {
                    debug!(
                        "GCP secret {} already exists without a readable version; adding one",
                        secret_name
                    );
                } else if !response.status().is_success() {
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
                    self_ref.prune_versions_logged(secret_name).await;
                    return Ok(false);
                }
                Some(op_type) => {
                    if op_type == "update" {
                        info!(
                            provider = "gcp",
                            project = self_ref.project_id,
                            secret_name = secret_name,
                            operation = "update",
                            "Secret value changed, updating GCP secret: project={}, secret={}",
                            self_ref.project_id,
                            secret_name
                        );
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

            info!(
                provider = "gcp",
                project = self_ref.project_id,
                secret_name = secret_name,
                operation = "add_version",
                path = path,
                method = "POST",
                "Building add_version path: project={}, secret={}, path={}",
                self_ref.project_id,
                secret_name,
                path
            );

            let response = self_ref
                .make_request(
                    "POST",
                    &path,
                    Some(serde_json::to_value(&add_version_request)?),
                )
                .send()
                .await
                .context("Failed to add secret version")?;

            info!(
                provider = "gcp",
                project = self_ref.project_id,
                secret_name = secret_name,
                operation = "add_version",
                status_code = response.status().as_u16(),
                "add_version response: project={}, secret={}, status={}",
                self_ref.project_id,
                secret_name,
                response.status()
            );

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
            self_ref.prune_versions_logged(secret_name).await;
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
                    // Latest version disabled or destroyed: there is no readable value.
                    if status == reqwest::StatusCode::BAD_REQUEST
                        && error_text.contains("FAILED_PRECONDITION")
                    {
                        span_clone.record("operation.success", true);
                        span_clone.record("operation.found", false);
                        debug!(
                            "Latest version of GCP secret {} is not enabled; treating as no value",
                            secret_name
                        );
                        return Ok(None);
                    }
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
        self.set_latest_version_state(secret_name, false).await
    }

    async fn enable_secret(&self, secret_name: &str) -> Result<bool> {
        self.set_latest_version_state(secret_name, true).await
    }
}

/// How many versions of each secret to keep: `GCP_SECRET_VERSIONS_TO_KEEP`, default 2 (the
/// current value and the previous one, which covers a rollover still in flight). Every
/// enabled or disabled version is billed as active; the values come from Git, which keeps
/// their history. 0 keeps every version.
fn versions_to_keep() -> usize {
    std::env::var("GCP_SECRET_VERSIONS_TO_KEEP")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(2)
}

impl SecretManagerREST {
    /// [`Self::prune_versions`] with [`versions_to_keep`], logging rather than failing: a
    /// version left behind costs a few cents and is pruned on the next reconcile.
    async fn prune_versions_logged(&self, secret_name: &str) {
        let keep = versions_to_keep();
        if keep == 0 {
            return;
        }
        if let Err(e) = self.prune_versions(secret_name, keep).await {
            warn!(
                "GCP secret {}: could not prune old versions: {:#}",
                secret_name, e
            );
        }
    }

    /// Destroy all but the newest `keep` versions of a secret that are not destroyed already.
    /// Destroying is irreversible. Returns how many versions were destroyed.
    pub(crate) async fn prune_versions(&self, secret_name: &str, keep: usize) -> Result<usize> {
        let list_path = PathBuilder::new()
            .gcp_operation(GcpOperation::ListVersions)
            .project(self.project_id())
            .secret(secret_name)
            .build_http_path()
            .context("Failed to build list versions path")?;

        let mut live: Vec<(u64, String)> = Vec::new();
        let mut page_token: Option<String> = None;
        loop {
            let mut path = format!("{list_path}?pageSize=100");
            if let Some(token) = &page_token {
                path.push_str(&format!("&pageToken={token}"));
            }
            let response = self
                .make_request("GET", &path, None)
                .send()
                .await
                .context("Failed to list secret versions")?;
            let status = response.status();
            if status == reqwest::StatusCode::NOT_FOUND {
                return Ok(0);
            }
            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_default();
                self.handle_error_response(status, error_text)
                    .context(format!(
                        "Failed to list versions of GCP secret: {secret_name}"
                    ))?;
                unreachable!()
            }
            let page: ListSecretVersionsResponse = response
                .json()
                .await
                .context("Failed to parse secret versions list")?;
            for version in page.versions {
                if version.state == "DESTROYED" {
                    continue;
                }
                if let Some(id) = version.version_id().and_then(|id| id.parse::<u64>().ok()) {
                    live.push((id, version.state));
                }
            }
            match page.next_page_token.filter(|t| !t.is_empty()) {
                Some(token) => page_token = Some(token),
                None => break,
            }
        }

        // Version ids only grow: the newest are the highest.
        live.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        let mut destroyed = 0;
        for (id, state) in live.into_iter().skip(keep) {
            let id = id.to_string();
            let path = PathBuilder::new()
                .gcp_operation(GcpOperation::DestroyVersion)
                .project(self.project_id())
                .secret(secret_name)
                .version(&id)
                .build_http_path()
                .context("Failed to build destroy version path")?;
            let response = self
                .make_request("POST", &path, Some(serde_json::json!({})))
                .send()
                .await
                .context("Failed to destroy secret version")?;
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                self.handle_error_response(status, error_text)
                    .context(format!(
                        "Failed to destroy version {id} of GCP secret: {secret_name}"
                    ))?;
                unreachable!()
            }
            debug!(
                "GCP secret {}: destroyed version {} ({})",
                secret_name, id, state
            );
            destroyed += 1;
        }
        if destroyed > 0 {
            info!(
                "GCP secret {}: destroyed {} old version(s), kept the newest {}",
                secret_name, destroyed, keep
            );
        }
        Ok(destroyed)
    }

    /// Metadata of the latest version of a secret; `None` when the secret or a version
    /// does not exist.
    async fn latest_version(&self, secret_name: &str) -> Result<Option<SecretVersionMetadata>> {
        let path = PathBuilder::new()
            .gcp_operation(GcpOperation::GetVersion)
            .project(self.project_id())
            .secret(secret_name)
            .version("latest")
            .build_http_path()
            .context("Failed to build get version path")?;
        let response = self
            .make_request("GET", &path, None)
            .send()
            .await
            .context("Failed to get latest secret version")?;

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            self.handle_error_response(status, error_text)
                .context(format!(
                    "Failed to get latest version of GCP secret: {secret_name}"
                ))?;
            unreachable!()
        }
        let version: SecretVersionMetadata = response
            .json()
            .await
            .context("Failed to parse secret version metadata")?;
        Ok(Some(version))
    }

    /// Enable or disable the latest version of a secret. Secret Manager has no
    /// secret-level enable or disable: the state belongs to each version.
    ///
    /// Returns `true` when the state was changed, `false` when the secret has no
    /// version, the version is already in the wanted state, or it is destroyed.
    async fn set_latest_version_state(&self, secret_name: &str, enable: bool) -> Result<bool> {
        let Some(version) = self.latest_version(secret_name).await? else {
            debug!("GCP secret {} has no version to change", secret_name);
            return Ok(false);
        };
        let (wanted_from, operation, verb) = if enable {
            ("DISABLED", GcpOperation::EnableVersion, "enable")
        } else {
            ("ENABLED", GcpOperation::DisableVersion, "disable")
        };
        if version.state != wanted_from {
            return Ok(false);
        }
        let version_id = version.version_id().ok_or_else(|| {
            anyhow::anyhow!("GCP returned a version without an id: {}", version.name)
        })?;

        info!(
            "GCP secret {}: {} version {}",
            secret_name, verb, version_id
        );
        let path = PathBuilder::new()
            .gcp_operation(operation)
            .project(self.project_id())
            .secret(secret_name)
            .version(version_id)
            .build_http_path()
            .context("Failed to build version state path")?;
        let response = self
            .make_request("POST", &path, Some(serde_json::json!({})))
            .send()
            .await
            .with_context(|| format!("Failed to {verb} secret version"))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            self.handle_error_response(status, error_text)
                .context(format!(
                    "Failed to {verb} version {version_id} of GCP secret: {secret_name}"
                ))?;
            unreachable!()
        }
        Ok(true)
    }
}
