# Testing Guide

This guide covers all testing approaches for the Secret Manager Controller.

## Quick SOPS Testing (Recommended for Development)

Test SOPS decryption without setting up Git repositories or FluxCD:

```bash
# Complete setup with checks and instructions
python3 scripts/test-sops-complete.py --env dev

# Quick file copy only
python3 scripts/test-sops-quick.py --env dev
```

**What it does:**
- Creates directory structure at `/tmp/flux-source-test-namespace-test-repo/`
- Copies SOPS-encrypted files from examples
- Provides next steps for Kubernetes setup

**See:** `docs/QUICK_TEST_SOPS.md` for detailed guide

## Unit Tests

Run Rust unit tests:

```bash
cargo test
```

Run specific test:

```bash
cargo test test_name
```

## Pact Contract Tests

Run Pact contract tests (requires Pact broker):

```bash
# Using Tilt (recommended)
tilt up
# Then trigger pact-tests-and-publish resource

# Or manually
python3 scripts/pact_publish.py
```

**Test Coverage:**
- GCP Secret Manager: 12 tests
- AWS Secrets Manager: 13 tests
- AWS Parameter Store: 6 tests
- Azure Key Vault: 14 tests
- Azure App Configuration: 6 tests

**Total:** 51 Pact contract tests

## Integration Testing

### Prerequisites

1. **Kubernetes Cluster**
   - Kind cluster (local development)
   - GKE/EKS/AKS cluster (cloud testing)

2. **Cloud Credentials**
   - GCP: Workload Identity or service account JSON
   - AWS: IRSA or access keys
   - Azure: Workload Identity or service principal

3. **SOPS Private Key** (if testing SOPS)
   ```bash
   kubectl create secret generic sops-private-key \
     --from-file=private-key=/path/to/gpg-key.asc \
     -n microscaler-system
   ```

### Test Setup

1. **Deploy Controller**
   ```bash
   kubectl apply -f config/crd/
   kubectl apply -f config/rbac/
   kubectl apply -f config/deployment/
   ```

2. **Create Test GitRepository** (or use quick test)
   ```bash
   kubectl apply -f examples/test-gitrepository-minimal.yaml
   ```

3. **Create SecretManagerConfig**
   ```bash
   kubectl apply -f examples/test-sops-config.yaml
   ```

4. **Verify**
   ```bash
   # Check controller logs
   kubectl logs -f deployment/secret-manager-controller -n microscaler-system

   # Check SecretManagerConfig status
   kubectl get secretmanagerconfig -o yaml

   # Verify secrets in cloud provider
   # GCP: gcloud secrets list
   # AWS: aws secretsmanager list-secrets
   # Azure: az keyvault secret list --vault-name <vault>
   ```

## End-to-End Testing

### Full GitOps Flow

1. **Create Git Repository**
   - Add SOPS-encrypted files
   - Commit and push

2. **Configure FluxCD**
   ```bash
   flux create source git test-repo \
     --url=https://github.com/your-org/test-repo.git \
     --branch=main \
     --interval=1m
   ```

3. **Create SecretManagerConfig**
   ```yaml
   apiVersion: secretmanager.microscaler.io/v1alpha1
   kind: SecretManagerConfig
   metadata:
     name: test-config
   spec:
     sourceRef:
       kind: GitRepository
       name: test-repo
       namespace: flux-system
     secrets:
       environment: dev
     provider:
       type: gcp
       gcp:
         projectId: your-project
   ```

4. **Verify Sync**
   - Check controller logs
   - Verify secrets in cloud provider
   - Check SecretManagerConfig status

## Test Files and Examples

### Quick Test Scripts
- `scripts/test-sops-complete.py` - Complete test setup
- `scripts/test-sops-quick.py` - Quick file copy

### Example Configurations
- `examples/test-sops-config.yaml` - SecretManagerConfig for testing
- `examples/test-gitrepository-minimal.yaml` - Minimal GitRepository
- `examples/sample-deployment-configuration/` - Sample SOPS files

### Sample Files
- `examples/sample-deployment-configuration/profiles/dev/` - Dev environment
- `examples/sample-deployment-configuration/profiles/prod/` - Prod environment

## Troubleshooting Tests

### SOPS Decryption Fails

**Symptoms:**
- Controller logs show "sops decryption failed"
- Secrets not syncing

**Solutions:**
1. Verify SOPS private key secret exists:
   ```bash
   kubectl get secret sops-private-key -n microscaler-system
   ```

2. Check GPG key format:
   ```bash
   kubectl get secret sops-private-key -n microscaler-system -o jsonpath='{.data.private-key}' | base64 -d | gpg --list-secret-keys
   ```

3. Verify sops/gpg binaries in container:
   ```bash
   kubectl exec -it <controller-pod> -- which sops
   kubectl exec -it <controller-pod> -- which gpg
   ```

### Files Not Found

**Symptoms:**
- Controller logs show "Base path does not exist"
- No application files found

**Solutions:**
1. Verify artifact path:
   ```bash
   kubectl get gitrepository test-repo -o jsonpath='{.status.artifact.path}'
   ```

2. Check directory structure:
   ```bash
   kubectl exec -it <controller-pod> -- ls -la /tmp/flux-source-*/
   ```

3. Verify environment name matches:
   - Directory: `profiles/dev/`
   - Config: `spec.secrets.environment: dev`

### Cloud Provider Authentication Fails

**Symptoms:**
- "Failed to authenticate" errors
- Secrets not syncing to cloud

**Solutions:**
1. **GCP:**
   - Verify Workload Identity binding
   - Check service account permissions
   - Verify project ID

2. **AWS:**
   - Verify IRSA role annotation
   - Check IAM role permissions
   - Verify region

3. **Azure:**
   - Verify Workload Identity federation
   - Check managed identity permissions
   - Verify vault/app config endpoint

## Performance Testing

### Load Testing

Test with multiple SecretManagerConfigs:

```bash
# Create multiple configs
for i in {1..10}; do
  kubectl apply -f - <<EOF
apiVersion: secretmanager.microscaler.io/v1alpha1
kind: SecretManagerConfig
metadata:
  name: test-config-$i
spec:
  sourceRef:
    kind: GitRepository
    name: test-repo
    namespace: flux-system
  secrets:
    environment: dev
  provider:
    type: gcp
    gcp:
      projectId: your-project
EOF
done
```

Monitor:
- Controller CPU/memory usage
- Reconciliation time
- Cloud provider API rate limits

## Continuous Testing

### CI/CD Integration

Add to your CI pipeline:

```yaml
# Example GitHub Actions
- name: Run tests
  run: |
    cargo test
    python3 scripts/pact_publish.py
```

### Pre-commit Hooks

```bash
# Run tests before commit
cargo test
cargo clippy
cargo fmt --check
```

## See Also

- `docs/QUICK_TEST_SOPS.md` - Quick SOPS testing guide
- `docs/SOPS_IMPLEMENTATION.md` - SOPS implementation details
- `examples/QUICK_START_SOPS.md` - SOPS quick start
- `docs/PRODUCTION_READINESS.md` - Production deployment guide

