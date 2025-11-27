//! Pact integration tests for Azure Key Vault Provider
//!
//! These tests verify that the Azure Key Vault provider implementation
//! works correctly with Pact mock servers by:
//! 1. Starting a Pact mock server
//! 2. Configuring the provider to use the mock server endpoint
//! 3. Calling the actual provider methods
//! 4. Verifying contracts are met
//!
//! **Note**: These tests are configured to run sequentially using a test-level mutex,
//! ensuring proper isolation regardless of test runner configuration. The mutex ensures
//! only one test runs at a time, preventing environment variable conflicts.
//!
//! Run with: `cargo test --test pact_provider_integration_azure`
//! (Sequential execution is enforced internally, so --test-threads=1 is optional)

#[cfg(test)]
mod common;

use common::init_rustls;
use controller::prelude::*;
use pact_consumer::prelude::*;
use serde_json::json;
use std::env;
use std::sync::{Mutex, Once};

static INIT: Once = Once::new();

/// Global mutex to ensure tests run sequentially
/// Each test must acquire this lock before running
/// This ensures proper test isolation even if test runner allows parallel execution
static TEST_MUTEX: Mutex<()> = Mutex::new(());

/// Initialize test environment - set up rustls only
/// Note: PACT_MODE is set per-test to ensure proper isolation
fn init_test() {
    INIT.call_once(|| {
        // Initialize rustls crypto provider FIRST (before any async operations)
        init_rustls();
    });
}

/// Test fixture guard that ensures proper cleanup after each test
/// This ensures test isolation by cleaning up environment variables and resetting state
struct TestFixture {
    endpoint: String,
}

