# Quick SOPS Testing Guide

This guide shows how to quickly test SOPS decryption without setting up full Git/Flux integration.

## Quick Test Method

Instead of setting up Git repositories and FluxCD, you can directly copy SOPS-encrypted files into the path where the controller expects them.

## Method 1: Complete Setup Script (Recommended)

```bash
# Complete setup with all steps and instructions (automatically copies to container)
python3 scripts/test-sops-complete.py --env dev

# Or specify custom paths
python3 scripts/test-sops-complete.py \
  --artifact-path /tmp/flux-source-my-namespace-my-repo \
  --env prod \
  --service my-service \
  --namespace microscaler-system \
  --pod-name secret-manager-controller-xxx
```

This script will:
- Check prerequisites (sops, gpg)
- Create directory structure locally
- Copy SOPS-encrypted files locally
- **Copy files into the controller container** (default behavior)
- Verify files in container
- Provide complete next steps

**Important:** Files are automatically copied into the controller container so the controller can access them.

## Method 2: Quick Copy Script

```bash
# Copy files locally and into container
python3 scripts/test-sops-quick.py --env dev --copy-to-container

# Just copy files locally (manual container copy needed)
python3 scripts/test-sops-quick.py --env dev

# Or specify custom artifact path
python3 scripts/test-sops-quick.py \
  --artifact-path /tmp/flux-source-my-namespace-my-repo \
  --env prod \
  --service my-service \
  --copy-to-container \
  --pod-name secret-manager-controller-xxx
```

**Note:** Use `--copy-to-container` flag to automatically copy files into the controller container.

## Method 3: Manual Copy

### Step 1: Determine Artifact Path

The controller expects files at one of these locations:

**For FluxCD:**
- Default: `/tmp/flux-source-<namespace>-<name>`
- Or from GitRepository status: `status.artifact.path`

**For ArgoCD:**
- Default: `/tmp/argocd-repo-<hash>`

### Step 2: Create Directory Structure

The controller looks for files in this structure:

```
{artifact_path}/
  {base_path}/                    # Optional base path
    {service}/                     # Service name (monolith) or skip (single service)
      deployment-configuration/
        profiles/
          {environment}/           # Environment name (dev, prod, etc.)
            application.secrets.env
            application.secrets.yaml
            application.properties
```

**Example for single service:**
```bash
mkdir -p /tmp/flux-source-test-namespace-test-repo/deployment-configuration/profiles/dev
```

**Example for monolith:**
```bash
mkdir -p /tmp/flux-source-test-namespace-test-repo/microservices/my-service/deployment-configuration/profiles/dev
```

### Step 3: Copy SOPS Files Locally

```bash
# Copy from examples (if you have SOPS-encrypted files there)
cp examples/sample-deployment-configuration/profiles/dev/application.secrets.env \
   /tmp/flux-source-test-namespace-test-repo/deployment-configuration/profiles/dev/

# Or copy your own SOPS-encrypted files
cp /path/to/your/application.secrets.env \
   /tmp/flux-source-test-namespace-test-repo/deployment-configuration/profiles/dev/
```

### Step 3b: Copy Files into Container

**Important:** Files must be copied INTO the controller container so the controller can access them:

```bash
# Find controller pod
CONTROLLER_POD=$(kubectl get pods -n microscaler-system -l app=secret-manager-controller -o jsonpath='{.items[0].metadata.name}')

# Copy entire directory into container
kubectl cp /tmp/flux-source-test-namespace-test-repo/deployment-configuration/ \
  microscaler-system/$CONTROLLER_POD:/tmp/flux-source-test-namespace-test-repo/deployment-configuration/

# Or copy individual files
kubectl cp /tmp/flux-source-test-namespace-test-repo/deployment-configuration/profiles/dev/application.secrets.env \
  microscaler-system/$CONTROLLER_POD:/tmp/flux-source-test-namespace-test-repo/deployment-configuration/profiles/dev/application.secrets.env
```

## Step 4: Create SecretManagerConfig

Create a `SecretManagerConfig` that points to this artifact path:

```bash
# Use the example config
kubectl apply -f examples/test-sops-config.yaml
```

Or create your own:

