//! Pact integration tests for AWS Secrets Manager Provider
//!
//! These tests verify that the AWS Secrets Manager provider implementation
//! works correctly with Pact mock servers by:
//! 1. Starting a Pact mock server
//! 2. Configuring the provider to use the mock server endpoint
//! 3. Calling the actual provider methods
//! 4. Verifying contracts are met
//!
//! **Note**: These tests must run sequentially to avoid environment variable conflicts.
//! Run with: `cargo test --test pact_provider_integration_aws -- --test-threads=1`

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

/// Create a test Kubernetes client for Pact tests
/// In Pact tests, we don't actually need a real kube client
/// The provider will use the mock endpoint we set
/// The AWS provider doesn't use the kube client when auth is None,
/// so we just need something that satisfies the type requirement
async fn create_test_kube_client() -> Option<kube::Client> {
    // Try to create a real client first (works in local dev with kubeconfig)
    if let Ok(client) = kube::Client::try_default().await {
        return Some(client);
    }

    // If that fails, try in-cluster config (works in CI with Kind cluster)
    if let Ok(config) = kube::Config::infer().await {
        if let Ok(client) = kube::Client::try_from(config) {
            return Some(client);
        }
    }

    // If both fail, return None - the test will be skipped
    None
}

#[tokio::test]
async fn test_aws_provider_create_secret_with_pact() {
    init_test();

    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");

    // Define contract: secret doesn't exist, so we create it
    pact_builder.interaction("describe secret - not found", "", |mut i| {
        i.given("the secret does not exist");
        i.request
            .method("POST")
            .path("/")
            .header("content-type", "application/x-amz-json-1.1")
            .header("x-amz-target", "secretsmanager.DescribeSecret")
            .body(
                json!({
                    "SecretId": "test-secret-name"
                })
                .to_string(),
            );
        i.response
            .status(400)
            .header("content-type", "application/x-amz-json-1.1")
            .json_body(json!({
                "__type": "ResourceNotFoundException",
                "message": "Secrets Manager can't find the specified secret."
            }));
        i
    });

    pact_builder
        .interaction("create a new secret", "", |mut i| {
            i.given("the secret does not exist");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.CreateSecret")
                // AWS SDK automatically adds ClientRequestToken (UUID) to CreateSecret requests
                // The UUID value will be different each time, so exact matching fails
                // Note: This is a known limitation - Pact does exact string matching for
                // application/x-amz-json-1.1 bodies, and we can't use matching rules on string bodies
                // Workaround: The test may need to be updated to handle this, or we need to
                // configure the AWS SDK to use a fixed ClientRequestToken in test mode
                // AWS SDK sends compact JSON (no spaces) with fields in specific order
                // SDK order: Name, ClientRequestToken, SecretString
                .body(r#"{"Name":"test-secret-name","ClientRequestToken":"00000000-0000-0000-0000-000000000000","SecretString":"test-secret-value"}"#);
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "VersionId": "test-version-id"
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
    env::set_var("AWS_SECRETS_MANAGER_ENDPOINT", &base_url);

    // Create AWS provider instance
    let config = AwsConfig {
        region: "us-east-1".to_string(),
        auth: None, // Use default (IRSA) - won't matter for Pact
    };

    // Create a minimal kube client for provider initialization
    let kube_client = match create_test_kube_client().await {
        Some(client) => client,
        None => {
            eprintln!("⚠️  Skipping test: No Kubernetes cluster available");
            eprintln!("💡 To run this test, ensure a Kubernetes cluster is available:");
            eprintln!("   - Run 'kind create cluster' for local testing");
            eprintln!("   - Or set KUBECONFIG environment variable");
            eprintln!("   - Or ensure in-cluster config is available");
            return; // Skip test if no cluster available
        }
    };

    let provider = AwsSecretManager::new(&config, &kube_client)
        .await
        .expect("Failed to create AWS provider");

    // Call the actual provider method
    let result = provider
        .create_or_update_secret("test-secret-name", "test-secret-value")
        .await;

    // Verify it succeeded
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true (secret was created)

    // Clean up
    env::remove_var("AWS_SECRETS_MANAGER_ENDPOINT");
}

#[tokio::test]
async fn test_aws_provider_update_secret_with_pact() {
    init_test();

    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");

    // Define contract: secret exists, value changed
    pact_builder
        .interaction("describe secret - exists", "", |mut i| {
            i.given("the secret exists");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.DescribeSecret")
                .body(json!({
                    "SecretId": "test-secret-name"
                }).to_string());
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name"
                }));
            i
        });

    pact_builder
        .interaction("get current secret value", "", |mut i| {
            i.given("the secret exists with a current value");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.GetSecretValue")
                .body(json!({
                    "SecretId": "test-secret-name"
                }).to_string());
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "SecretString": "old-secret-value",
                    "VersionId": "old-version-id"
                }));
            i
        });

    pact_builder
        .interaction("update secret value", "", |mut i| {
            i.given("the secret exists");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.PutSecretValue")
                // AWS SDK automatically adds ClientRequestToken (UUID) to PutSecretValue requests
                // In Pact mode, the provider sets a fixed UUID, so we match it exactly
                // AWS SDK sends compact JSON (no spaces) with fields in specific order
                // SDK order: SecretId, ClientRequestToken, SecretString
                .body(r#"{"SecretId":"test-secret-name","ClientRequestToken":"00000000-0000-0000-0000-000000000000","SecretString":"new-secret-value"}"#);
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "VersionId": "new-version-id",
                    "VersionStages": ["AWSCURRENT"]
                }));
            i
        });

    let _mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = _mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }

    env::set_var("AWS_SECRETS_MANAGER_ENDPOINT", &base_url);

    let config = AwsConfig {
        region: "us-east-1".to_string(),
        auth: None,
    };

    // Create a minimal kube client for provider initialization
    let kube_client = match create_test_kube_client().await {
        Some(client) => client,
        None => {
            eprintln!("⚠️  Skipping test: No Kubernetes cluster available");
            eprintln!("💡 To run this test, ensure a Kubernetes cluster is available:");
            eprintln!("   - Run 'kind create cluster' for local testing");
            eprintln!("   - Or set KUBECONFIG environment variable");
            eprintln!("   - Or ensure in-cluster config is available");
            return; // Skip test if no cluster available
        }
    };

    let provider = AwsSecretManager::new(&config, &kube_client)
        .await
        .expect("Failed to create AWS provider");

    // Call the actual provider method - should update since value changed
    let result = provider
        .create_or_update_secret("test-secret-name", "new-secret-value")
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true (secret was updated)

    env::remove_var("AWS_SECRETS_MANAGER_ENDPOINT");
}

