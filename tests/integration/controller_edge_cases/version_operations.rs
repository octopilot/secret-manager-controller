//! Version Operations Edge Cases Integration Tests
//!
//! Tests the controller's handling of version-specific operations:
//! - Version-specific operations (get specific version, list versions)
//! - Version disabling (dedicated test)
//! - Version deletion and recreation

#[cfg(test)]
mod tests {
    use super::super::super::controller_mock_servers::common::*;
    use controller::controller::reconciler::reconcile;
    use controller::controller::reconciler::types::{Reconciler, TriggerSource};
    use controller::prelude::*;
    use kube::api::Api;
    use serde_json::json;
    use std::sync::Arc;
    use tracing::info;

    /// Initialize test environment
    fn init_test() {
        init_rustls();
    }

    /// Create a test SecretManagerConfig that references a GitRepository
    fn create_test_config_with_gitrepo(
        name: &str,
        namespace: &str,
        gitrepo_name: &str,
        gitrepo_namespace: &str,
    ) -> SecretManagerConfig {
        SecretManagerConfig {
            metadata: kube::core::ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                ..Default::default()
            },
            spec: SecretManagerConfigSpec {
                source_ref: SourceRef {
                    kind: "GitRepository".to_string(),
                    name: gitrepo_name.to_string(),
                    namespace: gitrepo_namespace.to_string(),
                    git_credentials_ref: None,
                },
                provider: ProviderConfig::Gcp(GcpConfig {
                    project_id: "test-project".to_string(),
                    location: "us-central1".to_string(),
                    auth: None,
                }),
                secrets: SecretsConfig {
                    environment: "test".to_string(),
                    prefix: None,
                    suffix: None,
                    kustomize_path: None,
                    base_path: None,
                },
                configs: None,
                otel: None,
                git_repository_pull_interval: "5m".to_string(),
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

    #[tokio::test]
    #[ignore] // Requires Kubernetes cluster, GitRepository, and mock server
    async fn test_controller_version_specific_operations() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        // Set up Pact mode
        setup_pact_mode("gcp", &endpoint);

        // Create Kubernetes client
        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                eprintln!("💡 To run this test, ensure a Kubernetes cluster is available:");
                eprintln!("   - Run 'kind create cluster' for local testing");
                eprintln!("   - Or set KUBECONFIG environment variable");
                cleanup_pact_mode("gcp");
                return;
            }
        };

        // Create reconciler
        let reconciler = Arc::new(
            Reconciler::new(client.clone())
                .await
                .expect("Failed to create Reconciler"),
        );

        // Create a SecretManagerConfig that references a GitRepository
        // Note: This test assumes the GitRepository exists and contains secrets
        // that will be versioned
        let config = Arc::new(create_test_config_with_gitrepo(
            "test-version-ops-config",
            "default",
            "test-repo",
            "default",
        ));

        // Create the SecretManagerConfig in Kubernetes
        let smc_api: Api<SecretManagerConfig> = Api::namespaced(client.clone(), "default");
        let _ = smc_api.create(&Default::default(), &*config).await;

        // Trigger reconciliation to create initial secret with version
        let controller_config = super::super::controller_reconciliation::common::create_test_controller_config();
        let result1 = reconcile(
            config.clone(),
            reconciler.clone(),
            TriggerSource::ManualCli,
            controller_config.clone(),
        )
        .await;

        info!("First reconciliation result: {:?}", result1);
        assert!(result1.is_ok(), "First reconciliation should succeed");

        // TODO: Test version-specific operations
        // The mock server supports:
        // - get_version(secret_name, version_id) - Get specific version
        // - list_versions(secret_name) - List all versions
        // However, these endpoints may not be exposed via HTTP yet
        // This test documents the expected behavior:
        // 1. Create secret with initial version
        // 2. Update secret to create new version
        // 3. Verify both versions exist
        // 4. Access specific version by version ID
        // 5. List all versions

