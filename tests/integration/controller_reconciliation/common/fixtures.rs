//! # Test Fixtures
//!
//! Utilities for creating test fixtures including SecretManagerConfig resources,
//! test secret files, and managing test data.

use controller::prelude::*;
use std::env;
use std::path::PathBuf;
use tokio::fs;
use tracing::info;

/// Create a test SecretManagerConfig for GCP with FluxCD GitRepository reference
pub fn create_test_secret_manager_config_flux(
    name: &str,
    namespace: &str,
    project: &str,
    mock_endpoint: &str,
    git_repo_name: &str,
    git_repo_namespace: &str,
    environment: &str,
) -> SecretManagerConfig {
    create_test_secret_manager_config_flux_with_options(
        name,
        namespace,
        project,
        mock_endpoint,
        git_repo_name,
        git_repo_namespace,
        environment,
        true, // diff_discovery
        true, // trigger_update
    )
}

/// Create a test SecretManagerConfig for GCP with FluxCD GitRepository reference and custom options
pub fn create_test_secret_manager_config_flux_with_options(
    name: &str,
    namespace: &str,
    project: &str,
    mock_endpoint: &str,
    git_repo_name: &str,
    git_repo_namespace: &str,
    environment: &str,
    diff_discovery: bool,
    trigger_update: bool,
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
                environment: environment.to_string(),
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
            diff_discovery,
            trigger_update,
            suspend: false,
            suspend_git_pulls: false,
            notifications: None,
            hot_reload: None,
            logging: None,
        },
        status: None,
    }
}

/// Create a test SecretManagerConfig for GCP with ArgoCD Application reference
pub fn create_test_secret_manager_config_argocd(
    name: &str,
    namespace: &str,
    project: &str,
    mock_endpoint: &str,
    app_name: &str,
    app_namespace: &str,
    environment: &str,
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
                kind: "Application".to_string(),
                name: app_name.to_string(),
                namespace: app_namespace.to_string(),
                git_credentials: None,
            },
            secrets: SecretsConfig {
                environment: environment.to_string(),
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

/// Create a test SecretManagerConfig for AWS with FluxCD GitRepository reference
pub fn create_test_secret_manager_config_aws_flux(
    name: &str,
    namespace: &str,
    region: &str,
    mock_endpoint: &str,
    git_repo_name: &str,
    git_repo_namespace: &str,
    environment: &str,
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
                environment: environment.to_string(),
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

/// Create a test SecretManagerConfig for AWS with ArgoCD Application reference
pub fn create_test_secret_manager_config_aws_argocd(
    name: &str,
    namespace: &str,
    region: &str,
    mock_endpoint: &str,
    app_name: &str,
    app_namespace: &str,
    environment: &str,
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
                kind: "Application".to_string(),
                name: app_name.to_string(),
                namespace: app_namespace.to_string(),
                git_credentials: None,
            },
            secrets: SecretsConfig {
                environment: environment.to_string(),
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

/// Create a test SecretManagerConfig for Azure with FluxCD GitRepository reference
pub fn create_test_secret_manager_config_azure_flux(
    name: &str,
    namespace: &str,
    vault_name: &str,
    mock_endpoint: &str,
    git_repo_name: &str,
    git_repo_namespace: &str,
    environment: &str,
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
                environment: environment.to_string(),
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

/// Create a test SecretManagerConfig for Azure with ArgoCD Application reference
pub fn create_test_secret_manager_config_azure_argocd(
    name: &str,
    namespace: &str,
    vault_name: &str,
    mock_endpoint: &str,
    app_name: &str,
    app_namespace: &str,
    environment: &str,
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
                kind: "Application".to_string(),
                name: app_name.to_string(),
                namespace: app_namespace.to_string(),
                git_credentials: None,
            },
            secrets: SecretsConfig {
                environment: environment.to_string(),
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

/// Create test secret files in artifact path
///
/// Creates application.secrets.env file with the provided secrets.
pub async fn create_test_secret_files(
    artifact_path: &PathBuf,
    secrets: &[(&str, &str)],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let secret_file = artifact_path.join("application.secrets.env");
    let mut content = String::new();

    for (key, value) in secrets {
        content.push_str(&format!("{}={}\n", key, value));
    }

    fs::write(&secret_file, content).await?;
    info!("Created test secret file: {:?}", secret_file);

    Ok(secret_file)
}

/// Modify secret file to simulate Git changes
///
/// Updates an existing secret file with new values.
pub async fn modify_secret_file(
    secret_file: &PathBuf,
    secrets: &[(&str, &str)],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut content = String::new();

    for (key, value) in secrets {
        content.push_str(&format!("{}={}\n", key, value));
    }

    fs::write(secret_file, content).await?;
    info!("Modified secret file: {:?}", secret_file);

    Ok(())
}

/// Delete secret file to simulate Git deletion
///
/// Removes a secret file to test deletion scenarios.
pub async fn delete_secret_file(secret_file: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    if secret_file.exists() {
        fs::remove_file(secret_file).await?;
        info!("Deleted secret file: {:?}", secret_file);
    }

    Ok(())
}

/// Comment out a secret in a secret file
///
/// Adds a '#' prefix to a secret line to simulate commenting it out.
pub async fn comment_out_secret(
    secret_file: &PathBuf,
    secret_key: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(secret_file).await?;
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    for line in &mut lines {
        if line.starts_with(secret_key) && !line.starts_with('#') {
            *line = format!("#{}", line);
            break;
        }
    }

    let new_content = lines.join("\n") + "\n";
    fs::write(secret_file, new_content).await?;
    info!(
        "Commented out secret '{}' in file: {:?}",
        secret_key, secret_file
    );

    Ok(())
}

/// Uncomment a secret in a secret file
///
/// Removes the '#' prefix from a secret line to simulate uncommenting it.
pub async fn uncomment_secret(
    secret_file: &PathBuf,
    secret_key: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(secret_file).await?;
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    for line in &mut lines {
        if line.starts_with(&format!("#{}", secret_key)) {
            *line = line.trim_start_matches('#').to_string();
            break;
        }
    }

    let new_content = lines.join("\n") + "\n";
    fs::write(secret_file, new_content).await?;
    info!(
        "Uncommented secret '{}' in file: {:?}",
        secret_key, secret_file
    );

    Ok(())
}
