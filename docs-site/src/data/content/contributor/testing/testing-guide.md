# Testing Guide

Comprehensive guide to testing the Secret Manager Controller, covering unit tests, integration tests, Pact contract tests, and end-to-end testing.

## Overview

The Secret Manager Controller uses a multi-layered testing strategy:

1. **Unit Tests**: Fast, isolated tests for individual components
2. **Pact Contract Tests**: Consumer-driven contract tests for provider APIs
3. **Integration Tests**: Tests with mock servers in Kind cluster
4. **End-to-End Tests**: Full controller reconciliation with real GitOps components

## Test Organization

### Test Structure

```
tests/
├── unit/                    # Unit tests
│   ├── crd_validation.rs
│   ├── error_handling_tests.rs
│   ├── provider_config_deserialize.rs
│   ├── reconciler_tests.rs
│   ├── utils_tests.rs
│   └── validation_tests.rs
├── integration/             # Integration tests
│   ├── controller_reconciliation/  # Full reconciliation tests
│   ├── controller_mock_servers/    # Mock server integration
│   └── controller_edge_cases/      # Edge case testing
├── pact_*.rs               # Pact contract tests
└── common/                 # Shared test utilities
```

### Test Types

**Unit Tests** (`tests/unit/`):
- Fast execution (no external dependencies)
- Test individual functions and modules
- Mock external dependencies
- Run with `cargo test --lib`

**Integration Tests** (`tests/integration/`):
- Require Kubernetes cluster (Kind)
- Test controller behavior with mock servers
- Verify reconciliation logic
- Run with `cargo test --test integration_*`

**Pact Contract Tests** (`tests/pact_*.rs`):
- Require Pact broker and mock servers
- Test provider API contracts
- Verify request/response formats
- Run with `cargo test --test pact_*`

## Running Tests

### Unit Tests

Run all unit tests:

```bash
# Run all unit tests
cargo test --lib

# Run with output
cargo test --lib -- --nocapture

# Run specific test
cargo test --lib test_name

# Run tests in specific module
cargo test --lib -- module_name
```

### Integration Tests

Integration tests require a Kind cluster and mock servers:

```bash
# Set up Kind cluster
python3 scripts/setup_kind.py

# Run all integration tests
cargo test --test integration_*

# Run specific integration test
cargo test --test integration_controller_reconciliation

# Run with verbose output
cargo test --test integration_* -- --nocapture
```

### Pact Contract Tests

Pact tests require the Pact infrastructure to be running:

```bash
# Start Pact infrastructure (via Tilt or manually)
tilt up

# Wait for Pact infrastructure to be ready
kubectl wait --for=condition=ready pod -l app=pact-infrastructure -n secret-manager-controller-pact-broker --timeout=5m

# Run Pact tests
python3 scripts/pact_tests.py

# Or run specific Pact test file
cargo test --test pact_gcp_secret_manager
cargo test --test pact_aws_secrets_manager
cargo test --test pact_azure_key_vault
```

### Using Just Commands

The project includes `justfile` commands for common test operations:

```bash
# Run all tests
just test

# Run unit tests only
just test-unit

# Run unit tests with verbose output
just test-unit-verbose

# Run Pact contract tests
just test-pact

# Run specific provider Pact tests
just test-pact-gcp
just test-pact-aws
just test-pact-azure

# Run tests with coverage
just test-coverage
```

## Test Coverage

### Unit Test Coverage

Unit tests cover:
- CRD validation and deserialization
- Error handling and error types
- Provider configuration parsing
- Reconciliation logic (mocked)
- Utility functions
- Path building and validation

### Integration Test Coverage

Integration tests cover:
- Full reconciliation flow
- GitOps integration (FluxCD, ArgoCD)
- SOPS decryption
- Kustomize builds
- Provider operations (GCP, AWS, Azure)
- Error handling and retries
- Status updates and conditions

### Pact Test Coverage

Pact tests cover:
- **GCP Secret Manager**: 12 tests
- **GCP Parameter Manager**: Tests
- **AWS Secrets Manager**: 13 tests
- **AWS Parameter Store**: 6 tests
- **Azure Key Vault**: 14 tests
- **Azure App Configuration**: 6 tests

