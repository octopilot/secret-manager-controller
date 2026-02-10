# Pact Mocks Manager

The `pact-mocks-manager` (also called `manager`) is a sidecar container that manages the Pact infrastructure lifecycle. It coordinates Pact contract publication and ensures mock servers start only after contracts are available.

## Overview

The `pact-mocks-manager` runs alongside the Pact broker and is responsible for:

- **Broker Readiness**: Waits for Pact broker to be ready before publishing contracts
- **Contract Publication**: Publishes Pact contracts from ConfigMap to the broker
- **ConfigMap Watching**: Monitors ConfigMap changes and re-publishes contracts
- **Provider Tracking**: Tracks which providers have successfully published contracts
- **Health Monitoring**: Provides HTTP health endpoints for Kubernetes probes
- **Mock Server Coordination**: Ensures mock servers wait for contracts before starting

## Architecture

### Deployment Structure

The `pact-mocks-manager` runs as a sidecar container in the Pact broker pod:

```yaml
spec:
  containers:
    - name: pact-broker
      image: pactfoundation/pact-broker:latest
      # ... Broker configuration ...
    
    - name: manager
      image: manager
      # ... Manager configuration ...
```

### Container Responsibilities

1. **Pact Broker Container**: Runs the Pact broker server (stores and serves contracts)
2. **Manager Container**: Publishes contracts and coordinates mock server startup

## How It Works

### Startup Sequence

1. **Pact Broker Container**: Starts and initializes database
2. **Manager Container**:
   - Waits for broker to be ready (health check on port 9292)
   - Processes ConfigMap to extract Pact contracts
   - Publishes contracts to broker
   - Tracks published providers
   - Watches ConfigMap for changes

### Contract Publication Flow

```
ConfigMap (Pact JSON files)
    ↓
Manager processes files
    ↓
Publishes to Pact Broker
    ↓
Tracks provider status
    ↓
Mock servers query manager
    ↓
Mock servers start when contracts available
```

### Broker Readiness Check

The manager waits for the broker to be ready before publishing:

```rust
async fn wait_for_broker(config: &ManagerConfig) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("http://{}:{}{}", 
        config.broker_host, 
        config.broker_port, 
        config.broker_health_path
    );
    
    // Poll until broker responds
    loop {
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            _ => sleep(Duration::from_secs(2)).await,
        }
    }
}
```

### Contract Publishing

Contracts are published to the broker using the Pact broker API:

```rust
async fn publish_contract(
    broker_url: &str,
    username: &str,
    password: &str,
    consumer: &str,
    provider: &str,
    contract: &Value,
) -> Result<()> {
    let url = format!("{}/pacts/provider/{}/consumer/{}", 
        broker_url, provider, consumer
    );
    
    let client = reqwest::Client::new();
    let response = client
        .put(&url)
        .basic_auth(username, Some(password))
        .json(contract)
        .send()
        .await?;
    
    // Handle response...
}
```

### Provider Tracking

The manager tracks which providers have successfully published contracts:

```rust
let published_providers = Arc::new(tokio::sync::RwLock::new(HashSet::new()));

// After successful publication
published_providers.write().await.insert(provider.to_string());
```

Mock servers query the manager to check if their provider's contract is published:

```rust
// Mock server checks manager
GET /providers/{provider}/ready

// Manager responds
{
    "ready": true,
    "provider": "GCP-Secret-Manager",
    "published_at": "2024-01-01T00:00:00Z"
}
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `BROKER_URL` | `http://pact-broker:9292` | Pact broker URL |
| `BROKER_USERNAME` | `pact` | Broker authentication username |
| `BROKER_PASSWORD` | `pact` | Broker authentication password |
| `NAMESPACE` | `secret-manager-controller-pact-broker` | Kubernetes namespace |
| `CONFIGMAP_NAME` | `pact-contracts` | ConfigMap name containing contracts |
| `CONFIGMAP_PATH` | `/pact-contracts` | Path where ConfigMap is mounted |
| `GIT_BRANCH` | `main` | Git branch for contract versioning |
| `GIT_COMMIT` | `unknown` | Git commit SHA for contract versioning |
| `HEALTH_PORT` | `8081` | HTTP health server port |

### Health Endpoints

The manager provides HTTP health endpoints:

- **`GET /liveness`**: Returns 200 if manager is running
- **`GET /readiness`**: Returns 200 if broker is ready AND contracts are published
- **`GET /providers/{provider}/ready`**: Returns 200 if specific provider's contract is published

### Probe Configuration

```yaml
livenessProbe:
  httpGet:
    path: /liveness
    port: 8081
  initialDelaySeconds: 10
  periodSeconds: 10

readinessProbe:
  httpGet:
    path: /readiness
    port: 8081
  initialDelaySeconds: 30
  periodSeconds: 5
```

## Contract Files

### Contract Structure

Contracts are stored in the ConfigMap as JSON files:

