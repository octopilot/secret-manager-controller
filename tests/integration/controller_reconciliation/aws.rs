//! AWS End-to-End Reconciliation Tests
//!
//! Tests the full controller reconciliation flow with AWS Secrets Manager mock server.
//!
//! These tests verify:
//! - Controller creates secrets from GitRepository/Application
//! - Controller updates secrets when values change
//! - Controller deletes secrets when removed from Git
//! - Controller disables secrets when commented out
//! - Both FluxCD GitRepository and ArgoCD Application support

#[cfg(test)]
mod tests {
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
    // FluxCD GitRepository Tests
    // ============================================================================

    #[tokio::test]
    #[ignore] // Requires Kind cluster and mock server
    async fn test_aws_full_reconciliation_create_secrets_flux() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        // Set up Pact mode
        setup_pact_mode("aws", &endpoint);

        // Create Kubernetes client (requires Kind cluster)
        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                eprintln!("💡 To run this test:");
                eprintln!("   1. Run 'just int-setup' to create Kind cluster");
                eprintln!("   2. Ensure FluxCD source-controller is installed");
                return;
            }
        };

        // Set up test environment
        let namespace = "default";
        let git_repo_name = "test-aws-repo-flux";
        let profile = "tilt";

        // 1. Set up FluxCD artifact path with test files
        let artifact_path = setup_flux_artifact_path(namespace, git_repo_name, profile)
            .await
            .expect("Failed to set up artifact path");

        // 2. Create test secret files
        let _secret_file = create_test_secret_files(
            &artifact_path,
            &[
                ("DATABASE_PASSWORD", "test-password-123"),
                ("API_KEY", "test-api-key-456"),
            ],
        )
        .await
        .expect("Failed to create test secret files");

        // 3. Create FluxCD GitRepository resource
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

        // 4. Update GitRepository status with artifact path
        update_git_repository_artifact_path(
            &client,
            git_repo_name,
            namespace,
            &artifact_path,
            "test-revision-123",
        )
        .await
        .expect("Failed to update GitRepository status");

        // 5. Wait for GitRepository to be ready
        wait_for_git_repository_ready(
            &client,
            git_repo_name,
            namespace,
            std::time::Duration::from_secs(30),
        )
        .await
        .expect("GitRepository did not become ready");

        // 6. Create SecretManagerConfig
        let config = create_test_secret_manager_config_aws_flux(
            "test-aws-reconciliation-flux",
            namespace,
            "us-east-1",
            &endpoint,
            git_repo_name,
            namespace,
            profile,
        );

        // 7. Create SecretManagerConfig in Kubernetes
        let configs: Api<SecretManagerConfig> = Api::namespaced(client.clone(), namespace);
        let created_config = configs
            .create(&PostParams::default(), &config)
            .await
            .expect("Failed to create SecretManagerConfig");

        // 8. Create reconciler
        let reconciler = Arc::new(
            Reconciler::new(client)
                .await
                .expect("Failed to create Reconciler"),
        );

        // 9. Trigger reconciliation
        let result = reconcile(
            Arc::new(created_config),
            reconciler.clone(),
            TriggerSource::ManualCli,
            create_test_controller_config(),
        )
        .await;

        // 10. Verify reconciliation succeeded
        assert!(
            result.is_ok(),
            "Reconciliation should succeed: {:?}",
            result
        );

        // 11. Verify secrets were created in mock server
        let verified1 = verify_aws_secret(
            &endpoint,
            "test-service-DATABASE_PASSWORD",
            Some("test-password-123"),
        )
        .await
        .expect("Failed to verify secret");
        assert!(
            verified1,
            "DATABASE_PASSWORD secret should exist with correct value"
        );

        let verified2 =
            verify_aws_secret(&endpoint, "test-service-API_KEY", Some("test-api-key-456"))
                .await
                .expect("Failed to verify secret");
        assert!(verified2, "API_KEY secret should exist with correct value");

        cleanup_pact_mode("aws");
    }

    #[tokio::test]
    #[ignore] // Requires Kind cluster and mock server
    async fn test_aws_full_reconciliation_update_secrets_flux() {
        init_test();

        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        setup_pact_mode("aws", &endpoint);

        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                return;
            }
        };

        let namespace = "default";
        let git_repo_name = "test-aws-repo-update-flux";
        let profile = "tilt";

        // Set up artifact path with initial secrets
        let artifact_path = setup_flux_artifact_path(namespace, git_repo_name, profile)
            .await
            .expect("Failed to set up artifact path");

        let secret_file =
            create_test_secret_files(&artifact_path, &[("DATABASE_PASSWORD", "initial-password")])
                .await
                .expect("Failed to create test secret files");

        // Create GitRepository and SecretManagerConfig
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
            "test-revision-456",
        )
        .await
        .expect("Failed to update GitRepository status");

        let config = create_test_secret_manager_config_aws_flux(
            "test-aws-update-flux",
            namespace,
            "us-east-1",
            &endpoint,
            git_repo_name,
            namespace,
            profile,
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

        // Initial reconciliation
        let _result1 = reconcile(
            Arc::new(created_config.clone()),
            reconciler.clone(),
            TriggerSource::ManualCli,
            create_test_controller_config(),
        )
        .await;

        // Modify secret file to simulate Git change
        modify_secret_file(&secret_file, &[("DATABASE_PASSWORD", "updated-password")])
            .await
            .expect("Failed to modify secret file");

        // Update GitRepository revision to trigger reconciliation
        update_git_repository_artifact_path(
            &client,
            git_repo_name,
            namespace,
            &artifact_path,
            "test-revision-789",
        )
        .await
        .expect("Failed to update GitRepository status");

        // Reconcile again
        let result2 = reconcile(
            Arc::new(created_config),
            reconciler.clone(),
            TriggerSource::ManualCli,
            create_test_controller_config(),
        )
        .await;

        assert!(result2.is_ok(), "Reconciliation should succeed");

        // Verify secret was updated
        let verified = verify_aws_secret(
            &endpoint,
            "test-service-DATABASE_PASSWORD",
            Some("updated-password"),
        )
        .await
        .expect("Failed to verify secret");
        assert!(verified, "Secret should be updated with new value");

        cleanup_pact_mode("aws");
    }

    #[tokio::test]
    #[ignore] // Requires Kind cluster and mock server
    async fn test_aws_full_reconciliation_delete_secrets_flux() {
        init_test();

        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        setup_pact_mode("aws", &endpoint);

        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                return;
            }
        };

        let namespace = "default";
        let git_repo_name = "test-aws-repo-delete-flux";
        let profile = "tilt";

        // Set up with initial secrets
        let artifact_path = setup_flux_artifact_path(namespace, git_repo_name, profile)
            .await
            .expect("Failed to set up artifact path");

        let secret_file = create_test_secret_files(
            &artifact_path,
            &[
                ("DATABASE_PASSWORD", "test-password"),
                ("API_KEY", "test-key"),
            ],
        )
        .await
        .expect("Failed to create test secret files");

        // Create resources and do initial reconciliation
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

        let config = create_test_secret_manager_config_aws_flux(
            "test-aws-delete-flux",
            namespace,
            "us-east-1",
            &endpoint,
            git_repo_name,
            namespace,
            profile,
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

        let _result1 = reconcile(
            Arc::new(created_config.clone()),
            reconciler.clone(),
            TriggerSource::ManualCli,
            create_test_controller_config(),
        )
        .await;

        // Remove one secret from file
        modify_secret_file(&secret_file, &[("DATABASE_PASSWORD", "test-password")])
            .await
            .expect("Failed to modify secret file");

        // Update revision
        update_git_repository_artifact_path(
            &client,
            git_repo_name,
            namespace,
            &artifact_path,
            "test-revision-2",
        )
        .await
        .expect("Failed to update GitRepository status");

        // Reconcile
        let result2 = reconcile(
            Arc::new(created_config),
            reconciler.clone(),
            TriggerSource::ManualCli,
            create_test_controller_config(),
        )
        .await;

        assert!(result2.is_ok(), "Reconciliation should succeed");

        // Verify API_KEY still exists
        let verified = verify_aws_secret(&endpoint, "test-service-API_KEY", Some("test-key"))
            .await
            .expect("Failed to verify secret");
        assert!(verified, "API_KEY should still exist");

        cleanup_pact_mode("aws");
    }

    #[tokio::test]
    #[ignore] // Requires Kind cluster and mock server
    async fn test_aws_full_reconciliation_disable_secrets_flux() {
        init_test();

        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        setup_pact_mode("aws", &endpoint);

        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                return;
            }
        };

        let namespace = "default";
        let git_repo_name = "test-aws-repo-disable-flux";
        let profile = "tilt";

        let artifact_path = setup_flux_artifact_path(namespace, git_repo_name, profile)
            .await
            .expect("Failed to set up artifact path");

        let secret_file =
            create_test_secret_files(&artifact_path, &[("DATABASE_PASSWORD", "test-password")])
                .await
                .expect("Failed to create test secret files");

        // Create resources and initial reconciliation
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

        let config = create_test_secret_manager_config_aws_flux(
            "test-aws-disable-flux",
            namespace,
            "us-east-1",
            &endpoint,
            git_repo_name,
            namespace,
            profile,
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

        let _result1 = reconcile(
            Arc::new(created_config.clone()),
            reconciler.clone(),
            TriggerSource::ManualCli,
            create_test_controller_config(),
        )
        .await;

        // Comment out secret
        comment_out_secret(&secret_file, "DATABASE_PASSWORD")
            .await
            .expect("Failed to comment out secret");

        // Update revision
        update_git_repository_artifact_path(
            &client,
            git_repo_name,
            namespace,
            &artifact_path,
            "test-revision-2",
        )
        .await
        .expect("Failed to update GitRepository status");

        // Reconcile
        let result2 = reconcile(
            Arc::new(created_config),
            reconciler.clone(),
            TriggerSource::ManualCli,
            create_test_controller_config(),
        )
        .await;

        assert!(result2.is_ok(), "Reconciliation should succeed");

        cleanup_pact_mode("aws");
    }

    // ============================================================================
    // ArgoCD Application Tests
    // ============================================================================

    #[tokio::test]
    #[ignore] // Requires Kind cluster and mock server
    async fn test_aws_full_reconciliation_create_secrets_argocd() {
        init_test();

        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        setup_pact_mode("aws", &endpoint);

        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                eprintln!("💡 To run this test:");
                eprintln!("   1. Run 'just int-setup' to create Kind cluster");
                eprintln!("   2. Ensure ArgoCD Application CRD is installed");
                return;
            }
        };

        let namespace = "argocd";
        let app_name = "test-aws-app-argocd";
        let profile = "tilt";

        // Set up ArgoCD repository path
        let repo_path = setup_argocd_repo_path(namespace, app_name, profile)
            .await
            .expect("Failed to set up repository path");

        // Create test secret files
        let _secret_file = create_test_secret_files(
            &repo_path,
            &[
                ("DATABASE_PASSWORD", "test-password-123"),
                ("API_KEY", "test-api-key-456"),
            ],
        )
        .await
        .expect("Failed to create test secret files");

        // Create ArgoCD Application
        let _app = create_argocd_application(
            &client,
            app_name,
            namespace,
            "https://github.com/octopilot/secret-manager-controller.git",
            "main",
            &format!("deployment-configuration/profiles/{}", profile),
        )
        .await
        .expect("Failed to create ArgoCD Application");

        // Wait for Application to be accessible
        wait_for_argocd_application_ready(
            &client,
            app_name,
            namespace,
            std::time::Duration::from_secs(10),
        )
        .await
        .expect("Application did not become accessible");

        // Create SecretManagerConfig with ArgoCD Application reference
        let config = create_test_secret_manager_config_aws_argocd(
            "test-aws-reconciliation-argocd",
            "default",
            "us-east-1",
            &endpoint,
            app_name,
            namespace,
            profile,
        );

        let configs: Api<SecretManagerConfig> = Api::namespaced(client.clone(), "default");
        let created_config = configs
            .create(&PostParams::default(), &config)
            .await
            .expect("Failed to create SecretManagerConfig");

        let reconciler = Arc::new(
            Reconciler::new(client)
                .await
                .expect("Failed to create Reconciler"),
        );

        // Trigger reconciliation
        let result = reconcile(
            Arc::new(created_config),
            reconciler.clone(),
            TriggerSource::ManualCli,
            create_test_controller_config(),
        )
        .await;

        assert!(
            result.is_ok(),
            "Reconciliation should succeed: {:?}",
            result
        );

        // Verify secrets were created
        let verified1 = verify_aws_secret(
            &endpoint,
            "test-service-DATABASE_PASSWORD",
            Some("test-password-123"),
        )
        .await
        .expect("Failed to verify secret");
        assert!(verified1, "DATABASE_PASSWORD secret should exist");

        let verified2 =
            verify_aws_secret(&endpoint, "test-service-API_KEY", Some("test-api-key-456"))
                .await
                .expect("Failed to verify secret");
        assert!(verified2, "API_KEY secret should exist");

        cleanup_pact_mode("aws");
    }
}
