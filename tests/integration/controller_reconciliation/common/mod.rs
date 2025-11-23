//! Common utilities for end-to-end reconciliation tests
//!
//! Provides shared functionality for:
//! - Creating test SecretManagerConfig resources with GitRepository
//! - Creating test secret files
//! - Setting up test environments
//! - Verifying reconciliation results

use controller::prelude::*;
use kube::Client;
use serde_json::json;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tokio::fs;
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

/// Find an available port for the mock server
async fn find_available_port() -> Result<u16, Box<dyn std::error::Error>> {
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    Ok(port)
}

/// Wait for mock server to be ready
async fn wait_for_server(
    endpoint: &str,
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let client = reqwest::Client::new();

    while start.elapsed() < timeout {
        if let Ok(response) = client.get(&format!("{}/health", endpoint)).send().await {
            if response.status().is_success() {
                info!("Mock server ready at {}", endpoint);
                return Ok(());
            }
        }
        sleep(Duration::from_millis(100)).await;
    }

    Err(format!("Mock server did not become ready within {:?}", timeout).into())
}

/// Start a GCP mock server
pub async fn start_gcp_mock_server() -> Result<MockServer, Box<dyn std::error::Error>> {
    let port = find_available_port().await?;
    let endpoint = format!("http://localhost:{}", port);

    info!("Starting GCP mock server on {}", endpoint);

    let binary_path = "pact-broker/mock-server/target/release/gcp-mock-server";
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

/// Create a test Kubernetes client
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

/// Set up environment variables for Pact mode
/// Also initializes PactModeConfig singleton
pub fn setup_pact_mode(provider: &str, endpoint: &str) {
    env::set_var("PACT_MODE", "true");
    env::set_var("__PACT_MODE_TEST__", "true"); // Allow re-initialization in tests

    match provider {
        "gcp" => {
            env::set_var("GCP_SECRET_MANAGER_ENDPOINT", endpoint);
        }
        "aws" => {
            env::set_var("AWS_SECRETS_MANAGER_ENDPOINT", endpoint);
        }
        "azure" => {
            env::set_var("AZURE_KEY_VAULT_ENDPOINT", endpoint);
        }
        _ => {}
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
    env::remove_var("PACT_MODE");
    env::remove_var("__PACT_MODE_TEST__");

    match provider {
        "gcp" => {
            env::remove_var("GCP_SECRET_MANAGER_ENDPOINT");
        }
        "aws" => {
            env::remove_var("AWS_SECRETS_MANAGER_ENDPOINT");
        }
        "azure" => {
            env::remove_var("AZURE_KEY_VAULT_ENDPOINT");
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

/// Create a test SecretManagerConfig for GCP with GitRepository reference
pub fn create_gcp_reconciliation_config(
    name: &str,
    namespace: &str,
    project: &str,
    mock_endpoint: &str,
    git_repo_name: &str,
    git_repo_namespace: &str,
) -> SecretManagerConfig {
    // Set up Pact mode
    env::set_var("PACT_MODE", "true");
    env::set_var("GCP_SECRET_MANAGER_ENDPOINT", mock_endpoint);

    SecretManagerConfig {
        metadata: kube::api::ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        spec: SecretManagerConfigSpec {
            source_ref: SourceRef {
                kind: "GitRepository".to_string(),
                name: git_repo_name.to_string(),
                namespace: git_repo_namespace.to_string(),
                git_credentials: None,
            },
            secrets: SecretsConfig {
                environment: "test".to_string(),
                prefix: Some("test-service".to_string()),
                suffix: None,
                kustomize_path: None,
                base_path: None,
            },
            provider: ProviderConfig::Gcp(GcpConfig {
                project_id: project.to_string(),
                auth: None,
            }),
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

/// Create a test SecretManagerConfig for AWS with GitRepository reference
pub fn create_aws_reconciliation_config(
    name: &str,
    namespace: &str,
    region: &str,
    mock_endpoint: &str,
    git_repo_name: &str,
    git_repo_namespace: &str,
) -> SecretManagerConfig {
    // Set up Pact mode
    env::set_var("PACT_MODE", "true");
    env::set_var("AWS_SECRETS_MANAGER_ENDPOINT", mock_endpoint);

    SecretManagerConfig {
        metadata: kube::api::ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        spec: SecretManagerConfigSpec {
            source_ref: SourceRef {
                kind: "GitRepository".to_string(),
                name: git_repo_name.to_string(),
                namespace: git_repo_namespace.to_string(),
                git_credentials: None,
            },
            secrets: SecretsConfig {
                environment: "test".to_string(),
                prefix: Some("test-service".to_string()),
                suffix: None,
                kustomize_path: None,
                base_path: None,
            },
            provider: ProviderConfig::Aws(AwsConfig {
                region: region.to_string(),
                auth: None,
            }),
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

/// Create a test SecretManagerConfig for Azure with GitRepository reference
pub fn create_azure_reconciliation_config(
    name: &str,
    namespace: &str,
    vault_name: &str,
    mock_endpoint: &str,
    git_repo_name: &str,
    git_repo_namespace: &str,
) -> SecretManagerConfig {
    // Set up Pact mode
    env::set_var("PACT_MODE", "true");
    env::set_var("AZURE_KEY_VAULT_ENDPOINT", mock_endpoint);

    SecretManagerConfig {
        metadata: kube::api::ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        spec: SecretManagerConfigSpec {
            source_ref: SourceRef {
                kind: "GitRepository".to_string(),
                name: git_repo_name.to_string(),
                namespace: git_repo_namespace.to_string(),
                git_credentials: None,
            },
            secrets: SecretsConfig {
                environment: "test".to_string(),
                prefix: Some("test-service".to_string()),
                suffix: None,
                kustomize_path: None,
                base_path: None,
            },
            provider: ProviderConfig::Azure(AzureConfig {
                vault_name: vault_name.to_string(),
                auth: None,
            }),
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

/// Create a default SharedControllerConfig for tests
/// Uses default values suitable for testing
pub fn create_test_controller_config(
) -> std::sync::Arc<tokio::sync::RwLock<controller::config::ControllerConfig>> {
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let config = ControllerConfig::default();
    Arc::new(RwLock::new(config))
}

/// Create a test secret file (application.secrets.env format)
pub async fn create_test_secret_file(
    temp_dir: &Path,
    secrets: &[(&str, &str)],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let secret_file = temp_dir.join("application.secrets.env");
    let mut content = String::new();

    for (key, value) in secrets {
        content.push_str(&format!("{}={}\n", key, value));
    }

    fs::write(&secret_file, content).await?;
    Ok(secret_file)
}

/// Create a mock GitRepository resource in Kubernetes
pub async fn create_mock_git_repository(
    _client: &Client,
    name: &str,
    namespace: &str,
    artifact_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create a mock GitRepository that points to our test artifact
    // In a real scenario, this would be created by FluxCD
    // For testing, we'll create a minimal GitRepository resource
    let _git_repo = json!({
        "apiVersion": "source.toolkit.fluxcd.io/v1",
        "kind": "GitRepository",
        "metadata": {
            "name": name,
            "namespace": namespace,
        },
        "spec": {
            "url": "https://github.com/test/repo",
            "ref": {
                "branch": "main"
            }
        },
        "status": {
            "artifact": {
                "url": format!("file://{}", artifact_path.display()),
                "path": artifact_path.to_string_lossy(),
                "revision": "test-revision",
            },
            "conditions": [{
                "type": "Ready",
                "status": "True",
                "reason": "Succeeded",
                "message": "Fetched revision: test-revision"
            }]
        }
    });

    // Note: In a real test, we'd use the kube client to create this
    // For now, we'll just log that we would create it
    info!(
        "Would create GitRepository: {} in namespace: {}",
        name, namespace
    );
    info!("Artifact path: {}", artifact_path.display());

    Ok(())
}

/// Verify secret exists in mock server (GCP)
pub async fn verify_gcp_secret(
    mock_endpoint: &str,
    project: &str,
    secret_name: &str,
    expected_value: Option<&str>,
) -> Result<bool, Box<dyn std::error::Error>> {
    use base64::{engine::general_purpose, Engine as _};
    use reqwest::Client;

    let client = Client::new();

    // Get secret value
    let url = format!(
        "{}/v1/projects/{}/secrets/{}/versions/latest:access",
        mock_endpoint, project, secret_name
    );
    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Ok(false);
    }

    if let Some(expected) = expected_value {
        let json: serde_json::Value = response.json().await?;
        if let Some(data) = json
            .get("payload")
            .and_then(|p| p.get("data"))
            .and_then(|d| d.as_str())
        {
            let decoded = general_purpose::STANDARD.decode(data)?;
            let value = String::from_utf8(decoded)?;
            return Ok(value == expected);
        }
    }

    Ok(true)
}

/// Verify secret exists in mock server (AWS)
pub async fn verify_aws_secret(
    mock_endpoint: &str,
    secret_name: &str,
    expected_value: Option<&str>,
) -> Result<bool, Box<dyn std::error::Error>> {
    use reqwest::Client;

    let client = Client::new();

    // Get secret value
    let body = json!({
        "SecretId": secret_name
    });
    let response = client
        .post(&format!("{}/", mock_endpoint))
        .header("x-amz-target", "secretsmanager.GetSecretValue")
        .header("content-type", "application/x-amz-json-1.1")
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(false);
    }

    if let Some(expected) = expected_value {
        let json: serde_json::Value = response.json().await?;
        if let Some(value) = json.get("SecretString").and_then(|v| v.as_str()) {
            return Ok(value == expected);
        }
    }

    Ok(true)
}

/// Verify secret exists in mock server (Azure)
pub async fn verify_azure_secret(
    mock_endpoint: &str,
    secret_name: &str,
    expected_value: Option<&str>,
) -> Result<bool, Box<dyn std::error::Error>> {
    use reqwest::Client;

    let client = Client::new();

    // Get secret value
    let url = format!(
        "{}/secrets/{}/?api-version=2025-07-01",
        mock_endpoint, secret_name
    );
    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        return Ok(false);
    }

    if let Some(expected) = expected_value {
        let json: serde_json::Value = response.json().await?;
        if let Some(value) = json.get("value").and_then(|v| v.as_str()) {
            return Ok(value == expected);
        }
    }

    Ok(true)
}

// Export GitRepository utilities
pub mod gitrepository;

pub use gitrepository::{
    create_argocd_application, create_flux_git_repository, setup_argocd_repo_path,
    setup_flux_artifact_path, update_git_repository_artifact_path,
    wait_for_argocd_application_ready, wait_for_git_repository_ready,
};

// Export test fixtures
pub mod fixtures;

pub use fixtures::{
    comment_out_secret, create_test_secret_files, create_test_secret_manager_config_argocd,
    create_test_secret_manager_config_aws_argocd, create_test_secret_manager_config_aws_flux,
    create_test_secret_manager_config_azure_argocd, create_test_secret_manager_config_azure_flux,
    create_test_secret_manager_config_flux, delete_secret_file, modify_secret_file,
    uncomment_secret,
};

// Export Kind cluster utilities
pub mod kind_cluster;

pub use kind_cluster::{
    cleanup_kind_cluster, ensure_kind_cluster, install_argocd_application_crd,
    install_fluxcd_source_controller, setup_test_environment, wait_for_argocd_crd_ready,
    wait_for_fluxcd_ready,
};
