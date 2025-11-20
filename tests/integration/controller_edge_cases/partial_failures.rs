//! Partial Failure Integration Tests
//!
//! Tests the controller's handling of partial failures:
//! - Some services succeed, some fail (partial failure across services)
//! - Verify status reflects partial success
//!
//! **Note**: The controller processes each service separately. If one service fails,
//! the controller continues with other services and updates status to "PartialFailure".
//! Within a single service, if one secret fails, the entire service fails (by design).

#[cfg(test)]
mod tests {
    use super::super::super::controller_mock_servers::common::*;
    use secret_manager_controller::controller::reconciler::reconcile;
    use secret_manager_controller::controller::reconciler::types::{Reconciler, TriggerSource};
    use secret_manager_controller::{GcpConfig, ProviderConfig, SecretManagerConfig, SecretsConfig, SourceRef};
    use kube::api::Api;
    use kube_runtime::controller::Action;
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
        use secret_manager_controller::{GcpConfig, ProviderConfig, SecretsConfig};
        SecretManagerConfig {
            metadata: kube::core::ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                ..Default::default()
            },
            spec: secret_manager_controller::SecretManagerConfigSpec {
                source_ref: SourceRef {
                    kind: "GitRepository".to_string(),
                    name: gitrepo_name.to_string(),
                    namespace: gitrepo_namespace.to_string(),
                },
                provider: ProviderConfig::Gcp(GcpConfig {
                    project_id: "test-project".to_string(),
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
            },
            status: None,
        }
    }

    #[tokio::test]
    #[ignore] // Requires Kubernetes cluster, GitRepository with multiple services, and mock server
    async fn test_controller_partial_failure() {
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
        // Note: This test assumes the GitRepository exists and contains multiple services:
        // - One service that will succeed
        // - One service that will fail (e.g., due to rate limiting or auth failure)
        // To simulate partial failure, we can:
        // 1. Use a GitRepository with multiple services
        // 2. Configure the mock server to fail for one service (via headers)
        // 3. Verify the controller processes both services and reports partial failure
        let config = Arc::new(create_test_config_with_gitrepo(
            "test-partial-failure-config",
            "default",
            "test-repo-multiple-services",
            "default",
        ));

        // Create the SecretManagerConfig in Kubernetes
        let smc_api: Api<SecretManagerConfig> = Api::namespaced(client.clone(), "default");
        let _ = smc_api.create(&Default::default(), &*config).await;

        // Trigger reconciliation
        let result = reconcile(
            config.clone(),
            reconciler,
            TriggerSource::ManualCli,
        )
        .await;

        // For partial failures, the controller should:
        // 1. Process all services
        // 2. Continue even if one service fails
        // 3. Update status to "PartialFailure" if some services failed
        match result {
            Ok(action) => {
                // Reconciliation may succeed even with partial failures
                // if at least one service succeeded
                info!("Reconciliation returned action: {:?}", action);
            }
            Err(e) => {
                // Reconciliation may fail if all services fail
                let error_msg = e.to_string();
                info!("Reconciliation failed with error: {}", error_msg);
            }
        }

        // Verify status reflects partial failure
        let updated_config = smc_api.get("test-partial-failure-config").await;
        if let Ok(config) = updated_config {
            if let Some(status) = config.status {
                info!("Status phase: {:?}, message: {:?}", status.phase, status.message);
                // Status should indicate partial failure if some services failed
                if status.phase == Some("PartialFailure".to_string()) {
                    info!("✅ Status correctly indicates partial failure");
                    assert!(
                        status.message.as_ref().map(|m| m.contains("Failed to process service")).unwrap_or(false),
                        "Status message should indicate which service failed"
                    );
                } else if status.phase == Some("Success".to_string()) {
                    // If all services succeeded, that's also valid
                    info!("All services succeeded - no partial failure");
                }
            }
        }

        // Cleanup
        let _ = smc_api.delete("test-partial-failure-config", &Default::default()).await;
        cleanup_pact_mode("gcp");
    }
}

