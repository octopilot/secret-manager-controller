# Integration Testing

Comprehensive guide to integration testing for the Secret Manager Controller, including end-to-end tests with Kubernetes clusters and mock servers.

## Overview

Integration tests verify that the controller works correctly in a realistic environment by:
- Testing full reconciliation flows
- Using real Kubernetes clusters (Kind)
- Interacting with mock servers (Pact mode)
- Verifying end-to-end behavior

## Types of Integration Tests

### 1. Controller Mock Server Tests

**Location**: `tests/integration_controller_mock_servers.rs`

**Purpose**: Test controller reconciliation with mock servers

**What They Test**:
- Secret creation through controller reconciliation
- Secret updates and versioning
- Error handling
- Status updates

**Structure**:
```
tests/
├── integration_controller_mock_servers.rs
└── integration/
    └── controller_mock_servers/
        ├── mod.rs
        ├── common/
        │   └── mod.rs          # Shared utilities
        ├── gcp.rs              # GCP tests
        ├── aws.rs              # AWS tests
        └── azure.rs            # Azure tests
```

### 2. End-to-End Reconciliation Tests

**Location**: `tests/integration_reconciliation.rs`

**Purpose**: Test full reconciliation flow with Git repositories

**What They Test**:
- GitRepository resource handling
- Secret file processing (application.secrets.env, application.secrets.yaml)
- SOPS decryption
- Kustomize builds
- Multi-provider secret creation
- Status updates

**Structure**:
```
tests/
├── integration_reconciliation.rs
└── integration/
    └── controller_reconciliation/
        ├── mod.rs
        ├── gcp.rs
        ├── aws.rs
        ├── azure.rs
        ├── kustomize.rs
        └── gitops_features.rs
```

### 3. Edge Case Tests

**Location**: `tests/integration/controller_edge_cases/`

**Purpose**: Test edge cases and error scenarios

**What They Test**:
- Version operations (conflicts, ordering)
- Partial failures (some secrets succeed, others fail)
- Network errors
- Provider errors
- Invalid configurations

### 4. Artifact Resolver Tests

**Location**: `tests/integration_artifact_resolver_tests.rs`

**Purpose**: Test artifact resolution from Git repositories

**What They Test**:
- FluxCD source-controller artifact paths
- ArgoCD direct Git cloning
- Artifact caching
- Git credential handling

## Prerequisites

### Required

1. **Kubernetes Cluster**:
   ```bash
   # Set up Kind cluster
   python3 scripts/setup_kind.py
   ```

2. **Mock Server Binaries**:
   ```bash
   # Build mock servers
   cargo build --release --bin gcp-mock-server
   cargo build --release --bin aws-mock-server
   cargo build --release --bin azure-mock-server
   ```

3. **GitOps Components** (for reconciliation tests):
   ```bash
   # Install FluxCD source-controller
   python3 scripts/tilt/install_fluxcd.py
   
   # Install ArgoCD CRDs
   python3 scripts/tilt/install_argocd.py
   ```

### Optional

- **Tilt**: For managing test infrastructure
- **Pact Infrastructure**: For contract-based testing (see [Pact Testing Setup](./pact-testing/setup.md))

## Running Integration Tests

### Run All Integration Tests

```bash
# Run all integration test suites
cargo test --test integration_*

# Run with verbose output
cargo test --test integration_* -- --nocapture

# Run sequentially (recommended to avoid conflicts)
cargo test --test integration_* -- --test-threads=1
```

### Run Specific Test Suites

```bash
# Controller mock server tests
cargo test --test integration_controller_mock_servers

# End-to-end reconciliation tests
cargo test --test integration_reconciliation

# Artifact resolver tests
cargo test --test integration_artifact_resolver_tests
```

### Run Ignored Tests

Some tests are marked `#[ignore]` because they require additional setup:

