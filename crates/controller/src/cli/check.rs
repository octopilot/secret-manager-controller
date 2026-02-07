//! Check command for Secret Manager Controller
//!
//! Verifies that the controller is properly installed and healthy in the cluster.
//! Similar to `flux check`, this command performs a series of checks to validate
//! the installation.

use anyhow::{Context, Result};
use k8s_openapi::{
    api::apps::v1::Deployment,
    apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
};
use kube::{api::Api, Client};

/// Check the Secret Manager Controller installation
pub async fn check_command(client: Client, namespace: Option<String>, pre: bool) -> Result<()> {
    let ns = namespace.as_deref().unwrap_or("octopilot-system");

    if pre {
        return check_prerequisites(client).await;
    }

    println!("► checking prerequisites");
    check_prerequisites(client.clone()).await?;

    println!("► checking controller");
    check_controller(client.clone(), ns).await?;

    println!("► checking crds");
    check_crds(client.clone()).await?;

    println!("► checking rbac");
    check_rbac(client.clone(), ns).await?;

    println!("✅ all checks passed");

    Ok(())
}

/// Check prerequisites (Kubernetes version, kubectl availability)
async fn check_prerequisites(_client: Client) -> Result<()> {
    // Check Kubernetes version
    let output = std::process::Command::new("kubectl")
        .args(&["version", "--client", "--short"])
        .output()
        .context("Failed to execute kubectl. Ensure kubectl is installed and in PATH.")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("kubectl is not available or not working"));
    }

    // Parse Kubernetes version (minimal check - just verify it's available)
    let version_output = String::from_utf8_lossy(&output.stdout);
    if version_output.contains("Client Version:") {
        println!("✔ kubectl is available");
    } else {
        println!("✗ kubectl version check failed");
        return Err(anyhow::anyhow!("kubectl version check failed"));
    }

    // Check if we can connect to cluster
    let output = std::process::Command::new("kubectl")
        .args(&["cluster-info"])
        .output()
        .context("Failed to connect to Kubernetes cluster")?;

    if output.status.success() {
        println!("✔ Kubernetes cluster is accessible");
    } else {
        println!("✗ Cannot connect to Kubernetes cluster");
        return Err(anyhow::anyhow!("Cannot connect to Kubernetes cluster"));
    }

    Ok(())
}

/// Check controller deployment
async fn check_controller(client: Client, namespace: &str) -> Result<()> {
    let api: Api<Deployment> = Api::namespaced(client, namespace);

    // Check if deployment exists
    match api.get("secret-manager-controller").await {
        Ok(deployment) => {
            // Check deployment status
            let status = deployment.status.as_ref();
            let ready_replicas = status.and_then(|s| s.ready_replicas).unwrap_or(0);
            let replicas = deployment
                .spec
                .as_ref()
                .and_then(|s| s.replicas)
                .unwrap_or(1);

            if ready_replicas >= replicas {
                // Get image from deployment
                let image = deployment
                    .spec
                    .as_ref()
                    .and_then(|s| s.template.spec.as_ref())
                    .and_then(|spec| spec.containers.first().and_then(|c| c.image.as_ref()))
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());

                println!("✔ secret-manager-controller: deployment ready");
                println!("  ► {}", image);
            } else {
                println!(
                    "✗ secret-manager-controller: deployment not ready ({}/{} replicas)",
                    ready_replicas, replicas
                );
                return Err(anyhow::anyhow!("Controller deployment is not ready"));
            }
        }
        Err(kube::Error::Api(e)) if e.code == 404 => {
            println!("✗ secret-manager-controller: deployment not found");
            return Err(anyhow::anyhow!(
                "Controller deployment not found in namespace '{}'",
                namespace
            ));
        }
        Err(e) => {
            println!("✗ secret-manager-controller: error checking deployment");
            return Err(e.into());
        }
    }

    // Check if pods are running
    let output = std::process::Command::new("kubectl")
        .args(&[
            "get",
            "pods",
            "-n",
            namespace,
            "-l",
            "app=secret-manager-controller",
            "--field-selector=status.phase=Running",
            "-o",
            "name",
        ])
        .output()
        .context("Failed to check controller pods")?;

    if output.status.success() {
        let pod_output = String::from_utf8_lossy(&output.stdout);
        if pod_output.contains("secret-manager-controller") {
            println!("✔ secret-manager-controller: pods running");
        } else {
            println!("✗ secret-manager-controller: no running pods found");
            return Err(anyhow::anyhow!("No running controller pods found"));
        }
    } else {
        println!("✗ secret-manager-controller: error checking pods");
        return Err(anyhow::anyhow!("Failed to check controller pods"));
    }

    Ok(())
}

/// Check CRDs
async fn check_crds(client: Client) -> Result<()> {
    let api: Api<CustomResourceDefinition> = Api::all(client);

    // Check SecretManagerConfig CRD
    let crd_name = "secretmanagerconfigs.secret-management.octopilot.io";
    match api.get(crd_name).await {
        Ok(crd) => {
            // Check CRD version
            let version = crd
                .spec
                .versions
                .first()
                .map(|v| v.name.as_str())
                .unwrap_or("unknown");

            println!("✔ {}/{}", crd_name, version);
        }
        Err(kube::Error::Api(e)) if e.code == 404 => {
            println!("✗ {}: CRD not found", crd_name);
            return Err(anyhow::anyhow!("SecretManagerConfig CRD not found"));
        }
        Err(e) => {
            println!("✗ {}: error checking CRD", crd_name);
            return Err(e.into());
        }
    }

    Ok(())
}

/// Check RBAC resources
async fn check_rbac(_client: Client, namespace: &str) -> Result<()> {
    // Check ServiceAccount
    let output = std::process::Command::new("kubectl")
        .args(&[
            "get",
            "serviceaccount",
            "secret-manager-controller",
            "-n",
            namespace,
        ])
        .output()
        .context("Failed to check ServiceAccount")?;

    if output.status.success() {
        println!("✔ ServiceAccount: secret-manager-controller exists");
    } else {
        println!("✗ ServiceAccount: secret-manager-controller not found");
        return Err(anyhow::anyhow!("ServiceAccount not found"));
    }

    // Check ClusterRole
    let output = std::process::Command::new("kubectl")
        .args(&["get", "clusterrole", "secret-manager-controller"])
        .output()
        .context("Failed to check ClusterRole")?;

    if output.status.success() {
        println!("✔ ClusterRole: secret-manager-controller exists");
    } else {
        println!("✗ ClusterRole: secret-manager-controller not found");
        return Err(anyhow::anyhow!("ClusterRole not found"));
    }

    // Check ClusterRoleBinding
    let output = std::process::Command::new("kubectl")
        .args(&["get", "clusterrolebinding", "secret-manager-controller"])
        .output()
        .context("Failed to check ClusterRoleBinding")?;

    if output.status.success() {
        println!("✔ ClusterRoleBinding: secret-manager-controller exists");
    } else {
        println!("✗ ClusterRoleBinding: secret-manager-controller not found");
        return Err(anyhow::anyhow!("ClusterRoleBinding not found"));
    }

    Ok(())
}
