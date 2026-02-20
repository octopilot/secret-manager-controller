//! Azure Controller Integration Tests
//!
//! Tests the controller's reconciliation flow with Azure Key Vault mock server.
//!
//! These tests verify:
//! - Secret creation through controller reconciliation
//! - Secret updates and versioning with UUID version IDs
//! - Error handling
//! - Status updates

#[cfg(test)]
mod tests {
    use super::super::common::*;
    use controller::controller::reconciler::reconcile;
    use controller::controller::reconciler::types::{Reconciler, TriggerSource};
    use serde_json::json;
    use std::sync::Arc;

    /// Initialize test environment
    fn init_test() {
        init_rustls();
    }

    #[tokio::test]
    #[ignore] // Requires mock server and Kubernetes cluster
    async fn test_azure_controller_create_secret() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        // Set up Pact mode
        setup_pact_mode("azure", &endpoint);

        // Create test config
        let config =
            create_azure_test_config("test-azure-config", "default", "test-vault", &endpoint);

        // Create Kubernetes client
        // Note: This requires a Kubernetes cluster to be available
        let client = match create_test_kube_client().await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("⚠️  Skipping test: {}", e);
                eprintln!("💡 To run this test, ensure a Kubernetes cluster is available:");
                eprintln!("   - Run 'kind create cluster' for local testing");
                eprintln!("   - Or set KUBECONFIG environment variable");
                return; // Skip test if no cluster available
            }
        };

        // Create reconciler
        let reconciler = Arc::new(
            Reconciler::new(client)
                .await
                .expect("Failed to create Reconciler"),
        );

        // Trigger reconciliation
        let controller_config = create_test_controller_config();
        let result = reconcile(
            Arc::new(config),
            reconciler,
            TriggerSource::ManualCli,
            controller_config,
        )
        .await;

        // Verify reconciliation succeeded
        assert!(result.is_ok(), "Reconciliation should succeed");

        // Verify secret was created in mock server
        // Note: This would require the controller to actually process files
        // For now, this is a placeholder test structure

        cleanup_pact_mode("azure");
    }

    #[tokio::test]
    #[ignore] // Requires mock server and Kubernetes cluster
    async fn test_azure_controller_secret_versioning() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        setup_pact_mode("azure", &endpoint);

        // Test that updating a secret creates a new version with UUID version ID
        // This would require:
        // 1. Create initial secret
        // 2. Update secret value
        // 3. Verify new version exists with unique UUID version ID
        // 4. Verify versions are ordered by timestamp

        cleanup_pact_mode("azure");
    }

    #[tokio::test]
    #[ignore] // Requires mock server and Kubernetes cluster
    async fn test_azure_controller_secret_with_timestamps() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        setup_pact_mode("azure", &endpoint);

        // Test that secrets include created/updated timestamps in attributes
        // Verify timestamps are present and correctly formatted

        cleanup_pact_mode("azure");
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_azure_controller_secret_disabled() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";

        // 1. Create secret
        let create_url = format!(
            "{}/secrets/{}?api-version=2025-07-01",
            endpoint, secret_name
        );
        let create_body = json!({
            "value": "test-value"
        });
        let create_response = client
            .put(&create_url)
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Verify secret can be accessed
        let get_url = format!(
            "{}/secrets/{}/?api-version=2025-07-01",
            endpoint, secret_name
        );
        let get_response = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response.status().is_success(),
            "Secret should be accessible"
        );

        // 3. Disable secret
        let disable_body = json!({
            "attributes": {
                "enabled": false
            }
        });
        let disable_response = client
            .patch(&create_url)
            .json(&disable_body)
            .send()
            .await
            .expect("Failed to disable secret");
        assert!(
            disable_response.status().is_success(),
            "Secret disable should succeed"
        );

        // 4. Verify secret cannot be accessed (should return error)
        let get_response_after_disable = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            !get_response_after_disable.status().is_success(),
            "Disabled secret should not be accessible"
        );

        // 5. Re-enable secret
        let enable_body = json!({
            "attributes": {
                "enabled": true
            }
        });
        let enable_response = client
            .patch(&create_url)
            .json(&enable_body)
            .send()
            .await
            .expect("Failed to enable secret");
        assert!(
            enable_response.status().is_success(),
            "Secret enable should succeed"
        );

        // 6. Verify secret can be accessed again
        let get_response_after_enable = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response_after_enable.status().is_success(),
            "Re-enabled secret should be accessible"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_azure_controller_rate_limiting() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";

        // 1. Make request with X-Rate-Limit header
        let get_url = format!(
            "{}/secrets/{}/?api-version=2025-07-01",
            endpoint, secret_name
        );
        let response = client
            .get(&get_url)
            .header("X-Rate-Limit", "true")
            .send()
            .await
            .expect("Failed to make request");

        // 2. Verify 429 response
        assert_eq!(
            response.status(),
            429,
            "Should return 429 Too Many Requests"
        );

        // 3. Verify Retry-After header is present
        let retry_after = response.headers().get("retry-after");
        assert!(
            retry_after.is_some(),
            "Retry-After header should be present"
        );

        // 4. Make request without header - should succeed (or return normal response)
        let normal_response = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to make request");
        // Should not be 429
        assert_ne!(
            normal_response.status(),
            429,
            "Request without header should not be rate limited"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_azure_controller_service_unavailable() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";

        // 1. Make request with X-Service-Unavailable header
        let get_url = format!(
            "{}/secrets/{}/?api-version=2025-07-01",
            endpoint, secret_name
        );
        let response = client
            .get(&get_url)
            .header("X-Service-Unavailable", "true")
            .send()
            .await
            .expect("Failed to make request");

        // 2. Verify 503 response
        assert_eq!(
            response.status(),
            503,
            "Should return 503 Service Unavailable"
        );

        // 3. Verify error message
        let error_json: serde_json::Value =
            response.json().await.expect("Failed to parse response");
        assert_eq!(error_json["error"]["code"], 503);

        // 4. Health check should still work
        let health_url = format!("{}/health", endpoint);
        let health_response = client
            .get(&health_url)
            .send()
            .await
            .expect("Failed to check health");
        assert!(
            health_response.status().is_success(),
            "Health check should bypass unavailable mode"
        );

        // 5. Make request without header - should succeed (or return normal response)
        let normal_response = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to make request");
        // Should not be 503
        assert_ne!(
            normal_response.status(),
            503,
            "Request without header should not be unavailable"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_azure_controller_auth_failure_401() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";

        // 1. Make request with X-Auth-Failure: 401 header
        let get_url = format!(
            "{}/secrets/{}/?api-version=2025-07-01",
            endpoint, secret_name
        );
        let response = client
            .get(&get_url)
            .header("X-Auth-Failure", "401")
            .send()
            .await
            .expect("Failed to make request");

        // 2. Verify 401 response
        assert_eq!(response.status(), 401, "Should return 401 Unauthorized");

        // 3. Verify error message
        let error_json: serde_json::Value =
            response.json().await.expect("Failed to parse response");
        assert_eq!(error_json["error"]["code"], 401);

        // 4. Make request without header - should succeed (or return normal response)
        let normal_response = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to make request");
        // Should not be 401
        assert_ne!(
            normal_response.status(),
            401,
            "Request without header should not be unauthorized"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_azure_controller_auth_failure_403() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";

        // 1. Make request with X-Auth-Failure: 403 header
        let get_url = format!(
            "{}/secrets/{}/?api-version=2025-07-01",
            endpoint, secret_name
        );
        let response = client
            .get(&get_url)
            .header("X-Auth-Failure", "403")
            .send()
            .await
            .expect("Failed to make request");

        // 2. Verify 403 response
        assert_eq!(response.status(), 403, "Should return 403 Forbidden");

        // 3. Verify error message
        let error_json: serde_json::Value =
            response.json().await.expect("Failed to parse response");
        assert_eq!(error_json["error"]["code"], 403);

        // 4. Make request without header - should succeed (or return normal response)
        let normal_response = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to make request");
        // Should not be 403
        assert_ne!(
            normal_response.status(),
            403,
            "Request without header should not be forbidden"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_azure_controller_secret_size_limit() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";

        // 1. Try to create secret with value exactly at limit (25KB)
        let limit_url = format!(
            "{}/secrets/{}/?api-version=2025-07-01",
            endpoint, secret_name
        );
        let limit_body = json!({
            "value": "a".repeat(25 * 1024)
        });
        let limit_response = client
            .put(&limit_url)
            .json(&limit_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            limit_response.status().is_success(),
            "Secret at limit should succeed"
        );

        // 2. Try to create secret with value exceeding limit (26KB)
        let exceed_secret_name = "test-secret-exceed";
        let exceed_url = format!(
            "{}/secrets/{}/?api-version=2025-07-01",
            endpoint, exceed_secret_name
        );
        let exceed_body = json!({
            "value": "a".repeat(25 * 1024 + 1024)
        });
        let exceed_response = client
            .put(&exceed_url)
            .json(&exceed_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert_eq!(
            exceed_response.status(),
            400,
            "Secret exceeding limit should return 400"
        );

        let error_json: serde_json::Value = exceed_response
            .json()
            .await
            .expect("Failed to parse response");
        assert_eq!(error_json["error"]["code"], "BadParameter");
        assert!(
            error_json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("exceeds Azure limit")
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_azure_controller_secret_not_found() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "non-existent-secret";

        // 1. Try to get secret for non-existent secret
        let get_url = format!(
            "{}/secrets/{}/?api-version=2025-07-01",
            endpoint, secret_name
        );
        let response = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to make request");

        assert_eq!(
            response.status(),
            404,
            "Non-existent secret should return 404"
        );
        let error_json: serde_json::Value =
            response.json().await.expect("Failed to parse response");
        assert_eq!(error_json["error"]["code"], "SecretNotFound");
        assert!(
            error_json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("Secret")
                && error_json["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("not found")
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_azure_controller_secret_value_unchanged() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";
        let secret_value = "test-value-123";

        // 1. Create secret with value
        let create_url = format!(
            "{}/secrets/{}/?api-version=2025-07-01",
            endpoint, secret_name
        );
        let create_body = json!({
            "value": secret_value
        });
        let create_response = client
            .put(&create_url)
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );
        let create_json: serde_json::Value = create_response
            .json()
            .await
            .expect("Failed to parse response");
        let version1_id = create_json["id"]
            .as_str()
            .unwrap()
            .split('/')
            .last()
            .unwrap();

        // 2. Verify secret value
        let get_url = format!(
            "{}/secrets/{}/?api-version=2025-07-01",
            endpoint, secret_name
        );
        let get_response = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response.status().is_success(),
            "Secret should be accessible"
        );
        let get_json: serde_json::Value =
            get_response.json().await.expect("Failed to parse response");
        assert_eq!(
            get_json["value"].as_str().unwrap(),
            secret_value,
            "Retrieved value should match"
        );

        // 3. Update with same value (no-op scenario)
        let put_response = client
            .put(&create_url)
            .json(&create_body)
            .send()
            .await
            .expect("Failed to put secret value");
        assert!(
            put_response.status().is_success(),
            "Putting same value should succeed"
        );
        let put_json: serde_json::Value =
            put_response.json().await.expect("Failed to parse response");
        let version2_id = put_json["id"].as_str().unwrap().split('/').last().unwrap();

        // Note: Azure creates a new version even if value is the same (this is expected behavior)
        // The test verifies that the operation succeeds and the value remains accessible
        assert_ne!(
            version1_id, version2_id,
            "New version should be created even with same value"
        );

        // 4. Verify secret value is still the same
        let get_response2 = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response2.status().is_success(),
            "Secret should still be accessible"
        );
        let get_json2: serde_json::Value = get_response2
            .json()
            .await
            .expect("Failed to parse response");
        assert_eq!(
            get_json2["value"].as_str().unwrap(),
            secret_value,
            "Value should remain unchanged"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_azure_controller_secret_deletion() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";
        let secret_value = "test-value-123";

        // 1. Create secret with value
        let create_url = format!(
            "{}/secrets/{}/?api-version=2025-07-01",
            endpoint, secret_name
        );
        let create_body = json!({
            "value": secret_value
        });
        let create_response = client
            .put(&create_url)
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Verify secret exists
        let get_url = format!(
            "{}/secrets/{}/?api-version=2025-07-01",
            endpoint, secret_name
        );
        let get_response = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response.status().is_success(),
            "Secret should exist before deletion"
        );

        // 3. Delete secret
        let delete_url = format!(
            "{}/secrets/{}?api-version=2025-07-01",
            endpoint, secret_name
        );
        let delete_response = client
            .delete(&delete_url)
            .send()
            .await
            .expect("Failed to delete secret");
        assert_eq!(
            delete_response.status(),
            200,
            "Secret deletion should succeed"
        );

        // 4. Verify secret no longer exists
        let get_response2 = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to get secret");
        assert_eq!(
            get_response2.status(),
            404,
            "Deleted secret should return 404"
        );

        // 5. Try to delete non-existent secret
        let delete_nonexistent_url =
            format!("{}/secrets/non-existent?api-version=2025-07-01", endpoint);
        let delete_response2 = client
            .delete(&delete_nonexistent_url)
            .send()
            .await
            .expect("Failed to delete secret");
        assert_eq!(
            delete_response2.status(),
            404,
            "Deleting non-existent secret should return 404"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_azure_controller_version_specific_get() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";
        let secret_value1 = "value-1";
        let secret_value2 = "value-2";

        // 1. Create secret with first value
        let create_url1 = format!(
            "{}/secrets/{}?api-version=2025-07-01",
            endpoint, secret_name
        );
        let create_body1 = json!({
            "value": secret_value1
        });
        let create_response1 = client
            .put(&create_url1)
            .json(&create_body1)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response1.status().is_success(),
            "Secret creation should succeed"
        );
        let create_json1: serde_json::Value = create_response1
            .json()
            .await
            .expect("Failed to parse create response");
        let version1_id = create_json1["id"]
            .as_str()
            .unwrap()
            .split('/')
            .last()
            .unwrap();

        // 2. Update to second value (creates new version)
        let create_url2 = format!(
            "{}/secrets/{}?api-version=2025-07-01",
            endpoint, secret_name
        );
        let create_body2 = json!({
            "value": secret_value2
        });
        let create_response2 = client
            .put(&create_url2)
            .json(&create_body2)
            .send()
            .await
            .expect("Failed to update secret");
        assert!(
            create_response2.status().is_success(),
            "Secret update should succeed"
        );

        // 3. Get specific version 1
        let get_version_url = format!(
            "{}/secrets/{}/{}?api-version=2025-07-01",
            endpoint, secret_name, version1_id
        );
        let get_version_response = client
            .get(&get_version_url)
            .send()
            .await
            .expect("Failed to get version");
        assert!(
            get_version_response.status().is_success(),
            "Version 1 should be accessible"
        );

        let version_json: serde_json::Value = get_version_response
            .json()
            .await
            .expect("Failed to parse version response");
        assert_eq!(
            version_json["value"].as_str().unwrap(),
            secret_value1,
            "Version 1 should have value-1"
        );

        // 4. Verify latest version has value-2
        let get_latest_url = format!(
            "{}/secrets/{}/?api-version=2025-07-01",
            endpoint, secret_name
        );
        let get_latest_response = client
            .get(&get_latest_url)
            .send()
            .await
            .expect("Failed to get latest version");
        assert!(
            get_latest_response.status().is_success(),
            "Latest version should be accessible"
        );

        let latest_json: serde_json::Value = get_latest_response
            .json()
            .await
            .expect("Failed to parse latest version response");
        assert_eq!(
            latest_json["value"].as_str().unwrap(),
            secret_value2,
            "Latest version should have value-2"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_azure_controller_list_all_secrets() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();

        // 1. Create multiple secrets
        for i in 1..=3 {
            let secret_name = format!("test-secret-{}", i);
            let create_url = format!(
                "{}/secrets/{}?api-version=2025-07-01",
                endpoint, secret_name
            );
            let create_body = json!({
                "value": format!("value-{}", i)
            });
            let create_response = client
                .put(&create_url)
                .json(&create_body)
                .send()
                .await
                .expect(&format!("Failed to create secret {}", i));
            assert!(
                create_response.status().is_success(),
                "Secret {} creation should succeed",
                i
            );
        }

        // 2. List all secrets
        let list_url = format!("{}/secrets?api-version=2025-07-01", endpoint);
        let list_response = client
            .get(&list_url)
            .send()
            .await
            .expect("Failed to list secrets");
        assert!(
            list_response.status().is_success(),
            "List secrets should succeed"
        );

        let list_json: serde_json::Value = list_response
            .json()
            .await
            .expect("Failed to parse list response");
        let secret_list = list_json["value"].as_array().unwrap();
        assert!(secret_list.len() >= 3, "Should have at least 3 secrets");
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_azure_controller_list_versions() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";

        // 1. Create secret and add multiple versions
        for i in 1..=3 {
            let create_url = format!(
                "{}/secrets/{}?api-version=2025-07-01",
                endpoint, secret_name
            );
            let create_body = json!({
                "value": format!("value-{}", i)
            });
            let create_response = client
                .put(&create_url)
                .json(&create_body)
                .send()
                .await
                .expect(&format!("Failed to create version {}", i));
            assert!(
                create_response.status().is_success(),
                "Version {} creation should succeed",
                i
            );
        }

        // 2. List all versions
        let list_versions_url = format!(
            "{}/secrets/{}/versions?api-version=2025-07-01",
            endpoint, secret_name
        );
        let list_versions_response = client
            .get(&list_versions_url)
            .send()
            .await
            .expect("Failed to list versions");
        assert!(
            list_versions_response.status().is_success(),
            "List versions should succeed"
        );

        let versions_json: serde_json::Value = list_versions_response
            .json()
            .await
            .expect("Failed to parse versions response");
        let versions = versions_json["value"].as_array().unwrap();
        assert!(versions.len() >= 3, "Should have at least 3 versions");

        // Verify all versions have proper structure
        for version in versions {
            assert!(version["id"].as_str().is_some(), "Version should have id");
            assert!(
                version["attributes"].is_object(),
                "Version should have attributes"
            );
        }
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_azure_controller_update_secret_attributes() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";
        let secret_value = "test-value";

        // 1. Create secret
        let create_url = format!(
            "{}/secrets/{}?api-version=2025-07-01",
            endpoint, secret_name
        );
        let create_body = json!({
            "value": secret_value
        });
        let create_response = client
            .put(&create_url)
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Update secret attributes (disable)
        let update_url = format!(
            "{}/secrets/{}?api-version=2025-07-01",
            endpoint, secret_name
        );
        let update_body = json!({
            "attributes": {
                "enabled": false
            }
        });
        let update_response = client
            .patch(&update_url)
            .json(&update_body)
            .send()
            .await
            .expect("Failed to update secret");
        assert!(
            update_response.status().is_success(),
            "Update secret should succeed"
        );

        let update_json: serde_json::Value = update_response
            .json()
            .await
            .expect("Failed to parse update response");
        assert_eq!(
            update_json["attributes"]["enabled"].as_bool().unwrap(),
            false,
            "Secret should be disabled"
        );

        // 3. Verify secret cannot be accessed
        let get_url = format!(
            "{}/secrets/{}/?api-version=2025-07-01",
            endpoint, secret_name
        );
        let get_response = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            !get_response.status().is_success(),
            "Disabled secret should not be accessible"
        );

        // 4. Re-enable secret
        let enable_body = json!({
            "attributes": {
                "enabled": true
            }
        });
        let enable_response = client
            .patch(&update_url)
            .json(&enable_body)
            .send()
            .await
            .expect("Failed to enable secret");
        assert!(
            enable_response.status().is_success(),
            "Enable secret should succeed"
        );

        // 5. Verify secret is accessible again
        let get_response2 = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response2.status().is_success(),
            "Re-enabled secret should be accessible"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_azure_controller_secret_disable_enable() {
        init_test();

        // Start Azure mock server
        let mock_server = start_azure_mock_server()
            .await
            .expect("Failed to start Azure mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";
        let secret_value = "test-value";

        // 1. Create secret
        let create_url = format!(
            "{}/secrets/{}?api-version=2025-07-01",
            endpoint, secret_name
        );
        let create_body = json!({
            "value": secret_value
        });
        let create_response = client
            .put(&create_url)
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Verify secret is accessible
        let get_url = format!(
            "{}/secrets/{}/?api-version=2025-07-01",
            endpoint, secret_name
        );
        let get_response = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response.status().is_success(),
            "Secret should be accessible before disable"
        );

        // 3. Disable secret
        let update_url = format!(
            "{}/secrets/{}?api-version=2025-07-01",
            endpoint, secret_name
        );
        let update_body = json!({
            "attributes": {
                "enabled": false
            }
        });
        let update_response = client
            .patch(&update_url)
            .json(&update_body)
            .send()
            .await
            .expect("Failed to disable secret");
        assert!(
            update_response.status().is_success(),
            "Secret disable should succeed"
        );

        // 4. Verify secret is not accessible
        let get_response2 = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            !get_response2.status().is_success(),
            "Disabled secret should not be accessible"
        );

        // 5. Re-enable secret
        let enable_body = json!({
            "attributes": {
                "enabled": true
            }
        });
        let enable_response = client
            .patch(&update_url)
            .json(&enable_body)
            .send()
            .await
            .expect("Failed to enable secret");
        assert!(
            enable_response.status().is_success(),
            "Secret enable should succeed"
        );

        // 6. Verify secret is accessible again
        let get_response3 = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response3.status().is_success(),
            "Re-enabled secret should be accessible"
        );
    }
}
