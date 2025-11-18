//! Pact integration tests for GCP Secret Manager Provider
//!
//! These tests verify that the GCP Secret Manager provider implementation
//! works correctly with Pact mock servers by:
//! 1. Starting a Pact mock server
//! 2. Configuring the provider to use the mock server endpoint
//! 3. Calling the actual provider methods
//! 4. Verifying contracts are met
//!
//! These tests use the GCP REST client which works directly with Pact HTTP mock servers.
//! When PACT_MODE=true, the REST client is automatically selected.
//!
//! **Note**: These tests must run sequentially to avoid environment variable conflicts.
//! Run with: `cargo test --test pact_provider_integration_gcp -- --test-threads=1`

#[cfg(test)]
mod common;

use common::init_rustls;
use pact_consumer::prelude::*;
use secret_manager_controller::provider::gcp::SecretManagerREST;
use secret_manager_controller::provider::SecretManagerProvider;
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

/// Helper to set up mock server and return base URL
/// Note: The mock_server must be kept alive for the duration of the test
/// Returns the mock_server (which must be kept in scope) and the base_url
fn setup_provider(
    pact_builder: &mut PactBuilder,
    _project_id: &str,
) -> (
    Box<dyn pact_consumer::mock_server::ValidatingMockServer>,
    String,
) {
    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    env::set_var("GCP_SECRET_MANAGER_ENDPOINT", &base_url);
    (mock_server, base_url)
}

/// Helper to clean up environment
fn cleanup() {
    env::remove_var("GCP_SECRET_MANAGER_ENDPOINT");
}

/// Helper to base64 encode a string
fn base64_encode(s: &str) -> String {
    use base64::{engine::general_purpose, Engine as _};
    general_purpose::STANDARD.encode(s.as_bytes())
}

#[tokio::test]
async fn test_gcp_provider_create_secret_with_pact() {
    init_test();

    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");

    let secret_value = "test-secret-value";
    let encoded_value = base64_encode(secret_value);

    // Define contract: secret doesn't exist, so we create it
    pact_builder.interaction("get secret - not found", "", |mut i| {
        i.given("the secret does not exist");
        i.request
            .method("GET")
            .path("/v1/projects/test-project/secrets/test-secret-name/versions/latest:access")
            .header("authorization", "Bearer test-token");
        i.response
            .status(404)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": {
                    "code": 404,
                    "message": "Secret [test-secret-name] not found",
                    "status": "NOT_FOUND"
                }
            }));
        i
    });

    pact_builder.interaction("create a new secret", "", |mut i| {
        i.given("the secret does not exist");
        i.request
            .method("POST")
            .path("/v1/projects/test-project/secrets")
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json")
            .json_body(json!({
                "secretId": "test-secret-name",
                "replication": {
                    "automatic": {}
                }
            }));
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/secrets/test-secret-name",
                "replication": {
                    "automatic": {}
                },
                "createTime": "2024-01-01T00:00:00Z"
            }));
        i
    });

    pact_builder.interaction("add secret version", "", |mut i| {
        i.given("the secret exists");
        i.request
            .method("POST")
            .path("/v1/projects/test-project/secrets/test-secret-name:addVersion")
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json")
            .json_body(json!({
                "payload": {
                    "data": encoded_value
                }
            }));
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/secrets/test-secret-name/versions/1",
                "payload": {
                    "data": encoded_value
                },
                "createTime": "2024-01-01T00:00:00Z",
                "state": "ENABLED"
            }));
        i
    });

    let (_mock_server, _base_url) = setup_provider(&mut pact_builder, "test-project");

    let provider = SecretManagerREST::new("test-project".to_string(), None, None)
        .await
        .expect("Failed to create GCP REST provider");

    let result = provider
        .create_or_update_secret("test-secret-name", secret_value)
        .await;

    assert!(
        result.is_ok(),
        "Failed to create/update secret: {:?}",
        result
    );
    assert_eq!(result.unwrap(), true, "Secret should have been created");

    cleanup();
}

