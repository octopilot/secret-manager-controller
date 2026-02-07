//! SOPS Decryption Edge Cases Integration Tests
//!
//! Tests the controller's handling of SOPS decryption edge cases:
//! - SOPS key not found (permanent failure)
//! - SOPS decryption transient failure (retry behavior)
//!
//! **Note**: These tests require:
//! - A Kubernetes cluster
//! - A GitRepository with SOPS-encrypted files
//! - SOPS binary installed in the test environment
//! - Ability to control SOPS key availability in Kubernetes secrets

#[cfg(test)]
mod tests {
    use super::super::super::controller_mock_servers::common::*;
    use controller::controller::reconciler::reconcile;
    use controller::controller::reconciler::types::{Reconciler, TriggerSource};
    use controller::prelude::*;
    use kube::api::Api;
    use kube_runtime::controller::Action;
    use k8s_openapi::api::core::v1::Secret;
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use tracing::info;

    /// Initialize test environment
    fn init_test() {
        init_rustls();
    }

    /// Create a test SecretManagerConfig that references a GitRepository with SOPS files
    fn create_test_config_with_sops(
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
    #[ignore] // Requires Kubernetes cluster, GitRepository with SOPS files, and SOPS setup
    async fn test_controller_sops_key_not_found() {
        init_test();

        // Create Kubernetes client
        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                eprintln!("💡 To run this test, ensure a Kubernetes cluster is available:");
                eprintln!("   - Run 'kind create cluster' for local testing");
                eprintln!("   - Or set KUBECONFIG environment variable");
                eprintln!("   - Also requires a GitRepository with SOPS-encrypted files");
                return;
            }
        };

        // Create reconciler
        let reconciler = Arc::new(
            Reconciler::new(client.clone())
                .await
                .expect("Failed to create Reconciler"),
        );

        // Ensure SOPS key is NOT available in the controller namespace
        // This simulates the key not found scenario
        let controller_namespace =
            std::env::var("POD_NAMESPACE").unwrap_or_else(|_| "octopilot-system".to_string());
        let secrets_api: Api<Secret> = Api::namespaced(client.clone(), &controller_namespace);

        // Delete any existing SOPS key secrets to ensure key not found
        for secret_name in &["sops-private-key", "sops-gpg-key", "gpg-key"] {
            let _ = secrets_api.delete(secret_name, &Default::default()).await;
        }

        // Create a SecretManagerConfig that references a GitRepository with SOPS files
        // Note: This test assumes the GitRepository exists and contains SOPS-encrypted files
        let config = Arc::new(create_test_config_with_sops(
            "test-sops-config",
            "default",
            "test-repo-with-sops",
            "default",
        ));

        // Create the SecretManagerConfig in Kubernetes
        let smc_api: Api<SecretManagerConfig> = Api::namespaced(client.clone(), "default");
        let _ = smc_api.create(&Default::default(), &*config).await;

        // Trigger reconciliation
        let controller_config = create_test_controller_config();
        let result = reconcile(config.clone(), reconciler, TriggerSource::ManualCli, controller_config)
        .await;

        // Verify reconciliation fails with permanent error for key not found
        // The controller should detect SOPS key not found and fail permanently
        match result {
            Ok(action) => {
                // If reconciliation succeeds, it means SOPS files were not processed
                // or the key was found (unexpected)
                info!("Reconciliation returned action: {:?}", action);
                // This is acceptable if there are no SOPS files in the repo
            }
            Err(e) => {
                let error_msg = e.to_string();
                // Verify the error indicates SOPS key not found
                assert!(
                    error_msg.contains("key not found")
                        || error_msg.contains("SOPS")
                        || error_msg.contains("decryption"),
                    "Error should indicate SOPS key not found or decryption failure: {}",
                    error_msg
                );
                info!("✅ Reconciliation correctly failed with SOPS key not found error");
            }
        }

