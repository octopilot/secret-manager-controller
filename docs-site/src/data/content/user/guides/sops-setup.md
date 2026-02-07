# SOPS Setup

Guide for setting up SOPS encryption for secrets in Git repositories.

## Overview

SOPS (Secrets OPerationS) allows you to encrypt secret files before committing them to Git. The Secret Manager Controller automatically decrypts SOPS-encrypted files using GPG or AGE keys stored in Kubernetes Secrets.

## Prerequisites

- GPG key pair OR AGE key pair generated
- SOPS installed locally
- Kubernetes cluster with controller installed

## Encryption Methods

The controller supports two encryption methods:

1. **GPG (GNU Privacy Guard)** - Traditional PGP encryption
2. **AGE (Actually Good Encryption)** - Modern, simpler encryption

You can use either or both methods. This guide covers both.

## GPG Key Setup

### Step 1: Generate GPG Key

If you don't have a GPG key, generate one:

```bash
gpg --full-generate-key
```

Follow the prompts:
- Key type: RSA and RSA (default)
- Key size: 4096 (recommended)
- Expiration: Set as needed (or 0 for no expiration)
- Name and email: Use your identity

### Step 2: Export Public Key

Export your public key for sharing (if needed):

```bash
gpg --armor --export your-email@example.com > public-key.asc
```

### Step 3: Get GPG Fingerprint

Get your GPG key fingerprint:

```bash
gpg --list-keys --fingerprint
```

You'll see output like:
```
pub   rsa4096 2024-01-15 [SC]
      ABC1 2345 DEF6 7890 ABCD EF12 3456 7890 ABCD EF12
uid           [ultimate] Your Name <your-email@example.com>
```

The fingerprint is: `ABC12345DEF67890ABCDEF1234567890ABCDEF12`

## AGE Key Setup

### Step 1: Generate AGE Key

Generate a new AGE key pair:

```bash
# Generate key pair
age-keygen -o age-key.txt
```

This creates a file with:
- **Public key**: `age1...` (share this)
- **Private key**: `AGE-SECRET-KEY-1...` (keep this secret)

**Example output:**
```
# created: 2024-01-15T10:30:00Z
# public key: age1abc123def456ghi789jkl012mno345pqr678stu901vwx234yz
AGE-SECRET-KEY-1ABC123DEF456GHI789JKL012MNO345PQR678STU901VWX234YZ567890ABCDEF
```

### Step 2: Extract Keys

```bash
# Extract public key
grep "public key" age-key.txt | cut -d' ' -f4 > age-public-key.txt

# Extract private key
grep "AGE-SECRET-KEY" age-key.txt > age-private-key.txt
```

## Step 4: Create SOPS Configuration

Create a `.sops.yaml` file in your repository root:

### GPG-Only Configuration

```yaml
creation_rules:
  - path_regex: .*\.secrets\.(env|yaml)$
    encrypted_regex: ^(data|stringData|DATABASE_|API_|JWT_)
    pgp: >-
      ABC12345DEF67890ABCDEF1234567890ABCDEF12,
      XYZ98765UVW43210ZYXWVU9876543210ZYXWVU98
```

### AGE-Only Configuration

```yaml
creation_rules:
  - path_regex: .*\.secrets\.(env|yaml)$
    encrypted_regex: ^(data|stringData|DATABASE_|API_|JWT_)
    age: >-
      age1abc123def456ghi789jkl012mno345pqr678stu901vwx234yz,
      age1xyz987uvw654rst321qpo098nml765kji432hgf210edc876ba
```

### Combined GPG and AGE Configuration

```yaml
creation_rules:
  - path_regex: .*\.secrets\.(env|yaml)$
    encrypted_regex: ^(data|stringData|DATABASE_|API_|JWT_)
    pgp: >-
      ABC12345DEF67890ABCDEF1234567890ABCDEF12
    age: >-
      age1abc123def456ghi789jkl012mno345pqr678stu901vwx234yz
```

This allows decryption with either GPG or AGE keys (redundancy).

## Step 5: Encrypt Secrets

