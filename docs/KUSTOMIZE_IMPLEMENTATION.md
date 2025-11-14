# Kustomize Build Mode Implementation

## Overview

Implemented Option 2: Running `kustomize build` ourselves. This makes the controller GitOps-agnostic and works with FluxCD, ArgoCD, or any GitOps tool.

## Implementation Details

### CRD Changes

Added `kustomizePath` field to `SecretManagerConfigSpec`:
- Optional field - if specified, uses kustomize build mode
- If not specified, falls back to raw file mode (backward compatible)
- Path is relative to GitRepository root

### New Module: `kustomize.rs`

**Functions:**
1. `extract_secrets_from_kustomize()` - Runs `kustomize build` and extracts secrets from Secret resources
2. `extract_properties_from_kustomize()` - Extracts properties from ConfigMap resources (for future use)
3. `parse_kustomize_output()` - Parses YAML stream and extracts Secret data

**Process:**
1. Construct path to `kustomization.yaml`
2. Run `kustomize build` command
3. Parse YAML stream output (multiple resources separated by `---`)
4. Filter for `kind: Secret` resources
5. Extract `data` field (base64 encoded)
6. Decode base64 and convert to key-value pairs

### Reconciler Updates

**Logic Flow:**
```
if kustomize_path is specified:
    → Run kustomize build
    → Extract secrets from Secret resources
    → Sync to GCP Secret Manager
else:
    → Use raw file mode (existing logic)
    → Read application.secrets.env directly
    → Sync to GCP Secret Manager
```

### Benefits

1. **GitOps-Agnostic**: Works with FluxCD, ArgoCD, or any GitOps tool
2. **Supports Kustomize Features**: Overlays, patches, generators all work
3. **Backward Compatible**: Raw file mode still works
4. **Matches What Gets Applied**: Same output as kustomize-controller

### Requirements

1. **kustomize Binary**: Must be available in controller container
   - Version: v5.0+ recommended
   - Can be added via init container or base image

2. **kustomization.yaml**: Must exist at specified path
   - Must contain `secretGenerator` configuration
   - Can use overlays/patches/generators

### Example Usage

**Kustomize Build Mode:**
```yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: idam-dev-secrets
spec:
  gitRepository:
    name: pricewhisperer-manifests
    namespace: flux-system
  gcpProjectId: pricewhisperer-dev
  environment: dev
  kustomizePath: microservices/idam/deployment-configuration/profiles/dev
  secretPrefix: idam-dev
```

**Raw File Mode (backward compatible):**
```yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: idam-dev-secrets
spec:
  gitRepository:
    name: pricewhisperer-manifests
    namespace: flux-system
  gcpProjectId: pricewhisperer-dev
  environment: dev
  basePath: microservices
  secretPrefix: idam-dev
```

## Docker Image Requirements

The controller Docker image must include the `kustomize` binary. Options:

1. **Multi-stage build**: Copy kustomize binary from official image
2. **Base image**: Use image that includes kustomize
3. **Init container**: Download kustomize at startup

**Example Dockerfile addition:**
```dockerfile
# Install kustomize
RUN curl -s "https://raw.githubusercontent.com/kubernetes-sigs/kustomize/master/hack/install_kustomize.sh" | bash && \
    mv kustomize /usr/local/bin/ && \
    chmod +x /usr/local/bin/kustomize
```

## Testing

1. **Test kustomize build mode:**
   - Create SecretManagerConfig with `kustomizePath`
   - Verify kustomize build runs successfully
   - Verify secrets extracted correctly
   - Verify secrets synced to GCP

2. **Test raw file mode:**
   - Create SecretManagerConfig without `kustomizePath`
   - Verify raw file reading still works
   - Verify backward compatibility

3. **Test with overlays:**
   - Create base kustomization.yaml
   - Create overlay with patches
   - Verify overlayed secrets are extracted

## SOPS Decryption

**Important:** Kustomize needs SOPS-encrypted files to be decrypted before processing. Options:

1. **Kustomize SOPS Plugin** (Recommended):
   - Install SOPS plugin for kustomize
   - Kustomize will automatically decrypt SOPS-encrypted files
   - Requires `sops` binary and kustomize SOPS plugin

2. **Pre-decrypt Files** (Alternative):
   - Decrypt SOPS files before running kustomize build
   - Use controller's SOPS decryption capability
   - Write decrypted files to temp directory
   - Run kustomize build on temp directory

**Current Implementation:** Assumes kustomize has SOPS plugin support or files are pre-decrypted.

**Future Enhancement:** Add automatic SOPS decryption before kustomize build.

## Future Enhancements

1. **SOPS Integration**: Handle SOPS decryption before kustomize runs (or ensure kustomize SOPS plugin is available)
2. **ConfigMap Support**: Extract properties from ConfigMap resources
3. **Caching**: Cache kustomize build output to reduce execution time
4. **Error Handling**: Better error messages for kustomize failures
5. **Version Pinning**: Support specifying kustomize version
6. **SOPS Plugin Detection**: Check if kustomize SOPS plugin is available

## Files Modified

1. `src/main.rs` - Added `kustomizePath` field to CRD spec
2. `src/kustomize.rs` - **NEW** - Kustomize build execution and parsing
3. `src/reconciler.rs` - Added logic to choose between modes
4. `Cargo.toml` - Added `base64` dependency
5. `examples/idam-dev-kustomize-secret-manager-config.yaml` - **NEW** - Example for kustomize mode
6. `README.md` - Updated documentation
7. `examples/README.md` - Added operation modes section

