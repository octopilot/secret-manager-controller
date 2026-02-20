//! Pact contract tests for GCP Parameter Manager API
//!
//! These tests define the contract between the Secret Manager Controller and GCP Parameter Manager API.
//! They use Pact to create a mock server that simulates GCP Parameter Manager responses.
//!
//! GCP Parameter Manager REST API endpoints:
//! - POST /v1/projects/{project}/locations/{location}/parameters - Create parameter
//! - GET /v1/projects/{project}/locations/{location}/parameters - List parameters
//! - GET /v1/projects/{project}/locations/{location}/parameters/{parameter} - Get parameter
//! - PATCH /v1/projects/{project}/locations/{location}/parameters/{parameter} - Update parameter
//! - DELETE /v1/projects/{project}/locations/{location}/parameters/{parameter} - Delete parameter
//! - POST /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions - Create parameter version
//! - GET /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions - List versions
//! - GET /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version} - Get version
//! - PATCH /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version} - Update version
//! - DELETE /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version} - Delete version
//! - GET /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}:render - Render version
//! - GET /v1/projects/{project}/locations/{location} - Get location
//! - GET /v1/projects/{project}/locations - List locations
//!
//! API Reference: https://docs.cloud.google.com/secret-manager/parameter-manager/docs/reference/rest

#[cfg(test)]
mod common;

use common::init_rustls;
use std::sync::Once;

static RUSTLS_INIT: Once = Once::new();

/// Initialize rustls before tests
fn init() {
    RUSTLS_INIT.call_once(|| {
        init_rustls();
    });
}

use pact_consumer::prelude::*;
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
        "POST" => client.post(url),
        "PATCH" => client.patch(url),
        "DELETE" => client.delete(url),
        _ => panic!("Unsupported HTTP method: {method}"),
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
async fn test_gcp_create_parameter_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Parameter-Manager");

    pact_builder.interaction(
        "create a new parameter in GCP Parameter Manager",
        "",
        |mut i| {
            i.given("a GCP project exists");
            i.request
                .method("POST")
                .path("/v1/projects/test-project/locations/global/parameters".to_string())
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .json_body(json!({
                    "parameterId": "test-parameter-name",
                    "parameter": {
                        "format": "PLAIN_TEXT"
                    }
                }));
            i.response
                .status(200)
                .header("content-type", "application/json")
                .json_body(json!({
                    "name": "projects/test-project/locations/global/parameters/test-parameter-name",
                    "format": "PLAIN_TEXT",
                    "createTime": "2024-01-01T00:00:00Z"
                }));
            i
        },
    );

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{base_url}/v1/projects/test-project/locations/global/parameters");

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "parameterId": "test-parameter-name",
            "parameter": {
                "format": "PLAIN_TEXT"
            }
        })),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(
        body["name"],
        "projects/test-project/locations/global/parameters/test-parameter-name"
    );
}

