# SOPS Private Key Setup

This guide shows how to export your existing GPG key and create a Kubernetes secret for SOPS decryption.

## Quick Setup

If you have the `flux@pricewhisperer.ai` key in your local GPG keyring:

```bash
# Export key and create Kubernetes secret automatically
python3 scripts/setup-sops-key.sh
```

This uses the default key `flux@pricewhisperer.ai`. If you have a different key, use `--key-email` to specify it.

This will:
1. Export the `flux@pricewhisperer.ai` private key from your GPG keyring
2. Create a Kubernetes secret `sops-private-key` in `microscaler-system` namespace
3. Store the private key in the secret

## Manual Setup

### Step 1: Export GPG Private Key

```bash
# Export the private key for flux@pricewhisperer.ai
gpg --armor --export-secret-keys flux@pricewhisperer.ai > /tmp/flux-private-key.asc
```

### Step 2: Create Kubernetes Secret

```bash
# Create secret from exported key
kubectl create secret generic sops-private-key \
  --from-file=private-key=/tmp/flux-private-key.asc \
  -n microscaler-system

# Clean up temporary file
rm /tmp/flux-private-key.asc
```

## Verify Secret

```bash
# Check secret exists
kubectl get secret sops-private-key -n microscaler-system

# View secret details
kubectl describe secret sops-private-key -n microscaler-system

# Verify key format (should show BEGIN PGP PRIVATE KEY BLOCK)
kubectl get secret sops-private-key -n microscaler-system \
  -o jsonpath='{.data.private-key}' | base64 -d | head -5
```

## Alternative Secret Names

The controller looks for secrets in this order:
1. `sops-private-key`
2. `sops-gpg-key`
3. `gpg-key`

You can use any of these names:

```bash
# Using alternative name
python3 scripts/setup-sops-key.sh --secret-name sops-gpg-key
```

## Using Different Key

If you want to use a different GPG key:

```bash
# Export and create secret with different key
python3 scripts/setup-sops-key.sh --key-email your-email@example.com
```

## Script Options

```bash
python3 scripts/setup-sops-key.sh --help

Options:
  --key-email EMAIL      GPG key email (default: flux@pricewhisperer.io)
  --secret-name NAME     Kubernetes secret name (default: sops-private-key)
  --namespace NAMESPACE  Kubernetes namespace (default: microscaler-system)
  --dry-run              Show what would be done without creating secret
```

## Troubleshooting

### Key Not Found

**Error:** `Key not found for: flux@pricewhisperer.io`

**Solution:**
1. List available keys:
   ```bash
   gpg --list-secret-keys --keyid-format LONG
   ```

2. Use the correct email or key ID:
   ```bash
   python3 scripts/setup-sops-key.sh --key-email <actual-email>
   ```

### GPG Key Export Fails

**Error:** `Failed to export GPG key`

**Solution:**
1. Ensure GPG keyring is unlocked
2. Check key exists:
   ```bash
   gpg --list-secret-keys flux@pricewhisperer.io
   ```

3. Try exporting manually:
   ```bash
   gpg --armor --export-secret-keys flux@pricewhisperer.io
   ```

### Secret Creation Fails

**Error:** `Failed to create secret`

**Solution:**
1. Check kubectl access:
   ```bash
   kubectl get namespaces
   ```

2. Ensure namespace exists:
   ```bash
   kubectl get namespace microscaler-system
   ```

3. Create namespace if needed:
   ```bash
   kubectl create namespace microscaler-system
   ```

## Security Notes

- **Private Key Security**: The private key is stored in a Kubernetes secret, which is encrypted at rest (if etcd encryption is enabled)
- **Access Control**: Use RBAC to restrict access to the secret
- **Key Rotation**: Rotate keys periodically (every 90 days recommended)
- **Backup**: Keep a secure backup of the private key

## Next Steps

After creating the secret:

1. **Verify Secret**: Check secret exists and is accessible
2. **Test SOPS Decryption**: Use test scripts to verify decryption works
3. **Check Controller Logs**: Monitor controller logs for SOPS decryption

```bash
# Test SOPS setup
python3 scripts/test-sops-complete.py --env dev

# Check controller logs
kubectl logs -f <controller-pod> -n microscaler-system | grep -i sops
```

## See Also

- `docs/QUICK_TEST_SOPS.md` - Quick SOPS testing guide
- `docs/SOPS_IMPLEMENTATION.md` - SOPS implementation details
- `examples/QUICK_START_SOPS.md` - SOPS quick start