impl TestFixture {
    /// Set up a new test fixture with proper isolation
    async fn setup(endpoint: String) -> Self {
        // CRITICAL: Clean up any leftover state from previous tests FIRST
        // This ensures we start with a completely clean state
        // Do this multiple times to ensure it's really clean (for cargo llvm-cov)
        Self::cleanup_all();
        tokio::task::yield_now().await;
        Self::cleanup_all();
        tokio::task::yield_now().await;

        // Set up the new test environment
        env::set_var("PACT_MODE", "true");
        env::set_var("AZURE_KEY_VAULT_ENDPOINT", &endpoint);
        env::set_var("__PACT_MODE_TEST__", "true");

        // Small delay to ensure environment variables are visible
        tokio::task::yield_now().await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        tokio::task::yield_now().await;

        // Initialize PactModeConfig with the new endpoint
        eprintln!(
            "🔧 Setting up Azure test fixture with endpoint: {}",
            endpoint
        );
        match controller::config::PactModeConfig::init() {
            Ok(()) => {
                eprintln!("✅ PactModeConfig initialized successfully");
            }
            Err(e) => {
                eprintln!("ℹ️  PactModeConfig re-initialized: {}", e);
            }
        }

        // Verify the setup - retry if needed (for cargo llvm-cov)
        for _ in 0..3 {
            let pact_config = controller::config::PactModeConfig::get();
            if let Some(provider_config) =
                pact_config.get_provider(&controller::config::ProviderId::AzureKeyVault)
            {
                if let Some(config_endpoint) = &provider_config.endpoint {
                    if config_endpoint == &endpoint {
                        drop(pact_config);
                        return Self { endpoint };
                    }
                    eprintln!(
                        "⚠️  Endpoint mismatch, retrying... Expected: {}, Got: {}",
                        endpoint, config_endpoint
                    );
                }
            }
            drop(pact_config);
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        // Final verification
        let pact_config = controller::config::PactModeConfig::get();
        if let Some(provider_config) =
            pact_config.get_provider(&controller::config::ProviderId::AzureKeyVault)
        {
            if let Some(config_endpoint) = &provider_config.endpoint {
                assert_eq!(
                    config_endpoint, &endpoint,
                    "PactModeConfig endpoint mismatch. Expected: {}, Got: {}",
                    endpoint, config_endpoint
                );
            }
        }
        drop(pact_config);

        Self { endpoint }
    }

    /// Clean up all test-related environment variables and state
    fn cleanup_all() {
        env::remove_var("AZURE_KEY_VAULT_ENDPOINT");
        env::remove_var("PACT_MODE");
        env::remove_var("__PACT_MODE_TEST__");

        // Reset PactModeConfig if it exists
        // Note: We can't fully reset OnceLock, but we can clear the config
        // We use try-catch to safely handle the case where config doesn't exist yet
        let _ = std::panic::catch_unwind(|| {
            let mut config = controller::config::PactModeConfig::get();
            config.enabled = false;
            config.providers.clear();
        });
    }

    /// Explicitly clean up this fixture
    /// Call this at the end of tests to ensure cleanup happens synchronously
    pub fn teardown(self) {
        // Explicit cleanup - this consumes self
        eprintln!(
            "🧹 Tearing down Azure test fixture for endpoint: {}",
            self.endpoint
        );
        Self::cleanup_all();
    }
}

impl Drop for TestFixture {
    fn drop(&mut self) {
        // Clean up when the fixture goes out of scope
        // This ensures cleanup happens even if the test panics
        eprintln!(
            "🧹 Cleaning up Azure test fixture for endpoint: {}",
            self.endpoint
        );
        Self::cleanup_all();
    }
}

/// Set up Pact mode environment variables using the test fixture
/// Returns a guard that will automatically clean up when dropped
async fn setup_pact_environment(endpoint: String) -> TestFixture {
    TestFixture::setup(endpoint).await
}

#[tokio::test]
async fn test_azure_provider_create_secret_with_pact() {
    // Acquire test mutex to ensure sequential execution
    let _guard = TEST_MUTEX.lock().expect("Test mutex poisoned");

    init_test();

    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Azure-Key-Vault");

    // Define contract: secret doesn't exist, so we create it
    pact_builder
        .interaction("get secret - not found", "", |mut i| {
            i.given("the secret does not exist");
            i.request
                .method("GET")
                .path("/secrets/test-secret-name/")
                .header("authorization", "Bearer test-token")
                .query_param("api-version", "2025-07-01");
            i.response
                .status(404)
                .header("content-type", "application/json")
                .json_body(json!({
                    "error": {
                        "code": "SecretNotFound",
                        "message": "A secret with (name/id) test-secret-name was not found in this key vault."
                    }
                }));
            i
        });

    pact_builder.interaction("set a new secret", "", |mut i| {
        i.given("the secret does not exist");
        i.request
            .method("PUT")
            .path("/secrets/test-secret-name")
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json")
            .query_param("api-version", "2025-07-01")
            .json_body(json!({
                "value": "test-secret-value",
                "tags": {
                    "environment": "test",
                    "location": "eastus"
                }
            }));
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "value": "test-secret-value",
                "id": "https://test-vault.vault.azure.net/secrets/test-secret-name/abc123",
                "attributes": {
                    "enabled": true,
                    "created": 1704067200,
                    "updated": 1704067200,
                    "recoveryLevel": "Recoverable+Purgeable"
                }
            }));
        i
    });

    // Keep mock server alive for the duration of the test
    let _mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = _mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }

    // Set up Pact environment variables using test fixture
    // The fixture will automatically clean up when it goes out of scope
    let _fixture = setup_pact_environment(base_url.clone()).await;

    // Create Azure provider instance
    let config = AzureConfig {
        vault_name: "test-vault".to_string(),
        location: "eastus".to_string(),
        auth: None, // Use default (Managed Identity) - won't matter for Pact
    };

    // Create a minimal kube client for provider initialization
    let kube_client = kube::Client::try_default()
        .await
        .expect("Failed to create kube client");

    let provider = AzureKeyVault::new(&config, &kube_client)
        .await
        .expect("Failed to create Azure provider");

    // Call the actual provider method
    let result = provider
        .create_or_update_secret("test-secret-name", "test-secret-value", "test", "eastus")
        .await;

    // Verify it succeeded
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true (secret was created)

    // Explicit teardown to ensure cleanup happens synchronously
    _fixture.teardown();
}

