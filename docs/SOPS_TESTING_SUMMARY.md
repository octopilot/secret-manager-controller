# SOPS Testing Summary

## Overview

Complete testing setup for SOPS decryption functionality. This allows you to test SOPS decryption without setting up full Git/Flux integration.

## Quick Start

```bash
# Complete setup (recommended)
python3 scripts/test-sops-complete.py --env dev

# Quick copy only
python3 scripts/test-sops-quick.py --env dev --copy-to-container
```

## What Gets Created

### Local Files
- Directory structure at `/tmp/flux-source-test-namespace-test-repo/`
- SOPS-encrypted files copied from examples
- Files ready for container copy

### Container Files
- Same directory structure created in controller container
- SOPS files copied using `kubectl cp`
- Files accessible to controller for reconciliation

## Scripts

### `test-sops-complete.py` (Recommended)

**Features:**
- ✅ Checks prerequisites (sops, gpg)
- ✅ Creates local directory structure
- ✅ Copies SOPS files locally
- ✅ **Automatically copies files to container** (default)
- ✅ Auto-detects controller pod
- ✅ Verifies files in container
- ✅ Provides complete next steps

**Usage:**
```bash
# Default (copies to container)
python3 scripts/test-sops-complete.py --env dev

# Custom paths
python3 scripts/test-sops-complete.py \
  --artifact-path /tmp/flux-source-my-namespace-my-repo \
  --env prod \
  --service my-service \
  --namespace microscaler-system \
  --pod-name secret-manager-controller-xxx

# Skip container copy
python3 scripts/test-sops-complete.py --env dev --no-copy-to-container
```

### `test-sops-quick.py`

**Features:**
- ✅ Quick file copy
- ✅ Optional container copy with `--copy-to-container` flag
- ✅ Auto-detects controller pod

**Usage:**
```bash
# Local copy only
python3 scripts/test-sops-quick.py --env dev

# With container copy
python3 scripts/test-sops-quick.py --env dev --copy-to-container
```

## Testing Workflow

### 1. Run Setup Script

```bash
python3 scripts/test-sops-complete.py --env dev
```

This will:
- Create local directory structure
- Copy SOPS files locally
- Find controller pod automatically
- Copy files into container
- Verify files in container

### 2. Create SOPS Private Key Secret

```bash
kubectl create secret generic sops-private-key \
  --from-file=private-key=/path/to/gpg-private-key.asc \
  -n microscaler-system
```

### 3. Create Test Configurations

```bash
# Minimal GitRepository (for testing)
kubectl apply -f examples/test-gitrepository-minimal.yaml

# SecretManagerConfig
kubectl apply -f examples/test-sops-config.yaml
```

### 4. Verify

```bash
# Check files in container
kubectl exec -it <controller-pod> -n microscaler-system -- \
  ls -la /tmp/flux-source-test-namespace-test-repo/deployment-configuration/profiles/dev/

# Check controller logs
kubectl logs -f <controller-pod> -n microscaler-system | grep -i sops

# Check SecretManagerConfig status
kubectl get secretmanagerconfig -o yaml
```

## File Locations

### Local
```
/tmp/flux-source-test-namespace-test-repo/
  deployment-configuration/
    profiles/
      dev/
        application.secrets.env
        application.secrets.yaml
        application.properties
```

### Container
Same path structure inside the controller container:
```
/tmp/flux-source-test-namespace-test-repo/
  deployment-configuration/
    profiles/
      dev/
        application.secrets.env
        application.secrets.yaml
        application.properties
```

## Troubleshooting

### Controller Pod Not Found

**Error:** `Controller pod not found`

**Solution:**
1. Ensure controller is deployed:
   ```bash
   kubectl get pods -n microscaler-system -l app=secret-manager-controller
   ```

2. Specify pod name manually:
   ```bash
   python3 scripts/test-sops-complete.py --env dev --pod-name <pod-name>
   ```

### Files Not Copied to Container

**Error:** `Failed to copy <file> to container`

**Solution:**
1. Check pod is running:
   ```bash
   kubectl get pods -n microscaler-system
   ```

2. Manual copy:
   ```bash
   kubectl cp /tmp/flux-source-test-namespace-test-repo/deployment-configuration/ \
     microscaler-system/<pod-name>:/tmp/flux-source-test-namespace-test-repo/deployment-configuration/
   ```

### Files Not Found by Controller

**Symptoms:** Controller logs show "Base path does not exist"

**Solution:**
1. Verify artifact path matches GitRepository status:
   ```bash
   kubectl get gitrepository test-repo -o jsonpath='{.status.artifact.path}'
   ```

2. Ensure files are in container:
   ```bash
   kubectl exec -it <pod> -n microscaler-system -- \
     ls -la /tmp/flux-source-test-namespace-test-repo/deployment-configuration/profiles/dev/
   ```

3. Check SecretManagerConfig environment matches:
   ```yaml
   spec:
     secrets:
       environment: dev  # Must match directory name
   ```

## Example Configurations

### Minimal GitRepository

See `examples/test-gitrepository-minimal.yaml`:
- Sets artifact path manually for testing
- No actual Git repository needed

### SecretManagerConfig

See `examples/test-sops-config.yaml`:
- Points to test GitRepository
- Configured for dev environment
- Ready for GCP/AWS/Azure

## Next Steps

Once SOPS decryption is verified:
1. Set up proper Git repository with SOPS-encrypted files
2. Configure FluxCD GitRepository
3. Test end-to-end: Git → Flux → Controller → Cloud Provider

## See Also

- `docs/QUICK_TEST_SOPS.md` - Detailed quick testing guide
- `docs/SOPS_IMPLEMENTATION.md` - SOPS implementation details
- `examples/sample-deployment-configuration/` - Sample SOPS files
- `examples/QUICK_START_SOPS.md` - SOPS quick start guide