```bash
# Run ignored tests (requires mock servers and cluster)
cargo test --test integration_controller_mock_servers -- --ignored

# Run all tests including ignored
cargo test --test integration_controller_mock_servers -- --include-ignored
```

### Run Specific Provider Tests

```bash
# GCP tests
cargo test --test integration_controller_mock_servers gcp

# AWS tests
cargo test --test integration_controller_mock_servers aws

# Azure tests
cargo test --test integration_controller_mock_servers azure
```

## Test Structure

### Basic Test Template

```rust
#[tokio::test]
#[ignore] // Requires mock server and Kubernetes cluster
async fn test_provider_controller_operation() {
    // 1. Initialize test environment
    init_test();
    
    // 2. Start mock server
    let mock_server = start_provider_mock_server()
        .await
        .expect("Failed to start mock server");
    let endpoint = mock_server.endpoint().to_string();
    
    // 3. Set up Pact mode
    setup_pact_mode("provider", &endpoint);
    
    // 4. Create test configuration
    let config = create_provider_test_config(
        "test-config",
        "default",
        "provider-specific-params",
        &endpoint,
    );
    
    // 5. Create Kubernetes client
    let client = match create_test_kube_client().await {
        Ok(client) => client,
        Err(e) => {
            eprintln!("⚠️  Skipping test: {}", e);
            return; // Skip if no cluster available
        }
    };
    
    // 6. Create reconciler
    let reconciler = Arc::new(
        Reconciler::new(client)
            .await
            .expect("Failed to create Reconciler"),
    );
    
    // 7. Trigger reconciliation
    let controller_config = create_test_controller_config();
    let result = reconcile(
        Arc::new(config),
        reconciler,
        TriggerSource::ManualCli,
        controller_config,
    )
    .await;
    
    // 8. Verify results
    assert!(result.is_ok(), "Reconciliation should succeed");
    
    // 9. Cleanup
    cleanup_pact_mode("provider");
}
```

## Mock Server Management

### Starting Mock Servers

Mock servers are started as separate processes:

```rust
// Start GCP mock server
let mock_server = start_gcp_mock_server()
    .await
    .expect("Failed to start GCP mock server");

// Start AWS mock server
let mock_server = start_aws_mock_server()
    .await
    .expect("Failed to start AWS mock server");

// Start Azure mock server
let mock_server = start_azure_mock_server()
    .await
    .expect("Failed to start Azure mock server");
```

### Mock Server Lifecycle

1. **Start**: Mock server starts on an available port
2. **Health Check**: Wait for server to be ready
3. **Configure**: Set endpoint in controller configuration
4. **Test**: Run reconciliation
5. **Cleanup**: Mock server stops automatically on drop

### Port Management

Mock servers automatically find available ports:

```rust
// Mock server finds available port automatically
let mock_server = start_gcp_mock_server().await?;
let endpoint = mock_server.endpoint(); // e.g., "http://localhost:12345"
```

**Note**: Tests should run sequentially (`--test-threads=1`) to avoid port conflicts.

## Pact Mode Configuration

### Setting Up Pact Mode

```rust
// Set up Pact mode for a provider
setup_pact_mode("gcp", "http://localhost:12345");
setup_pact_mode("aws", "http://localhost:12346");
setup_pact_mode("azure", "http://localhost:12347");
```

### Environment Variables

Pact mode sets these environment variables:

```rust
env::set_var("PACT_MODE", "true");
env::set_var("GCP_SECRET_MANAGER_ENDPOINT", "http://localhost:12345");
env::set_var("AWS_SECRETS_MANAGER_ENDPOINT", "http://localhost:12346");
env::set_var("AZURE_KEY_VAULT_ENDPOINT", "http://localhost:12347");
```

### Cleaning Up

```rust
// Clean up Pact mode environment variables
cleanup_pact_mode("gcp");
cleanup_pact_mode("aws");
cleanup_pact_mode("azure");
```

## Creating Test Resources

### SecretManagerConfig

