# Sample Deployment Configuration

This directory contains example deployment configuration files for the Secret Manager Controller.

## Directory Structure

```
deployment-configuration/
├── .sops.yaml                    # SOPS encryption configuration
└── profiles/
    ├── dev/
    │   ├── application.properties      # Non-sensitive config (dev)
    │   ├── application.secrets.env     # SOPS-encrypted secrets (dev)
    │   └── application.secrets.yaml     # SOPS-encrypted secrets YAML (dev)
    └── prod/
        ├── application.properties      # Non-sensitive config (prod)
        └── application.secrets.env     # SOPS-encrypted secrets (prod)
```

## File Types

### `application.properties`
- Contains non-sensitive configuration values
- Synced to config stores (AWS Parameter Store, GCP Secret Manager, Azure App Configuration) when `configs.enabled = true`
- Format: `KEY=VALUE`

### `application.secrets.env`
- Contains SOPS-encrypted secrets in ENV format
- Synced to secret stores (AWS Secrets Manager, GCP Secret Manager, Azure Key Vault)
- Format: `KEY=VALUE`
- Comment support: Lines starting with `#` are treated as disabled secrets
- Must be encrypted with SOPS before committing
- **Merging**: If both `.env` and `.yaml` files exist, `.yaml` values override `.env` values

### `application.secrets.yaml`
- Contains SOPS-encrypted secrets in YAML format
- Supports nested structure (automatically flattened to dot notation)
- Comment support: Commented sections (starting with `#`) are treated as disabled secrets
- Synced to secret stores (AWS Secrets Manager, GCP Secret Manager, Azure Key Vault)
- Must be encrypted with SOPS before committing
- **Flattening**: Nested keys like `database.password` are flattened to `database.password` in the secret store
- **Merging**: If both `.env` and `.yaml` files exist, `.yaml` values override `.env` values

## SOPS Encryption

### Setup

1. **Install SOPS**:
   ```bash
   brew install sops  # macOS
   # or
   wget https://github.com/mozilla/sops/releases/download/v3.8.0/sops-v3.8.0.linux
   ```

2. **Generate GPG Keys** (if not already done):
   ```bash
   gpg --full-generate-key
   # Export public key
   gpg --armor --export YOUR_KEY_ID > public-key.asc
   ```

3. **Update `.sops.yaml`** with your GPG key IDs

### Encrypting Files

```bash
# Encrypt .env file
sops -e -i profiles/dev/application.secrets.env

# Encrypt .yaml file
sops -e -i profiles/dev/application.secrets.yaml

# Encrypt and edit in place
sops profiles/dev/application.secrets.env
sops profiles/dev/application.secrets.yaml

# Verify encryption
sops -d profiles/dev/application.secrets.env
sops -d profiles/dev/application.secrets.yaml
```

### Decrypting Files (for testing)

```bash
# Decrypt .env file to stdout
sops -d profiles/dev/application.secrets.env

# Decrypt .yaml file to stdout
sops -d profiles/dev/application.secrets.yaml

# Decrypt to file (for testing only - don't commit)
sops -d profiles/dev/application.secrets.env > profiles/dev/application.secrets.env.decrypted
sops -d profiles/dev/application.secrets.yaml > profiles/dev/application.secrets.yaml.decrypted
```

## Flux Integration

### 1. Create GitRepository

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
  secretRef:
    name: git-credentials  # Optional: if repo is private
```

### 2. Create SOPS Private Key Secret

The controller needs the SOPS private key to decrypt files:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: sops-private-key
  namespace: default  # Same namespace as SecretManagerConfig
type: Opaque
stringData:
  private-key: |
    -----BEGIN PGP PRIVATE KEY BLOCK-----
    ...
    -----END PGP PRIVATE KEY BLOCK-----
```

**Note**: In production, use Sealed Secrets, External Secrets Operator, or similar to manage this secret securely.

### 3. Create SecretManagerConfig

See `../gitops/cluster/env/tilt/secretmanagerconfig.yaml` for a complete example, or check the [examples directory](../examples/) for different use cases.

## Testing Locally

### Without SOPS Encryption

For testing, you can use unencrypted files:

```bash
# Copy example files
cp profiles/dev/application.secrets.env profiles/dev/application.secrets.env.test

# Edit and test (don't commit unencrypted files!)
```

### With SOPS Encryption

1. Set up GPG keys
2. Update `.sops.yaml` with your key IDs
3. Encrypt files:
   ```bash
   sops -e -i profiles/dev/application.secrets.env
   sops -e -i profiles/dev/application.secrets.yaml
   ```
4. Commit encrypted files

## Security Notes

⚠️ **Important**:
- Never commit unencrypted secret files
- Always encrypt files with SOPS before committing
- Rotate GPG keys regularly
- Use different keys for different environments in production
- Store SOPS private keys securely (use Kubernetes secrets with proper RBAC)

## Example Values

The example files contain placeholder values. Replace them with your actual secrets:

- Database credentials
- API keys
- OAuth client secrets
- Encryption keys
- Service account keys

## Next Steps

1. Set up GPG keys for SOPS
2. Update `.sops.yaml` with your key IDs
3. Replace placeholder values with real secrets
4. Encrypt files with SOPS
5. Commit to your Git repository
6. Create Flux GitRepository pointing to your repo
7. Create SecretManagerConfig to sync secrets

