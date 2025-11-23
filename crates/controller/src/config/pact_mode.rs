//! # PACT_MODE Configuration
//!
//! Centralized configuration for PACT_MODE - routes cloud provider API calls to Pact mock servers.
//!
//! This module provides:
//! - `PactModeAPIOverride` trait for provider-specific API endpoint overrides
//! - `PactModeConfig` singleton for centralized configuration
//! - Support for multiple providers (AWS, GCP, Azure, and future providers)

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Provider identifier for registration and lookup
#[allow(dead_code)] // Used by provider implementations
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProviderId {
    /// AWS Secrets Manager
    AwsSecretsManager,
    /// AWS Systems Manager Parameter Store
    AwsParameterStore,
    /// GCP Secret Manager
    GcpSecretManager,
    /// Azure Key Vault
    AzureKeyVault,
    /// Azure App Configuration
    AzureAppConfiguration,
    /// HashiCorp Vault (future)
    HashiCorpVault,
    /// Cloud.ru (future)
    CloudRu,
    /// Custom provider (for extensibility)
    Custom(String),
}

/// Provider-specific configuration for PACT_MODE
#[allow(dead_code)] // Used by provider implementations
#[derive(Debug, Clone)]
pub struct ProviderPactConfig {
    /// Provider identifier
    pub id: ProviderId,
    /// Endpoint URL for mock server
    pub endpoint: Option<String>,
    /// Additional environment variables to set (key-value pairs)
    pub env_vars: HashMap<String, String>,
    /// Whether this provider requires environment variable setup
    pub requires_env_setup: bool,
    /// Whether this provider uses SDK that reads env vars during async operations
    pub sdk_reads_env_vars: bool,
}

/// Trait for provider-specific PACT_MODE API override
/// Each provider implements this to override its API endpoint to point to Pact mock servers
///
/// This trait is designed to be extensible for future providers:
/// - HashiCorp Vault (HTTP client, custom auth)
/// - Cloud.ru (REST API, different auth mechanism)
/// - Any other provider with different SDK patterns
#[allow(dead_code)] // Used by provider implementations
pub trait PactModeAPIOverride: Send + Sync {
    /// Get the provider identifier
    fn provider_id(&self) -> ProviderId;

    /// Get the provider's Pact configuration
    fn get_pact_config(&self) -> Result<ProviderPactConfig>;

    /// Override the provider's API endpoint to point to Pact mock server
    /// This is called BEFORE creating the SDK/client
    ///
    /// Different providers may need different override mechanisms:
    /// - AWS: Set environment variables (SDK reads them during async load)
    /// - GCP: Set environment variables or configure HTTP client
    /// - Azure: Set environment variables or configure credential
    /// - HashiCorp Vault: Configure HTTP client with custom endpoint
    /// - Cloud.ru: Configure REST client with mock endpoint
    fn override_api_endpoint(&self) -> Result<()>;

    /// Validate the endpoint is safe (not pointing to production)
    /// Each provider can implement custom validation logic
    fn validate_endpoint(&self, endpoint: &str) -> Result<()>;

    /// Get the endpoint URL for this provider (if PACT_MODE is enabled)
    fn get_endpoint(&self) -> Option<String>;

    /// Clean up any environment variables or configuration set by override_api_endpoint()
    /// Useful for tests to avoid pollution
    fn cleanup(&self) -> Result<()>;
}

/// Default implementation for providers that use environment variables
impl PactModeAPIOverride for ProviderPactConfig {
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
    }

    fn get_pact_config(&self) -> Result<ProviderPactConfig> {
        Ok(self.clone())
    }

    fn override_api_endpoint(&self) -> Result<()> {
        if !self.requires_env_setup {
            return Ok(());
        }

        // Set all environment variables to override API endpoint
        for (key, value) in &self.env_vars {
            std::env::set_var(key, value);
        }

        // Validate endpoint if set
        if let Some(ref endpoint) = self.endpoint {
            self.validate_endpoint(endpoint)?;
        }

        Ok(())
    }

    fn validate_endpoint(&self, endpoint: &str) -> Result<()> {
        // Default validation: check for common production patterns
        let production_patterns = [
            "amazonaws.com",
            "googleapis.com",
            "azure.net",
            "vault.hashicorp.com",
        ];

        for pattern in &production_patterns {
            if endpoint.contains(pattern) {
                return Err(anyhow::anyhow!(
                    "PACT_MODE enabled but endpoint '{}' appears to point to production ({})",
                    endpoint,
                    pattern
                ));
            }
        }

        Ok(())
    }

    fn get_endpoint(&self) -> Option<String> {
        self.endpoint.clone()
    }

    fn cleanup(&self) -> Result<()> {
        // Remove environment variables
        for key in self.env_vars.keys() {
            std::env::remove_var(key);
        }
        Ok(())
    }
}

