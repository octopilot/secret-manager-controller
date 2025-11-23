//! # Kind Cluster Setup Utilities
//!
//! Utilities for managing Kind clusters for integration tests.
//! Provides functions to ensure cluster exists, deploy infrastructure,
//! and get service endpoints.

use anyhow::{Context, Result};
use kube::Client;
use kube::api::Api;
use kube::core::ObjectMeta;
use std::process::Command;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{info, warn};

/// Ensure Kind cluster exists and return Kubernetes client
///
/// Checks if a Kind cluster with the given name exists, and creates it if it doesn't.
/// Returns a Kubernetes client connected to the cluster.
pub async fn ensure_kind_cluster(name: &str) -> Result<Client> {
    info!("Checking if Kind cluster '{}' exists...", name);

    // Check if cluster exists by trying to get cluster info
    let output = Command::new("kubectl")
        .args(&["cluster-info", "--context", &format!("kind-{}", name)])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            info!("Kind cluster '{}' already exists", name);
        }
        _ => {
            info!("Kind cluster '{}' does not exist", name);
            warn!(
                "Cluster '{}' not found. Please ensure cluster is created before running tests.",
                name
            );
            warn!("In CI, the cluster should be created by the workflow.");
            warn!("Locally, run: kind create cluster --config kind-config.yaml --name {}", name);
        }
    }

    // Create Kubernetes client
    let client = Client::try_default()
        .await
        .context("Failed to create Kubernetes client. Ensure cluster is accessible.")?;

    info!("✅ Kubernetes client created successfully");
    Ok(client)
}

/// Get service endpoint from cluster
///
/// Returns the service endpoint in the format: http://<service-name>.<namespace>.svc.cluster.local:<port>
pub fn get_service_endpoint(service_name: &str, namespace: &str, port: u16) -> String {
    format!("http://{}.{}.svc.cluster.local:{}", service_name, namespace, port)
}

/// Wait for a service to be ready
///
/// Checks if the service has endpoints available.
pub async fn wait_for_service_ready(
    client: &Client,
    service_name: &str,
    namespace: &str,
    timeout: Duration,
) -> Result<()> {
    use kube::api::Api as KubeApi;
    use k8s_openapi::api::core::v1::{Service, Endpoints};

    let services: KubeApi<Service> = KubeApi::namespaced(client.clone(), namespace);
    let endpoints: KubeApi<Endpoints> = KubeApi::namespaced(client.clone(), namespace);

    let start = Instant::now();
    while start.elapsed() < timeout {
        // Check if service exists
        if services.get(service_name).await.is_ok() {
            // Check if service has endpoints
            if let Ok(endpoint) = endpoints.get(service_name).await {
                if let Some(subsets) = &endpoint.subsets {
                    if !subsets.is_empty() {
                        info!("✅ Service '{}' in namespace '{}' is ready", service_name, namespace);
                        return Ok(());
                    }
                }
            }
        }

        sleep(Duration::from_millis(500)).await;
    }

    Err(anyhow::anyhow!(
        "Service '{}' in namespace '{}' did not become ready within {:?}",
        service_name,
        namespace,
        timeout
    ))
}

/// Get mock server endpoint from cluster
///
/// Returns the endpoint for a mock server service running in the cluster.
pub async fn get_mock_server_endpoint(
    client: &Client,
    provider: &str,
) -> Result<String> {
    let namespace = "secret-manager-controller-pact-broker";
    
    // Map provider to service name
    let service_name = match provider {
        "aws" => "aws-mock-server",
        "gcp" => "gcp-mock-server",
        "azure" => "azure-mock-server",
        _ => {
            return Err(anyhow::anyhow!("Unknown provider: {}", provider));
        }
    };

    // Default port for mock servers
    let port = 1234;

    // Wait for service to be ready
    wait_for_service_ready(client, service_name, namespace, Duration::from_secs(60))
        .await
        .context(format!("Mock server '{}' not ready", service_name))?;

    let endpoint = get_service_endpoint(service_name, namespace, port);
    info!("✅ Mock server endpoint for {}: {}", provider, endpoint);
    Ok(endpoint)
}

/// Wait for controller to be ready
///
/// Checks if the controller deployment is available.
pub async fn wait_for_controller_ready(
    client: &Client,
    namespace: &str,
    timeout: Duration,
) -> Result<()> {
    use kube::api::Api as KubeApi;
    use k8s_openapi::api::apps::v1::Deployment;

    let deployments: KubeApi<Deployment> = KubeApi::namespaced(client.clone(), namespace);

    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(deployment) = deployments.get("secret-manager-controller").await {
            if let Some(status) = &deployment.status {
                if let Some(conditions) = &status.conditions {
                    for condition in conditions {
                        if condition.type_ == "Available" && condition.status == "True" {
                            info!("✅ Controller deployment is ready");
                            return Ok(());
                        }
                    }
                }
            }
        }

        sleep(Duration::from_millis(1000)).await;
    }

    Err(anyhow::anyhow!(
        "Controller deployment did not become ready within {:?}",
        timeout
    ))
}