#[tokio::test]
async fn test_gcp_add_parameter_version_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Parameter-Manager");

    pact_builder.interaction("create a version for a parameter", "", |mut i| {
        i.given("a parameter exists");
        i.request
            .method("POST")
            .path("/v1/projects/test-project/locations/global/parameters/test-parameter-name/versions".to_string())
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json")
            .json_body(json!({
                "parameterVersionId": "v1234567890",
                "parameterVersion": {
                    "payload": {
                        "data": "ZGIuZXhhbXBsZS5jb20="  // base64 encoded "db.example.com"
                    }
                }
            }));
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/locations/global/parameters/test-parameter-name/versions/v1234567890",
                "createTime": "2024-01-01T00:00:00Z"
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!(
        "{base_url}/v1/projects/test-project/locations/global/parameters/test-parameter-name/versions"
    );

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &mock_url,
        Some(json!({
            "parameterVersionId": "v1234567890",
            "parameterVersion": {
                "payload": {
                    "data": "ZGIuZXhhbXBsZS5jb20="
                }
            }
        })),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(
        body["name"]
            .as_str()
            .unwrap()
            .contains("test-parameter-name")
    );
}

#[tokio::test]
async fn test_gcp_get_parameter_version_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Parameter-Manager");

    pact_builder.interaction("list versions of a parameter", "", |mut i| {
        i.given("a parameter with versions exists");
        i.request
            .method("GET")
            .path("/v1/projects/test-project/locations/global/parameters/test-parameter-name/versions".to_string())
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "versions": [
                    {
                        "name": "projects/test-project/locations/global/parameters/test-parameter-name/versions/v1234567890",
                        "createTime": "2024-01-01T00:00:00Z",
                        "state": "ENABLED"
                    }
                ]
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!(
        "{base_url}/v1/projects/test-project/locations/global/parameters/test-parameter-name/versions"
    );

    let client = reqwest::Client::new();
    let response = make_request(&client, "GET", &mock_url, None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["versions"].is_array());
    assert_eq!(body["versions"].as_array().unwrap().len(), 1);
    assert!(
        body["versions"][0]["name"]
            .as_str()
            .unwrap()
            .contains("test-parameter-name")
    );
}

#[tokio::test]
async fn test_gcp_get_parameter_version_not_found_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Parameter-Manager");

    pact_builder.interaction("list versions for a parameter that does not exist", "", |mut i| {
        i.given("the parameter does not exist");
        i.request
            .method("GET")
            .path("/v1/projects/test-project/locations/global/parameters/nonexistent-parameter/versions".to_string())
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json");
        i.response
            .status(404)
            .header("content-type", "application/json")
            .json_body(json!({
                "error": {
                    "code": 404,
                    "message": "Parameter not found: nonexistent-parameter",
                    "status": "NOT_FOUND"
                }
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!(
        "{base_url}/v1/projects/test-project/locations/global/parameters/nonexistent-parameter/versions"
    );

    let client = reqwest::Client::new();
    let response = make_request(&client, "GET", &mock_url, None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 404);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["error"]["code"], 404);
}

#[tokio::test]
async fn test_gcp_delete_parameter_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Parameter-Manager");

    pact_builder.interaction(
        "delete a parameter from GCP Parameter Manager",
        "",
        |mut i| {
            i.given("a parameter exists");
            i.request
                .method("DELETE")
                .path(
                    "/v1/projects/test-project/locations/global/parameters/test-parameter-name"
                        .to_string(),
                )
                .header("authorization", "Bearer test-token");
            i.response
                .status(200)
                .header("content-type", "application/json")
                .json_body(json!({}));
            i
        },
    );

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!(
        "{base_url}/v1/projects/test-project/locations/global/parameters/test-parameter-name"
    );

    let client = reqwest::Client::new();
    let response = make_request(&client, "DELETE", &mock_url, None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_gcp_get_specific_parameter_version_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Parameter-Manager");

    pact_builder.interaction("get a specific parameter version", "", |mut i| {
        i.given("a parameter with a version exists");
        i.request
            .method("GET")
            .path("/v1/projects/test-project/locations/global/parameters/test-parameter-name/versions/v1234567890".to_string())
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/locations/global/parameters/test-parameter-name/versions/v1234567890",
                "payload": {
                    "data": "ZGIuZXhhbXBsZS5jb20="  // base64 encoded "db.example.com"
                },
                "createTime": "2024-01-01T00:00:00Z",
                "state": "ENABLED"
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!(
        "{base_url}/v1/projects/test-project/locations/global/parameters/test-parameter-name/versions/v1234567890"
    );

    let client = reqwest::Client::new();
    let response = make_request(&client, "GET", &mock_url, None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(
        body["name"]
            .as_str()
            .unwrap()
            .contains("test-parameter-name")
    );
    assert_eq!(body["payload"]["data"], "ZGIuZXhhbXBsZS5jb20=");
}

#[tokio::test]
async fn test_gcp_list_parameters_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Parameter-Manager");

    pact_builder.interaction("list parameters in a location", "", |mut i| {
        i.given("a GCP project exists with parameters");
        i.request
            .method("GET")
            .path("/v1/projects/test-project/locations/global/parameters".to_string())
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "parameters": [
                    {
                        "name": "projects/test-project/locations/global/parameters/test-parameter-name",
                        "format": "PLAIN_TEXT"
                    }
                ],
                "nextPageToken": null
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{base_url}/v1/projects/test-project/locations/global/parameters");

    let client = reqwest::Client::new();
    let response = make_request(&client, "GET", &mock_url, None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["parameters"].is_array());
}

#[tokio::test]
async fn test_gcp_get_parameter_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Parameter-Manager");

    pact_builder.interaction("get a parameter", "", |mut i| {
        i.given("a parameter exists");
        i.request
            .method("GET")
            .path(
                "/v1/projects/test-project/locations/global/parameters/test-parameter-name"
                    .to_string(),
            )
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/locations/global/parameters/test-parameter-name",
                "format": "PLAIN_TEXT"
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!(
        "{base_url}/v1/projects/test-project/locations/global/parameters/test-parameter-name"
    );

    let client = reqwest::Client::new();
    let response = make_request(&client, "GET", &mock_url, None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(
        body["name"],
        "projects/test-project/locations/global/parameters/test-parameter-name"
    );
    assert_eq!(body["format"], "PLAIN_TEXT");
}

#[tokio::test]
async fn test_gcp_update_parameter_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Parameter-Manager");

    pact_builder.interaction("update a parameter", "", |mut i| {
        i.given("a parameter exists");
        i.request
            .method("PATCH")
            .path(
                "/v1/projects/test-project/locations/global/parameters/test-parameter-name"
                    .to_string(),
            )
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json")
            .json_body(json!({
                "parameter": {
                    "format": "JSON",
                    "labels": {
                        "environment": "production"
                    }
                },
                "updateMask": "format,labels"
            }));
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/locations/global/parameters/test-parameter-name",
                "format": "JSON"
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!(
        "{base_url}/v1/projects/test-project/locations/global/parameters/test-parameter-name"
    );

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "PATCH",
        &mock_url,
        Some(json!({
            "parameter": {
                "format": "JSON",
                "labels": {
                    "environment": "production"
                }
            },
            "updateMask": "format,labels"
        })),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(
        body["name"],
        "projects/test-project/locations/global/parameters/test-parameter-name"
    );
}

#[tokio::test]
async fn test_gcp_patch_parameter_version_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Parameter-Manager");

    pact_builder.interaction("update a parameter version state", "", |mut i| {
        i.given("a parameter with a version exists");
        i.request
            .method("PATCH")
            .path("/v1/projects/test-project/locations/global/parameters/test-parameter-name/versions/v1234567890".to_string())
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json")
            .json_body(json!({
                "parameterVersion": {
                    "state": "DISABLED"
                },
                "updateMask": "state"
            }));
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/locations/global/parameters/test-parameter-name/versions/v1234567890",
                "payload": {
                    "data": "ZGIuZXhhbXBsZS5jb20="
                },
                "createTime": "2024-01-01T00:00:00Z"
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!(
        "{base_url}/v1/projects/test-project/locations/global/parameters/test-parameter-name/versions/v1234567890"
    );

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "PATCH",
        &mock_url,
        Some(json!({
            "parameterVersion": {
                "state": "DISABLED"
            },
            "updateMask": "state"
        })),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(
        body["name"]
            .as_str()
            .unwrap()
            .contains("test-parameter-name")
    );
}

#[tokio::test]
async fn test_gcp_delete_parameter_version_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Parameter-Manager");

    pact_builder.interaction("delete a parameter version", "", |mut i| {
        i.given("a parameter with a version exists");
        i.request
            .method("DELETE")
            .path("/v1/projects/test-project/locations/global/parameters/test-parameter-name/versions/v1234567890".to_string())
            .header("authorization", "Bearer test-token");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({}));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!(
        "{base_url}/v1/projects/test-project/locations/global/parameters/test-parameter-name/versions/v1234567890"
    );

    let client = reqwest::Client::new();
    let response = make_request(&client, "DELETE", &mock_url, None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_gcp_render_parameter_version_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Parameter-Manager");

    pact_builder.interaction("render a parameter version", "", |mut i| {
        i.given("a parameter with a version exists");
        i.request
            .method("GET")
            .path("/v1/projects/test-project/locations/global/parameters/test-parameter-name/versions/v1234567890:render".to_string())
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "renderedValue": "db.example.com"
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!(
        "{base_url}/v1/projects/test-project/locations/global/parameters/test-parameter-name/versions/v1234567890:render"
    );

    let client = reqwest::Client::new();
    let response = make_request(&client, "GET", &mock_url, None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["renderedValue"], "db.example.com");
}

#[tokio::test]
async fn test_gcp_get_location_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Parameter-Manager");

    pact_builder.interaction("get a location", "", |mut i| {
        i.given("a GCP project exists");
        i.request
            .method("GET")
            .path("/v1/projects/test-project/locations/global".to_string())
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/locations/global",
                "locationId": "global",
                "displayName": "Global"
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{base_url}/v1/projects/test-project/locations/global");

    let client = reqwest::Client::new();
    let response = make_request(&client, "GET", &mock_url, None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["name"], "projects/test-project/locations/global");
    assert_eq!(body["locationId"], "global");
}

#[tokio::test]
async fn test_gcp_list_locations_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Parameter-Manager");

    pact_builder.interaction("list locations", "", |mut i| {
        i.given("a GCP project exists");
        i.request
            .method("GET")
            .path("/v1/projects/test-project/locations".to_string())
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json");
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "locations": [
                    {
                        "name": "projects/test-project/locations/global",
                        "locationId": "global",
                        "displayName": "Global"
                    },
                    {
                        "name": "projects/test-project/locations/us-central1",
                        "locationId": "us-central1",
                        "displayName": "Iowa (Regional)"
                    }
                ],
                "nextPageToken": null
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    let mock_url = format!("{base_url}/v1/projects/test-project/locations");

    let client = reqwest::Client::new();
    let response = make_request(&client, "GET", &mock_url, None)
        .await
        .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert!(body["locations"].is_array());
    assert!(body["locations"].as_array().unwrap().len() >= 1);
}
