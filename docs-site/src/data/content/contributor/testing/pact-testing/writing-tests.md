# Writing Pact Tests

Complete guide to writing Pact contract tests for the Secret Manager Controller.

## Overview

Pact tests define the contract between the Secret Manager Controller (consumer) and cloud provider APIs (providers). They specify:
- What requests the controller will make
- What responses it expects
- Preconditions (provider states)

## Test Structure

### Basic Test Template

```rust
#[tokio::test]
async fn test_provider_operation_contract() {
    // 1. Initialize TLS (required for Pact)
    init();
    
    // 2. Create PactBuilder
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Provider-Name");
    
    // 3. Define interaction
    pact_builder.interaction("description of the interaction", "", |mut i| {
        // Provider state (precondition)
        i.given("precondition description");
        
        // Request specification
        i.request
            .method("POST")
            .path("/api/endpoint".to_string())
            .header("authorization", "Bearer test-token")
            .json_body(json!({
                "field": "value"
            }));
        
        // Response specification
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "result": "success"
            }));
        
        i
    });
    
    // 4. Start mock server
    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }
    
    // 5. Make request to mock server
    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &format!("{base_url}/api/endpoint"),
        Some(json!({ "field": "value" })),
    )
    .await
    .expect("Failed to make request");
    
    // 6. Verify response
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["result"], "success");
}
```

## Initialization

### TLS Initialization

All Pact tests must initialize TLS before running:

```rust
use common::init_rustls;
use std::sync::Once;

static RUSTLS_INIT: Once = Once::new();

fn init() {
    RUSTLS_INIT.call_once(|| {
        init_rustls();
    });
}

#[tokio::test]
async fn test_example() {
    init();  // Call this at the start of each test
    // ... rest of test
}
```

**Why**: Pact uses native libraries that require TLS initialization. The `Once` ensures it's only initialized once across all tests.

## Creating PactBuilder

### Basic Creation

```rust
let mut pact_builder = PactBuilder::new("Consumer-Name", "Provider-Name");
```

**Consumer Name**: Always use `"Secret-Manager-Controller"`  
**Provider Names**: Use exact provider names:
- `"GCP-Secret-Manager"`
- `"GCP-Parameter-Manager"`
- `"AWS-Secrets-Manager"`
- `"AWS-Parameter-Store"`
- `"Azure-Key-Vault"`
- `"Azure-App-Configuration"`

### Example

```rust
let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");
```

## Defining Interactions

### Interaction Structure

```rust
pact_builder.interaction("description", "", |mut i| {
    // Provider state
    i.given("precondition");
    
    // Request
    i.request
        .method("METHOD")
        .path("/path".to_string())
        // ... headers, body, etc.
    
    // Response
    i.response
        .status(200)
        // ... headers, body, etc.
    
    i  // Return the interaction
});
```

### Interaction Description

The description should clearly state what the interaction does:

```rust
// Good
pact_builder.interaction("create a new secret in GCP Secret Manager", "", |mut i| {

// Good
pact_builder.interaction("get the latest version of a secret", "", |mut i| {

// Good
pact_builder.interaction("update an existing secret value", "", |mut i| {
```

## Provider States

Provider states (`.given()`) specify preconditions for the interaction:

```rust
i.given("a GCP project exists");
i.given("a secret exists in GCP Secret Manager");
i.given("the secret does not exist");
i.given("AWS credentials are configured");
i.given("Azure Key Vault exists and credentials are configured");
```

**Purpose**: Documents what state the provider should be in before the request.

**Note**: Mock servers don't enforce provider states - they're documentation. Real providers would use these for setup.

## Request Specification

### HTTP Method

```rust
i.request.method("GET");
i.request.method("POST");
i.request.method("PUT");
i.request.method("PATCH");
i.request.method("DELETE");
```

### Path

```rust
// Exact path
i.request.path("/v1/projects/test-project/secrets".to_string());

// Path with parameters
i.request.path("/v1/projects/test-project/secrets/test-secret-name/versions/latest".to_string());
```

**Note**: Paths must be exact matches. Use `.to_string()` to convert string literals.

### Headers

