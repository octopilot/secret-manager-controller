# Tilt Integration for Secret Manager Controller

## Overview

The Secret Manager Controller is now fully integrated into Tilt for local development. This enables:

1. **Automatic CRD regeneration** when Rust code changes
2. **Fast iteration** with live updates to the running controller
3. **Automatic builds** and deployments
4. **Unified development workflow** with other services

## How It Works

### 1. CRD Generation (`secret-manager-controller-crd-gen`)

**Trigger:** Changes to Rust source code (`src/`), `Cargo.toml`, or `Cargo.lock`

**Action:** Automatically regenerates `config/crd/secretmanagerconfig.yaml` using `cargo run --bin crdgen`

**Why:** Ensures the CRD always matches the Rust type definitions

### 2. Binary Build (`secret-manager-controller-build`)

**Trigger:** Changes to Rust source code (`src/`), `Cargo.toml`, or `Cargo.lock`

**Action:** Builds the controller binary for `x86_64-unknown-linux-musl` target

**Output:** `target/x86_64-unknown-linux-musl/debug/secret-manager-controller`

**Why:** Debug builds are faster for development iteration

### 3. Docker Image Build

**Trigger:** Changes to binary or Dockerfile

**Action:** Builds Docker image `localhost:5001/pricewhisperer-secret-manager-controller:tilt`

**Live Updates:** When binary changes, Tilt syncs it into the running container and sends SIGHUP to restart the controller

**Why:** Enables hot-reloading without full container rebuilds

### 4. Kubernetes Deployment

**Trigger:** Changes to kustomize manifests in `config/`

**Action:** Deploys controller to `flux-system` namespace using kustomize

**Dependencies:** Waits for binary build and CRD generation to complete

## Usage

### Starting Tilt

```bash
cd /Users/casibbald/Workspace/microscaler/PriceWhisperer
tilt up
```

The controller will:
1. Generate CRD automatically
2. Build the binary
3. Build Docker image
4. Deploy to Kubernetes

### Watching Changes

Tilt automatically watches for changes and:
- **Rust code changes** → Regenerates CRD → Rebuilds binary → Updates Docker image → Live updates container
- **Kustomize manifest changes** → Redeploys Kubernetes resources
- **Dockerfile changes** → Rebuilds Docker image

### Viewing Logs

```bash
# View controller logs in Tilt UI
# Or use kubectl:
kubectl logs -n flux-system -l app=secret-manager-controller -f
```

### Port Forwarding

Metrics endpoint is automatically forwarded:
- **Local:** `http://localhost:8080`
- **Metrics:** `http://localhost:8080/metrics`
- **Health:** `http://localhost:8080/healthz`
- **Ready:** `http://localhost:8080/readyz`

## File Structure

```
hack/controllers/secret-manager-controller/
├── Tiltfile                    # Controller-specific Tilt configuration
├── Dockerfile                  # Docker image definition
├── Cargo.toml                  # Rust dependencies
├── src/
│   ├── main.rs                 # Main controller code
│   ├── crdgen.rs               # CRD generation binary
│   └── ...                     # Other source files
└── config/
    ├── crd/
    │   └── secretmanagerconfig.yaml  # Auto-generated CRD
    ├── deployment/
    │   └── deployment.yaml     # Kubernetes deployment
    └── ...                     # Other kustomize manifests
```

## Integration Points

### Main Tiltfile

The controller is loaded in the main `Tiltfile`:

```python
# ====================
# Secret Manager Controller
# ====================
# Load the controller Tiltfile

load('./hack/controllers/secret-manager-controller/Tiltfile')
```

### Deployment Image

The deployment manifest uses the Tilt-built image:

```yaml
image: localhost:5001/pricewhisperer-secret-manager-controller:tilt
imagePullPolicy: Never
```

## Development Workflow

1. **Make changes** to Rust code in `src/`
2. **Tilt automatically:**
   - Regenerates CRD
   - Rebuilds binary
   - Updates Docker image
   - Live updates running container
3. **Controller restarts** with new code
4. **Check logs** to verify changes

## Troubleshooting

### CRD Not Regenerating

- Check that `cargo run --bin crdgen` works manually
- Verify Tilt is watching `src/` directory
- Check Tilt logs for errors

### Binary Not Building

- Ensure Rust toolchain is installed: `rustup target add x86_64-unknown-linux-musl`
- Check `Cargo.toml` for correct dependencies
- Verify Tilt can access the controller directory

### Docker Image Not Building

- Ensure Docker is running
- Check that `localhost:5001` registry is accessible (Kind cluster)
- Verify Dockerfile syntax

### Controller Not Deploying

- Check Kubernetes cluster is accessible: `kubectl cluster-info`
- Verify `flux-system` namespace exists: `kubectl get namespace flux-system`
- Check RBAC permissions are correct

### Live Updates Not Working

- Ensure binary path is correct in Tiltfile
- Check that container has write permissions
- Verify SIGHUP signal handling in controller code

## Labels

The controller uses the `controllers` label, allowing you to filter resources in Tilt UI:

```bash
tilt up --label controllers
```

## Performance Tips

1. **Use debug builds** for faster iteration (already configured)
2. **Live updates** avoid full container rebuilds
3. **Parallel builds** enabled where possible
4. **Watch only necessary files** to reduce overhead

## Next Steps

- Add integration tests that run in Tilt
- Add health check validation
- Add metrics scraping configuration
- Add example SecretManagerConfig resources for testing

