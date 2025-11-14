# Kustomize Integration Analysis

## Problem Statement

The secret-manager-controller currently reads raw `application.secrets.env` and `application.secrets.yaml` files directly from the GitRepository artifact. However, if users employ kustomize overlays/patches to modify secrets, these modifications would be missed.

## Current Architecture

```
GitRepository (Flux) → SourceController → Artifact Cache (/tmp/flux-source-*)
                                         ↓
                    SecretManagerConfig (CRD)
                                         ↓
                    Secret Manager Controller
                    ├─ Reads raw application.secrets.env
                    ├─ Reads raw application.secrets.yaml
                    └─ Parses and syncs to GCP Secret Manager
```

**Current Flow:**
1. Controller gets GitRepository artifact path from SourceController
2. Reads raw `application.secrets.env` files directly from artifact
3. Parses and syncs to GCP Secret Manager

**Problem:** If kustomize overlays modify secrets, we miss those changes.

## FluxCD Kustomize Controller Architecture

FluxCD's kustomize-controller:
1. Watches `Kustomization` CRDs
2. References a `GitRepository` or `Bucket` source
3. Runs `kustomize build` on the specified path
4. Applies the resulting Kubernetes manifests
5. Stores the build output in an artifact

**Kustomization CRD Structure:**
```yaml
apiVersion: kustomize.toolkit.fluxcd.io/v1
kind: Kustomization
metadata:
  name: my-app
  namespace: flux-system
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
  path: ./apps/my-app
  # ... other fields
status:
  artifact:
    path: /tmp/flux-kustomize-<namespace>-<name>-<revision>
    url: http://source-controller.flux-system.svc.cluster.local/...
```

## Options for Integration

### Option 1: Consume Kustomization Artifact (Recommended)

**Approach:** Reference a FluxCD `Kustomization` resource instead of (or in addition to) `GitRepository`.

**Benefits:**
- ✅ Gets the final kustomize build output (includes overlays/patches)
- ✅ Matches what kustomize-controller actually applies
- ✅ Supports all kustomize features (generators, transformers, patches)

**Implementation:**
1. Add optional `kustomization` reference to `SecretManagerConfig` spec
2. Read artifact from `Kustomization.status.artifact.path`
3. Parse the generated Kubernetes Secret manifests (not raw files)
4. Extract secret data from the Secret resources

**Challenges:**
- Need to parse Kubernetes Secret YAML instead of raw env files
- More complex - need to handle multiple Secret resources
- Requires watching Kustomization CRDs

### Option 2: Run Kustomize Build Ourselves

**Approach:** Run `kustomize build` ourselves on the same path as the Kustomization.

**Benefits:**
- ✅ Get the same output as kustomize-controller
- ✅ Still consume raw files (but after kustomize processing)

**Implementation:**
1. Reference the same `GitRepository` and `path` as Kustomization
2. Run `kustomize build` on that path
3. Parse the output to extract Secret resources
4. Extract secret data from Secrets

**Challenges:**
- Need to embed/execute `kustomize` binary
- Must handle SOPS decryption before kustomize runs
- More complex execution model
- Potential for drift if kustomize version differs

### Option 3: Hybrid Approach

**Approach:** Support both raw files and Kustomization artifacts.

**Benefits:**
- ✅ Backward compatible (raw files still work)
- ✅ Advanced users can use Kustomization for overlays
- ✅ Flexible - users choose their approach

**Implementation:**
1. Add optional `kustomization` reference to `SecretManagerConfig` spec
2. If `kustomization` is specified, use Option 1
3. If only `gitRepository` is specified, use current approach (raw files)
4. Document when to use each approach

## Recommended Solution: Option 3 (Hybrid)

### CRD Changes

```rust
pub struct SecretManagerConfigSpec {
    /// GitRepository reference (Flux CRD) - required if kustomization not specified
    pub git_repository: Option<GitRepositoryRef>,
    
    /// Kustomization reference (Flux CD) - optional, takes precedence over git_repository
    /// If specified, controller will consume the kustomize build output artifact
    pub kustomization: Option<KustomizationRef>,
    
    // ... other fields
}

pub struct KustomizationRef {
    pub name: String,
    pub namespace: String,
}
```

### Implementation Steps

1. **Add Kustomization CRD watching** to ClusterRole
2. **Update reconciler** to:
   - Check if `kustomization` is specified
   - If yes: Get Kustomization artifact path, parse Secret manifests
   - If no: Use current GitRepository artifact path, parse raw files
3. **Add Secret manifest parser** to extract data from Kubernetes Secret YAML
4. **Update documentation** explaining both approaches

### Parsing Kustomize Output

When consuming Kustomization artifacts, we need to:
1. Read the artifact (contains `kustomize build` output)
2. Parse YAML stream (multiple resources)
3. Filter for `kind: Secret` resources
4. Extract `data` field (base64 encoded)
5. Decode and parse as key-value pairs

**Example Secret from kustomize output:**
```yaml
apiVersion: v1
kind: Secret
metadata:
  name: idam-secrets
  namespace: pricewhisperer
type: Opaque
data:
  SUPABASE_ANON_KEY: <base64>
  JWT_SECRET: <base64>
```

## Questions to Answer

1. **Does kustomize-controller store the build output artifact?**
   - Check: `Kustomization.status.artifact.path`
   - Verify: Is this accessible to other controllers?

2. **How does kustomize handle SOPS-encrypted files?**
   - Does it decrypt before processing?
   - Or does it need SOPS decryption plugins?

3. **What's the artifact format?**
   - Single YAML file with `---` separators?
   - Directory structure?
   - Tarball?

4. **Performance considerations:**
   - Is artifact path accessible from controller pod?
   - Same filesystem or HTTP endpoint?

## Next Steps

1. **Examine kustomize-controller code:**
   - How it processes generators
   - Where it stores artifacts
   - Artifact format/structure

2. **Examine source-controller code:**
   - How artifacts are served
   - Access patterns for other controllers

3. **Prototype Option 3:**
   - Add Kustomization reference to CRD
   - Implement artifact reading
   - Implement Secret manifest parsing
   - Test with example Kustomization

4. **Documentation:**
   - When to use raw files vs Kustomization
   - Examples for both approaches
   - Migration guide

## Current State

**Current Implementation:** Option 0 (Raw Files Only)
- ✅ Simple and straightforward
- ✅ Works for basic use cases
- ❌ Misses kustomize overlays/patches
- ❌ Doesn't match what kustomize-controller applies

**Recommended:** Option 3 (Hybrid)
- ✅ Backward compatible
- ✅ Supports advanced kustomize features
- ✅ Matches what kustomize-controller applies
- ⚠️ More complex implementation