Encrypt your secret files:

```bash
# Encrypt a YAML file
sops -e -i application.secrets.yaml

# Encrypt an ENV file
sops -e -i application.secrets.env

# Encrypt a properties file
sops -e -i application.properties
```

The files will be encrypted in place. SOPS will use the encryption method(s) specified in `.sops.yaml`.

## Step 6: Create Kubernetes Secrets

Export your private keys and create Kubernetes Secrets:

### GPG Key Setup

#### Automated Setup

Use the setup script:

```bash
python3 scripts/setup_sops_key.py --key-email your-email@example.com
```

This will:
1. Export the GPG private key
2. Create a Kubernetes Secret `sops-gpg-key` in `octopilot-system` namespace
3. Store the private key securely

#### Manual Setup

```bash
# Export private key
gpg --armor --export-secret-keys your-email@example.com > /tmp/private-key.asc

# Create Kubernetes Secret
kubectl create secret generic sops-gpg-key \
  --from-file=private.key=/tmp/private-key.asc \
  -n octopilot-system

# Clean up
rm /tmp/private-key.asc
```

### AGE Key Setup

```bash
# Extract private key from age-key.txt
grep "AGE-SECRET-KEY" age-key.txt > /tmp/age-private-key.txt

# Create Kubernetes Secret
kubectl create secret generic sops-age-key \
  --from-file=private.key=/tmp/age-private-key.txt \
  -n octopilot-system

# Clean up
rm /tmp/age-private-key.txt
```

**Note:** Keep `age-key.txt` secure - it contains both public and private keys.

## Step 7: Configure SecretManagerConfig

Reference the encryption keys in your SecretManagerConfig:

### GPG-Only Configuration

```yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-config
spec:
  secrets:
    sops:
      enabled: true
      gpgSecretRef:
        name: sops-gpg-key
        namespace: octopilot-system
        key: private.key
```

### AGE-Only Configuration

```yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-config
spec:
  secrets:
    sops:
      enabled: true
      ageSecretRef:
        name: sops-age-key
        namespace: octopilot-system
        key: private.key
```

### Combined GPG and AGE Configuration

You can specify both for redundancy:

```yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-config
spec:
  secrets:
    sops:
      enabled: true
      gpgSecretRef:
        name: sops-gpg-key
        namespace: octopilot-system
        key: private.key
      ageSecretRef:
        name: sops-age-key
        namespace: octopilot-system
        key: private.key
```

The controller will try GPG first, then AGE if GPG fails.

## Verification

### Test Encryption Locally

#### Test GPG Encryption

```bash
# Encrypt a test file with GPG
echo "password: secret123" > test.yaml
sops -e -i test.yaml

# Decrypt to verify
sops -d test.yaml
```

#### Test AGE Encryption

```bash
# Encrypt a test file with AGE
echo "password: secret123" > test.yaml
sops -e -i test.yaml

# Decrypt to verify (requires AGE key in environment)
export SOPS_AGE_KEY_FILE=age-key.txt
sops -d test.yaml
```

### Verify Controller Can Decrypt

Check controller logs:

```bash
kubectl logs -n octopilot-system -l app=secret-manager-controller | grep -i sops
```

You should see successful decryption messages:
```
✅ Loaded SOPS private key from secret 'octopilot-system/sops-gpg-key'
🔑 SOPS file requires GPG key fingerprints: ABC12345DEF67890...
✅ SOPS decryption successful
```

Or for AGE:
```
✅ Loaded SOPS private key from secret 'octopilot-system/sops-age-key'
✅ SOPS decryption successful
```

## Key Management

### Secret Names

The controller checks for secrets in this order:

1. `sops-private-key`
2. `sops-gpg-key`
3. `sops-age-key`
4. `gpg-key`

### Secret Keys

Within each secret, the controller checks for keys in this order:

1. `private.key`
2. `key`
3. `gpg-key`
4. `age-key`

### Namespace Placement

SOPS private keys should be placed in the **same namespace** as the `SecretManagerConfig` resource. The controller will:

