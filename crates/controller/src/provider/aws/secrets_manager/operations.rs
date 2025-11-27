//! # AWS Secrets Manager Operations
//!
//! Implements SecretManagerProvider trait for AWS Secrets Manager.

use crate::observability::metrics;
use crate::provider::SecretManagerProvider;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::time::Instant;
use tracing::{debug, info, info_span, warn, Instrument};

use super::AwsSecretManager;

#[async_trait]
impl SecretManagerProvider for AwsSecretManager {
    async fn create_or_update_secret(
        &self,
        secret_name: &str,
        secret_value: &str,
        environment: &str,
        location: &str,
    ) -> Result<bool> {
        let span = info_span!(
            "aws.secret.create_or_update",
            secret.name = secret_name,
            region = self._region
        );
        let span_clone = span.clone();
        let start = Instant::now();

        async move {
            // Check if secret exists
            let secret_exists = self
                .client
                .describe_secret()
                .secret_id(secret_name)
                .send()
                .await
                .is_ok();

            let operation_type = if !secret_exists {
                // Create secret
                info!(
                    provider = "aws",
                    region = self._region,
                    secret_name = secret_name,
                    operation = "create",
                    "Creating AWS secret: region={}, secret={}",
                    self._region,
                    secret_name
                );
                // In Pact mode, use a fixed ClientRequestToken for deterministic testing
                let mut create_request = self
                    .client
                    .create_secret()
                    .name(secret_name)
                    .secret_string(secret_value)
                    .tags(
                        aws_sdk_secretsmanager::types::Tag::builder()
                            .key("environment")
                            .value(environment)
                            .build(),
                    )
                    .tags(
                        aws_sdk_secretsmanager::types::Tag::builder()
                            .key("location")
                            .value(location)
                            .build(),
                    );

                if std::env::var("PACT_MODE").is_ok() {
                    // Use a fixed UUID for Pact testing to ensure request body matches
                    create_request =
                        create_request.client_request_token("00000000-0000-0000-0000-000000000000");
                }

                match create_request.send().await {
                    Ok(_) => {
                        metrics::record_secret_operation(
                            "aws",
                            "create",
                            start.elapsed().as_secs_f64(),
                        );
                        span_clone.record("operation.type", "create");
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        span_clone.record("operation.success", true);
                        return Ok(true);
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        span_clone.record("operation.success", false);
                        span_clone.record("operation.type", "create");
                        span_clone.record("error.message", error_msg.clone());
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::increment_provider_operation_errors("aws");
                        // Log detailed error information for debugging
                        let error_details = format!("{:?}", e);
                        warn!(
                            provider = "aws",
                            region = self._region,
                            secret_name = secret_name,
                            operation = "create",
                            error = %e,
                            error_details = %error_details,
                            "Failed to create AWS secret: {}",
                            e
                        );
                        return Err(anyhow::anyhow!(
                            "Failed to create AWS secret {secret_name}: {e}"
                        ));
                    }
                }
            } else {
                // Get current secret value
                let current_value = self.get_secret_value(secret_name).await?;

                if let Some(current) = current_value {
                    if current == secret_value {
                        debug!(
                            provider = "aws",
                            region = self._region,
                            secret_name = secret_name,
                            operation = "no_change",
                            "AWS secret {} unchanged, skipping update",
                            secret_name
                        );
                        metrics::record_secret_operation(
                            "aws",
                            "no_change",
                            start.elapsed().as_secs_f64(),
                        );
                        span_clone.record("operation.type", "no_change");
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        span_clone.record("operation.success", true);
                        return Ok(false);
                    }
                }

                // Update secret (creates new version automatically)
                info!(
                    provider = "aws",
                    region = self._region,
                    secret_name = secret_name,
                    operation = "update",
                    "Updating AWS secret: region={}, secret={}",
                    self._region,
                    secret_name
                );
                // In Pact mode, use a fixed ClientRequestToken for deterministic testing
                let mut put_request = self
                    .client
                    .put_secret_value()
                    .secret_id(secret_name)
                    .secret_string(secret_value);

                if std::env::var("PACT_MODE").is_ok() {
                    // Use a fixed UUID for Pact testing to ensure request body matches
                    put_request =
                        put_request.client_request_token("00000000-0000-0000-0000-000000000000");
                }

                match put_request.send().await {
                    Ok(_) => {
                        metrics::record_secret_operation(
                            "aws",
                            "update",
                            start.elapsed().as_secs_f64(),
                        );
                        "update"
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        span_clone.record("operation.success", false);
                        span_clone.record("operation.type", "update");
                        span_clone.record("error.message", error_msg.clone());
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::increment_provider_operation_errors("aws");
                        return Err(anyhow::anyhow!(
                            "Failed to update AWS secret {secret_name}: {e}"
                        ));
                    }
                }
            };

            span_clone.record("operation.type", operation_type);
            span_clone.record("operation.duration_ms", start.elapsed().as_millis() as u64);
            span_clone.record("operation.success", true);
            Ok(true)
        }
        .instrument(span)
        .await
    }