#[tokio::test]
async fn test_azure_provider_update_secret_with_pact() {
    // Acquire test mutex to ensure sequential execution
    let _guard = TEST_MUTEX.lock().expect("Test mutex poisoned");

    init_test();

    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Azure-Key-Vault");

    // Define contract: secret exists, value changed
    pact_builder.interaction("get current secret value", "", |mut i| {
        i.given("the secret exists with a current value");
        i.request
            .method("GET")
            .path("/secrets/test-secret-name/")
            .header("authorization", "Bearer test-token")
            .query_param("api-version", "2025-07-01");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "value": "old-secret-value",
                "id": "https://test-vault.vault.azure.net/secrets/test-secret-name/abc123",
                "attributes": {
                    "enabled": true,
                    "created": 1704067200,
                    "updated": 1704067200,
                    "recoveryLevel": "Recoverable+Purgeable"
                }
            }));
        i
    });

    pact_builder.interaction("update secret value", "", |mut i| {
        i.given("the secret exists");
        i.request
            .method("PUT")
            .path("/secrets/test-secret-name")
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json")
            .query_param("api-version", "2025-07-01")
            .json_body(json!({
                "value": "new-secret-value",
                "tags": {
                    "environment": "test",
                    "location": "eastus"
                }
            }));
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "value": "new-secret-value",
                "id": "https://test-vault.vault.azure.net/secrets/test-secret-name/def456",
                "attributes": {
                    "enabled": true,
                    "created": 1704067200,
                    "updated": 1704153600,
                    "recoveryLevel": "Recoverable+Purgeable"
                }
            }));
        i
    });

    let _mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = _mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }

    // Set up Pact environment variables using test fixture
    // The fixture will automatically clean up when it goes out of scope
    let _fixture = setup_pact_environment(base_url.clone()).await;

    let config = AzureConfig {
        vault_name: "test-vault".to_string(),
        location: "eastus".to_string(),
        auth: None,
    };

    let kube_client = kube::Client::try_default()
        .await
        .expect("Failed to create kube client");

    let provider = AzureKeyVault::new(&config, &kube_client)
        .await
        .expect("Failed to create Azure provider");

    // Call the actual provider method - should update since value changed
    let result = provider
        .create_or_update_secret("test-secret-name", "new-secret-value", "test", "eastus")
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true (secret was updated)

    // Explicit teardown to ensure cleanup happens synchronously
    _fixture.teardown();
}

#[tokio::test]
async fn test_azure_provider_no_change_with_pact() {
    // Acquire test mutex to ensure sequential execution
    let _guard = TEST_MUTEX.lock().expect("Test mutex poisoned");

    init_test();

    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Azure-Key-Vault");

    // Define contract: secret exists, value unchanged
    pact_builder.interaction("get current secret value - unchanged", "", |mut i| {
        i.given("the secret exists with the same value");
        i.request
            .method("GET")
            .path("/secrets/test-secret-name/")
            .header("authorization", "Bearer test-token")
            .query_param("api-version", "2025-07-01");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "value": "test-secret-value",
                "id": "https://test-vault.vault.azure.net/secrets/test-secret-name/abc123",
                "attributes": {
                    "enabled": true,
                    "created": 1704067200,
                    "updated": 1704067200,
                    "recoveryLevel": "Recoverable+Purgeable"
                }
            }));
        i
    });

    let _mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = _mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }

    // Set up Pact environment variables using test fixture
    // The fixture will automatically clean up when it goes out of scope
    let _fixture = setup_pact_environment(base_url.clone()).await;

    let config = AzureConfig {
        vault_name: "test-vault".to_string(),
        location: "eastus".to_string(),
        auth: None,
    };

    let kube_client = kube::Client::try_default()
        .await
        .expect("Failed to create kube client");

    let provider = AzureKeyVault::new(&config, &kube_client)
        .await
        .expect("Failed to create Azure provider");

    // Call the actual provider method - should return false (no change)
    let result = provider
        .create_or_update_secret("test-secret-name", "test-secret-value", "test", "eastus")
        .await;

    assert!(result.is_ok());
    assert!(!result.unwrap()); // Should return false (no change needed)

    // Explicit teardown to ensure cleanup happens synchronously
    _fixture.teardown();
}