#[tokio::test]
async fn test_aws_provider_no_change_with_pact() {
    init_test();

    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");

    // Define contract: secret exists, value unchanged
    pact_builder
        .interaction("describe secret - exists", "", |mut i| {
            i.given("the secret exists");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.DescribeSecret")
                .body(json!({
                    "SecretId": "test-secret-name"
                }).to_string());
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name"
                }));
            i
        });

    pact_builder
        .interaction("get current secret value - unchanged", "", |mut i| {
            i.given("the secret exists with the same value");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.GetSecretValue")
                .body(json!({
                    "SecretId": "test-secret-name"
                }).to_string());
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "SecretString": "test-secret-value",
                    "VersionId": "current-version-id"
                }));
            i
        });

    let _mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = _mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }

    env::set_var("AWS_SECRETS_MANAGER_ENDPOINT", &base_url);

    let config = AwsConfig {
        region: "us-east-1".to_string(),
        auth: None,
    };

    // Create a minimal kube client for provider initialization
    let kube_client = match create_test_kube_client().await {
        Some(client) => client,
        None => {
            eprintln!("⚠️  Skipping test: No Kubernetes cluster available");
            eprintln!("💡 To run this test, ensure a Kubernetes cluster is available:");
            eprintln!("   - Run 'kind create cluster' for local testing");
            eprintln!("   - Or set KUBECONFIG environment variable");
            eprintln!("   - Or ensure in-cluster config is available");
            return; // Skip test if no cluster available
        }
    };

    let provider = AwsSecretManager::new(&config, &kube_client)
        .await
        .expect("Failed to create AWS provider");

    // Call the actual provider method - should return false (no change)
    let result = provider
        .create_or_update_secret("test-secret-name", "test-secret-value")
        .await;

    assert!(result.is_ok());
    assert!(!result.unwrap()); // Should return false (no change needed)

    env::remove_var("AWS_SECRETS_MANAGER_ENDPOINT");
}