#[tokio::test]
async fn test_gcp_provider_update_secret_with_pact() {
    init_test();

    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");

    let old_value = "old-secret-value";
    let new_value = "new-secret-value";
    let encoded_old = base64_encode(old_value);
    let encoded_new = base64_encode(new_value);

    // Get existing secret value
    pact_builder.interaction("get existing secret value", "", |mut i| {
        i.given("the secret exists with a value");
        i.request
            .method("GET")
            .path("/v1/projects/test-project/secrets/test-secret-name/versions/latest:access")
            .header("authorization", "Bearer test-token");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/secrets/test-secret-name/versions/1",
                "payload": {
                    "data": encoded_old
                }
            }));
        i
    });

    // Add new version with updated value
    pact_builder.interaction("add new version with updated value", "", |mut i| {
        i.given("the secret exists");
        i.request
            .method("POST")
            .path("/v1/projects/test-project/secrets/test-secret-name:addVersion")
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json")
            .json_body(json!({
                "payload": {
                    "data": encoded_new
                }
            }));
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/secrets/test-secret-name/versions/2",
                "payload": {
                    "data": encoded_new
                },
                "createTime": "2024-01-01T00:00:00Z",
                "state": "ENABLED"
            }));
        i
    });

    let (_mock_server, _base_url) = setup_provider(&mut pact_builder, "test-project");

    let provider = SecretManagerREST::new("test-project".to_string(), None, None)
        .await
        .expect("Failed to create GCP REST provider");

    let result = provider
        .create_or_update_secret("test-secret-name", new_value)
        .await;

    assert!(result.is_ok(), "Failed to update secret: {:?}", result);
    assert_eq!(result.unwrap(), true, "Secret should have been updated");

    cleanup();
}

#[tokio::test]
async fn test_gcp_provider_no_change_with_pact() {
    init_test();

    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");

    let secret_value = "unchanged-value";
    let encoded_value = base64_encode(secret_value);

    // Get existing secret value (same as what we're trying to set)
    pact_builder.interaction("get existing secret value - no change", "", |mut i| {
        i.given("the secret exists with the same value");
        i.request
            .method("GET")
            .path("/v1/projects/test-project/secrets/test-secret-name/versions/latest:access")
            .header("authorization", "Bearer test-token");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/secrets/test-secret-name/versions/1",
                "payload": {
                    "data": encoded_value
                }
            }));
        i
    });

    let (_mock_server, _base_url) = setup_provider(&mut pact_builder, "test-project");

    let provider = SecretManagerREST::new("test-project".to_string(), None, None)
        .await
        .expect("Failed to create GCP REST provider");

    let result = provider
        .create_or_update_secret("test-secret-name", secret_value)
        .await;

    assert!(result.is_ok(), "Failed to check secret: {:?}", result);
    assert_eq!(
        result.unwrap(),
        false,
        "Secret should not have been updated (no change)"
    );

    cleanup();
}

#[tokio::test]
async fn test_gcp_provider_get_secret_value_success() {
    init_test();

    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");

    let secret_value = "retrieved-secret-value";
    let encoded_value = base64_encode(secret_value);

    pact_builder.interaction("get secret value - success", "", |mut i| {
        i.given("the secret exists");
        i.request
            .method("GET")
            .path("/v1/projects/test-project/secrets/test-secret-name/versions/latest:access")
            .header("authorization", "Bearer test-token");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/secrets/test-secret-name/versions/1",
                "payload": {
                    "data": encoded_value
                }
            }));
        i
    });

    let (_mock_server, _base_url) = setup_provider(&mut pact_builder, "test-project");

    let provider = SecretManagerREST::new("test-project".to_string(), None, None)
        .await
        .expect("Failed to create GCP REST provider");

    let result = provider.get_secret_value("test-secret-name").await;

    assert!(result.is_ok(), "Failed to get secret value: {:?}", result);
    assert_eq!(
        result.unwrap(),
        Some(secret_value.to_string()),
        "Secret value should match"
    );

    cleanup();
}

#[tokio::test]
async fn test_gcp_provider_get_secret_value_not_found() {
    init_test();

    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");

    pact_builder.interaction("get secret value - not found", "", |mut i| {
        i.given("the secret does not exist");
        i.request
            .method("GET")
            .path("/v1/projects/test-project/secrets/test-secret-name/versions/latest:access")
            .header("authorization", "Bearer test-token");
        i.response
            .status(404)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": {
                    "code": 404,
                    "message": "Secret [test-secret-name] not found",
                    "status": "NOT_FOUND"
                }
            }));
        i
    });

    let (_mock_server, _base_url) = setup_provider(&mut pact_builder, "test-project");

    let provider = SecretManagerREST::new("test-project".to_string(), None, None)
        .await
        .expect("Failed to create GCP REST provider");

    let result = provider.get_secret_value("test-secret-name").await;

    assert!(result.is_ok(), "Should handle 404 gracefully: {:?}", result);
    assert_eq!(
        result.unwrap(),
        None,
        "Should return None for non-existent secret"
    );

    cleanup();
}

#[tokio::test]
async fn test_gcp_provider_delete_secret_success() {
    init_test();

    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");

    pact_builder.interaction("delete secret - success", "", |mut i| {
        i.given("the secret exists");
        i.request
            .method("DELETE")
            .path("/v1/projects/test-project/secrets/test-secret-name")
            .header("authorization", "Bearer test-token");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({}));
        i
    });

    let (_mock_server, _base_url) = setup_provider(&mut pact_builder, "test-project");

    let provider = SecretManagerREST::new("test-project".to_string(), None, None)
        .await
        .expect("Failed to create GCP REST provider");

    let result = provider.delete_secret("test-secret-name").await;

    assert!(result.is_ok(), "Failed to delete secret: {:?}", result);

    cleanup();
}