/// Centralized PACT_MODE configuration
/// Designed to be extensible for future providers without code changes
#[derive(Debug)]
pub struct PactModeConfig {
    /// Whether PACT_MODE is enabled
    pub enabled: bool,
    /// Provider-specific configurations (extensible map)
    /// Key: ProviderId, Value: ProviderPactConfig
    pub providers: HashMap<ProviderId, ProviderPactConfig>,
}

static PACT_MODE_CONFIG: OnceLock<Mutex<PactModeConfig>> = OnceLock::new();

impl PactModeConfig {
    /// Initialize PACT_MODE configuration from environment variables
    ///
    /// When PACT_MODE is enabled, ALL providers are automatically registered with pact mode.
    /// This allows local testing with multiple providers in the cluster, even if not all
    /// endpoints are explicitly configured.
    ///
    /// Endpoint resolution:
    /// - If environment variable is set (e.g., `AWS_SECRETS_MANAGER_ENDPOINT`), use that value
    /// - Otherwise, use default `http://localhost:1234` for local testing
    ///
    /// This ensures that when PACT_MODE=true, all providers will route to mock servers,
    /// preventing accidental calls to production APIs during local development.
    pub fn init() -> Result<()> {
        let enabled = std::env::var("PACT_MODE").is_ok();

        let mut providers = HashMap::new();

        if enabled {
            // Helper function to get endpoint with default fallback
            // Defaults use Kubernetes service names for in-cluster communication
            // For local testing, use http://localhost:1234
            let get_endpoint = |env_var: &str, default: &str| -> String {
                std::env::var(env_var).unwrap_or_else(|_| default.to_string())
            };

            // AWS Secrets Manager - always register when PACT_MODE is enabled
            // Default: Kubernetes service name for in-cluster, or localhost for local testing
            let aws_sm_endpoint = get_endpoint(
                "AWS_SECRETS_MANAGER_ENDPOINT",
                "http://aws-mock-server.secret-manager-controller-pact-broker.svc.cluster.local:1234"
            );
            let mut aws_sm_env_vars = HashMap::new();
            aws_sm_env_vars.insert(
                "AWS_SECRETS_MANAGER_ENDPOINT".to_string(),
                aws_sm_endpoint.clone(),
            );
            aws_sm_env_vars.insert(
                "AWS_ENDPOINT_URL_SECRETSMANAGER".to_string(),
                aws_sm_endpoint.clone(),
            );

            providers.insert(
                ProviderId::AwsSecretsManager,
                ProviderPactConfig {
                    id: ProviderId::AwsSecretsManager,
                    endpoint: Some(aws_sm_endpoint),
                    env_vars: aws_sm_env_vars,
                    requires_env_setup: true,
                    sdk_reads_env_vars: true, // AWS SDK reads env vars during async load
                },
            );

            // AWS Parameter Store - always register when PACT_MODE is enabled
            let aws_ssm_endpoint = get_endpoint("AWS_SSM_ENDPOINT", 
                &get_endpoint("AWS_ENDPOINT_URL_SSM", 
                    "http://aws-mock-server.secret-manager-controller-pact-broker.svc.cluster.local:1234"));
            let mut aws_ssm_env_vars = HashMap::new();
            aws_ssm_env_vars.insert("AWS_SSM_ENDPOINT".to_string(), aws_ssm_endpoint.clone());
            aws_ssm_env_vars.insert("AWS_ENDPOINT_URL_SSM".to_string(), aws_ssm_endpoint.clone());

            providers.insert(
                ProviderId::AwsParameterStore,
                ProviderPactConfig {
                    id: ProviderId::AwsParameterStore,
                    endpoint: Some(aws_ssm_endpoint),
                    env_vars: aws_ssm_env_vars,
                    requires_env_setup: true,
                    sdk_reads_env_vars: true,
                },
            );

            // GCP Secret Manager - always register when PACT_MODE is enabled
            let gcp_endpoint = get_endpoint(
                "GCP_SECRET_MANAGER_ENDPOINT",
                "http://gcp-mock-server.secret-manager-controller-pact-broker.svc.cluster.local:1234"
            );
            let mut gcp_env_vars = HashMap::new();
            gcp_env_vars.insert(
                "GCP_SECRET_MANAGER_ENDPOINT".to_string(),
                gcp_endpoint.clone(),
            );

            providers.insert(
                ProviderId::GcpSecretManager,
                ProviderPactConfig {
                    id: ProviderId::GcpSecretManager,
                    endpoint: Some(gcp_endpoint),
                    env_vars: gcp_env_vars,
                    requires_env_setup: true,
                    sdk_reads_env_vars: false, // GCP uses HTTP client, not SDK env vars
                },
            );

            // Azure Key Vault - always register when PACT_MODE is enabled
            let azure_endpoint = get_endpoint(
                "AZURE_KEY_VAULT_ENDPOINT",
                "http://azure-mock-server.secret-manager-controller-pact-broker.svc.cluster.local:1234"
            );
            let mut azure_env_vars = HashMap::new();
            azure_env_vars.insert(
                "AZURE_KEY_VAULT_ENDPOINT".to_string(),
                azure_endpoint.clone(),
            );

            providers.insert(
                ProviderId::AzureKeyVault,
                ProviderPactConfig {
                    id: ProviderId::AzureKeyVault,
                    endpoint: Some(azure_endpoint),
                    env_vars: azure_env_vars,
                    requires_env_setup: true,
                    sdk_reads_env_vars: false, // Azure uses credential, not SDK env vars
                },
            );

            // Azure App Configuration - always register when PACT_MODE is enabled
            let azure_app_config_endpoint = get_endpoint(
                "AZURE_APP_CONFIGURATION_ENDPOINT",
                "http://azure-mock-server.secret-manager-controller-pact-broker.svc.cluster.local:1234"
            );
            let mut azure_app_config_env_vars = HashMap::new();
            azure_app_config_env_vars.insert(
                "AZURE_APP_CONFIGURATION_ENDPOINT".to_string(),
                azure_app_config_endpoint.clone(),
            );

            providers.insert(
                ProviderId::AzureAppConfiguration,
                ProviderPactConfig {
                    id: ProviderId::AzureAppConfiguration,
                    endpoint: Some(azure_app_config_endpoint),
                    env_vars: azure_app_config_env_vars,
                    requires_env_setup: true,
                    sdk_reads_env_vars: false,
                },
            );
        }

        let config = PactModeConfig { enabled, providers };

        // Validate all provider endpoints (only if explicitly set, not defaults)
        // Default endpoints (localhost:1234) are allowed for local testing
        if enabled {
            config.validate_with_defaults()?;
        }

        // Allow re-initialization for tests (replace existing config)
        // In production, this should only be called once at startup
        // Note: We check for test mode at runtime using an environment variable check
        // This allows tests to re-initialize the config with new endpoints
        // CRITICAL: When re-initializing, we must read fresh env vars, not use cached values
        if let Some(_existing) = PACT_MODE_CONFIG.get() {
            // Check if we're in a test environment (tests set this)
            // This allows re-initialization during tests
            if std::env::var("__PACT_MODE_TEST__").is_ok() || cfg!(test) {
                // IMPORTANT: We've already read fresh env vars above, so just replace the config
                // The config was built with current env vars, so this is correct
                let mut existing_guard = _existing.lock().expect("PactModeConfig mutex poisoned");
                *existing_guard = config;
                return Ok(());
            }
            // In production, return error if already initialized
            return Err(anyhow::anyhow!("PactModeConfig already initialized"));
        }

        PACT_MODE_CONFIG
            .set(Mutex::new(config))
            .map_err(|_| anyhow::anyhow!("PactModeConfig already initialized"))?;

        Ok(())
    }

