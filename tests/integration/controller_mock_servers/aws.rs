//! AWS Controller Integration Tests
//!
//! Tests the controller's reconciliation flow with AWS Secrets Manager mock server.
//!
//! These tests verify:
//! - Secret creation through controller reconciliation
//! - Secret updates and versioning with staging labels
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
    async fn test_aws_controller_create_secret() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        // Set up Pact mode
        setup_pact_mode("aws", &endpoint);

        // Create test config
        let config = create_aws_test_config("test-aws-config", "default", "us-east-1", &endpoint);

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

        cleanup_pact_mode("aws");
    }

    #[tokio::test]
    #[ignore] // Requires mock server and Kubernetes cluster
    async fn test_aws_controller_secret_versioning() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        setup_pact_mode("aws", &endpoint);

        // Test that updating a secret creates a new version with AWSCURRENT label
        // This would require:
        // 1. Create initial secret
        // 2. Update secret value
        // 3. Verify new version exists with AWSCURRENT label
        // 4. Verify previous version has AWSPREVIOUS label

        cleanup_pact_mode("aws");
    }

    #[tokio::test]
    #[ignore] // Requires mock server and Kubernetes cluster
    async fn test_aws_controller_secret_with_timestamp() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        setup_pact_mode("aws", &endpoint);

        // Test that secrets include CreatedDate timestamp in responses
        // Verify timestamp is present and correctly formatted

        cleanup_pact_mode("aws");
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_aws_controller_secret_disabled() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";

        // 1. Create secret
        let create_body = json!({
            "Name": secret_name,
            "SecretString": "test-value"
        });
        let create_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.CreateSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Verify secret can be accessed
        let get_body = json!({
            "SecretId": secret_name
        });
        let get_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&get_body)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response.status().is_success(),
            "Secret should be accessible"
        );

        // 3. Delete secret (marks for deletion, can be restored)
        let delete_body = json!({
            "SecretId": secret_name
        });
        let delete_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.DeleteSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&delete_body)
            .send()
            .await
            .expect("Failed to delete secret");
        assert!(
            delete_response.status().is_success(),
            "Secret delete should succeed"
        );

        // 4. Verify secret cannot be accessed (should return error)
        let get_response_after_delete = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&get_body)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            !get_response_after_delete.status().is_success(),
            "Deleted secret should not be accessible"
        );

        // 5. Restore secret
        let restore_body = json!({
            "SecretId": secret_name
        });
        let restore_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.RestoreSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&restore_body)
            .send()
            .await
            .expect("Failed to restore secret");
        assert!(
            restore_response.status().is_success(),
            "Secret restore should succeed"
        );

        // 6. Verify secret can be accessed again
        let get_response_after_restore = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&get_body)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response_after_restore.status().is_success(),
            "Restored secret should be accessible"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_aws_controller_rate_limiting() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";

        // 1. Make request with X-Rate-Limit header
        let body = json!({
            "SecretId": secret_name
        });
        let response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .header("X-Rate-Limit", "true")
            .json(&body)
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
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&body)
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
    async fn test_aws_controller_service_unavailable() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";

        // 1. Make request with X-Service-Unavailable header
        let body = json!({
            "SecretId": secret_name
        });
        let response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .header("X-Service-Unavailable", "true")
            .json(&body)
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
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&body)
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
    async fn test_aws_controller_auth_failure_401() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";

        // 1. Make request with X-Auth-Failure: 401 header
        let body = json!({
            "SecretId": secret_name
        });
        let response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .header("X-Auth-Failure", "401")
            .json(&body)
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
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&body)
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
    async fn test_aws_controller_auth_failure_403() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";

        // 1. Make request with X-Auth-Failure: 403 header
        let body = json!({
            "SecretId": secret_name
        });
        let response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .header("X-Auth-Failure", "403")
            .json(&body)
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
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&body)
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
    async fn test_aws_controller_secret_size_limit() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";

        // 1. Try to create secret with value exactly at limit (64KB)
        let limit_body = json!({
            "Name": secret_name,
            "SecretString": "a".repeat(64 * 1024)
        });
        let limit_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.CreateSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&limit_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            limit_response.status().is_success(),
            "Secret at limit should succeed"
        );

        // 2. Try to create secret with value exceeding limit (65KB)
        let exceed_body = json!({
            "Name": "test-secret-exceed",
            "SecretString": "a".repeat(64 * 1024 + 1024)
        });
        let exceed_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.CreateSecret")
            .header("content-type", "application/x-amz-json-1.1")
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
        assert_eq!(error_json["__type"], "InvalidParameterException");
        assert!(
            error_json["message"]
                .as_str()
                .unwrap()
                .contains("exceeds AWS limit")
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_aws_controller_secret_not_found() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "non-existent-secret";

        // 1. Try to get secret value for non-existent secret
        let body = json!({
            "SecretId": secret_name
        });
        let response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&body)
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
        assert_eq!(error_json["__type"], "ResourceNotFoundException");
        assert!(
            error_json["message"]
                .as_str()
                .unwrap()
                .contains("can't find the specified secret")
        );

        // 2. Try to describe non-existent secret
        let describe_body = json!({
            "SecretId": secret_name
        });
        let describe_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.DescribeSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&describe_body)
            .send()
            .await
            .expect("Failed to make request");

        // Note: DescribeSecret might return 200 with empty data, or 404 - depends on implementation
        // For now, we'll just verify it doesn't crash
        assert!(
            describe_response.status().is_client_error() || describe_response.status().is_success()
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_aws_controller_secret_value_unchanged() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";
        let secret_value = "test-value-123";

        // 1. Create secret with value
        let create_body = json!({
            "Name": secret_name,
            "SecretString": secret_value
        });
        let create_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.CreateSecret")
            .header("content-type", "application/x-amz-json-1.1")
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
        let version1_id = create_json["VersionId"].as_str().unwrap();

        // 2. Verify secret value
        let get_body = json!({
            "SecretId": secret_name
        });
        let get_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&get_body)
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
            get_json["SecretString"].as_str().unwrap(),
            secret_value,
            "Retrieved value should match"
        );

        // 3. Update with same value (no-op scenario)
        let put_body = json!({
            "SecretId": secret_name,
            "SecretString": secret_value
        });
        let put_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.PutSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&put_body)
            .send()
            .await
            .expect("Failed to put secret value");
        assert!(
            put_response.status().is_success(),
            "Putting same value should succeed"
        );
        let put_json: serde_json::Value =
            put_response.json().await.expect("Failed to parse response");
        let version2_id = put_json["VersionId"].as_str().unwrap();

        // Note: AWS creates a new version even if value is the same (this is expected behavior)
        // The test verifies that the operation succeeds and the value remains accessible
        assert_ne!(
            version1_id, version2_id,
            "New version should be created even with same value"
        );

        // 4. Verify secret value is still the same
        let get_response2 = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&get_body)
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
            get_json2["SecretString"].as_str().unwrap(),
            secret_value,
            "Value should remain unchanged"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_aws_controller_secret_deletion() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";
        let secret_value = "test-value-123";

        // 1. Create secret with value
        let create_body = json!({
            "Name": secret_name,
            "SecretString": secret_value
        });
        let create_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.CreateSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Verify secret exists
        let get_body = json!({
            "SecretId": secret_name
        });
        let get_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&get_body)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response.status().is_success(),
            "Secret should exist before deletion"
        );

        // 3. Delete secret (with recovery window)
        let delete_body = json!({
            "SecretId": secret_name,
            "RecoveryWindowInDays": 7
        });
        let delete_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.DeleteSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&delete_body)
            .send()
            .await
            .expect("Failed to delete secret");
        assert!(
            delete_response.status().is_success(),
            "Secret deletion should succeed"
        );
        let delete_json: serde_json::Value = delete_response
            .json()
            .await
            .expect("Failed to parse response");
        assert!(
            delete_json["DeletionDate"].is_string(),
            "Deletion response should include DeletionDate"
        );

        // 4. Verify secret is marked as deleted (should return error when trying to access)
        let get_response2 = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&get_body)
            .send()
            .await
            .expect("Failed to get secret");
        assert_eq!(
            get_response2.status(),
            400,
            "Deleted secret should return 400 (InvalidRequestException)"
        );
        let error_json: serde_json::Value = get_response2
            .json()
            .await
            .expect("Failed to parse response");
        assert_eq!(error_json["__type"], "InvalidRequestException");
        assert!(
            error_json["message"]
                .as_str()
                .unwrap()
                .contains("scheduled for deletion")
        );

        // 5. Restore secret
        let restore_body = json!({
            "SecretId": secret_name
        });
        let restore_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.RestoreSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&restore_body)
            .send()
            .await
            .expect("Failed to restore secret");
        assert!(
            restore_response.status().is_success(),
            "Secret restoration should succeed"
        );

        // 6. Verify secret is accessible again
        let get_response3 = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&get_body)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response3.status().is_success(),
            "Restored secret should be accessible"
        );

        // 7. Try to delete non-existent secret
        let delete_nonexistent_body = json!({
            "SecretId": "non-existent-secret"
        });
        let delete_response2 = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.DeleteSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&delete_nonexistent_body)
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
    async fn test_aws_controller_describe_secret() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";
        let secret_value = "test-value";

        // 1. Create secret
        let create_body = json!({
            "Name": secret_name,
            "SecretString": secret_value
        });
        let create_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.CreateSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Describe secret
        let describe_body = json!({
            "SecretId": secret_name
        });
        let describe_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.DescribeSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&describe_body)
            .send()
            .await
            .expect("Failed to describe secret");
        assert!(
            describe_response.status().is_success(),
            "Describe secret should succeed"
        );

        let describe_json: serde_json::Value = describe_response
            .json()
            .await
            .expect("Failed to parse describe response");
        assert_eq!(describe_json["Name"].as_str().unwrap(), secret_name);
        assert!(describe_json["ARN"].as_str().unwrap().contains(secret_name));
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_aws_controller_list_secrets() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();

        // 1. Create multiple secrets
        for i in 1..=3 {
            let secret_name = format!("test-secret-{}", i);
            let create_body = json!({
                "Name": secret_name,
                "SecretString": format!("value-{}", i)
            });
            let create_response = client
                .post(&format!("{}/", endpoint))
                .header("x-amz-target", "secretsmanager.CreateSecret")
                .header("content-type", "application/x-amz-json-1.1")
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
        let list_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.ListSecrets")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&json!({}))
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
        let secret_list = list_json["SecretList"].as_array().unwrap();
        assert!(secret_list.len() >= 3, "Should have at least 3 secrets");
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_aws_controller_list_secret_versions() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";

        // 1. Create secret
        let create_body = json!({
            "Name": secret_name,
            "SecretString": "value-1"
        });
        let create_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.CreateSecret")
            .header("content-type", "application/x-amz-json-1.1")
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
            .expect("Failed to parse create response");
        let version1_id = create_json["VersionId"].as_str().unwrap();

        // 2. Add second version
        let put_body = json!({
            "SecretId": secret_name,
            "SecretString": "value-2"
        });
        let put_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.PutSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&put_body)
            .send()
            .await
            .expect("Failed to put secret value");
        assert!(
            put_response.status().is_success(),
            "Put secret value should succeed"
        );
        let put_json: serde_json::Value = put_response
            .json()
            .await
            .expect("Failed to parse put response");
        let version2_id = put_json["VersionId"].as_str().unwrap();

        // 3. List versions
        let list_versions_body = json!({
            "SecretId": secret_name
        });
        let list_versions_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.ListSecretVersionIds")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&list_versions_body)
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
        let versions = versions_json["Versions"].as_array().unwrap();
        assert!(versions.len() >= 2, "Should have at least 2 versions");

        // Verify both versions are in the list
        let version_ids: Vec<&str> = versions
            .iter()
            .map(|v| v["VersionId"].as_str().unwrap())
            .collect();
        assert!(
            version_ids.contains(&version1_id),
            "Version 1 should be in list"
        );
        assert!(
            version_ids.contains(&version2_id),
            "Version 2 should be in list"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_aws_controller_get_secret_value_by_version() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";
        let secret_value1 = "value-1";
        let secret_value2 = "value-2";

        // 1. Create secret with first value
        let create_body = json!({
            "Name": secret_name,
            "SecretString": secret_value1
        });
        let create_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.CreateSecret")
            .header("content-type", "application/x-amz-json-1.1")
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
            .expect("Failed to parse create response");
        let version1_id = create_json["VersionId"].as_str().unwrap();

        // 2. Update to second value
        let put_body = json!({
            "SecretId": secret_name,
            "SecretString": secret_value2
        });
        let put_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.PutSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&put_body)
            .send()
            .await
            .expect("Failed to put secret value");
        assert!(
            put_response.status().is_success(),
            "Put secret value should succeed"
        );

        // 3. Get value by specific version ID (version 1)
        let get_version_body = json!({
            "SecretId": secret_name,
            "VersionId": version1_id
        });
        let get_version_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&get_version_body)
            .send()
            .await
            .expect("Failed to get secret value by version");
        assert!(
            get_version_response.status().is_success(),
            "Get secret value by version should succeed"
        );

        let get_version_json: serde_json::Value = get_version_response
            .json()
            .await
            .expect("Failed to parse get version response");
        assert_eq!(
            get_version_json["SecretString"].as_str().unwrap(),
            secret_value1,
            "Version 1 should have value-1"
        );
        assert_eq!(
            get_version_json["VersionId"].as_str().unwrap(),
            version1_id,
            "Should return correct version ID"
        );

        // 4. Get current value (without version ID)
        let get_current_body = json!({
            "SecretId": secret_name
        });
        let get_current_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&get_current_body)
            .send()
            .await
            .expect("Failed to get current secret value");
        assert!(
            get_current_response.status().is_success(),
            "Get current secret value should succeed"
        );

        let get_current_json: serde_json::Value = get_current_response
            .json()
            .await
            .expect("Failed to parse get current response");
        assert_eq!(
            get_current_json["SecretString"].as_str().unwrap(),
            secret_value2,
            "Current version should have value-2"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_aws_controller_update_secret() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";
        let secret_value = "test-value";

        // 1. Create secret
        let create_body = json!({
            "Name": secret_name,
            "SecretString": secret_value
        });
        let create_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.CreateSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Update secret metadata (description)
        let update_body = json!({
            "SecretId": secret_name,
            "Description": "Updated description"
        });
        let update_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.UpdateSecret")
            .header("content-type", "application/x-amz-json-1.1")
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
        assert_eq!(update_json["Name"].as_str().unwrap(), secret_name);
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_aws_controller_update_secret_version_stage() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";
        let secret_value1 = "test-value-1";
        let secret_value2 = "test-value-2";

        // 1. Create secret with first version
        let create_body = json!({
            "Name": secret_name,
            "SecretString": secret_value1
        });
        let create_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.CreateSecret")
            .header("content-type", "application/x-amz-json-1.1")
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
        let version_id_1 = create_json["VersionId"].as_str().unwrap();

        // 2. Add second version
        let put_body = json!({
            "SecretId": secret_name,
            "SecretString": secret_value2
        });
        let put_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.PutSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&put_body)
            .send()
            .await
            .expect("Failed to add version");
        assert!(
            put_response.status().is_success(),
            "Version addition should succeed"
        );
        let put_json: serde_json::Value =
            put_response.json().await.expect("Failed to parse response");
        let version_id_2 = put_json["VersionId"].as_str().unwrap();

        // 3. Verify AWSCURRENT points to version 2
        let describe_body = json!({
            "SecretId": secret_name
        });
        let describe_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.DescribeSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&describe_body)
            .send()
            .await
            .expect("Failed to describe secret");
        assert!(
            describe_response.status().is_success(),
            "Describe should succeed"
        );
        let describe_json: serde_json::Value = describe_response
            .json()
            .await
            .expect("Failed to parse response");
        let version_stages: &serde_json::Map<String, serde_json::Value> =
            describe_json["VersionIdToStages"].as_object().unwrap();
        assert_eq!(
            version_stages[version_id_2].as_array().unwrap()[0],
            "AWSCURRENT",
            "Version 2 should be AWSCURRENT"
        );
        assert_eq!(
            version_stages[version_id_1].as_array().unwrap()[0],
            "AWSPREVIOUS",
            "Version 1 should be AWSPREVIOUS"
        );

        // 4. Get secret value (should return version 2)
        let get_body = json!({
            "SecretId": secret_name
        });
        let get_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&get_body)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response.status().is_success(),
            "Get secret should succeed"
        );
        let get_json: serde_json::Value =
            get_response.json().await.expect("Failed to parse response");
        assert_eq!(
            get_json["SecretString"], secret_value2,
            "Should return version 2 value"
        );
        assert_eq!(
            get_json["VersionId"], version_id_2,
            "Should return version 2 ID"
        );

        // 5. Update staging: move AWSCURRENT back to version 1
        let update_stage_body = json!({
            "SecretId": secret_name,
            "VersionStage": "AWSCURRENT",
            "RemoveFromVersionId": version_id_2,
            "MoveToVersionId": version_id_1
        });
        let update_stage_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.UpdateSecretVersionStage")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&update_stage_body)
            .send()
            .await
            .expect("Failed to update version stage");
        assert!(
            update_stage_response.status().is_success(),
            "Update version stage should succeed"
        );

        // 6. Verify AWSCURRENT now points to version 1
        let describe_response2 = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.DescribeSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&describe_body)
            .send()
            .await
            .expect("Failed to describe secret");
        assert!(
            describe_response2.status().is_success(),
            "Describe should succeed"
        );
        let describe_json2: serde_json::Value = describe_response2
            .json()
            .await
            .expect("Failed to parse response");
        let version_stages2: &serde_json::Map<String, serde_json::Value> =
            describe_json2["VersionIdToStages"].as_object().unwrap();
        assert_eq!(
            version_stages2[version_id_1].as_array().unwrap()[0],
            "AWSCURRENT",
            "Version 1 should now be AWSCURRENT"
        );
        assert_eq!(
            version_stages2[version_id_2].as_array().unwrap()[0],
            "AWSPREVIOUS",
            "Version 2 should now be AWSPREVIOUS"
        );

        // 7. Get secret value (should now return version 1)
        let get_response2 = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&get_body)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response2.status().is_success(),
            "Get secret should succeed"
        );
        let get_json2: serde_json::Value = get_response2
            .json()
            .await
            .expect("Failed to parse response");
        assert_eq!(
            get_json2["SecretString"], secret_value1,
            "Should return version 1 value"
        );
        assert_eq!(
            get_json2["VersionId"], version_id_1,
            "Should return version 1 ID"
        );
    }

    #[tokio::test]
    #[ignore] // Requires mock server
    async fn test_aws_controller_secret_disable_enable() {
        init_test();

        // Start AWS mock server
        let mock_server = start_aws_mock_server()
            .await
            .expect("Failed to start AWS mock server");
        let endpoint = mock_server.endpoint().to_string();

        let client = reqwest::Client::new();
        let secret_name = "test-secret";
        let secret_value = "test-value";

        // 1. Create secret
        let create_body = json!({
            "Name": secret_name,
            "SecretString": secret_value
        });
        let create_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.CreateSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&create_body)
            .send()
            .await
            .expect("Failed to create secret");
        assert!(
            create_response.status().is_success(),
            "Secret creation should succeed"
        );

        // 2. Verify secret is accessible
        let get_body = json!({
            "SecretId": secret_name
        });
        let get_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&get_body)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response.status().is_success(),
            "Secret should be accessible before disable"
        );

        // 3. Disable secret (mark for deletion with recovery window)
        let delete_body = json!({
            "SecretId": secret_name,
            "RecoveryWindowInDays": 7
        });
        let delete_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.DeleteSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&delete_body)
            .send()
            .await
            .expect("Failed to disable secret");
        assert!(
            delete_response.status().is_success(),
            "Secret disable should succeed"
        );

        // 4. Verify secret is not accessible
        let get_response2 = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&get_body)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            !get_response2.status().is_success(),
            "Disabled secret should not be accessible"
        );

        // 5. Re-enable secret (restore)
        let restore_body = json!({
            "SecretId": secret_name
        });
        let restore_response = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.RestoreSecret")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&restore_body)
            .send()
            .await
            .expect("Failed to enable secret");
        assert!(
            restore_response.status().is_success(),
            "Secret enable should succeed"
        );

        // 6. Verify secret is accessible again
        let get_response3 = client
            .post(&format!("{}/", endpoint))
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header("content-type", "application/x-amz-json-1.1")
            .json(&get_body)
            .send()
            .await
            .expect("Failed to get secret");
        assert!(
            get_response3.status().is_success(),
            "Re-enabled secret should be accessible"
        );
    }
}
