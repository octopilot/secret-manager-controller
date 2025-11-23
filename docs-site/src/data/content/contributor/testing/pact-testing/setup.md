# Pact Testing Setup

Complete guide to setting up Pact testing infrastructure for local development and CI/CD.

## Prerequisites

### Required Tools

- **Kubernetes Cluster**: Kind cluster (local) or any Kubernetes cluster (CI/CD)
- **kubectl**: Configured to access your cluster
- **Docker**: For building mock server images
- **Rust**: For building mock server binaries
- **Python 3**: For setup scripts

### Optional Tools

- **Tilt**: For local development (recommended)
- **Pact CLI**: For manual contract publishing (optional, manager handles this)

## Quick Start (Tilt - Recommended)

The easiest way to set up Pact infrastructure is using Tilt:

```bash
# Start Tilt with Pact resources only
tilt up pact

# Or start everything (controller + Pact)
tilt up
```

Tilt automatically:
1. Builds mock server Docker images
2. Deploys Pact infrastructure to Kubernetes
3. Populates ConfigMap with Pact contracts
4. Waits for infrastructure to be ready
5. Runs Pact tests

### Tilt Resources

Pact-related resources in Tilt:

- **`copy-mock-server-binaries`**: Copies mock server binaries to build artifacts
- **`pact-mock-server`**: Builds Docker image for mock servers
- **`mock-webhook`**: Builds Docker image for webhook receiver
- **`populate-pact-configmap`**: Populates ConfigMap from `target/pacts/`
- **`pact-infrastructure`**: Deploys all Pact components to Kubernetes
- **`pact-tests`**: Runs Pact contract tests

### Tilt Labels

Use Tilt labels to filter Pact resources:

```bash
# View only Pact resources
tilt get uiresources --label pact

# Start only Pact resources
tilt up --label pact
```

## Manual Setup (Without Tilt)

If you're not using Tilt, follow these steps:

### 1. Build Mock Server Binaries

```bash
# Build all mock server binaries
cargo build --target x86_64-unknown-linux-musl --bin gcp-mock-server
cargo build --target x86_64-unknown-linux-musl --bin aws-mock-server
cargo build --target x86_64-unknown-linux-musl --bin azure-mock-server
cargo build --target x86_64-unknown-linux-musl --bin webhook
cargo build --target x86_64-unknown-linux-musl --bin manager
```

### 2. Build Docker Images

```bash
# Build mock server image
docker build -f dockerfiles/Dockerfile.pact-mock-server \
  -t localhost:5000/pact-mock-server:latest \
  --build-arg IMAGE_NAME=localhost:5000/pact-mock-server \
  .

# Build webhook image
docker build -f dockerfiles/Dockerfile.pact-webhook \
  -t localhost:5000/mock-webhook:latest \
  --build-arg IMAGE_NAME=localhost:5000/mock-webhook \
  .
```

### 3. Push Images to Local Registry

If using a local Docker registry (e.g., Kind):

```bash
# Load images into Kind cluster
kind load docker-image localhost:5000/pact-mock-server:latest
kind load docker-image localhost:5000/mock-webhook:latest
```

Or push to a registry accessible by your cluster:

```bash
docker push localhost:5000/pact-mock-server:latest
docker push localhost:5000/mock-webhook:latest
```

### 4. Generate Pact Contracts

```bash
# Run Pact tests to generate contract files
cargo test --test pact_*

# Contracts are generated in target/pacts/
ls target/pacts/
```

### 5. Populate ConfigMap

```bash
# Use the populate script
python3 scripts/tilt/populate_pact_configmap.py

# Or manually create ConfigMap
kubectl create configmap pact-contracts \
  --from-file=target/pacts/ \
  -n secret-manager-controller-pact-broker \
  --dry-run=client -o yaml | kubectl apply -f -
```

### 6. Deploy Pact Infrastructure

```bash
# Apply Kubernetes manifests
kubectl apply -k pact-broker/k8s/

# Wait for deployment to be ready
kubectl wait --for=condition=available \
  deployment/pact-infrastructure \
  -n secret-manager-controller-pact-broker \
  --timeout=5m
```

