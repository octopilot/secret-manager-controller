//! Pact contract tests for AWS Secrets Manager API
//!
//! These tests define the contract between the Secret Manager Controller and AWS Secrets Manager API.
//! They use Pact to create a mock server that simulates AWS Secrets Manager responses.

use pact_consumer::prelude::*;
use serde_json::json;

// Helper function to make HTTP requests to the mock server
async fn make_request(
    client: &reqwest::Client,
    method: &str,
    url: &str,
    body: Option<serde_json::Value>,
    x_amz_target: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut request = match method {
        "GET" => client.get(url),
        "POST" => client.post(url),
        "PUT" => client.put(url),
        _ => panic!("Unsupported HTTP method: {}", method),
    };

    // Add default headers
    request = request
        .header(
            "authorization",
            "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request",
        )
        .header("x-amz-target", x_amz_target);

    if let Some(body) = body {
        // AWS Secrets Manager uses application/x-amz-json-1.1, not application/json
        // Set body as string and content-type header manually
        request = request
            .header("content-type", "application/x-amz-json-1.1")
            .body(serde_json::to_string(&body).unwrap());
    }

    request.send().await
}

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
                .body(json!({
                    "Name": "test-secret-name",
                    "SecretString": "test-secret-value",
                    "Description": "Test secret"
                }).to_string());
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
    // mock_server.url() returns a Url struct - convert to string and strip trailing slash
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    // Path always starts with /
    let mock_url = format!("{}/", base_url);

    // Make the actual HTTP request to verify the contract
    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "Name": "test-secret-name",
            "SecretString": "test-secret-value",
            "Description": "Test secret"
        })),
        "secretsmanager.CreateSecret",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["Name"], "test-secret-name");
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
                .body(json!({
                    "SecretId": "test-secret-name",
                    "SecretString": "updated-secret-value"
                }).to_string());
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
    // mock_server.url() returns a Url struct - convert to string and strip trailing slash
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    // Path always starts with /
    let mock_url = format!("{}/", base_url);

    // Make the actual HTTP request to verify the contract
    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "SecretId": "test-secret-name",
            "SecretString": "updated-secret-value"
        })),
        "secretsmanager.PutSecretValue",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["Name"], "test-secret-name");
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
                .body(json!({
                    "SecretId": "test-secret-name"
                }).to_string());
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
    // mock_server.url() returns a Url struct - convert to string and strip trailing slash
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    // Path always starts with /
    let mock_url = format!("{}/", base_url);

    // Make the actual HTTP request to verify the contract
    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "SecretId": "test-secret-name"
        })),
        "secretsmanager.GetSecretValue",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["Name"], "test-secret-name");
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
                .body(json!({
                    "SecretId": "test-secret-name"
                }).to_string());
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
    // mock_server.url() returns a Url struct - convert to string and strip trailing slash
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    // Path always starts with /
    let mock_url = format!("{}/", base_url);

    // Make the actual HTTP request to verify the contract
    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "SecretId": "test-secret-name"
        })),
        "secretsmanager.DescribeSecret",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["Name"], "test-secret-name");
}

#[tokio::test]
async fn test_aws_secret_not_found_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");

    pact_builder.interaction("get a secret that does not exist", "", |mut i| {
        i.given("the secret does not exist");
        i.request
            .method("POST")
            .path("/")
            .header("content-type", "application/x-amz-json-1.1")
            .header("x-amz-target", "secretsmanager.GetSecretValue")
            .header(
                "authorization",
                "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request",
            )
            .body(
                json!({
                    "SecretId": "non-existent-secret"
                })
                .to_string(),
            );
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
    // mock_server.url() returns a Url struct - convert to string and strip trailing slash
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    // Path always starts with /
    let mock_url = format!("{}/", base_url);

    // Make the actual HTTP request to verify the contract
    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "SecretId": "non-existent-secret"
        })),
        "secretsmanager.GetSecretValue",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 400);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["__type"], "ResourceNotFoundException");
}

#[tokio::test]
async fn test_aws_list_secrets_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");

    pact_builder
        .interaction("list secrets in AWS Secrets Manager", "", |mut i| {
            i.given("AWS credentials are configured");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.ListSecrets")
                .header("authorization", "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request")
                .body(json!({}).to_string());
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "SecretList": [
                        {
                            "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:secret-1-abc123",
                            "Name": "secret-1",
                            "LastChangedDate": 1704067200.0
                        },
                        {
                            "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:secret-2-def456",
                            "Name": "secret-2",
                            "LastChangedDate": 1704153600.0
                        }
                    ],
                    "NextToken": null
                }));
            i
        });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{}/", base_url);

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({})),
        "secretsmanager.ListSecrets",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["SecretList"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_aws_list_secret_version_ids_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");

    pact_builder
        .interaction("list secret version IDs", "", |mut i| {
            i.given("a secret exists with multiple versions");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.ListSecretVersionIds")
                .header("authorization", "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request")
                .body(json!({
                    "SecretId": "test-secret-name"
                }).to_string());
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "Versions": [
                        {
                            "VersionId": "version-1",
                            "VersionStages": ["AWSCURRENT"],
                            "CreatedDate": 1704067200.0
                        },
                        {
                            "VersionId": "version-2",
                            "VersionStages": [],
                            "CreatedDate": 1704153600.0
                        }
                    ],
                    "NextToken": null
                }));
            i
        });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{}/", base_url);

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "SecretId": "test-secret-name"
        })),
        "secretsmanager.ListSecretVersionIds",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["Versions"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_aws_delete_secret_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");

    pact_builder
        .interaction("delete a secret", "", |mut i| {
            i.given("a secret exists in AWS Secrets Manager");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.DeleteSecret")
                .header("authorization", "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request")
                .body(json!({
                    "SecretId": "test-secret-name",
                    "ForceDeleteWithoutRecovery": true
                }).to_string());
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "DeletionDate": 1704067200.0
                }));
            i
        });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{}/", base_url);

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "SecretId": "test-secret-name",
            "ForceDeleteWithoutRecovery": true
        })),
        "secretsmanager.DeleteSecret",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["Name"], "test-secret-name");
}

