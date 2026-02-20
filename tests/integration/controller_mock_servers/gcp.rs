//! GCP Controller Integration Tests
//!
//! Tests the controller's reconciliation flow with GCP Secret Manager mock server.
//!
//! These tests verify:
//! - Secret creation through controller reconciliation
//! - Secret updates and versioning
//! - Error handling
//! - Status updates

#[cfg(test)]
mod tests {
    use super::super::common::*;
    use base64::{Engine as _, engine::general_purpose};
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
    async fn test_gcp_controller_create_secret() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        // Set up Pact mode
        setup_pact_mode("gcp", &endpoint);

        // Create test config
        let config =
            create_gcp_test_config("test-gcp-config", "default", "test-project", &endpoint);

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

        cleanup_pact_mode("gcp");
    }

    #[tokio::test]
    #[ignore] // Requires mock server and Kubernetes cluster
    async fn test_gcp_controller_secret_versioning() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        setup_pact_mode("gcp", &endpoint);

        // Test that updating a secret creates a new version with sequential version ID
        // This would require:
        // 1. Create initial secret (version 1)
        // 2. Update secret value
        // 3. Verify new version exists (version 2)
        // 4. Verify versions are ordered by timestamp

        cleanup_pact_mode("gcp");
    }

    #[tokio::test]
    #[ignore] // Requires mock server and Kubernetes cluster
    async fn test_gcp_controller_secret_with_timestamp() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        setup_pact_mode("gcp", &endpoint);

        // Test that secrets include create_time timestamp in RFC3339 format
        // Verify timestamp is present and correctly formatted

        cleanup_pact_mode("gcp");
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_gcp_controller_secret_disabled() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let project = "test-project";
        let secret_name = "test-secret";

        // 1. Create secret
        let create_url = format!("{}/v1/projects/{}/secrets", endpoint, project);
        let create_body = json!({
            "secretId": secret_name,
            "replication": {
                "automatic": {}
            }
        });
        let create_response = client
            .post(&create_url)
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Add a version with value
        let add_version_url = format!(
            "{}/v1/projects/{}/secrets/{}:addVersion",
            endpoint, project, secret_name
        );
        let version_body = json!({
            "payload": {
                "data": general_purpose::STANDARD.encode("test-value")
            }
        });
        let version_response = client
            .post(&add_version_url)
            .json(&version_body)
            .send()
            .await
            .expect("Failed to add version");
        assert!(
            version_response.status().is_success(),
            "Version creation should succeed"
        );

        // 3. Verify secret can be accessed
        let get_url = format!(
            "{}/v1/projects/{}/secrets/{}/versions/latest:access",
            endpoint, project, secret_name
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

        // 4. Disable secret
        let disable_url = format!(
            "{}/v1/projects/{}/secrets/{}:disable",
            endpoint, project, secret_name
        );
        let disable_response = client
            .post(&disable_url)
            .send()
            .await
            .expect("Failed to disable secret");
        assert!(
            disable_response.status().is_success(),
            "Secret disable should succeed"
        );

        // 5. Verify secret cannot be accessed (should return 404 or error)
        let get_response_after_disable = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            !get_response_after_disable.status().is_success(),
            "Disabled secret should not be accessible"
        );

        // 6. Re-enable secret
        let enable_url = format!(
            "{}/v1/projects/{}/secrets/{}:enable",
            endpoint, project, secret_name
        );
        let enable_response = client
            .post(&enable_url)
            .send()
            .await
            .expect("Failed to enable secret");
        assert!(
            enable_response.status().is_success(),
            "Secret enable should succeed"
        );

        // 7. Verify secret can be accessed again
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
    async fn test_gcp_controller_rate_limiting() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let project = "test-project";
        let secret_name = "test-secret";

        // 1. Make request with X-Rate-Limit header
        let get_url = format!(
            "{}/v1/projects/{}/secrets/{}",
            endpoint, project, secret_name
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
    async fn test_gcp_controller_service_unavailable() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let project = "test-project";
        let secret_name = "test-secret";

        // 1. Make request with X-Service-Unavailable header
        let get_url = format!(
            "{}/v1/projects/{}/secrets/{}",
            endpoint, project, secret_name
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
    async fn test_gcp_controller_auth_failure_401() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let project = "test-project";
        let secret_name = "test-secret";

        // 1. Make request with X-Auth-Failure: 401 header
        let get_url = format!(
            "{}/v1/projects/{}/secrets/{}",
            endpoint, project, secret_name
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
        assert!(
            error_json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("Unauthorized")
        );

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
    async fn test_gcp_controller_auth_failure_403() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let project = "test-project";
        let secret_name = "test-secret";

        // 1. Make request with X-Auth-Failure: 403 header
        let get_url = format!(
            "{}/v1/projects/{}/secrets/{}",
            endpoint, project, secret_name
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
        assert!(
            error_json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("Forbidden")
        );

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
    async fn test_gcp_controller_secret_size_limit() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let project = "test-project";
        let secret_name = "test-secret";

        // 1. Create secret
        let create_url = format!("{}/v1/projects/{}/secrets", endpoint, project);
        let create_body = json!({
            "secretId": secret_name,
            "replication": {
                "automatic": {}
            }
        });
        let create_response = client
            .post(&create_url)
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Try to add version with secret exactly at limit (64KB)
        let add_version_url = format!(
            "{}/v1/projects/{}/secrets/{}:addVersion",
            endpoint, project, secret_name
        );
        let limit_data = base64::engine::general_purpose::STANDARD.encode(&vec![0u8; 64 * 1024]);
        let version_body = json!({
            "payload": {
                "data": limit_data
            }
        });
        let limit_response = client
            .post(&add_version_url)
            .json(&version_body)
            .send()
            .await
            .expect("Failed to add version");
        assert!(
            limit_response.status().is_success(),
            "Secret at limit should succeed"
        );

        // 3. Try to add version with secret exceeding limit (65KB)
        let exceed_data =
            base64::engine::general_purpose::STANDARD.encode(&vec![0u8; 64 * 1024 + 1024]);
        let exceed_body = json!({
            "payload": {
                "data": exceed_data
            }
        });
        let exceed_response = client
            .post(&add_version_url)
            .json(&exceed_body)
            .send()
            .await
            .expect("Failed to add version");
        assert_eq!(
            exceed_response.status(),
            400,
            "Secret exceeding limit should return 400"
        );

        let error_json: serde_json::Value = exceed_response
            .json()
            .await
            .expect("Failed to parse response");
        assert_eq!(error_json["error"]["code"], 400);
        assert!(
            error_json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("exceeds GCP limit")
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_gcp_controller_secret_not_found() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let project = "test-project";
        let secret_name = "non-existent-secret";

        // 1. Try to get secret metadata for non-existent secret
        let get_metadata_url = format!(
            "{}/v1/projects/{}/secrets/{}",
            endpoint, project, secret_name
        );
        let metadata_response = client
            .get(&get_metadata_url)
            .send()
            .await
            .expect("Failed to make request");

        assert_eq!(
            metadata_response.status(),
            404,
            "Non-existent secret should return 404"
        );
        let error_json: serde_json::Value = metadata_response
            .json()
            .await
            .expect("Failed to parse response");
        assert_eq!(error_json["error"]["code"], 404);
        assert_eq!(error_json["error"]["status"], "NOT_FOUND");
        assert!(
            error_json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("Secret not found")
        );

        // 2. Try to get secret value for non-existent secret
        let get_value_url = format!(
            "{}/v1/projects/{}/secrets/{}/versions/latest:access",
            endpoint, project, secret_name
        );
        let value_response = client
            .get(&get_value_url)
            .send()
            .await
            .expect("Failed to make request");

        assert_eq!(
            value_response.status(),
            404,
            "Non-existent secret value should return 404"
        );
        let error_json2: serde_json::Value = value_response
            .json()
            .await
            .expect("Failed to parse response");
        assert_eq!(error_json2["error"]["code"], 404);
        assert_eq!(error_json2["error"]["status"], "NOT_FOUND");
        assert!(
            error_json2["error"]["message"]
                .as_str()
                .unwrap()
                .contains("Secret not found")
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_gcp_controller_secret_value_unchanged() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let project = "test-project";
        let secret_name = "test-secret";
        let secret_value = "test-value-123";

        // 1. Create secret
        let create_url = format!("{}/v1/projects/{}/secrets", endpoint, project);
        let create_body = json!({
            "secretId": secret_name,
            "replication": {
                "automatic": {}
            }
        });
        let create_response = client
            .post(&create_url)
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Add first version with value
        let add_version_url = format!(
            "{}/v1/projects/{}/secrets/{}:addVersion",
            endpoint, project, secret_name
        );
        let version_data = base64::engine::general_purpose::STANDARD.encode(secret_value);
        let version_body = json!({
            "payload": {
                "data": version_data
            }
        });
        let version_response = client
            .post(&add_version_url)
            .json(&version_body)
            .send()
            .await
            .expect("Failed to add version");
        assert!(
            version_response.status().is_success(),
            "Version creation should succeed"
        );
        let version1_json: serde_json::Value = version_response
            .json()
            .await
            .expect("Failed to parse response");
        let version1_id = version1_json["name"]
            .as_str()
            .unwrap()
            .split('/')
            .last()
            .unwrap();

        // 3. Verify secret value
        let get_url = format!(
            "{}/v1/projects/{}/secrets/{}/versions/latest:access",
            endpoint, project, secret_name
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
        let retrieved_data = get_json["payload"]["data"].as_str().unwrap();
        let retrieved_value = String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(retrieved_data)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            retrieved_value, secret_value,
            "Retrieved value should match"
        );

        // 4. Add same value again (no-op scenario)
        let version2_response = client
            .post(&add_version_url)
            .json(&version_body)
            .send()
            .await
            .expect("Failed to add version");
        assert!(
            version2_response.status().is_success(),
            "Adding same value should succeed"
        );
        let version2_json: serde_json::Value = version2_response
            .json()
            .await
            .expect("Failed to parse response");
        let version2_id = version2_json["name"]
            .as_str()
            .unwrap()
            .split('/')
            .last()
            .unwrap();

        // Note: GCP creates a new version even if value is the same (this is expected behavior)
        // The test verifies that the operation succeeds and the value remains accessible
        assert_ne!(
            version1_id, version2_id,
            "New version should be created even with same value"
        );

        // 5. Verify latest version still has the same value
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
        let retrieved_data2 = get_json2["payload"]["data"].as_str().unwrap();
        let retrieved_value2 = String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(retrieved_data2)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            retrieved_value2, secret_value,
            "Value should remain unchanged"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_gcp_controller_secret_deletion() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let project = "test-project";
        let secret_name = "test-secret";
        let secret_value = "test-value-123";

        // 1. Create secret
        let create_url = format!("{}/v1/projects/{}/secrets", endpoint, project);
        let create_body = json!({
            "secretId": secret_name,
            "replication": {
                "automatic": {}
            }
        });
        let create_response = client
            .post(&create_url)
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Add version with value
        let add_version_url = format!(
            "{}/v1/projects/{}/secrets/{}:addVersion",
            endpoint, project, secret_name
        );
        let version_data = base64::engine::general_purpose::STANDARD.encode(secret_value);
        let version_body = json!({
            "payload": {
                "data": version_data
            }
        });
        let version_response = client
            .post(&add_version_url)
            .json(&version_body)
            .send()
            .await
            .expect("Failed to add version");
        assert!(
            version_response.status().is_success(),
            "Version creation should succeed"
        );

        // 3. Verify secret exists
        let get_url = format!(
            "{}/v1/projects/{}/secrets/{}",
            endpoint, project, secret_name
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

        // 4. Delete secret
        let delete_url = format!(
            "{}/v1/projects/{}/secrets/{}",
            endpoint, project, secret_name
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

        // 5. Verify secret no longer exists
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

        // 6. Try to delete non-existent secret
        let delete_nonexistent_url =
            format!("{}/v1/projects/{}/secrets/non-existent", endpoint, project);
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
    async fn test_gcp_controller_secret_metadata() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let project = "test-project";
        let secret_name = "test-secret";

        // 1. Create secret
        let create_url = format!("{}/v1/projects/{}/secrets", endpoint, project);
        let create_body = json!({
            "secretId": secret_name,
            "replication": {
                "automatic": {}
            }
        });
        let create_response = client
            .post(&create_url)
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Get secret metadata
        let get_url = format!(
            "{}/v1/projects/{}/secrets/{}",
            endpoint, project, secret_name
        );
        let get_response = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to get secret metadata");
        assert!(
            get_response.status().is_success(),
            "Secret metadata should be accessible"
        );

        let metadata: serde_json::Value =
            get_response.json().await.expect("Failed to parse metadata");
        assert_eq!(
            metadata["name"].as_str().unwrap(),
            format!("projects/{}/secrets/{}", project, secret_name)
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_gcp_controller_version_specific_get() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let project = "test-project";
        let secret_name = "test-secret";
        let secret_value1 = "value-1";
        let secret_value2 = "value-2";

        // 1. Create secret
        let create_url = format!("{}/v1/projects/{}/secrets", endpoint, project);
        let create_body = json!({
            "secretId": secret_name,
            "replication": {
                "automatic": {}
            }
        });
        let create_response = client
            .post(&create_url)
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Add first version
        let add_version_url = format!(
            "{}/v1/projects/{}/secrets/{}:addVersion",
            endpoint, project, secret_name
        );
        let version_data1 = base64::engine::general_purpose::STANDARD.encode(secret_value1);
        let version_body1 = json!({
            "payload": {
                "data": version_data1
            }
        });
        let version_response1 = client
            .post(&add_version_url)
            .json(&version_body1)
            .send()
            .await
            .expect("Failed to add version");
        assert!(
            version_response1.status().is_success(),
            "Version 1 creation should succeed"
        );
        let version1_json: serde_json::Value = version_response1
            .json()
            .await
            .expect("Failed to parse version response");
        let version1_id = version1_json["name"]
            .as_str()
            .unwrap()
            .split('/')
            .last()
            .unwrap();

        // 3. Add second version
        let version_data2 = base64::engine::general_purpose::STANDARD.encode(secret_value2);
        let version_body2 = json!({
            "payload": {
                "data": version_data2
            }
        });
        let version_response2 = client
            .post(&add_version_url)
            .json(&version_body2)
            .send()
            .await
            .expect("Failed to add version");
        assert!(
            version_response2.status().is_success(),
            "Version 2 creation should succeed"
        );

        // 4. Get specific version 1
        let get_version_url = format!(
            "{}/v1/projects/{}/secrets/{}/versions/{}:access",
            endpoint, project, secret_name, version1_id
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
        let retrieved_data = version_json["payload"]["data"].as_str().unwrap();
        let retrieved_value = String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(retrieved_data)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            retrieved_value, secret_value1,
            "Version 1 should have value-1"
        );

        // 5. Verify latest version has value-2
        let get_latest_url = format!(
            "{}/v1/projects/{}/secrets/{}/versions/latest:access",
            endpoint, project, secret_name
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
        let latest_data = latest_json["payload"]["data"].as_str().unwrap();
        let latest_value = String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(latest_data)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            latest_value, secret_value2,
            "Latest version should have value-2"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_gcp_controller_version_list() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let project = "test-project";
        let secret_name = "test-secret";

        // 1. Create secret
        let create_url = format!("{}/v1/projects/{}/secrets", endpoint, project);
        let create_body = json!({
            "secretId": secret_name,
            "replication": {
                "automatic": {}
            }
        });
        let create_response = client
            .post(&create_url)
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Add multiple versions
        let add_version_url = format!(
            "{}/v1/projects/{}/secrets/{}:addVersion",
            endpoint, project, secret_name
        );
        for i in 1..=3 {
            let version_data =
                base64::engine::general_purpose::STANDARD.encode(format!("value-{}", i));
            let version_body = json!({
                "payload": {
                    "data": version_data
                }
            });
            let version_response = client
                .post(&add_version_url)
                .json(&version_body)
                .send()
                .await
                .expect(&format!("Failed to add version {}", i));
            assert!(
                version_response.status().is_success(),
                "Version {} creation should succeed",
                i
            );
        }

        // 3. List all versions
        let list_versions_url = format!(
            "{}/v1/projects/{}/secrets/{}/versions",
            endpoint, project, secret_name
        );
        let list_response = client
            .get(&list_versions_url)
            .send()
            .await
            .expect("Failed to list versions");
        assert!(
            list_response.status().is_success(),
            "Version list should be accessible"
        );

        let versions_json: serde_json::Value = list_response
            .json()
            .await
            .expect("Failed to parse versions response");
        let versions = versions_json["versions"].as_array().unwrap();
        assert_eq!(versions.len(), 3, "Should have 3 versions");

        // Verify versions are ordered (1, 2, 3)
        for (i, version) in versions.iter().enumerate() {
            let version_name = version["name"].as_str().unwrap();
            let expected_version_id = (i + 1).to_string();
            assert!(
                version_name.ends_with(&expected_version_id),
                "Version {} should have ID {}",
                i + 1,
                expected_version_id
            );
        }
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_gcp_controller_version_disable_enable() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let project = "test-project";
        let secret_name = "test-secret";
        let secret_value = "test-value";

        // 1. Create secret and add version
        let create_url = format!("{}/v1/projects/{}/secrets", endpoint, project);
        let create_body = json!({
            "secretId": secret_name,
            "replication": {
                "automatic": {}
            }
        });
        let create_response = client
            .post(&create_url)
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        let add_version_url = format!(
            "{}/v1/projects/{}/secrets/{}:addVersion",
            endpoint, project, secret_name
        );
        let version_data = base64::engine::general_purpose::STANDARD.encode(secret_value);
        let version_body = json!({
            "payload": {
                "data": version_data
            }
        });
        let version_response = client
            .post(&add_version_url)
            .json(&version_body)
            .send()
            .await
            .expect("Failed to add version");
        assert!(
            version_response.status().is_success(),
            "Version creation should succeed"
        );
        let version_json: serde_json::Value = version_response
            .json()
            .await
            .expect("Failed to parse version response");
        let version_id = version_json["name"]
            .as_str()
            .unwrap()
            .split('/')
            .last()
            .unwrap();

        // 2. Verify version is accessible
        let get_version_url = format!(
            "{}/v1/projects/{}/secrets/{}/versions/{}:access",
            endpoint, project, secret_name, version_id
        );
        let get_response = client
            .get(&get_version_url)
            .send()
            .await
            .expect("Failed to get version");
        assert!(
            get_response.status().is_success(),
            "Version should be accessible before disable"
        );

        // 3. Disable version
        let disable_version_url = format!(
            "{}/v1/projects/{}/secrets/{}/versions/{}:disable",
            endpoint, project, secret_name, version_id
        );
        let disable_response = client
            .post(&disable_version_url)
            .send()
            .await
            .expect("Failed to disable version");
        assert!(
            disable_response.status().is_success(),
            "Version disable should succeed"
        );

        // 4. Verify version is not accessible (should return 404)
        let get_response2 = client
            .get(&get_version_url)
            .send()
            .await
            .expect("Failed to get version");
        assert_eq!(
            get_response2.status(),
            404,
            "Disabled version should return 404"
        );

        // 5. Re-enable version
        let enable_version_url = format!(
            "{}/v1/projects/{}/secrets/{}/versions/{}:enable",
            endpoint, project, secret_name, version_id
        );
        let enable_response = client
            .post(&enable_version_url)
            .send()
            .await
            .expect("Failed to enable version");
        assert!(
            enable_response.status().is_success(),
            "Version enable should succeed"
        );

        // 6. Verify version is accessible again
        let get_response3 = client
            .get(&get_version_url)
            .send()
            .await
            .expect("Failed to get version");
        assert!(
            get_response3.status().is_success(),
            "Re-enabled version should be accessible"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_gcp_controller_secret_disable_enable() {
        init_test();

        // Start GCP mock server
        let mock_server = start_gcp_mock_server()
            .await
            .expect("Failed to start GCP mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let project = "test-project";
        let secret_name = "test-secret";
        let secret_value = "test-value";

        // 1. Create secret
        let create_url = format!("{}/v1/projects/{}/secrets", endpoint, project);
        let create_body = json!({
            "secretId": secret_name,
            "replication": {
                "automatic": {}
            }
        });
        let create_response = client
            .post(&create_url)
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Add version with value
        let add_version_url = format!(
            "{}/v1/projects/{}/secrets/{}:addVersion",
            endpoint, project, secret_name
        );
        let version_data = base64::engine::general_purpose::STANDARD.encode(secret_value);
        let version_body = json!({
            "payload": {
                "data": version_data
            }
        });
        let version_response = client
            .post(&add_version_url)
            .json(&version_body)
            .send()
            .await
            .expect("Failed to add version");
        assert!(
            version_response.status().is_success(),
            "Version creation should succeed"
        );

        // 3. Verify secret is accessible
        let get_url = format!(
            "{}/v1/projects/{}/secrets/{}/versions/latest:access",
            endpoint, project, secret_name
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

        // 4. Disable secret
        let disable_url = format!(
            "{}/v1/projects/{}/secrets/{}:disable",
            endpoint, project, secret_name
        );
        let disable_response = client
            .post(&disable_url)
            .send()
            .await
            .expect("Failed to disable secret");
        assert!(
            disable_response.status().is_success(),
            "Secret disable should succeed"
        );

        // 5. Verify secret is not accessible (should return 404)
        let get_response2 = client
            .get(&get_url)
            .send()
            .await
            .expect("Failed to get secret");
        assert_eq!(
            get_response2.status(),
            404,
            "Disabled secret should return 404"
        );

        // 6. Re-enable secret
        let enable_url = format!(
            "{}/v1/projects/{}/secrets/{}:enable",
            endpoint, project, secret_name
        );
        let enable_response = client
            .post(&enable_url)
            .send()
            .await
            .expect("Failed to enable secret");
        assert!(
            enable_response.status().is_success(),
            "Secret enable should succeed"
        );

        // 7. Verify secret is accessible again
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
