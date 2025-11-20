//! # Kind Cluster Setup Utilities
//!
//! Utilities for managing Kind clusters for integration tests.
//! Provides functions to ensure cluster exists, install required components,
//! and clean up after tests.

use anyhow::{Context, Result};
use kube::Client;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{info, warn};

/// Ensure Kind cluster exists
///
/// Checks if a Kind cluster with the given name exists, and creates it if it doesn't.
pub async fn ensure_kind_cluster(name: &str) -> Result<()> {
    info!("Checking if Kind cluster '{}' exists...", name);

    // Check if cluster exists
    let output = Command::new("kubectl")
        .args(&["cluster-info", "--context", &format!("kind-{}", name)])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            info!("Kind cluster '{}' already exists", name);
            return Ok(());
        }
        _ => {
            info!("Kind cluster '{}' does not exist, creating...", name);
        }
    }

    // Create cluster
    let create_output = Command::new("kind")
        .args(&["create", "cluster", "--name", name])
        .output()
        .context("Failed to execute 'kind create cluster'")?;

    if !create_output.status.success() {
        let stderr = String::from_utf8_lossy(&create_output.stderr);
        // Check if cluster already exists (this is okay)
        if stderr.contains("already exists") {
            info!(
                "Kind cluster '{}' already exists (from concurrent creation)",
                name
            );
            return Ok(());
        }
        return Err(anyhow::anyhow!("Failed to create Kind cluster: {}", stderr));
    }

    info!("Successfully created Kind cluster '{}'", name);
    Ok(())
}

/// Install FluxCD source-controller
///
/// Installs the FluxCD source-controller in the Kind cluster.
/// This is required for GitRepository support.
pub async fn install_fluxcd_source_controller() -> Result<()> {
    info!("Installing FluxCD source-controller...");

    // Apply FluxCD source-controller manifest
    let apply_output = Command::new("kubectl")
        .args(&[
            "apply",
            "-f",
            "https://github.com/fluxcd/source-controller/releases/latest/download/source-controller.yaml",
        ])
        .output()
        .context("Failed to execute 'kubectl apply' for FluxCD source-controller")?;

    if !apply_output.status.success() {
        let stderr = String::from_utf8_lossy(&apply_output.stderr);
        return Err(anyhow::anyhow!(
            "Failed to install FluxCD source-controller: {}",
            stderr
        ));
    }

    info!("FluxCD source-controller manifest applied, waiting for ready...");

    // Wait for source-controller to be ready
    wait_for_fluxcd_ready().await?;

    Ok(())
}

/// Wait for FluxCD source-controller to be ready
///
/// Polls until the source-controller pod is ready.
pub async fn wait_for_fluxcd_ready() -> Result<()> {
    let timeout = Duration::from_secs(300); // 5 minutes
    let start = Instant::now();
    let poll_interval = Duration::from_secs(2);

    while start.elapsed() < timeout {
        let output = Command::new("kubectl")
            .args(&[
                "wait",
                "--for=condition=ready",
                "pod",
                "-l",
                "app=source-controller",
                "-n",
                "flux-system",
                "--timeout=10s",
            ])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                info!("FluxCD source-controller is ready!");
                return Ok(());
            }
            _ => {
                // Continue polling
            }
        }

        sleep(poll_interval).await;
    }

    Err(anyhow::anyhow!(
        "FluxCD source-controller did not become ready within {:?}",
        timeout
    ))
}

/// Install ArgoCD Application CRD
///
/// Installs only the ArgoCD Application CRD (minimal installation).
/// This is sufficient because the controller clones repos itself using git binary.
pub async fn install_argocd_application_crd() -> Result<()> {
    info!("Installing ArgoCD Application CRD (minimal installation)...");

    // Apply ArgoCD Application CRD
    let apply_output = Command::new("kubectl")
        .args(&[
            "apply",
            "-f",
            "https://raw.githubusercontent.com/argoproj/argo-cd/stable/manifests/crds/application-crd.yaml",
        ])
        .output()
        .context("Failed to execute 'kubectl apply' for ArgoCD Application CRD")?;

    if !apply_output.status.success() {
        let stderr = String::from_utf8_lossy(&apply_output.stderr);
        return Err(anyhow::anyhow!(
            "Failed to install ArgoCD Application CRD: {}",
            stderr
        ));
    }

    info!("ArgoCD Application CRD manifest applied, waiting for established...");

    // Wait for CRD to be established
    wait_for_argocd_crd_ready().await?;

    Ok(())
}

/// Wait for ArgoCD Application CRD to be ready
///
/// Polls until the Application CRD is established.
pub async fn wait_for_argocd_crd_ready() -> Result<()> {
    let timeout = Duration::from_secs(60); // 1 minute
    let start = Instant::now();
    let poll_interval = Duration::from_millis(500);

    while start.elapsed() < timeout {
        let output = Command::new("kubectl")
            .args(&[
                "wait",
                "--for",
                "condition=established",
                "--timeout=10s",
                "crd",
                "applications.argoproj.io",
            ])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                info!("ArgoCD Application CRD is established!");
                return Ok(());
            }
            _ => {
                // Continue polling
            }
        }

        sleep(poll_interval).await;
    }

    Err(anyhow::anyhow!(
        "ArgoCD Application CRD did not become established within {:?}",
        timeout
    ))
}

/// Cleanup Kind cluster
///
/// Deletes the Kind cluster with the given name.
/// This is useful for CI/CD cleanup.
pub async fn cleanup_kind_cluster(name: &str) -> Result<()> {
    info!("Cleaning up Kind cluster '{}'...", name);

    let output = Command::new("kind")
        .args(&["delete", "cluster", "--name", name])
        .output()
        .context("Failed to execute 'kind delete cluster'")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Check if cluster doesn't exist (this is okay)
        if stderr.contains("not found") || stderr.contains("No such cluster") {
            warn!(
                "Kind cluster '{}' does not exist, nothing to clean up",
                name
            );
            return Ok(());
        }
        return Err(anyhow::anyhow!("Failed to delete Kind cluster: {}", stderr));
    }

    info!("Successfully deleted Kind cluster '{}'", name);
    Ok(())
}

/// Setup complete test environment
///
/// Ensures Kind cluster exists and all required components are installed.
/// This is a convenience function that sets up everything needed for integration tests.
pub async fn setup_test_environment(cluster_name: &str) -> Result<Client> {
    info!(
        "Setting up test environment for cluster '{}'...",
        cluster_name
    );

    // Ensure Kind cluster exists
    ensure_kind_cluster(cluster_name).await?;

    // Install FluxCD source-controller
    install_fluxcd_source_controller().await?;

    // Install ArgoCD Application CRD
    install_argocd_application_crd().await?;

    // Create Kubernetes client
    let client = Client::try_default()
        .await
        .context("Failed to create Kubernetes client")?;

    info!("Test environment setup complete!");
    Ok(client)
}