**Total**: 51+ Pact contract tests

## Test Fixtures

### TestFixture Pattern

Integration tests use a `TestFixture` pattern for setup and teardown:

```rust
let fixture = TestFixture::setup("test-name").await?;
// ... test code ...
fixture.teardown().await?;
```

The fixture:
- Sets up test environment
- Initializes Pact mode configuration
- Cleans up after test completion
- Ensures test isolation

### Common Test Utilities

Shared test utilities in `tests/common/`:

- **Cluster Setup**: Kind cluster management
- **Resource Creation**: Helper functions for creating test resources
- **Mock Server Setup**: Starting and stopping mock servers
- **Assertions**: Custom assertion helpers

## Pact Contract Testing

### Overview

Pact contract tests verify that the controller correctly interacts with cloud provider APIs. Contracts define:
- Request formats (headers, body, query parameters)
- Response formats (status codes, body structure)
- Error responses
- Authentication requirements

### Running Pact Tests

1. **Start Pact Infrastructure**:
   ```bash
   tilt up
   # Or manually deploy pact-infrastructure
   ```

2. **Wait for Infrastructure**:
   ```bash
   kubectl wait --for=condition=ready pod -l app=pact-infrastructure -n secret-manager-controller-pact-broker --timeout=5m
   ```

3. **Run Tests**:
   ```bash
   python3 scripts/pact_tests.py
   ```

The script:
- Sets up port forwarding to mock servers
- Waits for contracts to be published
- Runs all Pact test files sequentially
- Cleans up port forwarding

### Pact Test Structure

Pact tests follow this pattern:

```rust
#[tokio::test]
async fn test_provider_operation() {
    // Set up Pact mode
    setup_pact_mode("gcp", "http://localhost:1235").await;
    
    // Create consumer and provider
    let mut pact_builder = PactBuilder::new("Secret-Manager-Controller", "GCP-Secret-Manager");
    
    // Define interaction
    pact_builder
        .given("secret exists")
        .upon_receiving("a request to get secret")
        .with_request(Request::default())
        .will_respond_with(Response::default());
    
    // Get mock server
    let mock_server = pact_builder.start_mock_server(None).await;
    
    // Run test
    // ... test code ...
}
```

See [Pact Testing Overview](./pact-testing/overview.md) for details.

## Integration Testing

### Prerequisites

1. **Kind Cluster**: Local Kubernetes cluster
   ```bash
   python3 scripts/setup_kind.py
   ```

2. **Mock Servers**: Built and deployed
   ```bash
   # Build mock servers
   cargo build --release --bins
   
   # Deploy to cluster (via Tilt or manually)
   ```

3. **GitOps Components**: FluxCD and ArgoCD CRDs installed
   ```bash
   python3 scripts/tilt/install_fluxcd.py
   python3 scripts/tilt/install_argocd.py
   ```

### Running Integration Tests

```bash
# Run all integration tests
cargo test --test integration_*

# Run specific test suite
cargo test --test integration_controller_reconciliation

# Run with verbose output
cargo test --test integration_* -- --nocapture --test-threads=1
```

**Note**: Integration tests should run sequentially (`--test-threads=1`) to avoid:
- Port conflicts
- Environment variable conflicts
- Resource contention

### Test Isolation

Integration tests use:
- **TestFixture**: Automatic setup and teardown
- **Global Mutex**: Sequential execution for shared resources
- **Separate Namespaces**: Isolated test environments
- **Cleanup**: Automatic resource cleanup

## Code Coverage

### Generating Coverage Reports

```bash
# Install llvm-cov (if not already installed)
cargo install cargo-llvm-cov

# Run tests with coverage
cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info --tests

# View coverage report
# Open lcov.info in coverage tool (e.g., VS Code Coverage extension)
```

### Coverage Goals

- **Minimum**: 65% code coverage
- **Target**: 80% code coverage
- **Critical Paths**: 100% coverage (reconciliation, error handling)

## Test Best Practices

### Writing Unit Tests