```rust
// Single header
i.request.header("authorization", "Bearer test-token");

// Multiple headers
i.request
    .header("authorization", "Bearer test-token")
    .header("content-type", "application/json");

// AWS-specific headers
i.request
    .header("content-type", "application/x-amz-json-1.1")
    .header("x-amz-target", "secretsmanager.CreateSecret");
```

### Query Parameters

```rust
// Single query parameter
i.request.query_param("api-version", "7.4");

// Multiple query parameters
i.request
    .query_param("api-version", "7.4")
    .query_param("filter", "enabled");
```

### Request Body

**JSON Body** (for `application/json`):

```rust
i.request.json_body(json!({
    "secretId": "test-secret-name",
    "replication": {
        "automatic": {}
    }
}));
```

**String Body** (for `application/x-amz-json-1.1` or custom formats):

```rust
i.request.body(json!({
    "Name": "test-secret-name",
    "SecretString": "test-secret-value"
}).to_string());
```

**Note**: AWS Secrets Manager uses `application/x-amz-json-1.1` with string body, not JSON body.

## Response Specification

### Status Code

```rust
i.response.status(200);  // Success
i.response.status(201);  // Created
i.response.status(404);  // Not Found
i.response.status(400);  // Bad Request
i.response.status(500);  // Internal Server Error
```

### Response Headers

```rust
i.response.header("content-type", "application/json");
i.response.header("content-type", "application/x-amz-json-1.1");
```

### Response Body

**JSON Body**:

```rust
i.response.json_body(json!({
    "name": "projects/test-project/secrets/test-secret-name",
    "replication": {
        "automatic": {}
    },
    "createTime": "2024-01-01T00:00:00Z"
}));
```

**Error Response**:

```rust
i.response.json_body(json!({
    "error": {
        "code": 404,
        "message": "Secret [non-existent-secret] not found",
        "status": "NOT_FOUND"
    }
}));
```

## Starting Mock Server

### Basic Usage

```rust
let mock_server = pact_builder.start_mock_server(None, None);
```

### Getting Base URL

```rust
let mock_server = pact_builder.start_mock_server(None, None);
let mut base_url = mock_server.url().to_string();
if base_url.ends_with('/') {
    base_url.pop();  // Remove trailing slash
}
```

**Why**: Mock server URL may have a trailing slash, but paths always start with `/`, so we need to handle this.

## Making Requests

### Helper Function Pattern

Create a helper function for making requests:

```rust
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
```

### Using Helper Function

```rust
let client = reqwest::Client::new();
let response = make_request(
    &client,
    "POST",
    &format!("{base_url}/v1/projects/test-project/secrets"),
    Some(json!({
        "secretId": "test-secret-name"
    })),
)
.await
.expect("Failed to make request");
```

### AWS-Specific Requests

AWS Secrets Manager uses `application/x-amz-json-1.1`:

```rust
async fn make_request(
    client: &reqwest::Client,
    method: &str,
    url: &str,
    body: Option<serde_json::Value>,
    x_amz_target: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut request = match method {
        "POST" => client.post(url),
        _ => panic!("Unsupported HTTP method: {method}"),
    };

    request = request
        .header("authorization", "AWS4-HMAC-SHA256 Credential=test/20240101/us-east-1/secretsmanager/aws4_request")
        .header("x-amz-target", x_amz_target);

    if let Some(body) = body {
        request = request
            .header("content-type", "application/x-amz-json-1.1")
            .body(serde_json::to_string(&body).unwrap());
    }

    request.send().await
}
```

## Verifying Responses

### Status Code

```rust
assert_eq!(response.status(), 200);
```

### Response Body

```rust
let body: serde_json::Value = response.json().await.expect("Failed to parse response");
assert_eq!(body["name"], "projects/test-project/secrets/test-secret-name");
```

### Partial Verification

```rust
// Check specific fields
assert_eq!(body["name"], "expected-value");
assert_eq!(body["status"], "ENABLED");

// Check nested fields
assert_eq!(body["attributes"]["enabled"], true);
```

## Common Patterns

### GCP Secret Manager

