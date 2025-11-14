//! Pact contract tests for Azure App Configuration REST API
//!
//! These tests define the contract between the Secret Manager Controller and Azure App Configuration REST API.
//! They use Pact to create a mock server that simulates Azure App Configuration responses.
//!
//! Azure App Configuration REST API endpoints:
//! - PUT /kv - Create or update a key-value pair
//! - GET /kv/{key} - Get a key-value pair
//! - DELETE /kv/{key} - Delete a key-value pair

use pact_consumer::prelude::*;
use reqwest;
use serde_json::json;

// Helper function to make HTTP requests to the mock server
async fn make_request(
    client: &reqwest::Client,
    method: &str,
    url: &str,
    body: Option<serde_json::Value>,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut request = match method {
        "GET" => client.get(url),
        "PUT" => client.put(url),
        "DELETE" => client.delete(url),
        _ => panic!("Unsupported HTTP method: {}", method),
    };

    request = request
        .header("authorization", "Bearer test-token")
        .header("content-type", "application/json");

    if let Some(body) = body {
        request = request.json(&body);
    }

    request.send().await
}

#[tokio::test]
async fn test_azure_app_config_put_key_value_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Azure-App-Configuration");

    pact_builder.interaction(
        "create or update a key-value pair in Azure App Configuration",
        "",
        |mut i| {
            i.given("Azure App Configuration exists and credentials are configured");
            i.request
                .method("PUT")
                .path("/kv")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .json_body(json!({
                    "key": "my-service:prod:database.host",
                    "value": "db.example.com",
                    "content_type": "text/plain"
                }));
            i.response
                .status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "key": "my-service:prod:database.host",
                    "value": "db.example.com",
                    "content_type": "text/plain",
                    "etag": "etag-12345",
                    "last_modified": "2024-01-01T00:00:00Z"
                }));
            i
        },
    );

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{}/kv", base_url);

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "PUT",
        &mock_url,
        Some(json!({
            "key": "my-service:prod:database.host",
            "value": "db.example.com",
            "content_type": "text/plain"
        })),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["key"], "my-service:prod:database.host");
    assert_eq!(body["value"], "db.example.com");
}

#[tokio::test]
async fn test_azure_app_config_get_key_value_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Azure-App-Configuration");

    pact_builder.interaction(
        "get a key-value pair from Azure App Configuration",
        "",
        |mut i| {
            i.given("a key-value pair exists in Azure App Configuration");
            i.request
                .method("GET")
                .path("/kv/my-service:prod:database.host")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json");
            i.response
                .status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "key": "my-service:prod:database.host",
                    "value": "db.example.com",
                    "content_type": "text/plain",
                    "etag": "etag-12345",
                    "last_modified": "2024-01-01T00:00:00Z"
                }));
            i
        },
    );

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{}/kv/my-service:prod:database.host", base_url);

    let client = reqwest::Client::new();
    let response = make_request(&client, "GET", &mock_url, None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["key"], "my-service:prod:database.host");
    assert_eq!(body["value"], "db.example.com");
}

#[tokio::test]
async fn test_azure_app_config_get_key_value_not_found_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Azure-App-Configuration");

    pact_builder.interaction(
        "get a non-existent key-value pair from Azure App Configuration",
        "",
        |mut i| {
            i.given("the key-value pair does not exist");
            i.request
                .method("GET")
                .path("/kv/my-service:prod:nonexistent.key")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json");
            i.response
                .status(404)
                .header("content-type", "application/json");
            i
        },
    );

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{}/kv/my-service:prod:nonexistent.key", base_url);

    let client = reqwest::Client::new();
    let response = make_request(&client, "GET", &mock_url, None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_azure_app_config_delete_key_value_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Azure-App-Configuration");

    pact_builder.interaction(
        "delete a key-value pair from Azure App Configuration",
        "",
        |mut i| {
            i.given("a key-value pair exists in Azure App Configuration");
            i.request
                .method("DELETE")
                .path("/kv/my-service:prod:database.host")
                .header("authorization", "Bearer test-token");
            i.response
                .status(200)
                .header("content-type", "application/json");
            i
        },
    );

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{}/kv/my-service:prod:database.host", base_url);

    let client = reqwest::Client::new();
    let response = make_request(&client, "DELETE", &mock_url, None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_azure_app_config_delete_key_value_not_found_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Azure-App-Configuration");

    pact_builder.interaction(
        "delete a non-existent key-value pair from Azure App Configuration",
        "",
        |mut i| {
            i.given("the key-value pair does not exist");
            i.request
                .method("DELETE")
                .path("/kv/my-service:prod:nonexistent.key")
                .header("authorization", "Bearer test-token");
            i.response
                .status(404)
                .header("content-type", "application/json");
            i
        },
    );

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{}/kv/my-service:prod:nonexistent.key", base_url);

    let client = reqwest::Client::new();
    let response = make_request(&client, "DELETE", &mock_url, None)
        .await
        .expect("Failed to make request");

    // Azure App Configuration may return 404 or 200 for delete of non-existent key
    // Our implementation should handle both gracefully
    assert!(response.status() == 200 || response.status() == 404);
}

#[tokio::test]
async fn test_azure_app_config_update_existing_key_value_contract() {
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Azure-App-Configuration");

    pact_builder.interaction(
        "update an existing key-value pair in Azure App Configuration",
        "",
        |mut i| {
            i.given("a key-value pair exists in Azure App Configuration");
            i.request
                .method("PUT")
                .path("/kv")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .json_body(json!({
                    "key": "my-service:prod:database.host",
                    "value": "db-updated.example.com",
                    "content_type": "text/plain"
                }));
            i.response
                .status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "key": "my-service:prod:database.host",
                    "value": "db-updated.example.com",
                    "content_type": "text/plain",
                    "etag": "etag-67890",
                    "last_modified": "2024-01-02T00:00:00Z"
                }));
            i
        },
    );

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{}/kv", base_url);

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "PUT",
        &mock_url,
        Some(json!({
            "key": "my-service:prod:database.host",
            "value": "db-updated.example.com",
            "content_type": "text/plain"
        })),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["key"], "my-service:prod:database.host");
    assert_eq!(body["value"], "db-updated.example.com");
}
