# Development Setup

Guide for setting up a local development environment for the Secret Manager Controller.

## Prerequisites

- **Rust**: 1.70+ (install via [rustup](https://rustup.rs/))
- **Docker**: For building container images
- **kubectl**: For Kubernetes cluster access
- **Kind**: For local Kubernetes cluster (optional, for integration tests)
- **Tilt**: For local development (recommended)

## Quick Start

### 1. Clone the Repository

```bash
git clone https://github.com/microscaler/secret-manager-controller.git
cd secret-manager-controller
```

### 2. Install Dependencies

```bash
# Install Rust toolchain
rustup install stable

# Install required Rust targets
rustup target add x86_64-unknown-linux-musl

# Install musl tools (for cross-compilation)
# macOS
brew install musl-cross

# Linux
sudo apt-get install musl-tools
```

### 3. Build the Project

```bash
# Build the controller binary
cargo build

# Build for Linux (for Docker)
cargo build --target x86_64-unknown-linux-musl --release
```

### 4. Run Tests

```bash
# Run unit tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

## Development with Tilt

Tilt provides the best development experience with live code updates.

### Setup

1. **Install Tilt:**

```bash
# macOS
brew install tilt-dev/tap/tilt

# Linux
curl -fsSL https://raw.githubusercontent.com/tilt-dev/tilt/master/scripts/install.sh | bash
```

2. **Start Tilt:**

```bash
tilt up
```

This will:
- Set up a Kind cluster
- Build and deploy the controller
- Set up GitOps components (FluxCD/ArgoCD)
- Set up Pact infrastructure for testing
- Enable live code updates

### Live Updates

Tilt watches for code changes and automatically:
- Rebuilds the Rust binary
- Syncs it into the running container
- Restarts the controller (SIGHUP)

### Tilt Resources

Key resources in Tilt:
- `secret-manager-controller`: Main controller deployment
- `build-all-binaries`: Builds all Rust binaries
- `pact-infrastructure`: Pact broker and mock servers
- `apply-gitops-cluster`: GitOps components

See [Tilt Integration](./tilt-integration.md) for details.

## Manual Development Setup

If you prefer not to use Tilt:

### 1. Set Up Kind Cluster

```bash
# Create Kind cluster
python3 scripts/setup_kind.py

# Or manually
kind create cluster --name secret-manager-controller
```

### 2. Install GitOps Components

```bash
# Install FluxCD
python3 scripts/tilt/install_fluxcd.py

# Install ArgoCD CRDs
python3 scripts/tilt/install_argocd.py
```

### 3. Build and Deploy Controller

```bash
# Build binary
cargo build --target x86_64-unknown-linux-musl --release

# Build Docker image
docker build -t secret-manager-controller:dev -f dockerfiles/Dockerfile.controller .

# Load into Kind
kind load docker-image secret-manager-controller:dev --name secret-manager-controller

# Apply manifests
kubectl apply -k config/
```

### 4. Update Controller

```bash
# Rebuild
cargo build --target x86_64-unknown-linux-musl --release

# Rebuild image
docker build -t secret-manager-controller:dev -f dockerfiles/Dockerfile.controller .

# Restart controller
kubectl rollout restart deployment/secret-manager-controller -n microscaler-system
```

## Project Structure

```
secret-manager-controller/
├── crates/
│   ├── controller/          # Main controller crate
│   ├── providers/           # Cloud provider clients
│   ├── gitops/              # GitOps integration
│   ├── sops/                # SOPS decryption
│   ├── kustomize/           # Kustomize builder
│   └── ...
├── config/                  # Kubernetes manifests
├── scripts/                 # Automation scripts
├── tests/                   # Integration tests
└── docs/                    # Documentation
```

## Code Organization

### Controller Logic

- **Location**: `crates/controller/src/controller/`
- **Main entry**: `main.rs`
- **Reconciliation**: `reconcile.rs`

### CRD Definitions

- **Location**: `crates/controller/src/crd/`
- **Spec**: `spec.rs`
- **Status**: `status.rs`
- **Source**: `source.rs`
- **Provider**: `provider.rs`

### Provider Clients

- **Location**: `crates/controller/src/providers/`
- **GCP**: `gcp/`
- **AWS**: `aws/`
- **Azure**: `azure/`

## Development Workflow

### 1. Make Changes

Edit code in the appropriate crate.

### 2. Test Locally

```bash
# Run unit tests
cargo test

# Run with specific features
cargo test --features gcp,aws,azure
```

### 3. Test in Cluster

```bash
# With Tilt (automatic)
tilt up

# Or manually
# Rebuild, redeploy, check logs
kubectl logs -n microscaler-system -l app=secret-manager-controller -f
```

### 4. Run Integration Tests

```bash
# Set up Kind cluster
python3 scripts/setup_kind.py

# Run integration tests
cargo test --test integration
```

## Debugging

### View Logs

```bash
# Controller logs
kubectl logs -n microscaler-system -l app=secret-manager-controller -f

# With previous logs
kubectl logs -n microscaler-system -l app=secret-manager-controller --previous
```

### Enable Debug Logging

Set `RUST_LOG` environment variable:

```yaml
# In deployment
env:
  - name: RUST_LOG
    value: debug
```

Or via ConfigMap (if hot-reload enabled):

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: secret-manager-controller-config
  namespace: microscaler-system
data:
  RUST_LOG: debug
```

### Debug in IDE

1. Install Rust analyzer extension
2. Set breakpoints
3. Use `lldb` or `gdb` for debugging

## Code Generation

### CRD Generation

The CRD is auto-generated from Rust types:

```bash
# Generate CRD
cargo run -p controller --bin crdgen

# Output: config/crd/secretmanagerconfig.yaml
```

**Note:** Don't edit the CRD YAML directly - modify the Rust types instead.

## Next Steps

- [Tilt Integration](./tilt-integration.md) - Tilt development workflow
- [Kind Cluster Setup](./kind-cluster-setup.md) - Local cluster setup
- [Testing Guide](../testing/testing-guide.md) - Testing strategies