### 7. Verify Setup

```bash
# Check pod status
kubectl get pods -n secret-manager-controller-pact-broker

# Check manager health
kubectl port-forward -n secret-manager-controller-pact-broker \
  deployment/pact-infrastructure 1238:1238
curl http://localhost:1238/ready

# Check broker health
kubectl port-forward -n secret-manager-controller-pact-broker \
  svc/pact-broker 9292:9292
curl http://localhost:9292/diagnostic/status/heartbeat
```

### 8. Run Pact Tests

```bash
# Run tests using the script
python3 scripts/pact_tests.py

# Or run tests manually
cargo test --test pact_*
```

## CI/CD Setup

### GitHub Actions Example

```yaml
name: Pact Tests

on:
  pull_request:
  push:
    branches: [main]

jobs:
  pact-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Set up Kind cluster
        uses: helm/kind-action@v1.8.0
        with:
          cluster_name: secret-manager-controller
          config: kind-config.yaml
      
      - name: Set up local Docker registry
        run: |
          docker run -d --name registry -p 5000:5000 registry:2
      
      - name: Build Rust binaries
        run: |
          cargo build --target x86_64-unknown-linux-musl \
            --bin gcp-mock-server \
            --bin aws-mock-server \
            --bin azure-mock-server \
            --bin webhook \
            --bin manager
      
      - name: Build Docker images
        run: |
          docker build -f dockerfiles/Dockerfile.pact-mock-server \
            -t localhost:5000/pact-mock-server:latest .
          docker build -f dockerfiles/Dockerfile.pact-webhook \
            -t localhost:5000/mock-webhook:latest .
      
      - name: Load images into Kind
        run: |
          kind load docker-image localhost:5000/pact-mock-server:latest
          kind load docker-image localhost:5000/mock-webhook:latest
      
      - name: Generate Pact contracts
        run: cargo test --test pact_*
      
      - name: Populate ConfigMap
        run: python3 scripts/tilt/populate_pact_configmap.py
      
      - name: Deploy Pact infrastructure
        run: kubectl apply -k pact-broker/k8s/
      
      - name: Wait for infrastructure
        run: |
          kubectl wait --for=condition=available \
            deployment/pact-infrastructure \
            -n secret-manager-controller-pact-broker \
            --timeout=5m
      
      - name: Run Pact tests
        run: python3 scripts/pact_tests.py
```

### Using Tilt in CI

You can also use `tilt ci` in CI/CD:

```yaml
- name: Run Tilt CI
  run: |
    tilt ci --label pact
```

This runs Tilt in CI mode (non-interactive) and waits for all Pact resources to be ready.

## Configuration

### Environment Variables

Pact infrastructure uses these environment variables:

**Broker**:
- `PACT_BROKER_PORT`: Broker port (default: 9292)
- `PACT_BROKER_DATABASE_URL`: Database URL (default: `sqlite:///pacts/pact_broker.sqlite`)
- `PACT_BROKER_BASIC_AUTH_USERNAME`: Username (default: `pact`)
- `PACT_BROKER_BASIC_AUTH_PASSWORD`: Password (default: `pact`)

**Manager**:
- `BROKER_URL`: Broker URL (default: `http://localhost:9292`)
- `BROKER_USERNAME`: Broker username (default: `pact`)
- `BROKER_PASSWORD`: Broker password (default: `pact`)
- `CONFIGMAP_NAME`: ConfigMap name (default: `pact-contracts`)
- `HEALTH_PORT`: Manager health port (default: 1238)

**Mock Servers**:
- `PACT_BROKER_URL`: Broker URL (default: `http://localhost:9292`)
- `PACT_BROKER_USERNAME`: Broker username (default: `pact`)
- `PACT_BROKER_PASSWORD`: Broker password (default: `pact`)
- `PACT_PROVIDER`: Provider name (e.g., `GCP-Secret-Manager`)
- `PACT_CONSUMER`: Consumer name (default: `Secret-Manager-Controller`)
- `PORT`: Mock server port (1234, 1235, or 1236)
- `MANAGER_URL`: Manager health URL (default: `http://localhost:1238`)

