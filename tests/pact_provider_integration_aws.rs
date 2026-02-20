//! Pact integration tests for AWS Secrets Manager Provider
//!
//! These tests verify that the AWS Secrets Manager provider implementation
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
//! Run with: `cargo test --test pact_provider_integration_aws`
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

        // Set up the new test environment.
        // SAFETY: All tests in this file hold TEST_MUTEX before calling setup(),
        // so env mutations are serialised — no concurrent reads or writes.
        unsafe {
            env::set_var("PACT_MODE", "true");
            env::set_var("AWS_SECRETS_MANAGER_ENDPOINT", &endpoint);
            env::set_var("__PACT_MODE_TEST__", "true");
        }

        // Small delay to ensure environment variables are visible
        tokio::task::yield_now().await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        tokio::task::yield_now().await;

        // Initialize PactModeConfig with the new endpoint
        eprintln!("🔧 Setting up test fixture with endpoint: {}", endpoint);
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
                pact_config.get_provider(&controller::config::ProviderId::AwsSecretsManager)
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
            pact_config.get_provider(&controller::config::ProviderId::AwsSecretsManager)
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
        // SAFETY: See setup() — called under TEST_MUTEX.
        unsafe {
            env::remove_var("AWS_SECRETS_MANAGER_ENDPOINT");
            env::remove_var("AWS_ENDPOINT_URL_SECRETSMANAGER");
            env::remove_var("PACT_MODE");
            env::remove_var("__PACT_MODE_TEST__");
        }

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
            "🧹 Tearing down test fixture for endpoint: {}",
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
            "🧹 Cleaning up test fixture for endpoint: {}",
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
    // Acquire test mutex — recover from poison so a single test failure doesn't cascade
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    init_test();

    // Check for Kubernetes cluster BEFORE creating the mock server.
    // If we return early after the mock server is created, its Drop impl panics
    // when expected interactions were never triggered, poisoning the mutex.
    let kube_client = match create_test_kube_client().await {
        Some(client) => client,
        None => {
            eprintln!("⚠️  Skipping test: No Kubernetes cluster available");
            eprintln!("💡 To run this test, ensure a Kubernetes cluster is available:");
            eprintln!("   - Run 'kind create cluster' for local testing");
            eprintln!("   - Or set KUBECONFIG environment variable");
            eprintln!("   - Or ensure in-cluster config is available");
            return;
        }
    };

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
                // AWS SDK sends compact JSON with fields in specific order:
                // Name, ClientRequestToken, SecretString, Tags
                .body(r#"{"Name":"test-secret-name","ClientRequestToken":"00000000-0000-0000-0000-000000000000","SecretString":"test-secret-value","Tags":[{"Key":"environment","Value":"test"},{"Key":"location","Value":"us-east-1"}]}"#);
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

    // Set up Pact environment variables using test fixture
    let _fixture = setup_pact_environment(base_url.clone()).await;

    // Create AWS provider instance
    let config = AwsConfig {
        region: "us-east-1".to_string(),
        auth: None,
    };

    let provider = AwsSecretManager::new(&config, &kube_client)
        .await
        .expect("Failed to create AWS provider");

    // Call the actual provider method
    let result = provider
        .create_or_update_secret("test-secret-name", "test-secret-value", "test", "us-east-1")
        .await;

    // Verify it succeeded
    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true (secret was created)

    // Explicit teardown to ensure cleanup happens synchronously
    _fixture.teardown();
}