```json
{
  "consumer": {
    "name": "Secret-Manager-Controller"
  },
  "provider": {
    "name": "GCP-Secret-Manager"
  },
  "interactions": [
    {
      "description": "Create secret",
      "request": {
        "method": "POST",
        "path": "/v1/projects/test-project/secrets"
      },
      "response": {
        "status": 200
      }
    }
  ]
}
```

### Contract Naming

Contracts are named by provider:

- `gcp-secret-manager.json`: GCP Secret Manager contracts
- `aws-secrets-manager.json`: AWS Secrets Manager contracts
- `azure-key-vault.json`: Azure Key Vault contracts

### Contract Versioning

Contracts are versioned using Git information:

- **Consumer Version**: `{git-branch}-{git-commit}` (e.g., `main-abc123`)
- **Provider Version**: Latest published version
- **Pact Version**: SHA of contract content

## Mock Server Coordination

### Startup Sequence

Mock servers wait for the manager to indicate their provider's contract is ready:

```rust
// In mock server startup
async fn wait_for_manager_ready(
    manager_url: &str,
    provider: &str,
    timeout_secs: u64,
) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/providers/{}/ready", manager_url, provider);
    
    let start = Instant::now();
    while start.elapsed().as_secs() < timeout_secs {
        match client.get(&url).send().await {
            Ok(resp) if resp.status() == 200 => {
                let status: ProviderStatus = resp.json().await?;
                if status.ready {
                    return Ok(());
                }
            }
            _ => sleep(Duration::from_secs(2)).await,
        }
    }
    
    Err(anyhow::anyhow!("Timeout waiting for provider contract"))
}
```

### Fallback Behavior

If the manager is unavailable or contracts aren't published, mock servers start with default mock responses:

```rust
if let Err(e) = wait_for_manager_ready(&manager_url, &provider, 90).await {
    warn!("Failed to wait for manager: {}", e);
    warn!("Starting with default mock responses");
}
```

## Troubleshooting

### Contracts Not Publishing

**Problem**: Contracts don't appear in broker after ConfigMap update.

**Check**:
1. Verify ConfigMap exists: `kubectl get configmap pact-contracts -n secret-manager-controller-pact-broker`
2. Check manager logs: `kubectl logs pact-broker-<pod-id> -n secret-manager-controller-pact-broker -c manager`
3. Verify broker is ready: `kubectl logs pact-broker-<pod-id> -n secret-manager-controller-pact-broker -c pact-broker`

**Solution**: Ensure broker is healthy and manager can authenticate.

### Mock Servers Not Starting

**Problem**: Mock servers fail to start, waiting for contracts.

**Check**:
1. Verify provider contract is published: `kubectl exec pact-broker-<pod-id> -n secret-manager-controller-pact-broker -c manager -- curl http://localhost:8081/providers/GCP-Secret-Manager/ready`
2. Check manager readiness: `kubectl get pods -n secret-manager-controller-pact-broker -l app=pact-broker`
3. View mock server logs: `kubectl logs gcp-mock-server-<pod-id> -n secret-manager-controller-pact-broker`

**Solution**: Ensure manager published contracts successfully and mock servers can reach manager.

### Broker Connection Errors

**Problem**: Manager can't connect to broker.

**Check**:
1. Verify broker is running: `kubectl get pods -n secret-manager-controller-pact-broker`
2. Check broker logs: `kubectl logs pact-broker-<pod-id> -n secret-manager-controller-pact-broker -c pact-broker`
3. Test broker health: `kubectl exec pact-broker-<pod-id> -n secret-manager-controller-pact-broker -c manager -- curl http://localhost:9292`

**Solution**: Ensure broker container started before manager attempts connection.

## Development

### Building the Manager

```bash
# Build manager binary
cargo build --release --bin manager -p pact-mock-server

# Build Docker image
docker build -f dockerfiles/Dockerfile.manager -t manager:dev .
```

### Testing Locally

```bash
# Run manager locally (requires Pact broker)
BROKER_URL="http://localhost:9292" \
BROKER_USERNAME="pact" \
BROKER_PASSWORD="pact" \
NAMESPACE="default" \
CONFIGMAP_NAME="pact-contracts" \
cargo run --bin manager -p pact-mock-server
```

### Adding New Contracts

1. Generate contract from Pact test: `cargo test --test pact_gcp_secret_manager`
2. Contract is automatically published to broker (via `pact_tests.py`)
3. ConfigMap is updated (via Tilt)
4. Manager detects change and re-publishes

### Contract Best Practices

1. **Version Contracts**: Use semantic versioning for contract changes
2. **Test Compatibility**: Verify contracts work with existing mock servers
3. **Document Changes**: Add comments for breaking contract changes
4. **Validate Format**: Ensure contracts match Pact specification

## Related Documentation

- [Postgres Manager](./postgres-manager.md) - Database migration management
- [Pact Testing Overview](../testing/pact-testing/overview.md) - Pact testing introduction
- [Pact Testing Architecture](../testing/pact-testing/architecture.md) - Complete Pact infrastructure
- [Writing Pact Tests](../testing/pact-testing/writing-tests.md) - How to write Pact contracts