### Namespace

Pact infrastructure is deployed to:

```
secret-manager-controller-pact-broker
```

### Services

Services are created for each component:

- **`pact-broker`**: Port 9292
- **`aws-mock-server`**: Port 1234
- **`gcp-mock-server`**: Port 1234 (routes to container port 1235)
- **`azure-mock-server`**: Port 1234 (routes to container port 1236)
- **`mock-webhook`**: Port 1237

## Verifying Setup

### Check Pod Status

```bash
kubectl get pods -n secret-manager-controller-pact-broker
```

All containers should be `Running` and `Ready`:

```
NAME                                  READY   STATUS    RESTARTS   AGE
pact-infrastructure-64b66fcc75-xxxxx  6/6     Running   0          5m
```

### Check Manager Health

```bash
# Port forward to manager
kubectl port-forward -n secret-manager-controller-pact-broker \
  deployment/pact-infrastructure 1238:1238

# Check health
curl http://localhost:1238/ready | jq
```

Expected response:

```json
{
  "status": "ready",
  "broker_healthy": true,
  "pacts_published": true,
  "published_providers": [
    "GCP-Secret-Manager",
    "AWS-Secrets-Manager",
    "Azure-Key-Vault"
  ]
}
```

### Check Broker Health

```bash
# Port forward to broker
kubectl port-forward -n secret-manager-controller-pact-broker \
  svc/pact-broker 9292:9292

# Check health
curl -u pact:pact http://localhost:9292/diagnostic/status/heartbeat
```

Expected response: `200 OK`

### Check Published Contracts

```bash
# List published contracts
curl -u pact:pact http://localhost:9292/pacts/provider/GCP-Secret-Manager/consumer/Secret-Manager-Controller/latest | jq
```

### Check Mock Server Health

```bash
# Port forward to mock server
kubectl port-forward -n secret-manager-controller-pact-broker \
  svc/gcp-mock-server 1235:1234

# Check health
curl http://localhost:1235/health
```

Expected response:

```json
{
  "status": "ok",
  "service": "pact-mock-server"
}
```

## Enabling Pact Mode in Controller

To route controller requests to mock servers instead of real providers:

### Option 1: Kustomize Patch (Recommended)

Add to `config/kustomization.yaml`:

```yaml
patches:
  - path: deployment/pact-env-patch.yaml
    target:
      kind: Deployment
      name: secret-manager-controller
```

### Option 2: kubectl patch

```bash
kubectl patch deployment secret-manager-controller -n microscaler-system \
  --patch-file config/deployment/pact-env-patch.yaml
```

### Option 3: Environment Variables

```bash
kubectl set env deployment/secret-manager-controller -n microscaler-system \
  PACT_MODE=true \
  GCP_SECRET_MANAGER_ENDPOINT=http://gcp-mock-server.secret-manager-controller-pact-broker.svc.cluster.local:1234 \
  AWS_SECRETS_MANAGER_ENDPOINT=http://aws-mock-server.secret-manager-controller-pact-broker.svc.cluster.local:1234 \
  AZURE_KEY_VAULT_ENDPOINT=http://azure-mock-server.secret-manager-controller-pact-broker.svc.cluster.local:1234
```

## Troubleshooting

### Pod Not Starting

**Symptoms**: Pod stuck in `Pending` or `ContainerCreating`

**Diagnosis**:
```bash
kubectl describe pod -n secret-manager-controller-pact-broker -l app=pact-infrastructure
```

**Common Issues**:
- **Image pull errors**: Check image names and registry access
- **Resource constraints**: Check node resources
- **Init container failures**: Check init container logs

**Solution**:
```bash
# Check init container logs
kubectl logs -n secret-manager-controller-pact-broker \
  -l app=pact-infrastructure -c init-pact-db
```

### Broker Not Ready

**Symptoms**: Manager reports `broker_healthy: false`

**Diagnosis**:
```bash
kubectl logs -n secret-manager-controller-pact-broker \
  -l app=pact-infrastructure -c pact-broker
```