#[tokio::test]
async fn test_gcp_provider_delete_secret_not_found() {
    init_test();

    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");

    pact_builder.interaction("delete secret - not found", "", |mut i| {
        i.given("the secret does not exist");
        i.request
            .method("DELETE")
            .path("/v1/projects/test-project/secrets/test-secret-name")
            .header("authorization", "Bearer test-token");
        i.response
            .status(404)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": {
                    "code": 404,
                    "message": "Secret [test-secret-name] not found",
                    "status": "NOT_FOUND"
                }
            }));
        i
    });

    let (_mock_server, _base_url) = setup_provider(&mut pact_builder, "test-project");

    let provider = SecretManagerREST::new("test-project".to_string(), None, None)
        .await
        .expect("Failed to create GCP REST provider");

    let result = provider.delete_secret("test-secret-name").await;

    // Note: Current implementation may not handle 404 gracefully for delete
    // This test documents expected behavior - may need to update implementation
    assert!(
        result.is_err(),
        "Delete should return error for non-existent secret: {:?}",
        result
    );

    cleanup();
}

#[tokio::test]
async fn test_gcp_provider_error_handling_unauthorized() {
    init_test();

    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");

    pact_builder.interaction("get secret - unauthorized", "", |mut i| {
        i.given("authentication fails");
        i.request
            .method("GET")
            .path("/v1/projects/test-project/secrets/test-secret-name/versions/latest:access")
            .header("authorization", "Bearer test-token");
        i.response
            .status(401)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": {
                    "code": 401,
                    "message": "Request had invalid authentication credentials",
                    "status": "UNAUTHENTICATED"
                }
            }));
        i
    });

    let (_mock_server, _base_url) = setup_provider(&mut pact_builder, "test-project");

    let provider = SecretManagerREST::new("test-project".to_string(), None, None)
        .await
        .expect("Failed to create GCP REST provider");

    let result = provider.get_secret_value("test-secret-name").await;

    assert!(result.is_err(), "Should return error for 401: {:?}", result);
    let error = result.unwrap_err();
    let error_msg = error.to_string();
    let error_chain: String = error
        .chain()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join(": ");
    assert!(
        error_msg.contains("401")
            || error_msg.contains("UNAUTHENTICATED")
            || error_chain.contains("401")
            || error_chain.contains("UNAUTHENTICATED"),
        "Error message should indicate authentication failure. Message: {}, Chain: {}",
        error_msg,
        error_chain
    );

    cleanup();
}

#[tokio::test]
async fn test_gcp_provider_error_handling_forbidden() {
    init_test();

    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");

    pact_builder.interaction("create secret - forbidden", "", |mut i| {
        i.given("insufficient permissions");
        i.request
            .method("POST")
            .path("/v1/projects/test-project/secrets")
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json")
            .json_body(json!({
                "secretId": "test-secret-name",
                "replication": {
                    "automatic": {}
                }
            }));
        i.response
            .status(403)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": {
                    "code": 403,
                    "message": "Permission denied",
                    "status": "PERMISSION_DENIED"
                }
            }));
        i
    });

    // Also need the get secret interaction (it's called first)
    pact_builder.interaction("get secret - not found (before create)", "", |mut i| {
        i.given("the secret does not exist");
        i.request
            .method("GET")
            .path("/v1/projects/test-project/secrets/test-secret-name/versions/latest:access")
            .header("authorization", "Bearer test-token");
        i.response
            .status(404)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": {
                    "code": 404,
                    "message": "Secret [test-secret-name] not found",
                    "status": "NOT_FOUND"
                }
            }));
        i
    });

    let (_mock_server, _base_url) = setup_provider(&mut pact_builder, "test-project");

    let provider = SecretManagerREST::new("test-project".to_string(), None, None)
        .await
        .expect("Failed to create GCP REST provider");

    let result = provider
        .create_or_update_secret("test-secret-name", "test-value")
        .await;

    assert!(result.is_err(), "Should return error for 403: {:?}", result);
    let error = result.unwrap_err();
    // Check the full error chain for status code or status string
    let error_msg = error.to_string();
    let error_chain: String = error
        .chain()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join(": ");
    assert!(
        error_msg.contains("403")
            || error_msg.contains("PERMISSION_DENIED")
            || error_chain.contains("403")
            || error_chain.contains("PERMISSION_DENIED"),
        "Error message should indicate permission denied. Message: {}, Chain: {}",
        error_msg,
        error_chain
    );

    cleanup();
}
