//! # GCP Secret Manager PACT_MODE API Override
//!
//! Implements PactModeAPIOverride for GCP Secret Manager to route requests to Pact mock servers.

use crate::config::{PactModeAPIOverride, ProviderId, ProviderPactConfig};
use anyhow::Result;
use tracing::info;

pub struct GcpSecretManagerAPIOverride;

impl PactModeAPIOverride for GcpSecretManagerAPIOverride {
    fn provider_id(&self) -> ProviderId {
        ProviderId::GcpSecretManager
    }

    fn get_pact_config(&self) -> Result<ProviderPactConfig> {
        let config = crate::config::PactModeConfig::get();

        config
            .get_provider(&ProviderId::GcpSecretManager)
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "PACT_MODE enabled but GCP Secret Manager endpoint not configured. \
                    Set GCP_SECRET_MANAGER_ENDPOINT environment variable."
                )
            })
    }

    fn override_api_endpoint(&self) -> Result<()> {
        let pact_config = self.get_pact_config()?;

        // Check if env var is already set (from export command or parent process)
        if std::env::var("GCP_SECRET_MANAGER_ENDPOINT").is_ok() {
            // Already set via export - just validate
            let endpoint = std::env::var("GCP_SECRET_MANAGER_ENDPOINT")?;
            self.validate_endpoint(&endpoint)?;
            info!(
                "PACT_MODE: GCP Secret Manager endpoint already set to {}",
                endpoint
            );
            return Ok(());
        }

        // Not set - set it programmatically
        if let Some(endpoint) = &pact_config.endpoint {
            // GCP uses HTTP client, so we set the environment variable
            // The client will read it during initialization
            // SAFETY: Pact override runs under a test mutex that serialises all
            // env mutations; no other thread reads or writes env vars concurrently.
            unsafe {
                std::env::set_var("GCP_SECRET_MANAGER_ENDPOINT", endpoint);
            }

            self.validate_endpoint(endpoint)?;

            info!(
                "PACT_MODE: Overriding GCP Secret Manager endpoint to {}",
                endpoint
            );
        }

        Ok(())
    }

    fn validate_endpoint(&self, endpoint: &str) -> Result<()> {
        // GCP-specific validation
        if endpoint.contains("secretmanager.googleapis.com") {
            return Err(anyhow::anyhow!(
                "PACT_MODE enabled but endpoint '{}' points to production GCP. \
                This is not allowed in Pact mode. Use a mock server endpoint instead.",
                endpoint
            ));
        }

        // Warn if endpoint doesn't look like a typical mock server
        let looks_like_mock = endpoint.starts_with("http://localhost")
            || endpoint.starts_with("http://127.0.0.1")
            || endpoint.starts_with("http://[::1]")
            || endpoint.contains("host.docker.internal")
            || endpoint.contains(".svc.cluster.local")
            || endpoint.contains("pact")
            || endpoint.contains("mock");

        if !looks_like_mock {
            tracing::warn!(
                "PACT_MODE enabled but endpoint '{}' does not appear to be a mock server. \
                Verify this is correct and not pointing to production GCP.",
                endpoint
            );
        }

        Ok(())
    }

    fn get_endpoint(&self) -> Option<String> {
        self.get_pact_config()
            .ok()
            .and_then(|config| config.endpoint)
    }

    fn cleanup(&self) -> Result<()> {
        // SAFETY: See set_var above — runs under the test mutex.
        unsafe {
            std::env::remove_var("GCP_SECRET_MANAGER_ENDPOINT");
        }
        Ok(())
    }
}
