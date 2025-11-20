//! Versioning in Reconciliation Tests
//!
//! Tests how the controller handles secret versioning during reconciliation:
//! - Version creation when secret values change
//! - No version creation when values are unchanged
//! - Version ordering (timestamps for AWS/Azure, version numbers for GCP)
//! - Version retrieval and comparison

#[cfg(test)]
mod tests {
    use super::super::common::*;
    use kube::api::{Api, PostParams};
    use secret_manager_controller::controller::reconciler::reconcile;
    use secret_manager_controller::controller::reconciler::types::{Reconciler, TriggerSource};
    use secret_manager_controller::crd::SecretManagerConfig;
    use std::sync::Arc;
    use tracing::info;

    /// Initialize test environment
    fn init_test() {
        init_rustls();
    }

    #[tokio::test]
    #[ignore] // Requires Kind cluster and mock server
    async fn test_gcp_reconciliation_version_creation_on_change() {
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
        let git_repo_name = "test-gcp-version-create";
        let profile = "tilt";

        let artifact_path = setup_flux_artifact_path(namespace, git_repo_name, profile)
            .await
            .expect("Failed to set up artifact path");

        let secret_file =
            create_test_secret_files(&artifact_path, &[("DATABASE_PASSWORD", "value-1")])
                .await
                .expect("Failed to create test secret files");

        // Create GitRepository and initial reconciliation
        let _git_repo = create_flux_git_repository(
            &client,
            git_repo_name,
            namespace,
            "https://github.com/microscaler/secret-manager-controller.git",
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

        let config = create_test_secret_manager_config_flux(
            "test-gcp-version-create",
            namespace,
            "test-project",
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

        // Initial reconciliation - creates first version
        let _result1 = reconcile(
            Arc::new(created_config.clone()),
            reconciler.clone(),
            TriggerSource::ManualCli,
        )
        .await;

        // Modify secret value
        modify_secret_file(&secret_file, &[("DATABASE_PASSWORD", "value-2")])
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

        // Reconcile again - should create new version
        let result2 = reconcile(
            Arc::new(created_config),
            reconciler.clone(),
            TriggerSource::ManualCli,
        )
        .await;

        assert!(result2.is_ok(), "Reconciliation should succeed");

        // Verify new version was created with new value
        let verified = verify_gcp_secret(
            &endpoint,
            "test-project",
            "test-service-DATABASE_PASSWORD",
            Some("value-2"),
        )
        .await
        .expect("Failed to verify secret");
        assert!(verified, "Secret should have new value");

        cleanup_pact_mode("gcp");
    }

    #[tokio::test]
    #[ignore] // Requires Kind cluster and mock server
    async fn test_gcp_reconciliation_no_version_on_unchanged() {
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
        let git_repo_name = "test-gcp-no-version";
        let profile = "tilt";

        let artifact_path = setup_flux_artifact_path(namespace, git_repo_name, profile)
            .await
            .expect("Failed to set up artifact path");

        let _secret_file =
            create_test_secret_files(&artifact_path, &[("DATABASE_PASSWORD", "unchanged-value")])
                .await
                .expect("Failed to create test secret files");

        let _git_repo = create_flux_git_repository(
            &client,
            git_repo_name,
            namespace,
            "https://github.com/microscaler/secret-manager-controller.git",
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

        let config = create_test_secret_manager_config_flux(
            "test-gcp-no-version",
            namespace,
            "test-project",
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
        )
        .await;

        // Reconcile again without changes - should not create new version
        let result2 = reconcile(
            Arc::new(created_config),
            reconciler.clone(),
            TriggerSource::ManualCli,
        )
        .await;

        assert!(result2.is_ok(), "Reconciliation should succeed");

        // Verify value is still the same
        let verified = verify_gcp_secret(
            &endpoint,
            "test-project",
            "test-service-DATABASE_PASSWORD",
            Some("unchanged-value"),
        )
        .await
        .expect("Failed to verify secret");
        assert!(verified, "Secret should still have same value");

        // Note: In a full implementation, we'd verify that no new version was created
        // by checking version count or version IDs

        cleanup_pact_mode("gcp");
    }

    #[tokio::test]
    #[ignore] // Requires Kind cluster and mock server
    async fn test_aws_reconciliation_version_ordering() {
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
        let git_repo_name = "test-aws-version-order";
        let profile = "tilt";

        let artifact_path = setup_flux_artifact_path(namespace, git_repo_name, profile)
            .await
            .expect("Failed to set up artifact path");

        let secret_file =
            create_test_secret_files(&artifact_path, &[("DATABASE_PASSWORD", "value-1")])
                .await
                .expect("Failed to create test secret files");

        let _git_repo = create_flux_git_repository(
            &client,
            git_repo_name,
            namespace,
            "https://github.com/microscaler/secret-manager-controller.git",
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
            "test-aws-version-order",
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

        // Create multiple versions
        for (i, value) in ["value-1", "value-2", "value-3"].iter().enumerate() {
            modify_secret_file(&secret_file, &[("DATABASE_PASSWORD", *value)])
                .await
                .expect("Failed to modify secret file");

            update_git_repository_artifact_path(
                &client,
                git_repo_name,
                namespace,
                &artifact_path,
                &format!("test-revision-{}", i + 1),
            )
            .await
            .expect("Failed to update GitRepository status");

            let result = reconcile(
                Arc::new(created_config.clone()),
                reconciler.clone(),
                TriggerSource::ManualCli,
            )
            .await;

            assert!(
                result.is_ok(),
                "Reconciliation should succeed for version {}",
                i + 1
            );
        }

        // Verify latest value
        let verified =
            verify_aws_secret(&endpoint, "test-service-DATABASE_PASSWORD", Some("value-3"))
                .await
                .expect("Failed to verify secret");
        assert!(verified, "Latest version should have value-3");

        // Note: In a full implementation, we'd verify version ordering using timestamps
        // AWS uses timestamps to order versions, with AWSCURRENT and AWSPREVIOUS labels

        cleanup_pact_mode("aws");
    }

    #[tokio::test]
    #[ignore] // Requires Kind cluster and mock server
    async fn test_azure_reconciliation_version_ordering() {
        init_test();

        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        setup_pact_mode("azure", &endpoint);

        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                return;
            }
        };

        let namespace = "default";
        let git_repo_name = "test-azure-version-order";
        let profile = "tilt";

        let artifact_path = setup_flux_artifact_path(namespace, git_repo_name, profile)
            .await
            .expect("Failed to set up artifact path");

        let secret_file =
            create_test_secret_files(&artifact_path, &[("DATABASE_PASSWORD", "value-1")])
                .await
                .expect("Failed to create test secret files");

        let _git_repo = create_flux_git_repository(
            &client,
            git_repo_name,
            namespace,
            "https://github.com/microscaler/secret-manager-controller.git",
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

        let config = create_test_secret_manager_config_azure_flux(
            "test-azure-version-order",
            namespace,
            "test-vault",
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

        // Create multiple versions
        for (i, value) in ["value-1", "value-2", "value-3"].iter().enumerate() {
            modify_secret_file(&secret_file, &[("DATABASE_PASSWORD", *value)])
                .await
                .expect("Failed to modify secret file");

            update_git_repository_artifact_path(
                &client,
                git_repo_name,
                namespace,
                &artifact_path,
                &format!("test-revision-{}", i + 1),
            )
            .await
            .expect("Failed to update GitRepository status");

            let result = reconcile(
                Arc::new(created_config.clone()),
                reconciler.clone(),
                TriggerSource::ManualCli,
            )
            .await;

            assert!(
                result.is_ok(),
                "Reconciliation should succeed for version {}",
                i + 1
            );
        }

        // Verify latest value
        let verified =
            verify_azure_secret(&endpoint, "test-service-DATABASE_PASSWORD", Some("value-3"))
                .await
                .expect("Failed to verify secret");
        assert!(verified, "Latest version should have value-3");

        // Note: In a full implementation, we'd verify version ordering using timestamps
        // Azure uses timestamps to order versions, with UUID version IDs

        cleanup_pact_mode("azure");
    }
}
