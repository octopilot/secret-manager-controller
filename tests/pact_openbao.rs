//! Pact contract tests for the OpenBao (Vault-compatible) HTTP API.
//!
//! Defines the contract between the Secret Manager Controller and OpenBao:
//! Kubernetes auth login and the KV v2 secrets engine (read/write/delete/
//! undelete/metadata). Uses Pact to stand up a mock server that simulates
//! OpenBao responses.

#[cfg(test)]
mod common;

use common::init_rustls;
use std::sync::Once;

static RUSTLS_INIT: Once = Once::new();

/// Initialize rustls before tests.
fn init() {
    RUSTLS_INIT.call_once(|| {
        init_rustls();
    });
}

use pact_consumer::prelude::*;
use serde_json::json;

/// Make an HTTP request to the mock server with the OpenBao token header.
async fn make_request(
    client: &reqwest::Client,
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut request = match method {
        "GET" => client.get(url),
        "POST" => client.post(url),
        "DELETE" => client.delete(url),
        _ => panic!("Unsupported HTTP method: {method}"),
    };

    if let Some(token) = token {
        request = request.header("X-Vault-Token", token);
    }
    request = request.header("content-type", "application/json");

    if let Some(body) = body {
        request = request.json(&body);
    }

    request.send().await
}

/// Strip the trailing slash Pact's `mock_server.url()` includes.
fn strip_trailing_slash(mut base: String) -> String {
    if base.ends_with('/') {
        base.pop();
    }
    base
}

#[tokio::test]
async fn test_openbao_kubernetes_login_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "OpenBao");

    pact_builder.interaction(
        "authenticate via the Kubernetes auth method",
        "",
        |mut i| {
            i.given("a Kubernetes auth role bound to the controller ServiceAccount");
            i.request
                .method("POST")
                .path("/v1/auth/kubernetes/login".to_string())
                .header("content-type", "application/json")
                .json_body(json!({
                    "role": "secret-manager-controller",
                    "jwt": "test-jwt"
                }));
            i.response
                .status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "auth": {
                        "client_token": "s.testclienttoken",
                        "lease_duration": 3600,
                        "renewable": true
                    }
                }));
            i
        },
    );

    let mock_server = pact_builder.start_mock_server(None, None);
    let url = format!(
        "{}/v1/auth/kubernetes/login",
        strip_trailing_slash(mock_server.url().to_string())
    );

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &url,
        None,
        Some(json!({ "role": "secret-manager-controller", "jwt": "test-jwt" })),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["auth"]["client_token"], "s.testclienttoken");
}

#[tokio::test]
async fn test_openbao_kv_write_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "OpenBao");

    pact_builder.interaction("write a new version of a KV v2 secret", "", |mut i| {
        i.given("the KV v2 engine is mounted at secret/");
        i.request
            .method("POST")
            .path("/v1/secret/data/my-service/db-password".to_string())
            .header("X-Vault-Token", "s.testclienttoken")
            .header("content-type", "application/json")
            .json_body(json!({ "data": { "value": "s3cr3t" } }));
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "data": {
                    "version": 1,
                    "created_time": "2026-07-16T00:00:00Z",
                    "deletion_time": "",
                    "destroyed": false
                }
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let url = format!(
        "{}/v1/secret/data/my-service/db-password",
        strip_trailing_slash(mock_server.url().to_string())
    );

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &url,
        Some("s.testclienttoken"),
        Some(json!({ "data": { "value": "s3cr3t" } })),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["data"]["version"], 1);
}

