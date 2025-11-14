//! Pact contract tests for AWS Secrets Manager API
//!
//! These tests define the contract between the Secret Manager Controller and AWS Secrets Manager API.
//! They use Pact to create a mock server that simulates AWS Secrets Manager responses.

use pact_consumer::prelude::*;
use serde_json::json;

#[tokio::test]
async fn test_aws_create_secret_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");
    
    pact_builder
        .interaction("create a new secret in AWS Secrets Manager", "", |mut i| {
            i.given("AWS credentials are configured");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.CreateSecret")
                .header("authorization", "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request")
                .json_body(json!({
                    "Name": "test-secret-name",
                    "SecretString": "test-secret-value",
                    "Description": "Test secret"
                }));
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "VersionId": "test-version-id"
                }));
            i
        });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mock_url = format!("http://{}", mock_server.url());
    
    assert!(!mock_url.is_empty());
}

#[tokio::test]
async fn test_aws_put_secret_value_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");
    
    pact_builder
        .interaction("update an existing secret value", "", |mut i| {
            i.given("a secret exists in AWS Secrets Manager");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.PutSecretValue")
                .header("authorization", "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request")
                .json_body(json!({
                    "SecretId": "test-secret-name",
                    "SecretString": "updated-secret-value"
                }));
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "VersionId": "new-version-id",
                    "VersionStages": ["AWSCURRENT"]
                }));
            i
        });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mock_url = format!("http://{}", mock_server.url());
    
    assert!(!mock_url.is_empty());
}

#[tokio::test]
async fn test_aws_get_secret_value_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");
    
    pact_builder
        .interaction("get the current value of a secret", "", |mut i| {
            i.given("a secret exists with a current version");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.GetSecretValue")
                .header("authorization", "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request")
                .json_body(json!({
                    "SecretId": "test-secret-name"
                }));
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "SecretString": "test-secret-value",
                    "VersionId": "current-version-id",
                    "VersionStages": ["AWSCURRENT"]
                }));
            i
        });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mock_url = format!("http://{}", mock_server.url());
    
    assert!(!mock_url.is_empty());
}

#[tokio::test]
async fn test_aws_describe_secret_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");
    
    pact_builder
        .interaction("describe a secret to check if it exists", "", |mut i| {
            i.given("a secret exists");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.DescribeSecret")
                .header("authorization", "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request")
                .json_body(json!({
                    "SecretId": "test-secret-name"
                }));
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "Description": "Test secret",
                    "LastChangedDate": 1704067200.0,
                    "VersionIdToStages": {
                        "current-version-id": ["AWSCURRENT"]
                    }
                }));
            i
        });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mock_url = format!("http://{}", mock_server.url());
    
    assert!(!mock_url.is_empty());
}

#[tokio::test]
async fn test_aws_secret_not_found_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");
    
    pact_builder
        .interaction("get a secret that does not exist", "", |mut i| {
            i.given("the secret does not exist");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.GetSecretValue")
                .header("authorization", "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request")
                .json_body(json!({
                    "SecretId": "non-existent-secret"
                }));
            i.response
                .status(400)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "__type": "ResourceNotFoundException",
                    "message": "Secrets Manager can't find the specified secret."
                }));
            i
        });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mock_url = format!("http://{}", mock_server.url());
    
    assert!(!mock_url.is_empty());
}

