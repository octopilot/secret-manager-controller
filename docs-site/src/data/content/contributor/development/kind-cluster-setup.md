# Kind Cluster Setup

Complete guide for setting up a local Kind (Kubernetes IN Docker) cluster for development and testing.

## Overview

Kind provides a local Kubernetes cluster running in Docker containers. The Secret Manager Controller uses Kind for:

- **Local Development**: Testing controller behavior in a real Kubernetes environment
- **Integration Tests**: Running end-to-end tests with actual Kubernetes resources
- **CI/CD**: GitHub Actions uses Kind for automated testing

## Prerequisites

Before setting up a Kind cluster, ensure you have:

- **Docker**: Running Docker daemon (required for Kind)
- **kubectl**: Kubernetes command-line tool
- **Kind**: Kind binary installed
- **Python 3**: For setup scripts (Python 3.8+)

### Installing Prerequisites

#### Docker

```bash
# macOS
brew install docker

# Linux
sudo apt-get install docker.io
sudo systemctl start docker
sudo systemctl enable docker

# Verify
docker --version
```

#### kubectl

```bash
# macOS
brew install kubectl

# Linux
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl

# Verify
kubectl version --client
```

#### Kind

```bash
# macOS
brew install kind

# Linux
curl -Lo ./kind https://kind.sigs.k8s.io/dl/v0.20.0/kind-linux-amd64
chmod +x ./kind
sudo mv ./kind /usr/local/bin/kind

# Verify
kind version
```

## Quick Start

The easiest way to set up a Kind cluster is using the provided setup script:

```bash
# Run the setup script
python3 scripts/setup_kind.py
```

This script will:
1. Check prerequisites (Docker, kubectl, Kind)
2. Set up a local Docker registry (if needed)
3. Create the Kind cluster with proper configuration
4. Install GitOps components (FluxCD, ArgoCD CRDs)
5. Configure the cluster for local development

## Cluster Configuration

### Cluster Name

The default cluster name is `secret-manager-controller`. This can be changed in `scripts/setup_kind.py`:

```python
CLUSTER_NAME = "secret-manager-controller"
```

### Network Configuration

The cluster uses custom subnet allocation to prevent IP conflicts with other Kind clusters:

**Pod Subnet**: `10.202.0.0/16`  
**Service Subnet**: `10.203.0.0/16`

This configuration is defined in `kind-config.yaml`:

```yaml
networking:
  podSubnet: "10.202.0.0/16"
  serviceSubnet: "10.203.0.0/16"
```