    /// Get the global PactModeConfig instance
    pub fn get() -> std::sync::MutexGuard<'static, PactModeConfig> {
        PACT_MODE_CONFIG
            .get()
            .expect("PactModeConfig not initialized")
            .lock()
            .expect("PactModeConfig mutex poisoned")
    }

    /// Get configuration for a specific provider
    pub fn get_provider(&self, provider_id: &ProviderId) -> Option<&ProviderPactConfig> {
        self.providers.get(provider_id)
    }

    /// Register a new provider configuration (for extensibility)
    #[allow(dead_code)] // For future provider registration
    pub fn register_provider(&mut self, config: ProviderPactConfig) -> Result<()> {
        if self.providers.contains_key(&config.id) {
            return Err(anyhow::anyhow!(
                "Provider {:?} already registered",
                config.id
            ));
        }
        self.providers.insert(config.id.clone(), config);
        Ok(())
    }

    /// Validate all provider endpoints
    /// Skips validation for default Kubernetes service endpoints (allowed for local/cluster testing)
    fn validate_with_defaults(&self) -> Result<()> {
        for (id, config) in &self.providers {
            if let Some(ref endpoint) = config.endpoint {
                // Allow default Kubernetes service endpoints and localhost for local testing
                if endpoint == "http://localhost:1234"
                    || endpoint.contains(".svc.cluster.local")
                    || endpoint.contains("mock-server")
                {
                    continue;
                }

                config
                    .validate_endpoint(endpoint)
                    .with_context(|| format!("Validation failed for provider {:?}", id))?;
            }
        }
        Ok(())
    }

    /// Reset for tests (allows re-initialization)
    #[cfg(test)]
    #[allow(dead_code)] // For test utilities
    pub fn reset() {
        if let Some(mutex) = PACT_MODE_CONFIG.get() {
            let mut config = mutex.lock().expect("PactModeConfig mutex poisoned");
            config.enabled = false;
            config.providers.clear();
        }
    }
}
