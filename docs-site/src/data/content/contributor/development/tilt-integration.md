# Tilt Integration

Complete guide to using Tilt for local development of the Secret Manager Controller.

## Overview

Tilt provides a unified development environment with:
- Automatic builds and deployments
- Live code updates (hot reload)
- Integrated testing infrastructure
- GitOps component management

## Quick Start

### Start Tilt

```bash
tilt up
```

This will:
1. Create a Kind cluster (if needed)
2. Build all Rust binaries
3. Deploy the controller
4. Set up GitOps components (FluxCD/ArgoCD)
5. Deploy Pact infrastructure for testing

### Stop Tilt

```bash
tilt down
```

Or press `Ctrl+C` in the Tilt UI.

## Tilt Resources

### Core Resources

#### `build-all-binaries`

Builds all Rust binaries for the project.

**Triggers:**
- Changes to Rust source code
- Changes to `Cargo.toml` or `Cargo.lock`

**Outputs:**
- Controller binary
- CRD generator binary
- Mock server binaries
- Manager binary

#### `secret-manager-controller`

Main controller deployment.

**Dependencies:**
- `build-all-binaries`
- CRD application
- GitOps components

**Live Updates:**
- Binary changes are synced into container
- Controller restarted with SIGHUP

#### `pact-infrastructure`

Pact broker and mock servers for contract testing.

**Components:**
- Pact broker (port 9292)
- Mock webhook server (port 1237)
- AWS mock server (port 1234)
- GCP mock server (port 1235)
- Azure mock server (port 1236)
- Manager sidecar (port 1238)

### GitOps Resources

#### `apply-gitops-cluster`

Applies GitOps cluster configuration.

**Includes:**
- Namespaces
- GitRepository resources
- Application resources

#### `fluxcd-install`

Installs FluxCD components.

**Installs:**
- source-controller
- GitRepository CRD

#### `argocd-install`

Installs ArgoCD CRDs.

**Installs:**
- Application CRD
- ApplicationSet CRD

## Development Workflow

### 1. Make Code Changes

Edit Rust source files in `crates/`.

### 2. Automatic Rebuild

Tilt detects changes and:
1. Rebuilds the binary
2. Syncs it into the container
3. Restarts the controller

**Watch the Tilt UI** to see build progress.

### 3. Check Logs

View controller logs in Tilt UI or:

```bash
kubectl logs -n octopilot-system -l app=secret-manager-controller -f
```

### 4. Test Changes

Create or update a SecretManagerConfig:

```bash
kubectl apply -f examples/secretmanagerconfig.yaml
```

Watch reconciliation in logs.

## Live Updates

Tilt uses `sync` to update the binary without full container rebuilds:

```python
sync('./target/x86_64-unknown-linux-musl/debug/secret-manager-controller', '/app/secret-manager-controller')
```

When the binary changes:
1. Tilt syncs it into the container
2. Sends SIGHUP to the controller process
3. Controller restarts with new binary

**Benefits:**
- Fast iteration (seconds vs minutes)
- No container rebuilds
- Preserves container state

## CRD Generation

The CRD is auto-generated when Rust types change:

**Process:**
1. `build-all-binaries` builds `crdgen` binary
2. Runs `cargo run -p controller --bin crdgen`
3. Generates `config/crd/secretmanagerconfig.yaml`
4. Applies CRD to cluster

**Manual trigger:**

```bash
tilt trigger build-all-binaries
```

## Testing with Tilt

### Pact Tests

Pact infrastructure is automatically deployed:

```bash
# Run Pact tests
python3 scripts/pact_tests.py
```

### Integration Tests

```bash
# Run integration tests
cargo test --test integration
```

### Unit Tests

```bash
# Run unit tests (outside cluster)
cargo test
```

## Troubleshooting

### Controller Not Starting

**Check:**
1. Binary build succeeded
2. CRD is applied
3. GitOps components are ready

**View logs:**
```bash
kubectl logs -n octopilot-system -l app=secret-manager-controller
```

### Live Updates Not Working

**Check:**
1. Binary path is correct
2. Container has write permissions
3. Process can receive SIGHUP

**Manual sync:**
```bash
kubectl cp target/x86_64-unknown-linux-musl/debug/secret-manager-controller \
  octopilot-system/secret-manager-controller-xxx:/app/secret-manager-controller
```

### Build Failures

**Check:**
1. Rust toolchain is installed
2. musl target is installed
3. Dependencies are up to date

**Clean build:**
```bash
cargo clean
tilt trigger build-all-binaries
```

## Tiltfile Structure

The `Tiltfile` is organized into sections:

1. **Configuration**: Registry, context, settings
2. **Binary Builds**: Rust compilation
3. **Docker Builds**: Container images
4. **Kubernetes Resources**: Deployments, services
5. **GitOps Setup**: FluxCD, ArgoCD
6. **Pact Infrastructure**: Testing setup

## Best Practices

1. **Use Tilt for Development**: Fastest iteration cycle
2. **Watch Tilt UI**: See what's happening
3. **Check Logs Early**: Catch errors quickly
4. **Test in Cluster**: Integration tests catch real issues
5. **Clean Builds**: When things get weird, clean and rebuild

## Next Steps

- [Kind Cluster Setup](./kind-cluster-setup.md) - Cluster configuration
- [Testing Guide](../testing/testing-guide.md) - Testing strategies
- [Development Setup](./setup.md) - General development guide

