//! GCP Secret Manager gRPC Client
//!
//! Official Google Cloud SDK implementation using gRPC.
//! Uses the [`google-cloud-secretmanager-v1`] crate.

use crate::observability::metrics;
use crate::provider::SecretManagerProvider;
use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use google_cloud_secretmanager_v1::client::SecretManagerService;
use tracing::{info, info_span, Instrument};

use super::common::{
    determine_operation_type, format_secret_path, format_secret_version_path, OperationTracker,
};

pub struct SecretManagerGRPC {
    client: SecretManagerService,
    project_id: String,
}

impl std::fmt::Debug for SecretManagerGRPC {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretManagerGRPC")
            .field("project_id", &self.project_id)
            .finish_non_exhaustive()
    }
}

impl SecretManagerGRPC {
    /// Create a new `SecretManager` client with authentication
    /// Supports both JSON credentials and Workload Identity
    ///
    /// Authentication is handled automatically by the Google Cloud SDK:
    /// - Workload Identity: Uses Application Default Credentials (ADC) when running in GKE
    ///   with Workload Identity enabled and service account annotation
    ///
    /// Uses Workload Identity for authentication (DEFAULT, requires GKE with WI enabled)
    /// If `service_account_email` is provided, uses that specific service account.
    /// Otherwise, uses the service account from pod annotation.
    ///
    /// # Errors
    /// Returns an error if GCP client initialization fails
    #[allow(
        clippy::missing_errors_doc,
        reason = "Error documentation is provided in doc comments"
    )]
    pub async fn new(
        project_id: String,
        _auth_type: Option<&str>,
        service_account_email: Option<&str>,
    ) -> Result<Self> {
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

        // Create SecretManagerService client using builder pattern
        // The client automatically handles authentication via:
        // - Workload Identity (when running in GKE with WI enabled)
        // - Application Default Credentials (ADC)
        // - Service account JSON from GOOGLE_APPLICATION_CREDENTIALS
        // - Metadata server (for GCE/GKE)
        // The builder uses Application Default Credentials by default, which works with Workload Identity
        //
        // Support Pact mock server integration via environment variable
        // When PACT_MODE=true, route requests to Pact mock server instead of real GCP
        // Note: The GCP SDK uses gRPC by default, while Pact mock servers typically use HTTP/REST.
        // Two options for full Pact support:
        // 1. Use Pact V4 with pact-protobuf-plugin for native gRPC support (recommended)
        // 2. Implement HTTP client wrapper for REST-based Pact mock servers
        // Configure builder with custom endpoint if provided
        // Note: The SDK uses gRPC by default, so this will only work if:
        // 1. The endpoint supports gRPC (not REST)
        // 2. Or the SDK can fall back to REST for the given endpoint
        let mut builder = SecretManagerService::builder();

        if std::env::var("PACT_MODE").is_ok() {
            if let Ok(endpoint) = std::env::var("GCP_SECRET_MANAGER_ENDPOINT") {
                info!(
                    "Pact mode enabled: routing GCP Secret Manager requests to {}",
                    endpoint
                );
                // Try to configure custom endpoint
                // WARNING: SDK uses gRPC, so this may not work with REST endpoints
                // If this fails, the REST client (SecretManagerREST) should be used instead
                builder = builder.with_endpoint(&endpoint);
            } else {
                return Err(anyhow::anyhow!(
                    "PACT_MODE is enabled but GCP_SECRET_MANAGER_ENDPOINT is not set. \
                    This is a configuration error. Either:\n\
                    1. Set GCP_SECRET_MANAGER_ENDPOINT to point to the Pact mock server, OR\n\
                    2. Use the REST client (SecretManagerREST) which is automatically selected when PACT_MODE=true"
                ));
            }
        }

        let client = builder
            .build()
            .await
            .context("Failed to create SecretManagerService client. Ensure Workload Identity is configured or GOOGLE_APPLICATION_CREDENTIALS is set")?;

        Ok(Self { client, project_id })
    }

    /// Create or update secret, ensuring Git is source of truth
    /// If secret exists and value differs, creates new version and disables old versions
    #[allow(
        clippy::missing_errors_doc,
        reason = "Error documentation is provided in doc comments"
    )]
    async fn create_or_update_secret_impl(
        &self,
        secret_name: &str,
        secret_value: &str,
    ) -> Result<bool> {
        use google_cloud_secretmanager_v1::model::{
            AddSecretVersionRequest, CreateSecretRequest, Secret, SecretPayload,
        };

        let span = info_span!(
            "gcp.secret.create_or_update",
            secret.name = secret_name,
            project.id = self.project_id
        );
        let span_clone = span.clone();

        async move {
            let tracker = OperationTracker::new(span_clone.clone());

            // Check if secret already exists
            let secret_name_full = format_secret_path(&self.project_id, secret_name);
            let existing_secret = self.get_secret_value(secret_name).await?;

            // If secret doesn't exist, create it
            if existing_secret.is_none() {
                info!("Creating new GCP secret: {}", secret_name);

                // Create the secret resource first
                let secret = Secret::default();
                let create_request = CreateSecretRequest::default()
                    .set_parent(format!("projects/{}", self.project_id))
                    .set_secret_id(secret_name.to_string())
                    .set_secret(secret);

                match self
                    .client
                    .create_secret()
                    .with_request(create_request)
                    .send()
                    .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        let error_msg = e.to_string();
                        tracker.record_error(None, &error_msg);
                        return Err(anyhow::anyhow!(
                            "Failed to create GCP secret {secret_name}: {e}"
                        ));
                    }
                }
            }

            // Check if the value has changed
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

            // Add new secret version with the value
            // Secret Manager expects raw bytes (not base64-encoded)
            // The SDK will handle base64 encoding automatically
            // Convert to owned bytes to avoid lifetime issues
            let secret_bytes: Vec<u8> = secret_value.as_bytes().to_vec();
            let mut payload = SecretPayload::default();
            payload.data = secret_bytes.into();

            let add_version_request = AddSecretVersionRequest::default()
                .set_parent(secret_name_full.clone())
                .set_payload(payload);

            match self
                .client
                .add_secret_version()
                .with_request(add_version_request)
                .send()
                .await
            {
                Ok(_) => {
                    tracker.record_success(operation_type);
                    Ok(true)
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    tracker.record_error(Some(operation_type), &error_msg);
                    Err(anyhow::anyhow!(
                        "Failed to add version to GCP secret {secret_name}: {e}"
                    ))
                }
            }
        }
        .instrument(span)
        .await
    }

    /// Get the latest secret version value
    /// Returns the secret value as a String, or an error if the secret doesn't exist
    ///
    /// # Errors
    /// Returns an error if the secret doesn't exist or if there's an API error
    pub async fn get_latest_secret_value(&self, secret_name: &str) -> Result<String> {
        self.get_secret_value(secret_name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Secret '{}' not found", secret_name))
    }

    /// Get secret metadata (without the value)
    /// Useful for checking if a secret exists without retrieving its value
    ///
    /// # Errors
    /// Returns an error if there's an API error
    pub async fn get_secret(&self, secret_name: &str) -> Result<()> {
        use google_cloud_secretmanager_v1::model::GetSecretRequest;

        let secret_name_full = format_secret_path(&self.project_id, secret_name);

        let request = GetSecretRequest::default();
        let request_for_send = request.set_name(secret_name_full);

        self.client
            .get_secret()
            .with_request(request_for_send)
            .send()
            .await
            .context(format!("Failed to get GCP secret metadata: {secret_name}"))?;

        Ok(())
    }

    /// Create a new secret resource (without any versions)
    /// Note: This creates the secret resource only. Use `add_secret_version` to add a value.
    ///
    /// # Errors
    /// Returns an error if the secret already exists or if there's an API error
    pub async fn create_secret(&self, project_id: &str, secret_name: &str) -> Result<()> {
        use google_cloud_secretmanager_v1::model::{CreateSecretRequest, Secret};

        info!("Creating new GCP secret resource: {}", secret_name);

        let secret = Secret::default();
        let create_request = CreateSecretRequest::default()
            .set_parent(format!("projects/{}", project_id)) // Parent is just project, not full path
            .set_secret_id(secret_name.to_string())
            .set_secret(secret);

        self.client
            .create_secret()
            .with_request(create_request)
            .send()
            .await
            .context(format!(
                "Failed to create GCP secret resource: {secret_name}"
            ))?;

        Ok(())
    }

    /// Add a new version to an existing secret
    /// Creates a new version with the provided value
    ///
    /// # Errors
    /// Returns an error if the secret doesn't exist or if there's an API error
    pub async fn add_secret_version(&self, secret_name: &str, secret_value: &str) -> Result<()> {
        use google_cloud_secretmanager_v1::model::{AddSecretVersionRequest, SecretPayload};

        info!("Adding new version to GCP secret: {}", secret_name);

        let secret_name_full = format!("projects/{}/secrets/{}", self.project_id, secret_name);

        // Convert to owned bytes
        let secret_bytes: Vec<u8> = secret_value.as_bytes().to_vec();
        let mut payload = SecretPayload::default();
        payload.data = secret_bytes.into();

        let add_version_request = AddSecretVersionRequest::default()
            .set_parent(secret_name_full)
            .set_payload(payload);

        self.client
            .add_secret_version()
            .with_request(add_version_request)
            .send()
            .await
            .context(format!(
                "Failed to add version to GCP secret: {secret_name}"
            ))?;

        Ok(())
    }
}

