# SOPS Setup Guide

This guide walks you through setting up SOPS encryption for the sample deployment configuration.

## Prerequisites

- `sops` installed (see installation below)
- `gpg` installed (usually pre-installed on Linux/macOS)
- Access to the Git repository

## Step 1: Install SOPS

### macOS
```bash
brew install sops
```

### Linux
```bash
# Download latest release
wget https://github.com/mozilla/sops/releases/download/v3.8.0/sops-v3.8.0.linux
chmod +x sops-v3.8.0.linux
sudo mv sops-v3.8.0.linux /usr/local/bin/sops
```

### Verify Installation
```bash
sops --version
```

## Step 2: Generate GPG Keys

### Generate a New GPG Key Pair

```bash
# Generate key (use defaults, set name/email, no passphrase for automation)
gpg --batch --gen-key <<EOF
Key-Type: RSA
Key-Length: 2048
Name-Real: Secret Manager Controller
Name-Email: secret-manager@example.com
Expire-Date: 0
%no-protection
EOF
```

### List Your Keys

```bash
gpg --list-secret-keys --keyid-format LONG
```

You'll see output like:
```
sec   rsa2048/FBC7B9E2A4F9289AC0C1D4843D16CEE4A27381B4 2024-01-01
```

The key ID is: `FBC7B9E2A4F9289AC0C1D4843D16CEE4A27381B4`

### Export Public Key

```bash
# Export public key (for sharing/backup)
gpg --armor --export FBC7B9E2A4F9289AC0C1D4843D16CEE4A27381B4 > public-key.asc

# Export private key (for Kubernetes secret)
gpg --armor --export-secret-keys FBC7B9E2A4F9289AC0C1D4843D16CEE4A27381B4 > private-key.asc
```

## Step 3: Update .sops.yaml

Edit `.sops.yaml` and replace the placeholder GPG key IDs with your actual key IDs:

```yaml
creation_rules:
  - path_regex: profiles/dev/.*\.(secrets\.env|secrets\.yaml)$
    pgp: YOUR_KEY_ID_HERE  # Replace with your key ID
    encrypted_regex: ^(data|stringData|password|secret|key|token|credential|api_key|private_key|client_secret)$
```

## Step 4: Encrypt Files

### Encrypt ENV Files

```bash
# Encrypt dev secrets
sops -e -i profiles/dev/application.secrets.env

# Encrypt prod secrets
sops -e -i profiles/prod/application.secrets.env
```

### Encrypt YAML Files

```bash
# Encrypt dev YAML secrets
sops -e -i profiles/dev/application.secrets.yaml
```

### Verify Encryption

```bash
# Check that files are encrypted (should show SOPS metadata)
head -20 profiles/dev/application.secrets.env

# Decrypt to verify (don't commit decrypted files!)
sops -d profiles/dev/application.secrets.env
```

## Step 5: Create Kubernetes Secret

Create a Kubernetes secret with your GPG private key:

```bash
# Create secret from exported private key
kubectl create secret generic sops-private-key \
  --from-file=private-key=private-key.asc \
  --namespace=default
```

Or use the YAML file:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: sops-private-key
  namespace: default
type: Opaque
stringData:
  private-key: |
    -----BEGIN PGP PRIVATE KEY BLOCK-----
    (paste your private key here)
    -----END PGP PRIVATE KEY BLOCK-----
```

## Step 6: Test Decryption

### Test Locally

```bash
# Decrypt a file
sops -d profiles/dev/application.secrets.env

# Edit encrypted file (opens editor, saves encrypted)
sops profiles/dev/application.secrets.env
```

### Test with Controller

1. Create GitRepository pointing to your repo
2. Create SecretManagerConfig
3. Check controller logs for decryption:
   ```bash
   kubectl logs -n <namespace> <controller-pod> | grep -i sops
   ```

## Troubleshooting

### "No key could decrypt the message"

- Verify GPG key is imported: `gpg --list-secret-keys`
- Check `.sops.yaml` has correct key ID
- Ensure private key secret exists in Kubernetes

### "Failed to decrypt SOPS file"

- Check controller logs for detailed error
- Verify SOPS private key secret is in correct namespace
- Ensure secret field name is `private-key`, `key`, or `gpg-key`

### "SOPS file not encrypted"

- Files must be encrypted before committing
- Check file starts with SOPS metadata
- Re-encrypt if needed: `sops -e -i <file>`

## Security Best Practices

1. **Use Different Keys Per Environment**: Generate separate GPG keys for dev/staging/prod
2. **Rotate Keys Regularly**: Update keys every 90 days
3. **Limit Key Access**: Only grant decryption access to necessary services
4. **Backup Keys Securely**: Store private keys in secure vault (not Git!)
5. **Use Key Management**: Consider using cloud KMS (AWS KMS, GCP KMS, Azure Key Vault) instead of GPG

## Next Steps

1. ✅ SOPS installed
2. ✅ GPG keys generated
3. ✅ Files encrypted
4. ✅ Kubernetes secret created
5. Create GitRepository
6. Create SecretManagerConfig
7. Verify secrets sync to cloud provider