#[tokio::test]
async fn test_aws_provider_update_secret_with_pact() {
    // Acquire test mutex — recover from poison so a single test failure doesn't cascade
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    init_test();

    // Check for Kubernetes cluster BEFORE creating the mock server
    eprintln!("🔧 Creating Kubernetes client...");
    let kube_client = match create_test_kube_client().await {
        Some(client) => {
            eprintln!("✅ Kubernetes client created");
            client
        }
        None => {
            eprintln!("⚠️  Skipping test: No Kubernetes cluster available");
            eprintln!("💡 To run this test, ensure a Kubernetes cluster is available:");
            eprintln!("   - Run 'kind create cluster' for local testing");
            eprintln!("   - Or set KUBECONFIG environment variable");
            eprintln!("   - Or ensure in-cluster config is available");
            return;
        }
    };

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

    // Set up Pact environment variables using test fixture
    // The fixture will automatically clean up when it goes out of scope
    let _fixture = setup_pact_environment(base_url.clone()).await;

    let config = AwsConfig {
        region: "us-east-1".to_string(),
        auth: None,
    };

    eprintln!("🔧 Creating AWS Secrets Manager provider...");
    eprintln!("   Endpoint: {}", base_url);
    eprintln!(
        "   PACT_MODE: {}",
        env::var("PACT_MODE").unwrap_or_else(|_| "not set".to_string())
    );
    eprintln!(
        "   AWS_SECRETS_MANAGER_ENDPOINT: {}",
        env::var("AWS_SECRETS_MANAGER_ENDPOINT").unwrap_or_else(|_| "not set".to_string())
    );

    let provider = match AwsSecretManager::new(&config, &kube_client).await {
        Ok(p) => {
            eprintln!("✅ AWS Secrets Manager provider created successfully");
            p
        }
        Err(e) => {
            eprintln!("❌ Failed to create AWS provider: {}", e);
            eprintln!("   Error details: {:?}", e);
            panic!("Failed to create AWS provider: {}", e);
        }
    };

    // Call the actual provider method - should update since value changed
    let result = provider
        .create_or_update_secret("test-secret-name", "new-secret-value", "test", "us-east-1")
        .await;

    assert!(result.is_ok());
    assert!(result.unwrap()); // Should return true (secret was updated)

    // Explicit teardown to ensure cleanup happens synchronously
    _fixture.teardown();
}

#[tokio::test]
async fn test_aws_provider_no_change_with_pact() {
    // Acquire test mutex — recover from poison so a single test failure doesn't cascade
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    init_test();

    // Check for Kubernetes cluster BEFORE creating the mock server
    eprintln!("🔧 Creating Kubernetes client...");
    let kube_client = match create_test_kube_client().await {
        Some(client) => {
            eprintln!("✅ Kubernetes client created");
            client
        }
        None => {
            eprintln!("⚠️  Skipping test: No Kubernetes cluster available");
            eprintln!("💡 To run this test, ensure a Kubernetes cluster is available:");
            eprintln!("   - Run 'kind create cluster' for local testing");
            eprintln!("   - Or set KUBECONFIG environment variable");
            eprintln!("   - Or ensure in-cluster config is available");
            return;
        }
    };

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

    // Set up Pact environment variables using test fixture
    // The fixture will automatically clean up when it goes out of scope
    let _fixture = setup_pact_environment(base_url.clone()).await;

    let config = AwsConfig {
        region: "us-east-1".to_string(),
        auth: None,
    };

    eprintln!("🔧 Creating AWS Secrets Manager provider...");
    eprintln!("   Endpoint: {}", base_url);
    eprintln!(
        "   PACT_MODE: {}",
        env::var("PACT_MODE").unwrap_or_else(|_| "not set".to_string())
    );
    eprintln!(
        "   AWS_SECRETS_MANAGER_ENDPOINT: {}",
        env::var("AWS_SECRETS_MANAGER_ENDPOINT").unwrap_or_else(|_| "not set".to_string())
    );

    let provider = match AwsSecretManager::new(&config, &kube_client).await {
        Ok(p) => {
            eprintln!("✅ AWS Secrets Manager provider created successfully");
            p
        }
        Err(e) => {
            eprintln!("❌ Failed to create AWS provider: {}", e);
            eprintln!("   Error details: {:?}", e);
            panic!("Failed to create AWS provider: {}", e);
        }
    };

    // Call the actual provider method - should return false (no change)
    let result = provider
        .create_or_update_secret("test-secret-name", "test-secret-value", "test", "us-east-1")
        .await;

    assert!(result.is_ok());
    assert!(!result.unwrap()); // Should return false (no change needed)

    // Explicit teardown to ensure cleanup happens synchronously
    _fixture.teardown();
}
