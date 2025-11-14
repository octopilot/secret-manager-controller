# Quick Start: SOPS with Flux Integration

This guide provides a quick start for setting up SOPS-encrypted secrets with Flux and the Secret Manager Controller.

## Overview

The Secret Manager Controller syncs secrets from Git repositories to cloud secret stores (AWS Secrets Manager, GCP Secret Manager, Azure Key Vault). It supports SOPS-encrypted files for secure secret management.

## Architecture

```
Git Repository (SOPS-encrypted secrets)
    ↓
Flux GitRepository (watches repo)
    ↓
Secret Manager Controller (decrypts & syncs)
    ↓
Cloud Secret Stores (AWS/GCP/Azure)
```

## Step-by-Step Setup

### 1. Prepare Your Repository

```bash
# Clone or create your repository
git clone https://github.com/your-org/your-repo.git
cd your-repo

# Copy sample deployment-configuration
cp -r examples/sample-deployment-configuration deployment-configuration
```

### 2. Set Up SOPS

Follow the detailed guide: `sample-deployment-configuration/SOPS_SETUP.md`

Quick version:
```bash
# Install SOPS
brew install sops  # macOS

# Generate GPG key
gpg --batch --gen-key <<EOF
Key-Type: RSA
Key-Length: 2048
Name-Real: Secret Manager
Name-Email: secret-manager@example.com
Expire-Date: 0
%no-protection
EOF

# Get key ID
gpg --list-secret-keys --keyid-format LONG

# Update .sops.yaml with your key ID
# Then encrypt files
sops -e -i deployment-configuration/profiles/dev/application.secrets.env
sops -e -i deployment-configuration/profiles/prod/application.secrets.env
```

### 3. Commit to Git

```bash
# Commit encrypted files
git add deployment-configuration/
git commit -m "Add SOPS-encrypted secrets"
git push
```

### 4. Create Flux GitRepository

```bash
# Apply GitRepository
kubectl apply -f examples/sample-flux-gitrepository.yaml

# Update URL in the file to point to your repo
# Then apply again
```

Or create manually:
```yaml
apiVersion: source.toolkit.fluxcd.io/v1
kind: GitRepository
metadata:
  name: my-service-repo
  namespace: flux-system
spec:
  url: https://github.com/your-org/your-repo
  ref:
    branch: main
  interval: 1m
```

### 5. Create SOPS Private Key Secret

```bash
# Export your GPG private key
gpg --armor --export-secret-keys YOUR_KEY_ID > private-key.asc

# Create Kubernetes secret
kubectl create secret generic sops-private-key \
  --from-file=private-key=private-key.asc \
  --namespace=default
```

### 6. Create SecretManagerConfig

```bash
# Edit and apply SecretManagerConfig
kubectl apply -f examples/sample-secret-manager-config.yaml

# Update the config with:
# - Your GitRepository name
# - Your cloud provider credentials
# - Your environment name
```

### 7. Verify

```bash
# Check GitRepository status
kubectl get gitrepository -n flux-system

# Check SecretManagerConfig status
kubectl get secretmanagerconfig

# Check controller logs
kubectl logs -n <controller-namespace> <controller-pod> | grep -i sops

# Verify secrets in cloud provider
# AWS: aws secretsmanager list-secrets
# GCP: gcloud secrets list
# Azure: az keyvault secret list --vault-name <vault>
```

## Directory Structure

```
your-repo/
├── deployment-configuration/
│   ├── .sops.yaml                    # SOPS config
│   ├── .gitignore                    # Ignore unencrypted files
│   ├── README.md                     # Documentation
│   ├── SOPS_SETUP.md                 # SOPS setup guide
│   └── profiles/
│       ├── dev/
│       │   ├── application.properties      # Config (unencrypted)
│       │   ├── application.secrets.env     # Secrets (SOPS-encrypted)
│       │   └── application.secrets.yaml    # Secrets YAML (SOPS-encrypted)
│       └── prod/
│           ├── application.properties      # Config (unencrypted)
│           └── application.secrets.env     # Secrets (SOPS-encrypted)
```

## File Types

| File | Type | Encryption | Destination |
|------|------|------------|-------------|
| `application.properties` | Config | None | Config stores (when `configs.enabled=true`) |
| `application.secrets.env` | Secrets | SOPS | Secret stores |
| `application.secrets.yaml` | Secrets | SOPS | Secret stores |

## Troubleshooting

### Controller Can't Decrypt SOPS Files

1. **Check SOPS private key secret exists**:
   ```bash
   kubectl get secret sops-private-key -n default
   ```

2. **Verify key format**:
   ```bash
   kubectl get secret sops-private-key -n default -o yaml
   # Should have 'private-key', 'key', or 'gpg-key' field
   ```

3. **Check controller logs**:
   ```bash
   kubectl logs <controller-pod> | grep -i sops
   ```

### GitRepository Not Syncing

1. **Check GitRepository status**:
   ```bash
   kubectl describe gitrepository my-service-repo -n flux-system
   ```

2. **Check for authentication issues** (if private repo):
   ```bash
   kubectl get secret git-credentials -n flux-system
   ```

### Secrets Not Appearing in Cloud Provider

1. **Check SecretManagerConfig status**:
   ```bash
   kubectl describe secretmanagerconfig my-service-secrets-dev
   ```

2. **Verify cloud provider credentials**:
   - AWS: Check IRSA annotations
   - GCP: Check Workload Identity
   - Azure: Check Workload Identity federation

3. **Check controller logs for errors**:
   ```bash
   kubectl logs <controller-pod> | tail -50
   ```

## Next Steps

1. ✅ Repository structure created
2. ✅ SOPS encryption set up
3. ✅ Flux GitRepository created
4. ✅ SOPS private key secret created
5. ✅ SecretManagerConfig created
6. 🔄 Implement SOPS decryption in controller (next step)
7. Test end-to-end sync

## Related Files

- `sample-deployment-configuration/` - Sample directory structure
- `sample-flux-gitrepository.yaml` - Flux GitRepository example
- `sample-secret-manager-config.yaml` - SecretManagerConfig example
- `sample-deployment-configuration/SOPS_SETUP.md` - Detailed SOPS setup