```rust
// GCP configuration
let config = create_gcp_test_config(
    "test-config",      // Name
    "default",          // Namespace
    "test-project",     // GCP project
    &endpoint,          // Mock server endpoint
);

// AWS configuration
let config = create_aws_test_config(
    "test-config",
    "default",
    "us-east-1",        // AWS region
    &endpoint,
);

// Azure configuration
let config = create_azure_test_config(
    "test-config",
    "default",
    "test-vault",       // Azure Key Vault name
    &endpoint,
);
```

### Kubernetes Client

```rust
// Create Kubernetes client
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
```

### Reconciler

```rust
// Create reconciler
let reconciler = Arc::new(
    Reconciler::new(client)
        .await
        .expect("Failed to create Reconciler"),
);
```

## Test Scenarios

### Basic Operations

#### Create Secret

```rust
#[tokio::test]
async fn test_provider_create_secret() {
    // Set up test environment
    let mock_server = start_provider_mock_server().await?;
    setup_pact_mode("provider", &mock_server.endpoint());
    
    // Create config with secret data
    let config = create_provider_test_config_with_secrets(
        "test-config",
        "default",
        vec![
            ("secret1", "value1"),
            ("secret2", "value2"),
        ],
    );
    
    // Trigger reconciliation
    let result = reconcile(config, reconciler, TriggerSource::ManualCli, controller_config).await;
    
    // Verify
    assert!(result.is_ok());
    // Verify secrets were created in mock server
}
```

#### Update Secret

```rust
#[tokio::test]
async fn test_provider_update_secret() {
    // Create secret first
    // ...
    
    // Update secret value
    let updated_config = update_secret_value(config, "secret1", "new-value");
    
    // Trigger reconciliation
    let result = reconcile(updated_config, reconciler, TriggerSource::ManualCli, controller_config).await;
    
    // Verify
    assert!(result.is_ok());
    // Verify new version was created
}
```

### Versioning Tests

#### Version Creation

```rust
#[tokio::test]
async fn test_provider_secret_versioning() {
    // Create secret
    // ...
    
    // Update secret multiple times
    for i in 1..=3 {
        let config = update_secret_value(config, "secret1", &format!("value-{i}"));
        reconcile(config, reconciler, TriggerSource::ManualCli, controller_config).await?;
    }
    
    // Verify versions were created
    // GCP: Check version numbers (1, 2, 3)
    // AWS: Check version IDs and staging labels
    // Azure: Check version IDs (UUIDs)
}
```

### Error Handling Tests

#### Provider Errors

```rust
#[tokio::test]
async fn test_provider_error_handling() {
    // Configure mock server to return errors
    mock_server.set_error_response(500, "Internal Server Error");
    
    // Trigger reconciliation
    let result = reconcile(config, reconciler, TriggerSource::ManualCli, controller_config).await;
    
    // Verify error is handled gracefully
    assert!(result.is_err());
    // Verify status is updated with error
}
```

#### Network Errors

```rust
#[tokio::test]
async fn test_network_error_handling() {
    // Stop mock server
    drop(mock_server);
    
    // Trigger reconciliation
    let result = reconcile(config, reconciler, TriggerSource::ManualCli, controller_config).await;
    
    // Verify network error is handled
    assert!(result.is_err());
}
```

### Status Update Tests

```rust
#[tokio::test]
async fn test_status_updates() {
    // Trigger reconciliation
    let result = reconcile(config, reconciler, TriggerSource::ManualCli, controller_config).await;
    
    // Verify status phases
    let status = get_config_status(&client, "test-config", "default").await;
    assert_eq!(status.phase, "Ready");
    assert_eq!(status.secrets_synced, 2);
    assert_eq!(status.secrets_updated, 2);
}
```

## End-to-End Reconciliation Tests

### GitRepository Tests