**Common Issues**:
- **Database initialization**: Check init container completed
- **Port conflicts**: Check if port 9292 is in use
- **Resource constraints**: Check pod resources

**Solution**:
```bash
# Check broker logs
kubectl logs -n secret-manager-controller-pact-broker \
  -l app=pact-infrastructure -c pact-broker --tail=50

# Check broker health directly
kubectl exec -n secret-manager-controller-pact-broker \
  -l app=pact-infrastructure -c pact-broker -- \
  curl http://localhost:9292/diagnostic/status/heartbeat
```

### Contracts Not Published

**Symptoms**: Manager reports `pacts_published: false`

**Diagnosis**:
```bash
kubectl logs -n secret-manager-controller-pact-broker \
  -l app=pact-infrastructure -c manager
```

**Common Issues**:
- **ConfigMap missing**: Check if ConfigMap exists
- **ConfigMap empty**: Check if Pact files are in ConfigMap
- **Broker not ready**: Wait for broker to be healthy
- **RBAC permissions**: Check ServiceAccount, Role, RoleBinding

**Solution**:
```bash
# Check ConfigMap
kubectl get configmap pact-contracts -n secret-manager-controller-pact-broker -o yaml

# Check manager logs
kubectl logs -n secret-manager-controller-pact-broker \
  -l app=pact-infrastructure -c manager --tail=50

# Check RBAC
kubectl get role,rolebinding,serviceaccount -n secret-manager-controller-pact-broker
```

### Mock Servers Not Starting

**Symptoms**: Mock server containers fail startup probe

**Diagnosis**:
```bash
kubectl logs -n secret-manager-controller-pact-broker \
  -l app=pact-infrastructure -c gcp-mock-server
```

**Common Issues**:
- **Broker not ready**: Wait for broker health check
- **Manager not ready**: Wait for manager `/ready` endpoint
- **Contracts not published**: Wait for manager to publish contracts
- **Timeout waiting**: Increase timeout or check broker/manager

**Solution**:
```bash
# Check mock server logs
kubectl logs -n secret-manager-controller-pact-broker \
  -l app=pact-infrastructure -c gcp-mock-server --tail=50

# Check manager status
curl http://localhost:1238/ready

# Check broker directly
curl -u pact:pact http://localhost:9292/diagnostic/status/heartbeat
```

### Port Forwarding Issues

**Symptoms**: Cannot connect to broker or mock servers via port forward

**Diagnosis**:
```bash
# Check if port is already in use
lsof -i :9292
lsof -i :1234
```

**Common Issues**:
- **Port already in use**: Kill existing port forward processes
- **Service not ready**: Wait for services to be ready
- **Network issues**: Check cluster networking

**Solution**:
```bash
# Kill existing port forwards
pkill -f "kubectl port-forward"

# Wait for services
kubectl wait --for=condition=ready \
  svc/pact-broker -n secret-manager-controller-pact-broker

# Retry port forward
kubectl port-forward -n secret-manager-controller-pact-broker \
  svc/pact-broker 9292:9292
```

### Test Failures

**Symptoms**: Pact tests fail with "No matching interaction"

**Diagnosis**:
```bash
# Check if contracts are published
curl -u pact:pact http://localhost:9292/pacts/provider/GCP-Secret-Manager/consumer/Secret-Manager-Controller/latest
```

**Common Issues**:
- **Contracts not published**: Re-run populate ConfigMap script
- **Wrong provider/consumer names**: Check contract files
- **Contracts expired**: Re-publish contracts

**Solution**:
```bash
# Re-populate ConfigMap
python3 scripts/tilt/populate_pact_configmap.py

# Wait for manager to re-publish
sleep 30

# Re-run tests
python3 scripts/pact_tests.py
```

## Next Steps

- [Pact Testing Overview](./overview.md) - Pact concepts and workflow
- [Pact Testing Architecture](./architecture.md) - Detailed architecture and diagrams
- [Writing Pact Tests](./writing-tests.md) - How to write contract tests