    async fn get_secret_value(&self, secret_name: &str) -> Result<Option<String>> {
        let span = tracing::debug_span!(
            "aws.secret.get",
            secret.name = secret_name,
            region = self._region
        );
        let span_clone = span.clone();
        let start = Instant::now();

        async move {
            match self
                .client
                .get_secret_value()
                .secret_id(secret_name)
                .send()
                .await
            {
                Ok(response) => {
                    let value = response
                        .secret_string()
                        .map(ToString::to_string)
                        .or_else(|| {
                            response
                                .secret_binary()
                                .map(|blob| String::from_utf8_lossy(blob.as_ref()).to_string())
                        });

                    match value {
                        Some(v) => {
                            span_clone.record("operation.success", true);
                            span_clone.record("operation.found", true);
                            span_clone.record(
                                "operation.duration_ms",
                                start.elapsed().as_millis() as u64,
                            );
                            metrics::record_secret_operation(
                                "aws",
                                "get",
                                start.elapsed().as_secs_f64(),
                            );
                            Ok(Some(v))
                        }
                        None => {
                            span_clone.record("operation.success", false);
                            span_clone
                                .record("error.message", "Secret has no string or binary value");
                            span_clone.record(
                                "operation.duration_ms",
                                start.elapsed().as_millis() as u64,
                            );
                            metrics::increment_provider_operation_errors("aws");
                            Err(anyhow::anyhow!("Secret has no string or binary value"))
                        }
                    }
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    if error_msg.contains("ResourceNotFoundException") {
                        span_clone.record("operation.success", true);
                        span_clone.record("operation.found", false);
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::record_secret_operation(
                            "aws",
                            "get",
                            start.elapsed().as_secs_f64(),
                        );
                        Ok(None)
                    } else {
                        span_clone.record("operation.success", false);
                        span_clone.record("error.message", error_msg.clone());
                        span_clone
                            .record("operation.duration_ms", start.elapsed().as_millis() as u64);
                        metrics::increment_provider_operation_errors("aws");
                        Err(anyhow::anyhow!("Failed to get AWS secret: {e}"))
                    }
                }
            }
        }
        .instrument(span)
        .await
    }

    async fn delete_secret(&self, secret_name: &str) -> Result<()> {
        info!("Deleting AWS secret: {}", secret_name);
        self.client
            .delete_secret()
            .secret_id(secret_name)
            .force_delete_without_recovery(true)
            .send()
            .await
            .context("Failed to delete AWS secret")?;
        Ok(())
    }

    async fn disable_secret(&self, secret_name: &str) -> Result<bool> {
        info!("Disabling AWS secret: {}", secret_name);

        // AWS doesn't have a direct "disable" operation, but we can mark it for deletion
        // with a recovery window, which effectively disables it. However, for our use case,
        // we'll use a different approach: we'll update the secret to mark it as deleted
        // but with a long recovery window, which makes it inaccessible but recoverable.

        // Check if secret exists
        let secret_exists = self
            .client
            .describe_secret()
            .secret_id(secret_name)
            .send()
            .await
            .is_ok();

        if !secret_exists {
            debug!("Secret {} does not exist, cannot disable", secret_name);
            return Ok(false);
        }

        // AWS uses DeleteSecret with recovery window to "disable" a secret
        // For our purposes, we'll use a very long recovery window (7 days default)
        // This makes the secret inaccessible but not permanently deleted
        match self
            .client
            .delete_secret()
            .secret_id(secret_name)
            .recovery_window_in_days(7) // 7 days recovery window
            .send()
            .await
        {
            Ok(_) => {
                info!("Marked AWS secret {} for deletion (disabled)", secret_name);
                Ok(true)
            }
            Err(e) => {
                let error_msg = e.to_string();
                // If already deleted/disabled, return false
                if error_msg.contains("not found") || error_msg.contains("InvalidRequestException")
                {
                    Ok(false)
                } else {
                    Err(anyhow::anyhow!(
                        "Failed to disable AWS secret {secret_name}: {e}"
                    ))
                }
            }
        }
    }

    async fn enable_secret(&self, secret_name: &str) -> Result<bool> {
        info!("Enabling AWS secret: {}", secret_name);

        // AWS uses RestoreSecret to re-enable a deleted secret
        match self
            .client
            .restore_secret()
            .secret_id(secret_name)
            .send()
            .await
        {
            Ok(_) => {
                info!("Restored AWS secret {} (enabled)", secret_name);
                Ok(true)
            }
            Err(e) => {
                let error_msg = e.to_string();
                // Log detailed error information for debugging
                let error_details = format!("{:?}", e);
                warn!(
                    provider = "aws",
                    region = self._region,
                    secret_name = secret_name,
                    operation = "enable",
                    error = %e,
                    error_details = %error_details,
                    "Failed to enable AWS secret: {}",
                    e
                );
                // If secret doesn't exist or is already enabled, return false
                if error_msg.contains("not found") || error_msg.contains("InvalidRequestException")
                {
                    debug!(
                        "Secret {} does not exist or is already enabled",
                        secret_name
                    );
                    Ok(false)
                } else {
                    Err(anyhow::anyhow!(
                        "Failed to enable AWS secret {secret_name}: {e}"
                    ))
                }
            }
        }
    }
}