```rust
#[tokio::test]
async fn test_gitrepository_reconciliation() {
    // 1. Create GitRepository resource
    let git_repo = create_test_gitrepository(
        "test-repo",
        "default",
        "https://github.com/example/repo",
        "main",
    );
    
    // 2. Create SecretManagerConfig referencing GitRepository
    let config = create_config_with_gitrepository(
        "test-config",
        "default",
        "test-repo",
    );
    
    // 3. Create test secret files in Git repository
    create_test_secret_files(
        "application.secrets.env",
        vec![("SECRET1", "value1"), ("SECRET2", "value2")],
    );
    
    // 4. Trigger reconciliation
    let result = reconcile(config, reconciler, TriggerSource::Reconcile, controller_config).await;
    
    // 5. Verify secrets were created
    assert!(result.is_ok());
    // Verify secrets in provider
}
```

### SOPS Decryption Tests

```rust
#[tokio::test]
async fn test_sops_decryption() {
    // 1. Create SOPS-encrypted secret file
    create_sops_encrypted_file(
        "application.secrets.env",
        vec![("SECRET1", "value1")],
        sops_key_id,
    );
    
    // 2. Configure SOPS key in controller
    setup_sops_key(sops_key_id, sops_private_key);
    
    // 3. Trigger reconciliation
    let result = reconcile(config, reconciler, TriggerSource::Reconcile, controller_config).await;
    
    // 4. Verify decryption and secret creation
    assert!(result.is_ok());
}
```

### Kustomize Build Tests

```rust
#[tokio::test]
async fn test_kustomize_build() {
    // 1. Create kustomization.yaml
    create_kustomization_yaml("base", vec!["application.secrets.env"]);
    
    // 2. Create overlay
    create_kustomization_overlay("overlays/prod", "base");
    
    // 3. Configure config to use overlay
    let config = create_config_with_kustomize(
        "test-config",
        "overlays/prod",
    );
    
    // 4. Trigger reconciliation
    let result = reconcile(config, reconciler, TriggerSource::Reconcile, controller_config).await;
    
    // 5. Verify Kustomize build and secret creation
    assert!(result.is_ok());
}
```

## Test Fixtures and Utilities

### Common Utilities

Located in `tests/integration/controller_mock_servers/common/mod.rs`:

- `init_rustls()`: Initialize TLS for tests
- `start_gcp_mock_server()`: Start GCP mock server
- `start_aws_mock_server()`: Start AWS mock server
- `start_azure_mock_server()`: Start Azure mock server
- `setup_pact_mode()`: Configure Pact mode
- `cleanup_pact_mode()`: Clean up Pact mode
- `create_test_kube_client()`: Create Kubernetes client
- `create_gcp_test_config()`: Create GCP test configuration
- `create_aws_test_config()`: Create AWS test configuration
- `create_azure_test_config()`: Create Azure test configuration

### Test Fixture Pattern

```rust
struct TestFixture {
    mock_server: MockServer,
    endpoint: String,
    config: SecretManagerConfig,
    reconciler: Arc<Reconciler>,
}

impl TestFixture {
    async fn setup() -> Self {
        // Initialize mock server
        let mock_server = start_provider_mock_server().await?;
        let endpoint = mock_server.endpoint().to_string();
        
        // Set up Pact mode
        setup_pact_mode("provider", &endpoint);
        
        // Create config
        let config = create_provider_test_config("test-config", "default", &endpoint);
        
        // Create reconciler
        let client = create_test_kube_client().await?;
        let reconciler = Arc::new(Reconciler::new(client).await?);
        
        Self {
            mock_server,
            endpoint,
            config,
            reconciler,
        }
    }
    
    async fn cleanup(self) {
        cleanup_pact_mode("provider");
        // Mock server cleans up on drop
    }
}
```

## Sequential Execution

### Why Sequential?

Integration tests should run sequentially to avoid:
- **Port Conflicts**: Mock servers need unique ports
- **Environment Variable Conflicts**: Pact mode sets global env vars
- **Resource Contention**: Kubernetes resources may conflict
- **Test Isolation**: Ensure clean state between tests