1. **Test One Thing**: Each test should verify one behavior
2. **Use Descriptive Names**: Test names should describe what they test
3. **Arrange-Act-Assert**: Structure tests clearly
4. **Mock Dependencies**: Use mocks for external dependencies
5. **Test Edge Cases**: Include boundary conditions and error cases

### Writing Integration Tests

1. **Use TestFixture**: Always use TestFixture for setup/teardown
2. **Test Isolation**: Ensure tests don't interfere with each other
3. **Sequential Execution**: Run with `--test-threads=1`
4. **Clean Up**: Always clean up test resources
5. **Wait for Readiness**: Wait for resources to be ready before testing

### Writing Pact Tests

1. **Define Contracts First**: Write contracts before implementation
2. **Test Real Scenarios**: Use realistic request/response examples
3. **Verify All Operations**: Test all provider operations
4. **Test Error Cases**: Include error response contracts
5. **Keep Contracts Updated**: Update contracts when APIs change

## Debugging Tests

### Unit Tests

```bash
# Run with debug output
RUST_LOG=debug cargo test --lib test_name -- --nocapture

# Run with backtrace
RUST_BACKTRACE=1 cargo test --lib test_name
```

### Integration Tests

```bash
# Enable debug logging
RUST_LOG=debug cargo test --test integration_* -- --nocapture

# View controller logs
kubectl logs -n octopilot-system -l app=secret-manager-controller -f

# View mock server logs
kubectl logs -n secret-manager-controller-pact-broker -l app=pact-infrastructure -f
```

### Pact Tests

```bash
# View Pact broker logs
kubectl logs -n secret-manager-controller-pact-broker -l app=pact-infrastructure -c pact-broker -f

# View mock server logs
kubectl logs -n secret-manager-controller-pact-broker -l app=pact-infrastructure -c gcp-mock-server -f

# Check contract publishing
curl http://localhost:9292/pacts/provider/GCP-Secret-Manager/consumer/Secret-Manager-Controller/latest
```

## CI/CD Testing

### GitHub Actions

The CI pipeline runs:

1. **Unit Tests**: Fast tests without cluster
2. **Linting**: Code quality checks
3. **Integration Tests**: Full Kind cluster setup and tests
4. **Pact Tests**: Contract verification

See `.github/workflows/ci.yml` for details.

### Local CI Simulation

Simulate CI locally:

```bash
# Run unit tests
cargo test --lib

# Run linting
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check

# Set up Kind cluster
CI=1 python3 scripts/setup_kind.py

# Run integration tests
cargo test --test integration_* -- --test-threads=1
```

## Troubleshooting

### Tests Fail Intermittently

**Problem**: Tests pass individually but fail when run together.

**Solution**:
- Run tests sequentially: `--test-threads=1`
- Ensure proper cleanup in test fixtures
- Check for shared state or global variables

### Mock Server Connection Errors

**Problem**: Tests can't connect to mock servers.

**Solution**:
- Verify mock servers are running: `kubectl get pods -n secret-manager-controller-pact-broker`
- Check port forwarding: `kubectl port-forward -n secret-manager-controller-pact-broker svc/gcp-mock-server 1235:1235`
- Verify PACT_MODE is set correctly

### Pact Contracts Not Found

**Problem**: Pact tests fail with "contract not found".

**Solution**:
- Wait for contracts to be published: Check manager logs
- Verify ConfigMap exists: `kubectl get configmap pact-contracts -n secret-manager-controller-pact-broker`
- Check broker is ready: `curl http://localhost:9292/diagnostic/status/heartbeat`

### Kind Cluster Issues

**Problem**: Integration tests fail with cluster errors.

**Solution**:
- Verify cluster is running: `kind get clusters`
- Check cluster connectivity: `kubectl cluster-info`
- Recreate cluster if needed: `kind delete cluster --name secret-manager-controller && python3 scripts/setup_kind.py`

## Next Steps

- [Pact Testing Overview](./pact-testing/overview.md) - Pact contract testing guide
- [Pact Testing Architecture](./pact-testing/architecture.md) - Pact infrastructure architecture
- [Integration Testing](./integration-testing.md) - Detailed integration testing guide
- [Kind Cluster Setup](../development/kind-cluster-setup.md) - Setting up test cluster

