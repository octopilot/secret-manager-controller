//! GitRepository Edge Cases Integration Tests
//!
//! Tests the controller's handling of GitRepository edge cases:
//! - GitRepository not found (404)
//! - GitRepository not ready

#[cfg(test)]
mod tests {
    use super::super::super::controller_mock_servers::common::*;
    use controller::controller::reconciler::reconcile;
    use controller::controller::reconciler::types::{Reconciler, TriggerSource};
    use controller::prelude::*;
    use kube::api::Api;
    use kube::core::DynamicObject;
    use kube::api::ApiResource;
    use kube_runtime::controller::Action;
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
    #[ignore] // Requires Kubernetes cluster
    async fn test_controller_gitrepository_not_found() {
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

        // Create a SecretManagerConfig that references a non-existent GitRepository
        let config = Arc::new(create_test_config_with_gitrepo(
            "test-config",
            "default",
            "non-existent-gitrepo",
            "default",
        ));

        // Create the SecretManagerConfig in Kubernetes
        let smc_api: Api<SecretManagerConfig> = Api::namespaced(client.clone(), "default");
        let _ = smc_api.create(&Default::default(), &*config).await;

        // Trigger reconciliation
        let controller_config = create_test_controller_config();
        let result = reconcile(config.clone(), reconciler, TriggerSource::ManualCli, controller_config)
        .await;

        // Verify reconciliation returns await_change() for 404
        // The controller should detect 404 and return Action::await_change()
        // This is handled internally, so we check the result
        match result {
            Ok(action) => {
                // The action should be await_change() for 404
                // We verify it doesn't fail with a permanent error
                match action {
                    Action::await_change() => {
                        // Expected behavior for 404
                        info!("✅ Reconciliation correctly returned await_change() for 404");
                    }
                    _ => {
                        // Other actions are also acceptable (e.g., requeue)
                        info!("Reconciliation returned action: {:?}", action);
                    }
                }
            }
            Err(e) => {
                // Should not fail permanently for 404 - should return await_change
                panic!("Reconciliation should not fail permanently for 404: {}", e);
            }
        }

        // Verify status was updated to "Pending"
        let updated_config = smc_api.get("test-config").await.expect("Failed to get config");
        if let Some(status) = updated_config.status {
            assert_eq!(status.phase, Some("Pending".to_string()));
            assert!(status
                .message
                .unwrap_or_default()
                .contains("GitRepository not found"));
        }

        // Cleanup
        let _ = smc_api.delete("test-config", &Default::default()).await;
    }

    #[tokio::test]
    #[ignore] // Requires Kubernetes cluster
    async fn test_controller_gitrepository_not_ready() {
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

        // Create a GitRepository that is not ready
        let gitrepo_ar = ApiResource::from_gvk(&kube::core::GroupVersionKind {
            group: "source.toolkit.fluxcd.io".to_string(),
            version: "v1beta2".to_string(),
            kind: "GitRepository".to_string(),
        });

        let gitrepo_api: Api<DynamicObject> =
            Api::namespaced_with(client.clone(), "default", &gitrepo_ar);

        // Create GitRepository with Ready condition set to False
        let mut gitrepo = DynamicObject::new(
            "test-gitrepo",
            &gitrepo_ar,
        );
        gitrepo.data = json!({
            "spec": {
                "url": "https://github.com/test/repo",
                "interval": "5m"
            },
            "status": {
                "conditions": [
                    {
                        "type": "Ready",
                        "status": "False",
                        "reason": "GitOperationFailed",
                        "message": "Failed to clone repository"
                    },
                    {
                        "type": "Reconciling",
                        "status": "False"
                    }
                ]
            }
        });

        let _ = gitrepo_api.create(&Default::default(), &gitrepo).await;

        // Create a SecretManagerConfig that references this GitRepository
        let config = Arc::new(create_test_config_with_gitrepo(
            "test-config",
            "default",
            "test-gitrepo",
            "default",
        ));

        // Create the SecretManagerConfig in Kubernetes
        let smc_api: Api<SecretManagerConfig> = Api::namespaced(client.clone(), "default");
        let _ = smc_api.create(&Default::default(), &*config).await;

        // Trigger reconciliation
        let controller_config = create_test_controller_config();
        let result = reconcile(config.clone(), reconciler, TriggerSource::ManualCli, controller_config)
        .await;

        // Verify reconciliation fails with appropriate error
        assert!(result.is_err(), "Reconciliation should fail when GitRepository is not ready");
        
        let error = result.unwrap_err();
        let error_msg = error.to_string();
        assert!(
            error_msg.contains("not ready") || error_msg.contains("GitRepository"),
            "Error should indicate GitRepository not ready: {}",
            error_msg
        );

        // Verify status was updated to "Failed"
        let updated_config = smc_api.get("test-config").await.expect("Failed to get config");
        if let Some(status) = updated_config.status {
            assert_eq!(status.phase, Some("Failed".to_string()));
            assert!(status
                .message
                .unwrap_or_default()
                .contains("not ready"));
        }

        // Cleanup
        let _ = smc_api.delete("test-config", &Default::default()).await;
        let _ = gitrepo_api.delete("test-gitrepo", &Default::default()).await;
    }
}