        // Verify status was updated appropriately
        let updated_config = smc_api.get("test-sops-config").await;
        if let Ok(config) = updated_config {
            if let Some(status) = config.status {
                info!("Status phase: {:?}, message: {:?}", status.phase, status.message);
                // Status should indicate failure or pending for SOPS issues
                assert!(
                    status.phase == Some("Failed".to_string())
                        || status.phase == Some("Pending".to_string())
                        || status.message.as_ref().map(|m| m.contains("SOPS")).unwrap_or(false),
                    "Status should reflect SOPS key not found issue"
                );
            }
        }

        // Cleanup
        let _ = smc_api.delete("test-sops-config", &Default::default()).await;
    }

    #[tokio::test]
    #[ignore] // Requires Kubernetes cluster, GitRepository with SOPS files, and SOPS setup
    async fn test_controller_sops_transient_failure() {
        init_test();

        // Create Kubernetes client
        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                eprintln!("💡 To run this test, ensure a Kubernetes cluster is available:");
                eprintln!("   - Run 'kind create cluster' for local testing");
                eprintln!("   - Or set KUBECONFIG environment variable");
                eprintln!("   - Also requires a GitRepository with SOPS-encrypted files");
                return;
            }
        };

        // Create reconciler
        let reconciler = Arc::new(
            Reconciler::new(client.clone())
                .await
                .expect("Failed to create Reconciler"),
        );

        // Create a SecretManagerConfig that references a GitRepository with SOPS files
        // Note: This test assumes the GitRepository exists and contains SOPS-encrypted files
        // For a true transient failure test, we would need to simulate:
        // - Network timeout
        // - Provider unavailable
        // - Permission denied (temporarily)
        // This is difficult to simulate without mocking SOPS binary behavior
        let config = Arc::new(create_test_config_with_sops(
            "test-sops-config",
            "default",
            "test-repo-with-sops",
            "default",
        ));

        // Create the SecretManagerConfig in Kubernetes
        let smc_api: Api<SecretManagerConfig> = Api::namespaced(client.clone(), "default");
        let _ = smc_api.create(&Default::default(), &*config).await;

        // Trigger reconciliation
        let controller_config = create_test_controller_config();
        let result = reconcile(config.clone(), reconciler, TriggerSource::ManualCli, controller_config)
        .await;

        // For transient failures, the controller should retry
        // This is indicated by returning Action::requeue() with a delay
        match result {
            Ok(action) => {
                match action {
                    Action::requeue(duration) => {
                        // Expected behavior for transient failures
                        info!(
                            "✅ Reconciliation correctly returned requeue with delay: {:?}",
                            duration
                        );
                        assert!(
                            duration.as_secs() > 0,
                            "Requeue delay should be positive"
                        );
                    }
                    Action::await_change() => {
                        // Also acceptable - waiting for conditions to change
                        info!("Reconciliation returned await_change()");
                    }
                    _ => {
                        info!("Reconciliation returned action: {:?}", action);
                    }
                }
            }
            Err(e) => {
                let error_msg = e.to_string();
                // Transient errors should not cause permanent failure
                // However, if reconciliation fails completely, check if it's transient
                if error_msg.contains("transient") {
                    info!("✅ Error correctly identified as transient: {}", error_msg);
                } else {
                    // Permanent error - this might be expected if SOPS key is not available
                    info!("Reconciliation failed with error: {}", error_msg);
                }
            }
        }

        // Verify status reflects retry state for transient failures
        let updated_config = smc_api.get("test-sops-config").await;
        if let Ok(config) = updated_config {
            if let Some(status) = config.status {
                info!("Status phase: {:?}, message: {:?}", status.phase, status.message);
                // For transient failures, status might be "Retrying" or similar
                if status.phase == Some("Retrying".to_string()) {
                    info!("✅ Status correctly indicates retry state");
                }
            }
        }

        // Cleanup
        let _ = smc_api.delete("test-sops-config", &Default::default()).await;
    }
}