```rust
#[tokio::test]
async fn test_gcp_create_secret_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");

    pact_builder.interaction("create a new secret in GCP Secret Manager", "", |mut i| {
        i.given("a GCP project exists");
        i.request
            .method("POST")
            .path("/v1/projects/test-project/secrets".to_string())
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json")
            .json_body(json!({
                "secretId": "test-secret-name",
                "replication": {
                    "automatic": {}
                }
            }));
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "name": "projects/test-project/secrets/test-secret-name",
                "replication": {
                    "automatic": {}
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

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &format!("{base_url}/v1/projects/test-project/secrets"),
        Some(json!({
            "secretId": "test-secret-name",
            "replication": {
                "automatic": {}
            }
        })),
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["name"], "projects/test-project/secrets/test-secret-name");
}
```

### AWS Secrets Manager

```rust
#[tokio::test]
async fn test_aws_create_secret_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "AWS-Secrets-Manager");

    pact_builder.interaction("create a new secret in AWS Secrets Manager", "", |mut i| {
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
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "POST",
        &format!("{base_url}/"),
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
```

### Azure Key Vault

```rust
#[tokio::test]
async fn test_azure_set_secret_contract() {
    init();
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "Azure-Key-Vault");

    pact_builder.interaction("set a secret in Azure Key Vault", "", |mut i| {
        i.given("Azure Key Vault exists and credentials are configured");
        i.request
            .method("PUT")
            .path("/secrets/test-secret-name")
            .header("authorization", "Bearer test-token")
            .header("content-type", "application/json")
            .query_param("api-version", "7.4")
            .json_body(json!({
                "value": "test-secret-value"
            }));
        i.response
            .status(200)
            .header("content-type", "application/json")
            .json_body(json!({
                "value": "test-secret-value",
                "id": "https://test-vault.vault.azure.net/secrets/test-secret-name/abc123",
                "attributes": {
                    "enabled": true,
                    "created": 1704067200,
                    "updated": 1704067200,
                    "recoveryLevel": "Recoverable+Purgeable"
                }
            }));
        i
    });

    let mock_server = pact_builder.start_mock_server(None, None);
    let mut base_url = mock_server.url().to_string();
    if base_url.ends_with('/') {
        base_url.pop();
    }

    let client = reqwest::Client::new();
    let response = make_request(
        &client,
        "PUT",
        &format!("{base_url}/secrets/test-secret-name?api-version=7.4"),
        Some(json!({
            "value": "test-secret-value"
        })),
        None,
    )
    .await
    .expect("Failed to make request");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("Failed to parse response");
    assert_eq!(body["value"], "test-secret-value");
}
```

## Error Handling Tests

### Not Found (404)

```rust
pact_builder.interaction("get a secret that does not exist", "", |mut i| {
    i.given("the secret does not exist");
    i.request
        .method("GET")
        .path("/v1/projects/test-project/secrets/non-existent-secret/versions/latest".to_string())
        .header("authorization", "Bearer test-token");
    i.response
        .status(404)
        .header("content-type", "application/json")
        .json_body(json!({
            "error": {
                "code": 404,
                "message": "Secret [non-existent-secret] not found",
                "status": "NOT_FOUND"
            }
        }));
    i
});
```

### Bad Request (400)

```rust
pact_builder.interaction("create secret with invalid data", "", |mut i| {
    i.given("a GCP project exists");
    i.request
        .method("POST")
        .path("/v1/projects/test-project/secrets".to_string())
        .header("authorization", "Bearer test-token")
        .json_body(json!({
            "secretId": ""  // Invalid: empty name
        }));
    i.response
        .status(400)
        .header("content-type", "application/json")
        .json_body(json!({
            "error": {
                "code": 400,
                "message": "Secret ID cannot be empty",
                "status": "INVALID_ARGUMENT"
            }
        }));
    i
});
```

## Integration Tests vs Contract Tests

### Contract Tests (`pact_*.rs`)

**Purpose**: Define contracts between consumer and provider

**Characteristics**:
- Use `PactBuilder` to define interactions
- Start mock server
- Make direct HTTP requests to mock server
- Verify contract matches

**Location**: `tests/pact_*.rs`

**Example**: `tests/pact_gcp_secret_manager.rs`

### Integration Tests (`pact_provider_integration_*.rs`)

**Purpose**: Test controller using real provider implementations with Pact mode

