//! Pact contract tests for AWS Systems Manager Parameter Store API
//!
//! These tests define the contract between the Secret Manager Controller and AWS Parameter Store API.
//! They use Pact to create a mock server that simulates AWS Parameter Store responses.

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
            "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/ssm/aws4_request",
        )
        .header("x-amz-target", x_amz_target);

    if let Some(body) = body {
        // AWS SSM uses application/x-amz-json-1.1, not application/json
        request = request
            .header("content-type", "application/x-amz-json-1.1")
            .body(serde_json::to_string(&body).unwrap());
    }

    request.send().await
}

#[tokio::test]
async fn test_aws_put_parameter_create_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Parameter-Store");

    pact_builder.interaction(
        "create a new parameter in AWS Parameter Store",
        "",
        |mut i| {
            i.given("AWS credentials are configured");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "AmazonSSM.PutParameter")
                .header(
                    "authorization",
                    "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/ssm/aws4_request",
                )
                .body(
                    json!({
                        "Name": "/my-service/dev/database_host",
                        "Value": "db.example.com",
                        "Type": "String",
                        "Overwrite": false
                    })
                    .to_string(),
                );
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "Version": 1,
                    "Tier": "Standard"
                }));
            i
        },
    );

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
            "Name": "/my-service/dev/database_host",
            "Value": "db.example.com",
            "Type": "String",
            "Overwrite": false
        })),
        "AmazonSSM.PutParameter",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["Version"], 1);
}

#[tokio::test]
async fn test_aws_put_parameter_update_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Parameter-Store");

    pact_builder.interaction(
        "update an existing parameter in AWS Parameter Store",
        "",
        |mut i| {
            i.given("a parameter exists");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "AmazonSSM.PutParameter")
                .header(
                    "authorization",
                    "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/ssm/aws4_request",
                )
                .body(
                    json!({
                        "Name": "/my-service/dev/database_host",
                        "Value": "db-updated.example.com",
                        "Type": "String",
                        "Overwrite": true
                    })
                    .to_string(),
                );
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "Version": 2,
                    "Tier": "Standard"
                }));
            i
        },
    );

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
            "Name": "/my-service/dev/database_host",
            "Value": "db-updated.example.com",
            "Type": "String",
            "Overwrite": true
        })),
        "AmazonSSM.PutParameter",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["Version"], 2);
}

#[tokio::test]
async fn test_aws_get_parameter_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Parameter-Store");

    pact_builder
        .interaction("get the value of a parameter", "", |mut i| {
            i.given("a parameter exists");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "AmazonSSM.GetParameter")
                .header("authorization", "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/ssm/aws4_request")
                .body(json!({
                    "Name": "/my-service/dev/database_host",
                    "WithDecryption": true
                }).to_string());
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({
                    "Parameter": {
                        "Name": "/my-service/dev/database_host",
                        "Type": "String",
                        "Value": "db.example.com",
                        "Version": 1,
                        "LastModifiedDate": 1704067200.0,
                        "ARN": "arn:aws:ssm:us-east-1:123456789012:parameter/my-service/dev/database_host"
                    }
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
            "Name": "/my-service/dev/database_host",
            "WithDecryption": true
        })),
        "AmazonSSM.GetParameter",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["Parameter"]["Name"], "/my-service/dev/database_host");
    assert_eq!(body["Parameter"]["Value"], "db.example.com");
}

#[tokio::test]
async fn test_aws_get_parameter_not_found_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Parameter-Store");

    pact_builder.interaction("get a parameter that does not exist", "", |mut i| {
        i.given("the parameter does not exist");
        i.request
            .method("POST")
            .path("/")
            .header("content-type", "application/x-amz-json-1.1")
            .header("x-amz-target", "AmazonSSM.GetParameter")
            .header(
                "authorization",
                "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/ssm/aws4_request",
            )
            .body(
                json!({
                    "Name": "/my-service/dev/nonexistent",
                    "WithDecryption": true
                })
                .to_string(),
            );
        i.response
            .status(400)
            .header("content-type", "application/x-amz-json-1.1")
            .json_body(json!({
                "__type": "ParameterNotFound",
                "message": "Parameter /my-service/dev/nonexistent not found"
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
            "Name": "/my-service/dev/nonexistent",
            "WithDecryption": true
        })),
        "AmazonSSM.GetParameter",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 400);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["__type"], "ParameterNotFound");
}

#[tokio::test]
async fn test_aws_delete_parameter_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Parameter-Store");

    pact_builder.interaction(
        "delete a parameter from AWS Parameter Store",
        "",
        |mut i| {
            i.given("a parameter exists");
            i.request
                .method("POST")
                .path("/")
                .header("content-type", "application/x-amz-json-1.1")
                .header("x-amz-target", "AmazonSSM.DeleteParameter")
                .header(
                    "authorization",
                    "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/ssm/aws4_request",
                )
                .body(
                    json!({
                        "Name": "/my-service/dev/database_host"
                    })
                    .to_string(),
                );
            i.response
                .status(200)
                .header("content-type", "application/x-amz-json-1.1")
                .json_body(json!({}));
            i
        },
    );

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
            "Name": "/my-service/dev/database_host"
        })),
        "AmazonSSM.DeleteParameter",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_aws_get_parameters_by_path_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Parameter-Store");

    pact_builder.interaction("list parameters by path", "", |mut i| {
        i.given("multiple parameters exist under the path");
        i.request
            .method("POST")
            .path("/")
            .header("content-type", "application/x-amz-json-1.1")
            .header("x-amz-target", "AmazonSSM.GetParametersByPath")
            .header(
                "authorization",
                "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/ssm/aws4_request",
            )
            .body(
                json!({
                    "Path": "/my-service/dev",
                    "Recursive": false,
                    "WithDecryption": true
                })
                .to_string(),
            );
        i.response
            .status(200)
            .header("content-type", "application/x-amz-json-1.1")
            .json_body(json!({
                "Parameters": [
                    {
                        "Name": "/my-service/dev/database_host",
                        "Type": "String",
                        "Value": "db.example.com",
                        "Version": 1
                    },
                    {
                        "Name": "/my-service/dev/database_port",
                        "Type": "String",
                        "Value": "5432",
                        "Version": 1
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
            "Path": "/my-service/dev",
            "Recursive": false,
            "WithDecryption": true
        })),
        "AmazonSSM.GetParametersByPath",
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["Parameters"].is_array());
    assert_eq!(body["Parameters"].as_array().unwrap().len(), 2);
}
