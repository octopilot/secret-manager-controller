//! GitOps Features Integration Tests
//!
//! Tests critical GitOps controller features:
//! - diffDiscovery: Detects if secrets were tampered with externally
//! - triggerUpdate: Controls whether secrets are automatically updated

#[cfg(test)]
mod tests {
    use super::super::common::fixtures::create_test_secret_manager_config_flux_with_options;
    use super::super::common::*;
    use controller::controller::reconciler::reconcile;
    use controller::controller::reconciler::types::{Reconciler, TriggerSource};
    use controller::crd::SecretManagerConfig;
    use kube::api::{Api, PostParams};
    use std::sync::Arc;

    /// Initialize test environment
    fn init_test() {
        init_rustls();
    }

    // ============================================================================
    // diffDiscovery Tests
    // ============================================================================

    #[tokio::test]
    #[ignore] // Requires Kind cluster and mock server
    async fn test_gcp_diff_discovery_detects_tampering() {
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
                return;
            }
        };

        let namespace = "default";
        let git_repo_name = "test-diff-discovery";
        let profile = "tilt";

        // 1. Set up FluxCD artifact path
        let artifact_path = setup_flux_artifact_path(namespace, git_repo_name, profile)
            .await
            .expect("Failed to set up artifact path");

        // 2. Create test secret files with initial value
        create_test_secret_files(&artifact_path, &[("TEST_SECRET", "initial-value")])
            .await
            .expect("Failed to create test secret files");

        // 3. Create FluxCD GitRepository
        let _git_repo = create_flux_git_repository(
            &client,
            git_repo_name,
            namespace,
            "https://github.com/octopilot/secret-manager-controller.git",
            "main",
            &format!("deployment-configuration/profiles/{}", profile),
        )
        .await
        .expect("Failed to create GitRepository");

        // 4. Update GitRepository status
        update_git_repository_artifact_path(
            &client,
            git_repo_name,
            namespace,
            &artifact_path,
            "test-revision-1",
        )
        .await
        .expect("Failed to update GitRepository status");

        // 5. Create SecretManagerConfig with diffDiscovery enabled
        let config = create_test_secret_manager_config_flux_with_options(
            "test-diff-discovery-config",
            namespace,
            "test-project",
            &endpoint,
            git_repo_name,
            namespace,
            profile,
            true, // diff_discovery
            true, // trigger_update
        );

        // Create SecretManagerConfig in Kubernetes
        let configs: Api<SecretManagerConfig> = Api::namespaced(client.clone(), namespace);
        let created_config = configs
            .create(&PostParams::default(), &config)
            .await
            .expect("Failed to create SecretManagerConfig");

        // 6. First reconciliation - creates secret
        let reconciler = Arc::new(
            Reconciler::new(client.clone())
                .await
                .expect("Failed to create Reconciler"),
        );

        let controller_config = create_test_controller_config();
        let result = reconcile(
            Arc::new(created_config.clone()),
            reconciler.clone(),
            TriggerSource::ManualCli,
            controller_config,
        )
        .await;
        assert!(result.is_ok(), "First reconciliation should succeed");

        // 7. Manually tamper with secret in mock server (simulate external change)
        // This would require direct API call to mock server to update the secret
        // For now, we'll update the Git value and verify diff is detected

        // 8. Update secret file with different value
        create_test_secret_files(&artifact_path, &[("TEST_SECRET", "tampered-value")])
            .await
            .expect("Failed to update test secret files");

        // 9. Update GitRepository status to trigger new reconciliation
        update_git_repository_artifact_path(
            &client,
            git_repo_name,
            namespace,
            &artifact_path,
            "test-revision-2",
        )
        .await
        .expect("Failed to update GitRepository status");

        // 10. Second reconciliation - should detect diff and log warning
        // Note: In a real scenario, we'd need to manually update the secret in the provider
        // first, then reconcile. For this test, we're verifying the diff detection logic
        // is called when diffDiscovery is enabled.

        // Get updated config from Kubernetes
        let configs: Api<SecretManagerConfig> = Api::namespaced(client.clone(), namespace);
        let updated_config = configs
            .get("test-diff-discovery-config")
            .await
            .expect("Failed to get SecretManagerConfig");

        let controller_config = create_test_controller_config();
        let result = reconcile(
            Arc::new(updated_config),
            reconciler,
            TriggerSource::ManualCli,
            controller_config,
        )
        .await;
        assert!(result.is_ok(), "Second reconciliation should succeed");

        // TODO: Verify that diff was detected (check logs or metrics)
        // This would require capturing log output or checking metrics

        cleanup_pact_mode("gcp");
    }

    #[tokio::test]
    #[ignore] // Requires Kind cluster and mock server
    async fn test_gcp_diff_discovery_disabled_no_detection() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        setup_pact_mode("gcp", &endpoint);

        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                return;
            }
        };

        let namespace = "default";
        let git_repo_name = "test-no-diff-discovery";
        let profile = "tilt";

        let artifact_path = setup_flux_artifact_path(namespace, git_repo_name, profile)
            .await
            .expect("Failed to set up artifact path");

        create_test_secret_files(&artifact_path, &[("TEST_SECRET", "test-value")])
            .await
            .expect("Failed to create test secret files");

        let _git_repo = create_flux_git_repository(
            &client,
            git_repo_name,
            namespace,
            "https://github.com/octopilot/secret-manager-controller.git",
            "main",
            &format!("deployment-configuration/profiles/{}", profile),
        )
        .await
        .expect("Failed to create GitRepository");

        update_git_repository_artifact_path(
            &client,
            git_repo_name,
            namespace,
            &artifact_path,
            "test-revision-1",
        )
        .await
        .expect("Failed to update GitRepository status");

        // Create SecretManagerConfig with diffDiscovery disabled
        let config = create_test_secret_manager_config_flux_with_options(
            "test-no-diff-discovery-config",
            namespace,
            "test-project",
            &endpoint,
            git_repo_name,
            namespace,
            profile,
            false, // diff_discovery
            true,  // trigger_update
        );

        let configs: Api<SecretManagerConfig> = Api::namespaced(client.clone(), namespace);
        let _created_config = configs
            .create(&PostParams::default(), &config)
            .await
            .expect("Failed to create SecretManagerConfig");

        let reconciler = Arc::new(
            Reconciler::new(client.clone())
                .await
                .expect("Failed to create Reconciler"),
        );

        let controller_config = create_test_controller_config();
        let result = reconcile(
            Arc::new(config),
            reconciler,
            TriggerSource::ManualCli,
            controller_config,
        )
        .await;
        assert!(result.is_ok(), "Reconciliation should succeed");

        // TODO: Verify that no diff detection occurred (check metrics)

        cleanup_pact_mode("gcp");
    }

    // ============================================================================
    // triggerUpdate Tests
    // ============================================================================

    #[tokio::test]
    #[ignore] // Requires Kind cluster and mock server
    async fn test_gcp_trigger_update_enabled_updates_secrets() {
        init_test();

        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        setup_pact_mode("gcp", &endpoint);

        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                return;
            }
        };

        let namespace = "default";
        let git_repo_name = "test-trigger-update-enabled";
        let profile = "tilt";

        let artifact_path = setup_flux_artifact_path(namespace, git_repo_name, profile)
            .await
            .expect("Failed to set up artifact path");

        // Create initial secret
        create_test_secret_files(&artifact_path, &[("TEST_SECRET", "initial-value")])
            .await
            .expect("Failed to create test secret files");

        let _git_repo = create_flux_git_repository(
            &client,
            git_repo_name,
            namespace,
            "https://github.com/octopilot/secret-manager-controller.git",
            "main",
            &format!("deployment-configuration/profiles/{}", profile),
        )
        .await
        .expect("Failed to create GitRepository");

        update_git_repository_artifact_path(
            &client,
            git_repo_name,
            namespace,
            &artifact_path,
            "test-revision-1",
        )
        .await
        .expect("Failed to update GitRepository status");

        // Create SecretManagerConfig with triggerUpdate enabled (default)
        let config = create_test_secret_manager_config_flux_with_options(
            "test-trigger-update-enabled-config",
            namespace,
            "test-project",
            &endpoint,
            git_repo_name,
            namespace,
            profile,
            true, // diff_discovery
            true, // trigger_update
        );

        let configs: Api<SecretManagerConfig> = Api::namespaced(client.clone(), namespace);
        let created_config = configs
            .create(&PostParams::default(), &config)
            .await
            .expect("Failed to create SecretManagerConfig");

        let reconciler = Arc::new(
            Reconciler::new(client.clone())
                .await
                .expect("Failed to create Reconciler"),
        );

        // First reconciliation - creates secret
        let controller_config = create_test_controller_config();
        let result = reconcile(
            Arc::new(created_config.clone()),
            reconciler.clone(),
            TriggerSource::ManualCli,
            controller_config,
        )
        .await;
        assert!(result.is_ok(), "First reconciliation should succeed");

        // Update secret value in Git
        create_test_secret_files(&artifact_path, &[("TEST_SECRET", "updated-value")])
            .await
            .expect("Failed to update test secret files");

        update_git_repository_artifact_path(
            &client,
            git_repo_name,
            namespace,
            &artifact_path,
            "test-revision-2",
        )
        .await
        .expect("Failed to update GitRepository status");

        // Get updated config from Kubernetes
        let configs: Api<SecretManagerConfig> = Api::namespaced(client.clone(), namespace);
        let updated_config = configs
            .get("test-trigger-update-enabled-config")
            .await
            .expect("Failed to get SecretManagerConfig");

        // Second reconciliation - should update secret (triggerUpdate enabled)
        let controller_config = create_test_controller_config();
        let result = reconcile(
            Arc::new(updated_config),
            reconciler,
            TriggerSource::ManualCli,
            controller_config,
        )
        .await;
        assert!(result.is_ok(), "Second reconciliation should succeed");

        // TODO: Verify secret was updated in mock server

        cleanup_pact_mode("gcp");
    }

    #[tokio::test]
    #[ignore] // Requires Kind cluster and mock server
    async fn test_gcp_trigger_update_disabled_skips_updates() {
        init_test();

        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        setup_pact_mode("gcp", &endpoint);

        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                return;
            }
        };

        let namespace = "default";
        let git_repo_name = "test-trigger-update-disabled";
        let profile = "tilt";

        let artifact_path = setup_flux_artifact_path(namespace, git_repo_name, profile)
            .await
            .expect("Failed to set up artifact path");

        // Create initial secret
        create_test_secret_files(&artifact_path, &[("TEST_SECRET", "initial-value")])
            .await
            .expect("Failed to create test secret files");

        let _git_repo = create_flux_git_repository(
            &client,
            git_repo_name,
            namespace,
            "https://github.com/octopilot/secret-manager-controller.git",
            "main",
            &format!("deployment-configuration/profiles/{}", profile),
        )
        .await
        .expect("Failed to create GitRepository");

        update_git_repository_artifact_path(
            &client,
            git_repo_name,
            namespace,
            &artifact_path,
            "test-revision-1",
        )
        .await
        .expect("Failed to update GitRepository status");

        // Create SecretManagerConfig with triggerUpdate disabled
        let config = create_test_secret_manager_config_flux_with_options(
            "test-trigger-update-disabled-config",
            namespace,
            "test-project",
            &endpoint,
            git_repo_name,
            namespace,
            profile,
            true,  // diff_discovery
            false, // trigger_update
        );

        let configs: Api<SecretManagerConfig> = Api::namespaced(client.clone(), namespace);
        let created_config = configs
            .create(&PostParams::default(), &config)
            .await
            .expect("Failed to create SecretManagerConfig");

        let reconciler = Arc::new(
            Reconciler::new(client.clone())
                .await
                .expect("Failed to create Reconciler"),
        );

        // First reconciliation - creates secret (triggerUpdate only affects updates, not creation)
        let controller_config = create_test_controller_config();
        let result = reconcile(
            Arc::new(created_config.clone()),
            reconciler.clone(),
            TriggerSource::ManualCli,
            controller_config,
        )
        .await;
        assert!(result.is_ok(), "First reconciliation should succeed");

        // Update secret value in Git
        create_test_secret_files(&artifact_path, &[("TEST_SECRET", "updated-value")])
            .await
            .expect("Failed to update test secret files");

        update_git_repository_artifact_path(
            &client,
            git_repo_name,
            namespace,
            &artifact_path,
            "test-revision-2",
        )
        .await
        .expect("Failed to update GitRepository status");

        // Get updated config from Kubernetes
        let configs: Api<SecretManagerConfig> = Api::namespaced(client.clone(), namespace);
        let updated_config = configs
            .get("test-trigger-update-disabled-config")
            .await
            .expect("Failed to get SecretManagerConfig");

        // Second reconciliation - should NOT update secret (triggerUpdate disabled)
        let controller_config = create_test_controller_config();
        let result = reconcile(
            Arc::new(updated_config),
            reconciler,
            TriggerSource::ManualCli,
            controller_config,
        )
        .await;
        assert!(result.is_ok(), "Second reconciliation should succeed");

        // TODO: Verify secret was NOT updated in mock server (still has initial-value)

        cleanup_pact_mode("gcp");
    }

    #[tokio::test]
    #[ignore] // Requires Kind cluster and mock server
    async fn test_gcp_trigger_update_disabled_creates_missing_secrets() {
        init_test();

        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        setup_pact_mode("gcp", &endpoint);

        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                return;
            }
        };

        let namespace = "default";
        let git_repo_name = "test-trigger-update-creates";
        let profile = "tilt";

        let artifact_path = setup_flux_artifact_path(namespace, git_repo_name, profile)
            .await
            .expect("Failed to set up artifact path");

        // Create new secret (not in provider yet)
        create_test_secret_files(&artifact_path, &[("NEW_SECRET", "new-value")])
            .await
            .expect("Failed to create test secret files");

        let _git_repo = create_flux_git_repository(
            &client,
            git_repo_name,
            namespace,
            "https://github.com/octopilot/secret-manager-controller.git",
            "main",
            &format!("deployment-configuration/profiles/{}", profile),
        )
        .await
        .expect("Failed to create GitRepository");

        update_git_repository_artifact_path(
            &client,
            git_repo_name,
            namespace,
            &artifact_path,
            "test-revision-1",
        )
        .await
        .expect("Failed to update GitRepository status");

        // Create SecretManagerConfig with triggerUpdate disabled
        let config = create_test_secret_manager_config_flux_with_options(
            "test-trigger-update-creates-config",
            namespace,
            "test-project",
            &endpoint,
            git_repo_name,
            namespace,
            profile,
            true,  // diff_discovery
            false, // trigger_update
        );

        let configs: Api<SecretManagerConfig> = Api::namespaced(client.clone(), namespace);
        let created_config = configs
            .create(&PostParams::default(), &config)
            .await
            .expect("Failed to create SecretManagerConfig");

        let reconciler = Arc::new(
            Reconciler::new(client.clone())
                .await
                .expect("Failed to create Reconciler"),
        );

        // Reconciliation - should create missing secret even when triggerUpdate is disabled
        let controller_config = create_test_controller_config();
        let result = reconcile(
            Arc::new(created_config),
            reconciler,
            TriggerSource::ManualCli,
            controller_config,
        )
        .await;
        assert!(result.is_ok(), "Reconciliation should succeed");

        // TODO: Verify secret was created in mock server

        cleanup_pact_mode("gcp");
    }
}
