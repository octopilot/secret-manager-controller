//! Common utilities for controller integration tests with mock servers
//!
//! Provides shared functionality for:
//! - Starting/connecting to mock servers
//! - Creating test SecretManagerConfig resources
//! - Setting up test environments
//! - Verifying secret state in mock servers

use controller::prelude::*;
use kube::Client;
use serde_json::json;
use std::env;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::info;

/// Initialize rustls crypto provider for tests
pub fn init_rustls() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("Failed to install rustls crypto provider");
    });
}

/// Mock server process handle
#[derive(Debug)]
pub struct MockServer {
    process: Child,
    endpoint: String,
}

impl MockServer {
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn stop(mut self) -> std::io::Result<()> {
        self.process.kill()?;
        self.process.wait()?;
        Ok(())
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

/// Start a GCP mock server
/// Returns the mock server process and endpoint URL
///
/// **Note**: Requires the mock server binary to be built first:
/// `cd pact-broker/mock-server && cargo build --bin gcp-mock-server --release`
pub async fn start_gcp_mock_server() -> Result<MockServer, Box<dyn std::error::Error>> {
    let port = find_available_port().await?;
    let endpoint = format!("http://localhost:{}", port);

    info!("Starting GCP mock server on {}", endpoint);

    // Try to use release binary first, fall back to cargo run
    let binary_path = "pact-broker/mock-server/target/release/gcp-mock-server";
    let mut cmd = if std::path::Path::new(binary_path).exists() {
        let mut c = Command::new(binary_path);
        c.env("PORT", port.to_string())
            .env("PACT_MODE", "false")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        c
    } else {
        // Fall back to cargo run (slower but works if binary not built)
        let mut c = Command::new("cargo");
        c.args(&[
            "run",
            "--bin",
            "gcp-mock-server",
            "--manifest-path",
            "pact-broker/mock-server/Cargo.toml",
            "--release",
        ])
        .env("PORT", port.to_string())
        .env("PACT_MODE", "false")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
        c
    };

    let process = cmd.spawn()?;

    // Wait for server to be ready (longer timeout for cargo run)
    wait_for_server(&endpoint, Duration::from_secs(30)).await?;

    Ok(MockServer { process, endpoint })
}

/// Start an AWS mock server
pub async fn start_aws_mock_server() -> Result<MockServer, Box<dyn std::error::Error>> {
    let port = find_available_port().await?;
    let endpoint = format!("http://localhost:{}", port);

    info!("Starting AWS mock server on {}", endpoint);

    let binary_path = "pact-broker/mock-server/target/release/aws-mock-server";
    let mut cmd = if std::path::Path::new(binary_path).exists() {
        let mut c = Command::new(binary_path);
        c.env("PORT", port.to_string())
            .env("PACT_MODE", "false")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        c
    } else {
        let mut c = Command::new("cargo");
        c.args(&[
            "run",
            "--bin",
            "aws-mock-server",
            "--manifest-path",
            "pact-broker/mock-server/Cargo.toml",
            "--release",
        ])
        .env("PORT", port.to_string())
        .env("PACT_MODE", "false")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
        c
    };

    let process = cmd.spawn()?;

    wait_for_server(&endpoint, Duration::from_secs(30)).await?;

    Ok(MockServer { process, endpoint })
}

/// Start an Azure mock server
pub async fn start_azure_mock_server() -> Result<MockServer, Box<dyn std::error::Error>> {
    let port = find_available_port().await?;
    let endpoint = format!("http://localhost:{}", port);

    info!("Starting Azure mock server on {}", endpoint);

    let binary_path = "pact-broker/mock-server/target/release/azure-mock-server";
    let mut cmd = if std::path::Path::new(binary_path).exists() {
        let mut c = Command::new(binary_path);
        c.env("PORT", port.to_string())
            .env("PACT_MODE", "false")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        c
    } else {
        let mut c = Command::new("cargo");
        c.args(&[
            "run",
            "--bin",
            "azure-mock-server",
            "--manifest-path",
            "pact-broker/mock-server/Cargo.toml",
            "--release",
        ])
        .env("PORT", port.to_string())
        .env("PACT_MODE", "false")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
        c
    };

    let process = cmd.spawn()?;

    wait_for_server(&endpoint, Duration::from_secs(30)).await?;

    Ok(MockServer { process, endpoint })
}

/// Find an available port for the mock server
async fn find_available_port() -> Result<u16, Box<dyn std::error::Error>> {
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    Ok(port)
}

/// Wait for a server to be ready by checking the health endpoint
async fn wait_for_server(
    endpoint: &str,
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let health_url = format!("{}/health", endpoint);
    let start = Instant::now();

    while start.elapsed() < timeout {
        if let Ok(response) = client.get(&health_url).send().await {
            if response.status().is_success() {
                info!("Mock server ready at {}", endpoint);
                return Ok(());
            }
        }
        sleep(Duration::from_millis(100)).await;
    }

    Err(format!("Mock server did not become ready within {:?}", timeout).into())
}

/// Create a default SharedControllerConfig for tests
/// Uses default values suitable for testing
pub fn create_test_controller_config()
-> std::sync::Arc<tokio::sync::RwLock<controller::config::ControllerConfig>> {
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let config = ControllerConfig::default();
    Arc::new(RwLock::new(config))
}

/// Create a test SecretManagerConfig for GCP
pub fn create_gcp_test_config(
    name: &str,
    namespace: &str,
    project_id: &str,
    _mock_server_endpoint: &str,
) -> SecretManagerConfig {
    SecretManagerConfig {
        metadata: kube::api::ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        spec: SecretManagerConfigSpec {
            source_ref: SourceRef {
                kind: "GitRepository".to_string(),
                name: "test-repo".to_string(),
                namespace: "default".to_string(),
                git_credentials: None,
            },
            provider: ProviderConfig::Gcp(GcpConfig {
                project_id: project_id.to_string(),
                location: "us-central1".to_string(),
                auth: None,
            }),
            secrets: SecretsConfig {
                environment: "test".to_string(),
                prefix: Some("test-service".to_string()),
                suffix: None,
                kustomize_path: None,
                base_path: None,
            },
            configs: None,
            otel: None,
            git_repository_pull_interval: "1m".to_string(),
            reconcile_interval: "1m".to_string(),
            diff_discovery: true,
            trigger_update: true,
            suspend: false,
            suspend_git_pulls: false,
            notifications: None,
            hot_reload: None,
            logging: None,
        },
        status: None,
    }
}

/// Create a test SecretManagerConfig for AWS
pub fn create_aws_test_config(
    name: &str,
    namespace: &str,
    region: &str,
    _mock_server_endpoint: &str,
) -> SecretManagerConfig {
    SecretManagerConfig {
        metadata: kube::api::ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        spec: SecretManagerConfigSpec {
            source_ref: SourceRef {
                kind: "GitRepository".to_string(),
                name: "test-repo".to_string(),
                namespace: "default".to_string(),
                git_credentials: None,
            },
            provider: ProviderConfig::Aws(AwsConfig {
                region: region.to_string(),
                auth: None,
            }),
            secrets: SecretsConfig {
                environment: "test".to_string(),
                prefix: Some("test-service".to_string()),
                suffix: None,
                kustomize_path: None,
                base_path: None,
            },
            configs: None,
            otel: None,
            git_repository_pull_interval: "1m".to_string(),
            reconcile_interval: "1m".to_string(),
            diff_discovery: true,
            trigger_update: true,
            suspend: false,
            suspend_git_pulls: false,
            notifications: None,
            hot_reload: None,
            logging: None,
        },
        status: None,
    }
}

/// Create a test SecretManagerConfig for Azure
pub fn create_azure_test_config(
    name: &str,
    namespace: &str,
    vault_name: &str,
    _mock_server_endpoint: &str,
) -> SecretManagerConfig {
    SecretManagerConfig {
        metadata: kube::api::ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        spec: SecretManagerConfigSpec {
            source_ref: SourceRef {
                kind: "GitRepository".to_string(),
                name: "test-repo".to_string(),
                namespace: "default".to_string(),
                git_credentials: None,
            },
            provider: ProviderConfig::Azure(AzureConfig {
                vault_name: vault_name.to_string(),
                location: "eastus".to_string(),
                auth: None,
            }),
            secrets: SecretsConfig {
                environment: "test".to_string(),
                prefix: Some("test-service".to_string()),
                suffix: None,
                kustomize_path: None,
                base_path: None,
            },
            configs: None,
            otel: None,
            git_repository_pull_interval: "1m".to_string(),
            reconcile_interval: "1m".to_string(),
            diff_discovery: true,
            trigger_update: true,
            suspend: false,
            suspend_git_pulls: false,
            notifications: None,
            hot_reload: None,
            logging: None,
        },
        status: None,
    }
}

/// Set up environment variables for Pact mode with mock server endpoint
/// Also initializes PactModeConfig singleton
pub fn setup_pact_mode(provider: &str, endpoint: &str) {
    // SAFETY: Helper called from single-threaded integration test context.
    unsafe {
        env::set_var("PACT_MODE", "true");
        env::set_var("__PACT_MODE_TEST__", "true"); // Allow re-initialization in tests
    }

    match provider {
        "gcp" => {
            // SAFETY: Helper called from single-threaded integration test context.
            unsafe {
                env::set_var("GCP_SECRET_MANAGER_ENDPOINT", endpoint);
            }
        }
        "aws" => {
            // SAFETY: Helper called from single-threaded integration test context.
            unsafe {
                env::set_var("AWS_SECRETS_MANAGER_ENDPOINT", endpoint);
            }
        }
        "azure" => {
            // SAFETY: Helper called from single-threaded integration test context.
            unsafe {
                env::set_var("AZURE_KEY_VAULT_ENDPOINT", endpoint);
            }
        }
        _ => panic!("Unknown provider: {}", provider),
    }

    // Initialize PactModeConfig singleton
    // This is critical - the controller code requires PactModeConfig to be initialized
    if let Err(e) = controller::config::PactModeConfig::init() {
        // If already initialized, that's okay - we're in test mode so it can be re-initialized
        tracing::warn!(
            "PactModeConfig initialization warning (may be expected in tests): {}",
            e
        );
    }
}

/// Clean up environment variables and reset PactModeConfig
pub fn cleanup_pact_mode(provider: &str) {
    // SAFETY: Helper called from single-threaded integration test context.
    unsafe {
        env::remove_var("PACT_MODE");
        env::remove_var("__PACT_MODE_TEST__");
    }

    match provider {
        "gcp" => {
            // SAFETY: Helper called from single-threaded integration test context.
            unsafe {
                env::remove_var("GCP_SECRET_MANAGER_ENDPOINT");
            }
        }
        "aws" => {
            // SAFETY: Helper called from single-threaded integration test context.
            unsafe {
                env::remove_var("AWS_SECRETS_MANAGER_ENDPOINT");
            }
        }
        "azure" => {
            // SAFETY: Helper called from single-threaded integration test context.
            unsafe {
                env::remove_var("AZURE_KEY_VAULT_ENDPOINT");
            }
        }
        _ => {}
    }

    // Reset PactModeConfig state (clear providers map)
    // Note: We can't fully reset OnceLock, but we can clear the config
    let _ = std::panic::catch_unwind(|| {
        let mut config = controller::config::PactModeConfig::get();
        config.enabled = false;
        config.providers.clear();
    });
}

/// Create a test Kubernetes client
/// In tests, this will use the default kubeconfig or in-cluster config
///
/// **Note**: This will fail if no Kubernetes cluster is available.
/// For integration tests, ensure a cluster is running (e.g., `kind create cluster`)
/// or set KUBECONFIG environment variable.
pub async fn create_test_kube_client() -> Result<Client, Box<dyn std::error::Error>> {
    Client::try_default().await.map_err(|e| {
        format!(
            "Failed to create Kubernetes client. Ensure a cluster is available:\n\
                 - Run 'kind create cluster' for local testing\n\
                 - Or set KUBECONFIG environment variable\n\
                 - Or ensure in-cluster config is available\n\
                 Error: {}",
            e
        )
        .into()
    })
}

/// Verify a secret exists in the GCP mock server
pub async fn verify_gcp_secret(
    endpoint: &str,
    project: &str,
    secret_name: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/v1/projects/{}/secrets/{}",
        endpoint, project, secret_name
    );
    let response = client.get(&url).send().await?;
    Ok(response.status().is_success())
}

/// Verify a secret exists in the AWS mock server
pub async fn verify_aws_secret(
    endpoint: &str,
    secret_name: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let url = format!("{}/", endpoint);
    let body = json!({
        "SecretId": secret_name
    });
    let response = client
        .post(&url)
        .header("x-amz-target", "secretsmanager.DescribeSecret")
        .header("content-type", "application/x-amz-json-1.1")
        .json(&body)
        .send()
        .await?;
    Ok(response.status().is_success())
}

/// Verify a secret exists in the Azure mock server
pub async fn verify_azure_secret(
    endpoint: &str,
    secret_name: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/secrets/{}/?api-version=2025-07-01",
        endpoint, secret_name
    );
    let response = client.get(&url).send().await?;
    Ok(response.status().is_success())
}
