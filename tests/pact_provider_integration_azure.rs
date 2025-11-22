//! Pact integration tests for Azure Key Vault Provider
//!
//! These tests verify that the Azure Key Vault provider implementation
//! works correctly with Pact mock servers by:
//! 1. Starting a Pact mock server
//! 2. Configuring the provider to use the mock server endpoint
//! 3. Calling the actual provider methods
//! 4. Verifying contracts are met
//!
//! **Note**: These tests must run sequentially to avoid environment variable conflicts.
//! Run with: `cargo test --test pact_provider_integration_azure -- --test-threads=1`

#[cfg(test)]
mod common;

use common::init_rustls;
use controller::prelude::*;
use pact_consumer::prelude::*;
use serde_json::json;
use std::env;
use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize test environment - set up Pact mode and rustls
fn init_test() {
    INIT.call_once(|| {
        // Initialize rustls crypto provider FIRST (before any async operations)
        init_rustls();

        // Enable Pact mode
        env::set_var("PACT_MODE", "true");
    });
}

#[tokio::test]
async fn test_azure_provider_create_secret_with_pact() {
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
                "value": "test-secret-value"
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

    // Set endpoint environment variable
    env::set_var("AZURE_KEY_VAULT_ENDPOINT", &base_url);

    // Create Azure provider instance
    let config = AzureConfig {
        vault_name: "test-vault".to_string(),
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
        .create_or_update_secret("test-secret-name", "test-secret-value")
        .await;

    // Verify it succeeded
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true (secret was created)

    // Clean up
    env::remove_var("AZURE_KEY_VAULT_ENDPOINT");
}

#[tokio::test]
async fn test_azure_provider_update_secret_with_pact() {
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
                "value": "new-secret-value"
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

    env::set_var("AZURE_KEY_VAULT_ENDPOINT", &base_url);

    let config = AzureConfig {
        vault_name: "test-vault".to_string(),
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
        .create_or_update_secret("test-secret-name", "new-secret-value")
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true (secret was updated)

    env::remove_var("AZURE_KEY_VAULT_ENDPOINT");
}

#[tokio::test]
async fn test_azure_provider_no_change_with_pact() {
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

    env::set_var("AZURE_KEY_VAULT_ENDPOINT", &base_url);

    let config = AzureConfig {
        vault_name: "test-vault".to_string(),
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
        .create_or_update_secret("test-secret-name", "test-secret-value")
        .await;

    assert!(result.is_ok());
    assert!(!result.unwrap()); // Should return false (no change needed)

    env::remove_var("AZURE_KEY_VAULT_ENDPOINT");
}