#[tokio::test]
async fn test_openbao_kv_read_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "OpenBao");

    pact_builder.interaction("read the latest version of a KV v2 secret", "", |mut i| {
        i.given("a secret exists at secret/my-service/db-password");
        i.request
            .method("GET")
            .path("/v1/secret/data/my-service/db-password".to_string())
            .header("X-Vault-Token", "s.testclienttoken");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "data": {
                    "data": { "value": "s3cr3t" },
                    "metadata": {
                        "version": 1,
                        "created_time": "2026-07-16T00:00:00Z",
                        "deletion_time": "",
                        "destroyed": false
                    }
                }
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let url = format!(
        "{}/v1/secret/data/my-service/db-password",
        strip_trailing_slash(mock_server.url().to_string())
    );

    let client = reqwest::Client::new();
    let response = make_request(&client, "GET", &url, Some("s.testclienttoken"), None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["data"]["data"]["value"], "s3cr3t");
}

#[tokio::test]
async fn test_openbao_kv_read_not_found_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "OpenBao");

    pact_builder.interaction("read a secret that does not exist", "", |mut i| {
        i.given("no secret exists at the path");
        i.request
            .method("GET")
            .path("/v1/secret/data/my-service/missing".to_string())
            .header("X-Vault-Token", "s.testclienttoken");
        i.response
            .status(404)
            .header("content-type", "application/json")
            .json_body(json!({ "errors": [] }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let url = format!(
        "{}/v1/secret/data/my-service/missing",
        strip_trailing_slash(mock_server.url().to_string())
    );

    let client = reqwest::Client::new();
    let response = make_request(&client, "GET", &url, Some("s.testclienttoken"), None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_openbao_kv_soft_delete_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "OpenBao");

    pact_builder.interaction("soft-delete the latest version (disable)", "", |mut i| {
        i.given("a secret exists at secret/my-service/db-password");
        i.request
            .method("DELETE")
            .path("/v1/secret/data/my-service/db-password".to_string())
            .header("X-Vault-Token", "s.testclienttoken");
        i.response.status(204);
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let url = format!(
        "{}/v1/secret/data/my-service/db-password",
        strip_trailing_slash(mock_server.url().to_string())
    );

    let client = reqwest::Client::new();
    let response = make_request(&client, "DELETE", &url, Some("s.testclienttoken"), None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 204);
}

#[tokio::test]
async fn test_openbao_kv_undelete_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "OpenBao");

    pact_builder.interaction("undelete a soft-deleted version (enable)", "", |mut i| {
        i.given("version 1 of the secret is soft-deleted");
        i.request
            .method("POST")
            .path("/v1/secret/undelete/my-service/db-password".to_string())
            .header("X-Vault-Token", "s.testclienttoken")
            .header("content-type", "application/json")
            .json_body(json!({ "versions": [1] }));
        i.response.status(204);
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let url = format!(
        "{}/v1/secret/undelete/my-service/db-password",
        strip_trailing_slash(mock_server.url().to_string())
    );

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &url,
        Some("s.testclienttoken"),
        Some(json!({ "versions": [1] })),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 204);
}

#[tokio::test]
async fn test_openbao_kv_metadata_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "OpenBao");

    pact_builder.interaction("read version metadata for a secret", "", |mut i| {
        i.given("a secret exists with at least one version");
        i.request
            .method("GET")
            .path("/v1/secret/metadata/my-service/db-password".to_string())
            .header("X-Vault-Token", "s.testclienttoken");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "data": {
                    "current_version": 3,
                    "oldest_version": 1,
                    "max_versions": 0
                }
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let url = format!(
        "{}/v1/secret/metadata/my-service/db-password",
        strip_trailing_slash(mock_server.url().to_string())
    );

    let client = reqwest::Client::new();
    let response = make_request(&client, "GET", &url, Some("s.testclienttoken"), None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["data"]["current_version"], 3);
}

#[tokio::test]
async fn test_openbao_kv_delete_all_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "OpenBao");

    pact_builder.interaction("delete a secret and all its versions", "", |mut i| {
        i.given("a secret exists at secret/my-service/db-password");
        i.request
            .method("DELETE")
            .path("/v1/secret/metadata/my-service/db-password".to_string())
            .header("X-Vault-Token", "s.testclienttoken");
        i.response.status(204);
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let url = format!(
        "{}/v1/secret/metadata/my-service/db-password",
        strip_trailing_slash(mock_server.url().to_string())
    );

    let client = reqwest::Client::new();
    let response = make_request(&client, "DELETE", &url, Some("s.testclienttoken"), None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 204);
}