```yaml
apiVersion: secretmanager.microscaler.io/v1alpha1
kind: SecretManagerConfig
metadata:
  name: test-sops-config
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: test-repo
    namespace: test-namespace
  secrets:
    environment: dev
    basePath: ""  # Empty for single service, or specify base path
  provider:
    type: gcp
    gcp:
      projectId: your-gcp-project
```

**Note**: The controller will use the artifact path from FluxCD GitRepository status. For quick testing, you can create a minimal GitRepository:

```bash
kubectl apply -f examples/test-gitrepository-minimal.yaml
```

## Step 5: Ensure SOPS Private Key Secret Exists

```bash
# Create SOPS private key secret
kubectl create secret generic sops-private-key \
  --from-file=private-key=/path/to/gpg-private-key.asc \
  -n <controller-namespace>

# Or use one of these secret names:
# - sops-private-key
# - sops-gpg-key
# - gpg-key
```

The controller looks for secrets in this order:
1. `sops-private-key`
2. `sops-gpg-key`
3. `gpg-key`

And extracts the key from these fields (in order):
1. `private-key`
2. `key`
3. `gpg-key`

## Step 6: Verify Files in Container

```bash
# List files in container
kubectl exec -it <controller-pod> -- ls -la /tmp/flux-source-test-namespace-test-repo/deployment-configuration/profiles/dev/

# Check if SOPS file is detected
kubectl exec -it <controller-pod> -- head -5 /tmp/flux-source-test-namespace-test-repo/deployment-configuration/profiles/dev/application.secrets.env
```

You should see SOPS metadata in the file (if encrypted):
```yaml
sops:
    kms: []
    gcp_kms: []
    azure_kv: []
    hc_vault: []
    age: []
    lastmodified: "2024-01-01T00:00:00Z"
    mac: "..."
    pgp:
    -   created_at: "2024-01-01T00:00:00Z"
        enc: |
            -----BEGIN PGP MESSAGE-----
            ...
```

## Step 7: Check Controller Logs

```bash
# Watch controller logs for SOPS decryption
kubectl logs -f <controller-pod> | grep -i sops

# Look for messages like:
# "Detected SOPS-encrypted file: ..."
# "Successfully decrypted SOPS content using sops binary"
# "Using sops binary at: ..."
```

## Troubleshooting

### "sops binary not found"

**Solution**: Install sops in the controller container:
```dockerfile
# In Dockerfile
RUN brew install sops  # macOS
# or
RUN apt-get install -y sops  # Linux
```

### "gpg binary not found"

**Solution**: Install gpg in the controller container:
```dockerfile
RUN brew install gnupg  # macOS
# or
RUN apt-get install -y gnupg  # Linux
```

### "Failed to import GPG private key"

**Possible Causes**:
- Invalid GPG key format
- Key is password-protected
- GPG keyring permissions issue

**Solution**: 
- Verify key format: `gpg --list-secret-keys`
- Ensure key is not password-protected for automation
- Check controller logs for detailed error messages

### Files Not Found

**Check**:
1. Artifact path matches GitRepository status
2. Directory structure matches expected format
3. File names match (`application.secrets.env`, `application.secrets.yaml`, `application.properties`)
4. Environment name matches `spec.secrets.environment`

## Example: Complete Test Setup

```bash
# 1. Create directory structure
mkdir -p /tmp/flux-source-test-namespace-test-repo/deployment-configuration/profiles/dev

# 2. Copy SOPS-encrypted files
cp examples/sample-deployment-configuration/profiles/dev/application.secrets.env \
   /tmp/flux-source-test-namespace-test-repo/deployment-configuration/profiles/dev/

# 3. Create SOPS private key secret
kubectl create secret generic sops-private-key \
  --from-file=private-key=~/.gnupg/private-key.asc \
  -n microscaler-system

# 4. Create SecretManagerConfig (see example above)

# 5. Check controller logs
kubectl logs -f deployment/secret-manager-controller -n microscaler-system | grep -i sops
```

## Next Steps

Once SOPS decryption is verified:
1. Set up proper Git repository with SOPS-encrypted files
2. Configure FluxCD GitRepository
3. Test end-to-end: Git → Flux → Controller → Cloud Provider

## See Also

- `examples/sample-deployment-configuration/SOPS_SETUP.md` - SOPS setup guide
- `examples/QUICK_START_SOPS.md` - Quick start guide
- `docs/SOPS_IMPLEMENTATION.md` - Implementation details