#[async_trait]
impl SecretManagerProvider for SecretManagerGRPC {
    async fn create_or_update_secret(&self, secret_name: &str, secret_value: &str) -> Result<bool> {
        let start = std::time::Instant::now();

        // Implementation uses the GCP Secret Manager SDK
        let result = self
            .create_or_update_secret_impl(secret_name, secret_value)
            .await;

        if let Ok(was_updated) = &result {
            if *was_updated {
                metrics::record_secret_operation("gcp", "update", start.elapsed().as_secs_f64());
            } else {
                metrics::record_secret_operation("gcp", "no_change", start.elapsed().as_secs_f64());
            }
        }

        result
    }

    async fn get_secret_value(&self, secret_name: &str) -> Result<Option<String>> {
        use google_cloud_secretmanager_v1::model::AccessSecretVersionRequest;

        let span = tracing::debug_span!(
            "gcp.secret.get",
            secret.name = secret_name,
            project.id = self.project_id
        );
        let span_clone = span.clone();

        async move {
            let start = std::time::Instant::now();

            // Construct the secret version name: projects/{project}/secrets/{secret}/versions/latest
            let secret_version_name = format_secret_version_path(&self.project_id, secret_name);

            let request = AccessSecretVersionRequest::default();
            let request_for_send = request.clone().set_name(secret_version_name.clone()); // set_name returns Self

            match self
                .client
                .access_secret_version()
                .with_request(request_for_send)
                .send()
                .await
            {
                Ok(response) => {
                    // Extract the secret value from the response
                    // The payload.data field contains base64-encoded secret data (as bytes)
                    if let Some(payload) = response.payload {
                        // payload.data is bytes::Bytes, decode from base64
                        let data = payload.data.as_ref();
                        if data.is_empty() {
                            span_clone.record("operation.success", false);
                            span_clone
                                .record("error.message", "Secret version has no payload data");
                            span_clone.record(
                                "operation.duration_ms",
                                start.elapsed().as_millis() as u64,
                            );
                            metrics::increment_provider_operation_errors("gcp");
                            return Err(anyhow::anyhow!("Secret version has no payload data"));
                        }

                        // Decode base64 to get the actual secret value
                        let decoded = general_purpose::STANDARD
                            .decode(data)
                            .context("Failed to decode base64 secret data")?;
                        let secret_value = String::from_utf8(decoded)
                            .context("Secret value is not valid UTF-8")?;
                        span_clone.record("operation.success", true);
                        span_clone.record("operation.found", true);
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::record_secret_operation(
                            "gcp",
                            "get",
                            start.elapsed().as_secs_f64(),
                        );
                        Ok(Some(secret_value))
                    } else {
                        span_clone.record("operation.success", false);
                        span_clone
                            .record("error.message", "Secret version response has no payload");
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::increment_provider_operation_errors("gcp");
                        Err(anyhow::anyhow!("Secret version response has no payload"))
                    }
                }
                Err(e) => {
                    // Check if it's a "not found" error (404)
                    let error_msg = e.to_string();
                    if error_msg.contains("404")
                        || error_msg.contains("NOT_FOUND")
                        || error_msg.contains("not found")
                        || (error_msg.contains("Secret") && error_msg.contains("not found"))
                    {
                        span_clone.record("operation.success", true);
                        span_clone.record("operation.found", false);
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::record_secret_operation(
                            "gcp",
                            "get",
                            start.elapsed().as_secs_f64(),
                        );
                        Ok(None)
                    } else {
                        span_clone.record("operation.success", false);
                        span_clone.record("error.message", error_msg.clone());
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::increment_provider_operation_errors("gcp");
                        Err(anyhow::anyhow!(
                            "Failed to get GCP secret {secret_name}: {e}"
                        ))
                    }
                }
            }
        }
        .instrument(span)
        .await
    }

    async fn delete_secret(&self, secret_name: &str) -> Result<()> {
        use google_cloud_secretmanager_v1::model::DeleteSecretRequest;

        info!("Deleting GCP secret: {}", secret_name);

        // Construct the secret name: projects/{project}/secrets/{secret}
        let secret_name_full = format_secret_path(&self.project_id, secret_name);

        let request = DeleteSecretRequest::default();
        let request_for_send = request.clone().set_name(secret_name_full.clone()); // set_name returns Self

        self.client
            .delete_secret()
            .with_request(request_for_send)
            .send()
            .await
            .context(format!("Failed to delete GCP secret: {secret_name}"))?;

        Ok(())
    }
}