### Running Sequentially

```bash
# Use --test-threads=1
cargo test --test integration_* -- --test-threads=1

# Or use global mutex in test code
static TEST_MUTEX: Mutex<()> = Mutex::new(());

#[tokio::test]
async fn test_example() {
    let _guard = TEST_MUTEX.lock().unwrap();
    // Test code
}
```

## Troubleshooting

### Mock Server Not Starting

**Symptoms**: Test fails with "Failed to start mock server"

**Diagnosis**:
```bash
# Check if port is in use
lsof -i :12345

# Check mock server binary exists
ls -la target/release/gcp-mock-server
```

**Solution**:
```bash
# Kill processes using the port
kill -9 $(lsof -ti:12345)

# Rebuild mock servers
cargo build --release --bin gcp-mock-server
```

### Kubernetes Cluster Not Available

**Symptoms**: Test skips with "Skipping test: no cluster available"

**Diagnosis**:
```bash
# Check kubectl context
kubectl config current-context

# Check cluster connectivity
kubectl cluster-info
```

**Solution**:
```bash
# Set up Kind cluster
python3 scripts/setup_kind.py

# Or set KUBECONFIG
export KUBECONFIG=/path/to/kubeconfig
```

### Environment Variable Conflicts

**Symptoms**: Tests interfere with each other

**Solution**:
```rust
// Use test fixture with cleanup
let fixture = TestFixture::setup().await;
// Run test
fixture.cleanup().await;

// Or use mutex for sequential execution
let _guard = TEST_MUTEX.lock().unwrap();
```

### Reconciliation Failures

**Symptoms**: Reconciliation returns errors

**Diagnosis**:
```rust
// Enable verbose logging
env::set_var("RUST_LOG", "debug");
```

**Common Issues**:
- Mock server not ready
- Invalid configuration
- Missing GitOps components
- Network connectivity issues

## Best Practices

### 1. Use Test Fixtures

Create reusable test fixtures for common setup:

```rust
let fixture = TestFixture::setup().await;
// Use fixture
fixture.cleanup().await;
```

### 2. Clean Up Resources

Always clean up test resources:

```rust
// Clean up Pact mode
cleanup_pact_mode("provider");

// Delete Kubernetes resources
api.delete("test-config", &DeleteParams::default()).await?;
```

### 3. Handle Missing Dependencies Gracefully

```rust
let client = match create_test_kube_client().await {
    Ok(client) => client,
    Err(e) => {
        eprintln!("⚠️  Skipping test: {}", e);
        return; // Skip if dependencies not available
    }
};
```

### 4. Use Descriptive Test Names

```rust
// Good
#[tokio::test]
async fn test_gcp_controller_create_secret_with_versioning() {

// Bad
#[tokio::test]
async fn test1() {
```

### 5. Verify Both Success and Failure Cases

```rust
// Test success case
assert!(result.is_ok());

// Test failure case
let error_result = reconcile(invalid_config, ...).await;
assert!(error_result.is_err());
```

### 6. Test Status Updates

Always verify status is updated correctly:

```rust
let status = get_config_status(&client, "test-config", "default").await;
assert_eq!(status.phase, "Ready");
assert_eq!(status.secrets_synced, expected_count);
```

## CI/CD Integration

### GitHub Actions Example

```yaml
- name: Run Integration Tests
  run: |
    # Set up Kind cluster
    python3 scripts/setup_kind.py
    
    # Build mock servers
    cargo build --release --bin gcp-mock-server
    cargo build --release --bin aws-mock-server
    cargo build --release --bin azure-mock-server
    
    # Run integration tests
    cargo test --test integration_* -- --test-threads=1
```

## Next Steps

- [Testing Guide](./testing-guide.md) - General testing overview
- [Pact Testing Overview](./pact-testing/overview.md) - Pact contract testing
- [Kind Cluster Setup](../development/kind-cluster-setup.md) - Setting up Kind cluster