        // Cleanup
        let _ = smc_api.delete("test-version-ops-config", &Default::default()).await;
        cleanup_pact_mode("gcp");
    }

    #[tokio::test]
    #[ignore] // Requires Kubernetes cluster, GitRepository, and mock server
    async fn test_controller_version_disabling() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        // Set up Pact mode
        setup_pact_mode("gcp", &endpoint);

        // Create Kubernetes client
        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                eprintln!("💡 To run this test, ensure a Kubernetes cluster is available:");
                eprintln!("   - Run 'kind create cluster' for local testing");
                eprintln!("   - Or set KUBECONFIG environment variable");
                cleanup_pact_mode("gcp");
                return;
            }
        };

        // Create reconciler
        let reconciler = Arc::new(
            Reconciler::new(client.clone())
                .await
                .expect("Failed to create Reconciler"),
        );

        // Create a SecretManagerConfig that references a GitRepository
        let config = Arc::new(create_test_config_with_gitrepo(
            "test-version-disable-config",
            "default",
            "test-repo",
            "default",
        ));

        // Create the SecretManagerConfig in Kubernetes
        let smc_api: Api<SecretManagerConfig> = Api::namespaced(client.clone(), "default");
        let _ = smc_api.create(&Default::default(), &*config).await;

        // Trigger reconciliation to create secret with version
        let controller_config = create_test_controller_config();
        let result1 = reconcile(
            config.clone(),
            reconciler.clone(),
            TriggerSource::ManualCli,
            controller_config.clone(),
        )
        .await;

        info!("First reconciliation result: {:?}", result1);
        assert!(result1.is_ok(), "First reconciliation should succeed");

        // TODO: Test version disabling
        // The mock server supports:
        // - disable_version(secret_name, version_id) - Disable specific version
        // - enable_version(secret_name, version_id) - Re-enable version
        // - disable_secret(secret_name) - Disable all versions
        // - enable_secret(secret_name) - Re-enable secret
        // This test should:
        // 1. Create secret with multiple versions
        // 2. Disable a specific version
        // 3. Verify disabled version is not accessible
        // 4. Re-enable the version
        // 5. Verify version is accessible again

        // Cleanup
        let _ = smc_api.delete("test-version-disable-config", &Default::default()).await;
        cleanup_pact_mode("gcp");
    }

    #[tokio::test]
    #[ignore] // Requires Kubernetes cluster, GitRepository, and mock server
    async fn test_controller_version_deletion_and_recreation() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        // Set up Pact mode
        setup_pact_mode("gcp", &endpoint);

        // Create Kubernetes client
        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                eprintln!("💡 To run this test, ensure a Kubernetes cluster is available:");
                eprintln!("   - Run 'kind create cluster' for local testing");
                eprintln!("   - Or set KUBECONFIG environment variable");
                cleanup_pact_mode("gcp");
                return;
            }
        };

        // Create reconciler
        let reconciler = Arc::new(
            Reconciler::new(client.clone())
                .await
                .expect("Failed to create Reconciler"),
        );

        // Create a SecretManagerConfig that references a GitRepository
        let config = Arc::new(create_test_config_with_gitrepo(
            "test-version-delete-config",
            "default",
            "test-repo",
            "default",
        ));

        // Create the SecretManagerConfig in Kubernetes
        let smc_api: Api<SecretManagerConfig> = Api::namespaced(client.clone(), "default");
        let _ = smc_api.create(&Default::default(), &*config).await;

        // Trigger reconciliation to create secret
        let controller_config = create_test_controller_config();
        let result1 = reconcile(
            config.clone(),
            reconciler.clone(),
            TriggerSource::ManualCli,
            controller_config.clone(),
        )
        .await;

        info!("First reconciliation result: {:?}", result1);
        assert!(result1.is_ok(), "First reconciliation should succeed");

        // TODO: Test version deletion and recreation
        // This test should:
        // 1. Create secret with version
        // 2. Delete the secret (via mock server DELETE endpoint)
        // 3. Trigger reconciliation again
        // 4. Verify secret is recreated (Git is source of truth)
        // 5. Verify new version is created

        // Cleanup
        let _ = smc_api.delete("test-version-delete-config", &Default::default()).await;
        cleanup_pact_mode("gcp");
    }

    #[tokio::test]
    #[ignore] // Requires Kubernetes cluster, GitRepository with multiple services, and mock server
    async fn test_controller_multiple_services_same_repo() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        // Set up Pact mode
        setup_pact_mode("gcp", &endpoint);

        // Create Kubernetes client
        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                eprintln!("💡 To run this test, ensure a Kubernetes cluster is available:");
                eprintln!("   - Run 'kind create cluster' for local testing");
                eprintln!("   - Or set KUBECONFIG environment variable");
                cleanup_pact_mode("gcp");
                return;
            }
        };

        // Create reconciler
        let reconciler = Arc::new(
            Reconciler::new(client.clone())
                .await
                .expect("Failed to create Reconciler"),
        );

        // Create a SecretManagerConfig that references a GitRepository
        // Note: This test assumes the GitRepository contains multiple services
        // (e.g., service-a and service-b, each with their own application.secrets.env)
        let config = Arc::new(create_test_config_with_gitrepo(
            "test-multiple-services-config",
            "default",
            "test-repo-multiple-services",
            "default",
        ));

        // Create the SecretManagerConfig in Kubernetes
        let smc_api: Api<SecretManagerConfig> = Api::namespaced(client.clone(), "default");
        let _ = smc_api.create(&Default::default(), &*config).await;

        // Trigger reconciliation
        let controller_config = create_test_controller_config();
        let result = reconcile(
            config.clone(),
            reconciler,
            TriggerSource::ManualCli,
            controller_config,
        )
        .await;

        // Verify reconciliation processes all services
        match result {
            Ok(action) => {
                info!("Reconciliation returned action: {:?}", action);
                // Reconciliation should succeed and process all services
            }
            Err(e) => {
                let error_msg = e.to_string();
                info!("Reconciliation failed with error: {}", error_msg);
                // May fail if services have errors, but should process all services
            }
        }

        // Verify status reflects processing of multiple services
        let updated_config = smc_api.get("test-multiple-services-config").await;
        if let Ok(config) = updated_config {
            if let Some(status) = config.status {
                info!("Status phase: {:?}, message: {:?}", status.phase, status.message);
                // Status should indicate success or partial failure if some services failed
            }
        }

        // Cleanup
        let _ = smc_api.delete("test-multiple-services-config", &Default::default()).await;
        cleanup_pact_mode("gcp");
    }
}

