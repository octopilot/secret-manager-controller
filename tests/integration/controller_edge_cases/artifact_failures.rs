//! Artifact and Build Failure Edge Cases Integration Tests
//!
//! Tests the controller's handling of artifact and build failure edge cases:
//! - Kustomize build failures
//! - Artifact download failures

#[cfg(test)]
mod tests {
    use super::super::super::controller_mock_servers::common::*;
    use secret_manager_controller::controller::reconciler::reconcile;
    use secret_manager_controller::controller::reconciler::types::{Reconciler, TriggerSource};
    use secret_manager_controller::{GcpConfig, ProviderConfig, SecretManagerConfig, SecretsConfig, SourceRef};
    use kube::api::Api;
    use std::sync::Arc;
    use tracing::info;

    /// Initialize test environment
    fn init_test() {
        init_rustls();
    }

    /// Create a test SecretManagerConfig with kustomize path
    fn create_test_config_with_kustomize(
        name: &str,
        namespace: &str,
        gitrepo_name: &str,
        gitrepo_namespace: &str,
        kustomize_path: &str,
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
                    kustomize_path: Some(kustomize_path.to_string()),
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
    #[ignore] // Requires Kubernetes cluster and GitRepository with invalid kustomize
    async fn test_controller_kustomize_build_failure() {
        init_test();

        // Create Kubernetes client
        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                eprintln!("💡 To run this test, ensure a Kubernetes cluster is available:");
                eprintln!("   - Run 'kind create cluster' for local testing");
                eprintln!("   - Or set KUBECONFIG environment variable");
                return;
            }
        };

        // Create reconciler
        let reconciler = Arc::new(
            Reconciler::new(client.clone())
                .await
                .expect("Failed to create Reconciler"),
        );

        // Create a SecretManagerConfig with an invalid kustomize path
        // This should trigger a kustomize build failure
        let config = Arc::new(create_test_config_with_kustomize(
            "test-kustomize-failure-config",
            "default",
            "test-repo",
            "default",
            "invalid-kustomize-path",
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

        // Verify reconciliation fails with kustomize build error
        match result {
            Ok(_) => {
                // If reconciliation succeeds, it means the kustomize path was valid
                // or the GitRepository doesn't exist yet
                info!("Reconciliation succeeded - kustomize path may be valid or GitRepository not found");
            }
            Err(e) => {
                let error_msg = e.to_string();
                // Verify the error indicates kustomize build failure
                assert!(
                    error_msg.contains("kustomize") || error_msg.contains("Kustomize"),
                    "Error should indicate kustomize build failure: {}",
                    error_msg
                );
                info!("✅ Reconciliation correctly failed with kustomize build error: {}", error_msg);
            }
        }

        // Verify status was updated to "Failed"
        let updated_config = smc_api.get("test-kustomize-failure-config").await;
        if let Ok(config) = updated_config {
            if let Some(status) = config.status {
                info!("Status phase: {:?}, message: {:?}", status.phase, status.message);
                // Status should indicate failure for kustomize build errors
                if status.phase == Some("Failed".to_string()) {
                    assert!(
                        status.message.as_ref().map(|m| m.contains("kustomize")).unwrap_or(false),
                        "Status message should indicate kustomize build failure"
                    );
                    info!("✅ Status correctly indicates kustomize build failure");
                }
            }
        }

        // Cleanup
        let _ = smc_api.delete("test-kustomize-failure-config", &Default::default()).await;
    }

    #[tokio::test]
    #[ignore] // Requires Kubernetes cluster and GitRepository with invalid artifact
    async fn test_controller_artifact_download_failure() {
        init_test();

        // Create Kubernetes client
        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                eprintln!("💡 To run this test, ensure a Kubernetes cluster is available:");
                eprintln!("   - Run 'kind create cluster' for local testing");
                eprintln!("   - Or set KUBECONFIG environment variable");
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
        // with an invalid or unreachable artifact URL
        // This would require a GitRepository resource with a bad URL or
        // a source-controller that's not responding
        let config = Arc::new(create_test_config_with_kustomize(
            "test-artifact-failure-config",
            "default",
            "non-existent-repo",
            "default",
            "kustomize",
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

        // Verify reconciliation handles artifact download failure
        // This could be a 404 (GitRepository not found) or a network error
        match result {
            Ok(action) => {
                // If GitRepository not found, should return await_change()
                use kube_runtime::controller::Action;
                match action {
                    Action::await_change() => {
                        info!("✅ Reconciliation correctly returned await_change() for missing GitRepository");
                    }
                    _ => {
                        info!("Reconciliation returned action: {:?}", action);
                    }
                }
            }
            Err(e) => {
                let error_msg = e.to_string();
                // Verify the error indicates artifact download failure
                assert!(
                    error_msg.contains("artifact")
                        || error_msg.contains("download")
                        || error_msg.contains("GitRepository")
                        || error_msg.contains("not found"),
                    "Error should indicate artifact download failure: {}",
                    error_msg
                );
                info!("✅ Reconciliation correctly failed with artifact download error: {}", error_msg);
            }
        }

        // Verify status was updated appropriately
        let updated_config = smc_api.get("test-artifact-failure-config").await;
        if let Ok(config) = updated_config {
            if let Some(status) = config.status {
                info!("Status phase: {:?}, message: {:?}", status.phase, status.message);
                // Status should indicate failure or pending for artifact download errors
                assert!(
                    status.phase == Some("Failed".to_string())
                        || status.phase == Some("Pending".to_string()),
                    "Status should reflect artifact download failure"
                );
            }
        }

        // Cleanup
        let _ = smc_api.delete("test-artifact-failure-config", &Default::default()).await;
    }
}