**Characteristics**:
- Use `PactBuilder` to define interactions
- Start mock server
- Set `PACT_MODE=true` environment variable
- Use `TestFixture` for setup/teardown
- Create actual `SecretManagerConfig` resources
- Test end-to-end controller behavior

**Location**: `tests/pact_provider_integration_*.rs`

**Example**: `tests/pact_provider_integration_gcp.rs`

## Best Practices

### 1. Use Descriptive Interaction Names

```rust
// Good
pact_builder.interaction("create a new secret in GCP Secret Manager", "", |mut i| {

// Bad
pact_builder.interaction("test1", "", |mut i| {
```

### 2. Include Provider States

```rust
// Good
i.given("a GCP project exists");

// Bad (missing provider state)
// No .given() call
```

### 3. Match Real API Responses

Use actual API response structures from provider documentation:

```rust
// Good - matches GCP API
i.response.json_body(json!({
    "name": "projects/test-project/secrets/test-secret-name",
    "replication": {
        "automatic": {}
    },
    "createTime": "2024-01-01T00:00:00Z"
}));
```

### 4. Test Error Cases

Include tests for error scenarios:

```rust
#[tokio::test]
async fn test_secret_not_found_contract() {
    // Test 404 error
}

#[tokio::test]
async fn test_invalid_request_contract() {
    // Test 400 error
}
```

### 5. Use Helper Functions

Create reusable helper functions for common operations:

```rust
async fn make_request(...) -> Result<reqwest::Response, reqwest::Error> {
    // Common request logic
}
```

### 6. Verify Response Structure

Check that responses match expected structure:

```rust
assert_eq!(response.status(), 200);
let body: serde_json::Value = response.json().await.expect("Failed to parse response");
assert_eq!(body["name"], "expected-value");
assert!(body["createTime"].is_string());
```

### 7. Handle Base URL Correctly

Always handle trailing slashes:

```rust
let mut base_url = mock_server.url().to_string();
if base_url.ends_with('/') {
    base_url.pop();
}
```

## Common Pitfalls

### 1. Forgetting TLS Initialization

```rust
// ❌ Missing init()
#[tokio::test]
async fn test_example() {
    let mut pact_builder = PactBuilder::new(...);
    // ...
}

// ✅ Correct
#[tokio::test]
async fn test_example() {
    init();
    let mut pact_builder = PactBuilder::new(...);
    // ...
}
```

### 2. Wrong Provider Name

```rust
// ❌ Wrong
PactBuilder::new("Secret-Manager-Controller", "GCP");

// ✅ Correct
PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");
```

### 3. Path Not Converted to String

```rust
// ❌ Wrong
i.request.path("/v1/projects/test-project/secrets");

// ✅ Correct
i.request.path("/v1/projects/test-project/secrets".to_string());
```

### 4. AWS Body Format

```rust
// ❌ Wrong (for AWS)
i.request.json_body(json!({...}));

// ✅ Correct (for AWS)
i.request.body(json!({...}).to_string());
i.request.header("content-type", "application/x-amz-json-1.1");
```

### 5. Not Handling Base URL

```rust
// ❌ Wrong
let base_url = mock_server.url().to_string();
let url = format!("{base_url}/path");  // May have double slash

// ✅ Correct
let mut base_url = mock_server.url().to_string();
if base_url.ends_with('/') {
    base_url.pop();
}
let url = format!("{base_url}/path");
```

## Running Tests

### Run All Pact Tests

```bash
cargo test --test pact_*
```

### Run Specific Provider Tests

```bash
cargo test --test pact_gcp_secret_manager
cargo test --test pact_aws_secrets_manager
cargo test --test pact_azure_key_vault
```

### Run with Verbose Output

```bash
cargo test --test pact_* -- --nocapture
```

### Generate Contract Files

Tests automatically generate contract files in `target/pacts/`:

```
target/pacts/
├── secret-manager-controller-gcp-secret-manager.json
├── secret-manager-controller-aws-secrets-manager.json
└── secret-manager-controller-azure-key-vault.json
```

## Next Steps

- [Pact Testing Overview](./overview.md) - Pact concepts and workflow
- [Pact Testing Setup](./setup.md) - Setting up Pact infrastructure
- [Pact Testing Architecture](./architecture.md) - Detailed architecture and diagrams