#[tokio::test]
async fn test_aws_tag_secret_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");

    pact_builder.interaction("tag a secret", "", |mut i| {
        i.given("a secret exists in AWS Secrets Manager");
        i.request
            .method("POST")
            .path("/")
            .header("content-type", "application/x-amz-json-1.1")
            .header("x-amz-target", "secretsmanager.TagResource")
            .header(
                "authorization",
                "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request",
            )
            .body(
                json!({
                    "SecretId": "test-secret-name",
                    "Tags": [
                        {
                            "Key": "Environment",
                            "Value": "Production"
                        },
                        {
                            "Key": "Team",
                            "Value": "DevOps"
                        }
                    ]
                })
                .to_string(),
            );
        i.response
            .status(200)
            .header("content-type", "application/x-amz-json-1.1")
            .json_body(json!({}));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{}/", base_url);

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "SecretId": "test-secret-name",
            "Tags": [
                {
                    "Key": "Environment",
                    "Value": "Production"
                },
                {
                    "Key": "Team",
                    "Value": "DevOps"
                }
            ]
        })),
        "secretsmanager.TagResource",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_aws_untag_secret_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");

    pact_builder.interaction("untag a secret", "", |mut i| {
        i.given("a secret exists with tags");
        i.request
            .method("POST")
            .path("/")
            .header("content-type", "application/x-amz-json-1.1")
            .header("x-amz-target", "secretsmanager.UntagResource")
            .header(
                "authorization",
                "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request",
            )
            .body(
                json!({
                    "SecretId": "test-secret-name",
                    "TagKeys": ["Environment", "Team"]
                })
                .to_string(),
            );
        i.response
            .status(200)
            .header("content-type", "application/x-amz-json-1.1")
            .json_body(json!({}));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{}/", base_url);

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "SecretId": "test-secret-name",
            "TagKeys": ["Environment", "Team"]
        })),
        "secretsmanager.UntagResource",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_aws_update_secret_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");

    pact_builder
        .interaction("update secret metadata", "", |mut i| {
            i.given("a secret exists in AWS Secrets Manager");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.UpdateSecret")
                .header("authorization", "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request")
                .body(json!({
                    "SecretId": "test-secret-name",
                    "Description": "Updated description"
                }).to_string());
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "VersionId": "current-version-id"
                }));
            i
        });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{}/", base_url);

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "SecretId": "test-secret-name",
            "Description": "Updated description"
        })),
        "secretsmanager.UpdateSecret",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["Name"], "test-secret-name");
}

#[tokio::test]
async fn test_aws_restore_secret_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");

    pact_builder
        .interaction("restore a deleted secret", "", |mut i| {
            i.given("a deleted secret exists");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.RestoreSecret")
                .header("authorization", "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request")
                .body(json!({
                    "SecretId": "test-secret-name"
                }).to_string());
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name"
                }));
            i
        });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{}/", base_url);

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "SecretId": "test-secret-name"
        })),
        "secretsmanager.RestoreSecret",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["Name"], "test-secret-name");
}

#[tokio::test]
async fn test_aws_get_resource_policy_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");

    pact_builder
        .interaction("get resource policy for a secret", "", |mut i| {
            i.given("a secret exists with a resource policy");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "secretsmanager.GetResourcePolicy")
                .header("authorization", "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request")
                .body(json!({
                    "SecretId": "test-secret-name"
                }).to_string());
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:test-secret-name-abc123",
                    "Name": "test-secret-name",
                    "ResourcePolicy": "{\"Version\":\"2012-10-17\",\"Statement\":[{\"Effect\":\"Allow\",\"Principal\":{\"AWS\":\"arn:aws:iam::123456789012:root\"},\"Action\":\"secretsmanager:GetSecretValue\",\"Resource\":\"*\"}]}"
                }));
            i
        });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{}/", base_url);

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "SecretId": "test-secret-name"
        })),
        "secretsmanager.GetResourcePolicy",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["Name"], "test-secret-name");
    assert!(body["ResourcePolicy"].as_str().is_some());
}