See [Subnet Allocation](#subnet-allocation) for details.

### Cluster Configuration File

The cluster configuration is defined in `kind-config.yaml`:

```yaml
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
name: secret-manager-controller
networking:
  podSubnet: "10.202.0.0/16"
  serviceSubnet: "10.203.0.0/16"
nodes:
- role: control-plane
  kubeadmConfigPatches:
  - |
    kind: InitConfiguration
    nodeRegistration:
      kubeletExtraArgs:
        node-labels: "ingress-ready=true"
  extraPortMappings:
  - containerPort: 80
    hostPort: 8080
    protocol: TCP
  - containerPort: 443
    hostPort: 8443
    protocol: TCP
```

## Local Docker Registry

The setup script automatically configures a local Docker registry for development. This is essential for local development because:

1. **No External Registry Needed**: Build images locally without pushing to Docker Hub or other registries
2. **Fast Iteration**: Push images instantly without network delays
3. **CI/CD Compatibility**: Same workflow works in CI/CD (GitHub Actions uses local registry)
4. **Tilt Integration**: Tilt automatically pushes images to local registry

### Registry Details

- **Registry Name**: `secret-manager-controller-registry`
- **Port**: `5000` (host port, also accessible from Kind cluster)
- **Image**: `registry:2` (official Docker registry image)
- **Volume**: `secret-manager-controller-registry-data` (persists registry data)

### Why We Need a Local Registry

**Without Local Registry**:
- Would need to push images to external registry (Docker Hub, etc.)
- Requires authentication and network access
- Slower iteration (push/pull over network)
- CI/CD complexity (managing registry credentials)

**With Local Registry**:
- Build and push images locally instantly
- No authentication needed
- Works offline
- Same workflow in CI/CD and local development

### Registry Setup

The script checks for an existing registry on port 5000 and uses it if found. Otherwise, it creates a new registry container:

```bash
# Check if registry exists
docker ps | grep registry:2

# Create registry manually (if needed)
docker run -d \
  --name secret-manager-controller-registry \
  --restart=always \
  -p 5000:5000 \
  registry:2
```

### Connecting Registry to Cluster

The registry is automatically connected to the Kind cluster network. This is critical for pods to pull images:

**Network Connection**:
```bash
# Connect registry to Kind network
docker network connect kind secret-manager-controller-registry
```

**Why This Matters**:
- Kind cluster runs in Docker containers
- Registry must be on same Docker network for pods to access it
- Without network connection, pods get `ImagePullBackOff` errors

**How It Works**:
1. Kind creates a Docker network named `kind`
2. All Kind nodes are on this network
3. Registry container is connected to same network
4. Pods can now access registry via container name or IP

### Containerd Registry Mirror Configuration

The setup script configures containerd on Kind nodes to use the local registry as a mirror for `localhost:5000`:

**Configuration**:
```toml
[plugins."io.containerd.grpc.v1.cri".registry.mirrors."localhost:5000"]
  endpoint = ["http://secret-manager-controller-registry:5000"]
```

**Why This Is Needed**:
- Kubernetes images are referenced as `localhost:5000/image:tag`
- Containerd needs to know where to pull from
- Registry mirror tells containerd to pull from local registry instead of external

**How It Works**:
1. Script finds registry container IP address
2. Updates containerd config on all Kind nodes
3. Restarts containerd to apply changes
4. Pods can now pull images from `localhost:5000/*`

**Troubleshooting**: If images fail to pull, run `python3 scripts/fix_registry_config.py` to reconfigure.

### Using the Registry

Build and push images to the local registry:

```bash
# Build image
docker build -t localhost:5000/secret-manager-controller:dev .

# Push to local registry
docker push localhost:5000/secret-manager-controller:dev

# Use in Kubernetes manifests
image: localhost:5000/secret-manager-controller:dev
```

## GitOps Components

The setup script automatically installs GitOps components. These are required for the controller to work with GitOps repositories.

### Why Install GitOps Components?

The Secret Manager Controller integrates with GitOps tools to sync secrets from Git repositories:

- **FluxCD**: Provides `GitRepository` CRD for FluxCD-managed repositories
- **ArgoCD**: Provides `Application` CRD for ArgoCD-managed repositories

The controller reads these CRs to discover Git repositories and clone them for secret extraction.

### FluxCD

FluxCD source-controller is installed for `GitRepository` support:

```bash
# Install FluxCD (via script)
python3 scripts/tilt/install_fluxcd.py
```

**What Gets Installed**:
- `GitRepository` CRD: Defines Git repository sources
- `Bucket` CRD: Defines bucket sources (not used by controller)
- Source controller deployment: Manages Git repository lifecycle
- Required RBAC resources: ServiceAccount, ClusterRole, ClusterRoleBinding

**Why We Need It**:
The controller watches for `GitRepository` CRs and uses them to:
1. Discover Git repository URLs
2. Clone repositories for secret extraction
3. Monitor repository changes

**What We Don't Install**:
- FluxCD controller (not needed - controller handles reconciliation)
- FluxCD UI (not needed for development)
- Other FluxCD components (Kustomize controller, Helm controller, etc.)

### ArgoCD

ArgoCD CRDs are installed for `Application` support:

```bash
# Install ArgoCD CRDs (via script)
python3 scripts/tilt/install_argocd.py
```

**What Gets Installed**:
- `Application` CRD: Defines ArgoCD applications
- `ApplicationSet` CRD: Defines application sets (not used by controller)
- Other required ArgoCD CRDs: For CRD validation

**Why We Need It**:
The controller watches for `Application` CRs and uses them to:
1. Discover Git repository URLs from Application spec
2. Clone repositories for secret extraction
3. Monitor application changes

**What We Don't Install**:
- ArgoCD server (not needed - controller clones repos directly)
- ArgoCD application controller (not needed - controller handles reconciliation)
- ArgoCD UI (not needed for development)
- ArgoCD repo server (not needed - controller clones repos directly)

**Important**: The controller does NOT use ArgoCD's Git repository management. It clones repositories directly using the information from `Application` CRs.

## Subnet Allocation

To prevent IP conflicts with other Kind clusters, the controller uses custom subnet allocation:

| Cluster/Application | Pod Subnet | Service Subnet |
|---------------------|------------|----------------|
| `secret-manager-controller` | `10.202.0.0/16` | `10.203.0.0/16` |
| `brrtrouter-dev` | `10.200.0.0/16` | `10.201.0.0/16` |
| Other clusters | `10.244.0.0/16` (default) | `10.96.0.0/12` (default) |

### Why Custom Subnets?

When running multiple Kind clusters on the same host:
- **Pod IP Conflicts**: Overlapping pod subnets cause routing issues
- **Service IP Conflicts**: Overlapping service subnets cause DNS problems
- **Network Isolation**: Custom subnets prevent conflicts

### Changing Subnets

To use different subnets, edit `kind-config.yaml`:

```yaml
networking:
  podSubnet: "10.204.0.0/16"    # Change this
  serviceSubnet: "10.205.0.0/16" # Change this
```

**Important**: Ensure subnets don't overlap with other Kind clusters or your host network.

## Manual Setup

If you prefer to set up the cluster manually:

### 1. Create Cluster

```bash
# Create cluster from config
kind create cluster --config kind-config.yaml --name secret-manager-controller

# Or create with defaults
kind create cluster --name secret-manager-controller
```

### 2. Set Up Registry

```bash
# Create registry
docker run -d \
  --name secret-manager-controller-registry \
  --restart=always \
  -p 5000:5000 \
  registry:2

# Connect to Kind network
docker network connect kind secret-manager-controller-registry
```

### 3. Configure containerd

The setup script automatically configures containerd to use the local registry. For manual setup:

```bash
# Get cluster node name
NODE_NAME=$(kubectl get nodes -o jsonpath='{.items[0].metadata.name}')

# Configure containerd
docker exec $NODE_NAME sh -c 'echo "plugins.\"io.containerd.grpc.v1.cri\".registry.mirrors.\"localhost:5000\".endpoint = [\"http://secret-manager-controller-registry:5000\"]" >> /etc/containerd/config.toml'

# Restart containerd
docker exec $NODE_NAME sh -c 'systemctl restart containerd'
```

### 4. Install GitOps Components

```bash
# Install FluxCD
python3 scripts/tilt/install_fluxcd.py

# Install ArgoCD CRDs
python3 scripts/tilt/install_argocd.py
```

## Verifying Setup

### Check Cluster Status

```bash
# Verify cluster is running
kind get clusters

# Check cluster nodes
kubectl get nodes

# Check cluster info
kubectl cluster-info --context kind-secret-manager-controller
```

### Check Registry

```bash
# Verify registry is running
docker ps | grep registry

# Test registry connectivity
curl http://localhost:5000/v2/
```

### Check GitOps Components

```bash
# Check FluxCD
kubectl get crds | grep fluxcd
kubectl get pods -n flux-system

# Check ArgoCD CRDs
kubectl get crds | grep argoproj
```

## Using the Cluster

### Set kubectl Context

```bash
# Set context to Kind cluster
kubectl config use-context kind-secret-manager-controller

# Verify context
kubectl config current-context
```

### Deploy Resources

```bash
# Apply manifests
kubectl apply -k config/

# Check deployments
kubectl get deployments -n microscaler-system

# View logs
kubectl logs -n microscaler-system -l app=secret-manager-controller -f
```

### Load Images

```bash
# Build image
docker build -t secret-manager-controller:dev .

# Load into Kind
kind load docker-image secret-manager-controller:dev --name secret-manager-controller

# Or use local registry
docker tag secret-manager-controller:dev localhost:5000/secret-manager-controller:dev
docker push localhost:5000/secret-manager-controller:dev
```

## Troubleshooting

### Cluster Creation Fails

**Problem**: Cluster creation fails with network errors.

**Solution**: Check for port conflicts and ensure Docker has enough resources:

```bash
# Check Docker resources
docker info

# Check for port conflicts
netstat -an | grep 6443  # Kubernetes API port
netstat -an | grep 5000  # Registry port
```

### Registry Not Accessible

**Problem**: Pods can't pull images from local registry.

**Solution**: Verify registry is connected to Kind network:

```bash
# Check network connection
docker network inspect kind | grep secret-manager-controller-registry

# Reconnect if needed
docker network connect kind secret-manager-controller-registry
```

### Image Pull Errors

**Problem**: `ImagePullBackOff` errors when deploying.

**Solution**: Ensure images are loaded or pushed to registry:

```bash
# Load image directly
kind load docker-image secret-manager-controller:dev --name secret-manager-controller

# Or push to registry
docker push localhost:5000/secret-manager-controller:dev
```

### GitOps Components Missing

**Problem**: FluxCD or ArgoCD CRDs not found.

**Solution**: Reinstall GitOps components:

```bash
# Reinstall FluxCD
python3 scripts/tilt/install_fluxcd.py

# Reinstall ArgoCD
python3 scripts/tilt/install_argocd.py
```

### Subnet Conflicts

**Problem**: Network connectivity issues with other Kind clusters.

**Solution**: Verify subnet allocation doesn't overlap:

```bash
# Check current cluster subnets
kubectl get nodes -o jsonpath='{.items[0].spec.podCIDR}'
kubectl cluster-info dump | grep service-cluster-ip-range

# Compare with other clusters
kind get clusters
```

## CI/CD Integration

The Kind cluster setup is used in GitHub Actions CI:

```yaml
- name: Create Kind Cluster
  uses: helm/kind-action@v1.9.0
  with:
    config: kind-config.yaml

- name: Setup Kind
  run: python3 scripts/setup_kind.py
```

The setup script detects CI environment and runs non-interactively:

```python
# Detects CI via environment variables
is_non_interactive = (
    os.getenv("CI", "").lower() in ("1", "true", "yes") or
    os.getenv("GITHUB_ACTIONS", "").lower() in ("1", "true", "yes")
)
```

## Cleaning Up

### Delete Cluster

```bash
# Delete Kind cluster
kind delete cluster --name secret-manager-controller
```

### Remove Registry

```bash
# Stop and remove registry
docker stop secret-manager-controller-registry
docker rm secret-manager-controller-registry
```

### Clean Up All

```bash
# Delete cluster and registry
kind delete cluster --name secret-manager-controller
docker stop secret-manager-controller-registry 2>/dev/null || true
docker rm secret-manager-controller-registry 2>/dev/null || true
```

## Moving Parts Summary

### What Gets Installed and Why

| Component | Purpose | Why We Need It |
|-----------|---------|----------------|
| **Kind Cluster** | Local Kubernetes environment | Test controller in real K8s environment |
| **Docker Registry** | Local image storage | Build/push images without external registry |
| **FluxCD CRDs** | GitRepository support | Controller watches GitRepository CRs |
| **ArgoCD CRDs** | Application support | Controller watches Application CRs |
| **microscaler-system Namespace** | Controller namespace | Where controller runs |

### Network Architecture

```
Host Machine
├── Docker Network: kind
│   ├── Kind Control Plane Node
│   │   └── Kubernetes API Server
│   ├── Registry Container (secret-manager-controller-registry:5000)
│   └── Pods (can access registry via network)
└── Host Port Mappings
    ├── 6443 → Kubernetes API
    ├── 5000 → Registry
    └── 8080/8443 → Ingress (if configured)
```

### Image Pull Flow

1. **Tilt builds image**: `docker build -t localhost:5000/controller:dev .`
2. **Tilt pushes to registry**: `docker push localhost:5000/controller:dev`
3. **Kubernetes pulls image**: Pod references `localhost:5000/controller:dev`
4. **Containerd resolves**: Uses registry mirror config to find local registry
5. **Image pulled**: From local registry (fast, no network)

### Component Interactions

```
┌─────────────────┐
│  setup_kind.py  │
└────────┬────────┘
         │
         ├── Creates Kind cluster
         ├── Creates registry
         ├── Connects registry to network
         ├── Configures containerd
         ├── Installs FluxCD CRDs
         ├── Installs ArgoCD CRDs
         └── Creates namespace
              │
              ▼
┌─────────────────┐
│  Kind Cluster   │
│                 │
│  ┌───────────┐  │
│  │ Registry  │  │◄─── Images pushed here
│  └─────┬─────┘  │
│        │        │
│  ┌─────▼─────┐  │
│  │  Pods     │  │◄─── Pull images from registry
│  └───────────┘  │
└─────────────────┘
```

## Next Steps

- [Development Setup](./setup.md) - Complete development environment setup
- [Tilt Integration](./tilt-integration.md) - Using Tilt for development
- [Postgres Manager](./postgres-manager.md) - Database migration management
- [Pact Mocks Manager](./pact-mocks-manager.md) - Pact infrastructure management
- [Python Scripts](./python-scripts.md) - Development automation scripts
- [Testing Guide](../testing/testing-guide.md) - Running tests in Kind cluster