1. First check the resource's namespace for the SOPS key secret
2. If not found, log a critical error (no fallback to controller namespace)
3. This ensures proper namespace isolation and prevents configuration errors

## Best Practices

1. **Multiple Keys**: Use multiple GPG or AGE keys for redundancy
2. **Combined Methods**: Use both GPG and AGE for maximum redundancy
3. **Key Rotation**: Rotate keys periodically
4. **Backup Keys**: Store private keys securely (not in Git!)
5. **Access Control**: Limit who has access to the Kubernetes Secret
6. **Key Management**: Use a key management system for production
7. **AGE for Simplicity**: Consider AGE for new projects (simpler than GPG)
8. **GPG for Compatibility**: Use GPG if you need compatibility with existing tools

## Troubleshooting

### Decryption Fails

**Error:** `Failed to decrypt SOPS file`

**Solutions:**

#### For GPG Keys

1. Verify GPG key matches the encryption key:
   ```bash
   kubectl get secret sops-gpg-key -n octopilot-system -o jsonpath='{.data.private\.key}' | base64 -d | gpg --import
   ```
2. Check key fingerprint matches `.sops.yaml`:
   ```bash
   gpg --list-keys --fingerprint
   ```
3. Verify the secret name and namespace are correct

#### For AGE Keys

1. Verify AGE key format:
   ```bash
   kubectl get secret sops-age-key -n octopilot-system -o jsonpath='{.data.private\.key}' | base64 -d
   ```
   Should start with: `AGE-SECRET-KEY-1`
2. Check public key matches `.sops.yaml`:
   ```bash
   # Extract public key from private key
   age-keygen -y < age-key.txt
   ```
3. Verify the secret name and namespace are correct

### Key Not Found

**Error:** `GPG key secret not found` or `AGE key secret not found`

**Solutions:**
1. Verify the secret exists:
   ```bash
   # For GPG
   kubectl get secret sops-gpg-key -n octopilot-system
   
   # For AGE
   kubectl get secret sops-age-key -n octopilot-system
   ```
2. Check the `gpgSecretRef` or `ageSecretRef` in SecretManagerConfig
3. Ensure the namespace matches the SecretManagerConfig namespace
4. Verify the secret is in the correct namespace (same as SecretManagerConfig)

### Invalid Key Format

**Error:** `Invalid GPG key format` or `Invalid AGE key format`

**Solutions:**

#### For GPG Keys

1. Verify the key is in ASCII-armored format:
   ```bash
   kubectl get secret sops-gpg-key -n octopilot-system -o jsonpath='{.data.private\.key}' | base64 -d | head -1
   ```
   Should show: `-----BEGIN PGP PRIVATE KEY BLOCK-----`
2. Re-export the key if needed:
   ```bash
   gpg --armor --export-secret-keys your-email@example.com
   ```

#### For AGE Keys

1. Verify the key format:
   ```bash
   kubectl get secret sops-age-key -n octopilot-system -o jsonpath='{.data.private\.key}' | base64 -d | head -1
   ```
   Should start with: `AGE-SECRET-KEY-1`
2. Regenerate the key if needed:
   ```bash
   age-keygen -o age-key.txt
   ```

## GPG vs AGE Comparison

| Feature | GPG | AGE |
|---------|-----|-----|
| **Complexity** | More complex | Simpler |
| **Key Size** | Larger (4096-bit RSA) | Smaller (128-bit) |
| **Performance** | Slower | Faster |
| **Compatibility** | Widely supported | Modern tooling |
| **Key Management** | More complex | Simpler |
| **Recommended For** | Existing projects, compatibility | New projects, simplicity |

**Recommendation:**
- **New projects**: Use AGE for simplicity
- **Existing projects**: Use GPG for compatibility
- **Production**: Use both for redundancy

## Next Steps

- [Application Files Guide](./application-files.md) - Learn about file formats
- [GitOps Integration](./gitops-integration.md) - Set up GitOps workflow
- [Quick Start](../getting-started/quick-start.md) - Get started quickly
- [Configuration](../getting-started/configuration.md) - Complete configuration guide
