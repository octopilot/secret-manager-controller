# Implementation Details

Detailed implementation decisions and rationale for key architectural components of the Secret Manager Controller.

## Source Handling Architecture

The controller handles GitOps sources differently depending on the GitOps tool (FluxCD vs ArgoCD). This section explains the rationale behind these design decisions.

### FluxCD: Source-Controller Artifact Paths

The controller uses FluxCD's source-controller artifact paths for FluxCD `GitRepository` resources.

#### Why Source-Controller Artifact Paths?

**FluxCD's source-controller provides:**

1. **Artifact Caching**: Clones Git repositories and caches artifacts as tar.gz files
2. **HTTP Artifact Server**: Exposes artifacts via HTTP endpoints at predictable URLs
3. **Git Credential Handling**: Manages Git authentication automatically via Kubernetes secrets
4. **Artifact Validation**: Validates and checksums artifacts before serving

**Benefits of Using Artifact Paths:**

- **Reuses Caching**: Leverages source-controller's existing artifact caching mechanism
- **Avoids Duplicate Clones**: No need to clone repositories ourselves (source-controller already did it)
- **Credential Management**: Uses source-controller's Git credential handling (no need to manage credentials ourselves)
- **Performance**: Artifacts are pre-cloned, validated, and cached by source-controller
- **Consistency**: Uses the same artifacts that FluxCD uses for GitOps operations

#### Implementation

The controller downloads artifacts from source-controller's HTTP service:

```rust
// Artifact URL format
let artifact_url = format!(
    "http://source-controller.{}.svc.cluster.local/gitrepository/{}/{}/latest.tar.gz",
    namespace, namespace, name
);

// Download and extract artifact
let artifact_path = download_and_extract_artifact(&artifact_url).await?;
```

**Artifact Path Structure:**

Artifacts are downloaded and extracted to:
```
/tmp/smc/flux-artifact/{namespace}/{name}/{branch}-sha-{short_sha}/
```

**Example:**
```
/tmp/smc/flux-artifact/flux-system/my-repo/main-sha-7680da4/
```

**Rationale for Hierarchical Structure:**

1. **Namespace Isolation**: Separates artifacts by namespace to avoid conflicts
2. **Name Isolation**: Separates artifacts by GitRepository name
3. **Revision Tracking**: Uses branch name and short SHA (7 characters) to track specific revisions
4. **Cache Invalidation**: Hash-based directory names allow easy cache invalidation
5. **Performance**: Avoids performance issues from having many files in a single directory
6. **PVC Support**: Allows cluster owners to mount a PVC at `/tmp/smc` for persistent storage

**Revision Format Handling:**

FluxCD revision format: `"main@sha1:7680da431ea59ae7d3f4fdbb903a0f4509da9078"`

The controller:
- Extracts branch name (before `@`)
- Extracts SHA (after `sha1:` or `sha256:`)
- Uses short SHA (7 characters) for directory name
- Sanitizes branch names for filesystem safety

This ensures:
- Same SHA on different branches doesn't conflict
- Long branch names are handled safely
- Directory names are filesystem-safe

#### Git Credential Handling

**FluxCD Approach:**
- Credentials are managed by source-controller via `GitRepository.spec.secretRef`
- Controller doesn't need to handle credentials (source-controller does it)
- Artifacts are already authenticated and available via HTTP

**Benefits:**
- No credential management in controller code
- Reuses source-controller's credential handling
- Credentials are managed at the GitRepository level (standard FluxCD pattern)

---

### ArgoCD: Direct Git Cloning

The controller clones Git repositories directly for ArgoCD `Application` resources.

#### Why Direct Cloning?

**ArgoCD doesn't provide artifact paths:**

1. **No Artifact Server**: ArgoCD doesn't expose artifacts via HTTP like FluxCD's source-controller
2. **Application Spec**: ArgoCD Applications reference Git repos but don't provide artifact URLs
3. **Different Architecture**: ArgoCD uses a different architecture (Application CRD vs GitRepository CRD)

**Benefits of Direct Cloning:**

- **Works with Any Git Source**: Not limited to ArgoCD-managed repos
- **Full Control**: Complete control over clone process, caching, and credential handling
- **Private Repo Support**: Handles private repos via Kubernetes secrets specified in CRD
- **Flexibility**: Supports both HTTPS and SSH authentication

#### Implementation

The controller extracts Git source from ArgoCD Application and clones directly:

```rust
// Extract Git source from Application spec
let application = api.get(&source_ref.name).await?;
let repo_url = extract_repo_url(&application)?;
let target_revision = extract_target_revision(&application)?;

// Load git credentials if specified
let git_credentials = if let Some(ref git_creds_ref) = source_ref.git_credentials {
    load_git_credentials(reconciler, git_creds_ref, &source_ref.namespace).await?
} else {
    None
};

// Clone repository
let repo_path = clone_repository(&repo_url, &target_revision, &git_credentials).await?;
```

**Cache Path Structure:**

Repositories are cloned to:
```
/tmp/smc/argocd-repo/{namespace}/{name}/{hash}/
```

**Example:**
```
/tmp/smc/argocd-repo/argocd/my-app/a1b2c3d4e5f6g7h8/
```

**Rationale for Hierarchical Structure:**

1. **Namespace Isolation**: Separates repositories by namespace
2. **Name Isolation**: Separates repositories by Application name
3. **Hash-based Revision**: Uses MD5 hash of `{namespace}-{name}-{revision}` for cache directory
4. **Safe Revision Handling**: Hash handles long branch names, tags, and SHAs safely
5. **Performance**: Avoids performance issues from flat directory structures
6. **PVC Support**: Allows cluster owners to mount a PVC at `/tmp/smc` for persistent storage

**Why Hash Instead of Branch Name?**

- **Long Branch Names**: Some branch names are too long for filesystem paths
- **Special Characters**: Branch names may contain special characters that aren't filesystem-safe
- **Tags and SHAs**: Supports tags and SHAs which may have various formats
- **Collision Avoidance**: Hash ensures unique directory names even with similar names

#### Git Credential Handling

**ArgoCD Approach:**
- Credentials are specified in `SecretManagerConfig.spec.sourceRef.gitCredentials`
- Controller loads credentials from Kubernetes secrets
- Supports multiple credential types:
  - **HTTPS**: `username` and `password` (or `token`) keys
  - **SSH**: `identity` key containing SSH private key
  - **GitHub Token**: Special handling for GitHub tokens (`ghp_`, `github_pat_`, `gho_` prefixes)

**Credential Loading:**

```rust
// Load credentials from Kubernetes secret
let secret = secrets.get(&git_credentials_ref.name).await?;

// Check for SSH key first
if let Some(identity) = secret.data.get("identity") {
    // Use SSH authentication
}

// Check for GitHub token
if let Some(token) = secret.data.get("token") {
    if token.starts_with("ghp_") {
        // Use GitHub token authentication
    }
}

// Check for HTTPS credentials
if let (Some(username), Some(password)) = (username, password) {
    // Use HTTPS authentication
}
```

**Differences from FluxCD:**

| Aspect | FluxCD | ArgoCD |
|--------|--------|--------|
| **Credential Management** | Handled by source-controller | Handled by controller |
| **Credential Location** | `GitRepository.spec.secretRef` | `SecretManagerConfig.spec.sourceRef.gitCredentials` |
| **Credential Types** | Managed by source-controller | Controller handles HTTPS, SSH, GitHub tokens |
| **Public Repos** | Works automatically | Works automatically (no credentials needed) |

**Why Different Approaches?**

- **FluxCD**: Source-controller already handles credentials, so we reuse that
- **ArgoCD**: No equivalent service, so we handle credentials ourselves
- **Consistency**: Each approach uses the standard pattern for its GitOps tool

---

### Hierarchical Cache Structure Rationale

Both FluxCD and ArgoCD use hierarchical cache structures under `/tmp/smc/`:

```
/tmp/smc/
├── flux-artifact/
│   └── {namespace}/
│       └── {name}/
│           └── {branch}-sha-{short_sha}/
└── argocd-repo/
    └── {namespace}/
        └── {name}/
            └── {hash}/
```

#### Why Hierarchical?

**1. Performance**

Flat directory structures (all files in one directory) cause performance issues:
- Filesystem operations become slow with many entries
- Directory listing is slow
- File system inodes can be exhausted

Hierarchical structure:
- Limits files per directory
- Faster directory operations
- Better filesystem performance

**2. Organization**

- **Namespace isolation**: Prevents conflicts between namespaces
- **Resource isolation**: Each GitRepository/Application has its own directory
- **Revision tracking**: Clear organization by revision/branch

**3. PVC Support**

- **Mount point**: Cluster owners can mount a PVC at `/tmp/smc`
- **Persistent storage**: Cache survives pod restarts
- **Shared storage**: Can be shared across controller replicas (if needed)

**4. Cleanup**

- **Per-resource cleanup**: Easy to clean up old revisions per resource
- **Namespace-level cleanup**: Can clean up entire namespaces
- **Age-based cleanup**: Uses filesystem modification time (mtime) for cleanup decisions

**5. Debugging**

- **Clear paths**: Easy to identify which resource a cache directory belongs to
- **Predictable structure**: Developers can easily find cache directories
- **Logging**: Cache paths are logged for troubleshooting

---

### Git Credential Handling Differences

#### FluxCD: Source-Controller Managed

**How it works:**
1. User creates `GitRepository` with `spec.secretRef` pointing to a Kubernetes secret
2. Source-controller loads credentials from the secret
3. Source-controller clones repository using those credentials
4. Controller downloads pre-authenticated artifacts via HTTP

**Benefits:**
- No credential handling in controller code
- Reuses source-controller's credential management
- Credentials managed at GitRepository level (standard FluxCD pattern)

**Limitations:**
- Only works with FluxCD GitRepository resources
- Credentials must be in the GitRepository namespace
- Limited to credential types supported by source-controller

#### ArgoCD: Controller Managed

**How it works:**
1. User creates `SecretManagerConfig` with `spec.sourceRef.gitCredentials` pointing to a Kubernetes secret
2. Controller loads credentials from the secret
3. Controller clones repository using those credentials
4. Controller manages the entire cloning process

**Benefits:**
- Works with any Git source (not just ArgoCD)
- Supports multiple credential types (HTTPS, SSH, GitHub tokens)
- Full control over authentication process
- Credentials can be in any namespace

**Credential Types Supported:**

1. **HTTPS**: `username` and `password` (or `token`) keys
2. **SSH**: `identity` key containing SSH private key
3. **GitHub Token**: Special handling for GitHub personal access tokens

**Example Secret:**

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: git-credentials
  namespace: my-namespace
type: Opaque
stringData:
  username: git
  password: <token-or-password>
  # OR for SSH:
  identity: |
    -----BEGIN OPENSSH PRIVATE KEY-----
    ...
    -----END OPENSSH PRIVATE KEY-----
```

---

## Pact Testing Architecture

The Pact testing infrastructure uses a combined deployment approach with all components in a single pod. This section explains the rationale behind this design.

### Why Combined Deployment?

All Pact infrastructure components are deployed in a single `pact-infrastructure` pod:

- **Pact Broker**: Main service (port 9292)
- **Manager Sidecar**: Contract publishing and ConfigMap watching (port 1238)
- **Mock Servers**: AWS (port 1234), GCP (port 1235), Azure (port 1236)
- **Mock Webhook**: Webhook receiver (port 1237)

#### Benefits

**1. Reduced Startup Time**

- **Single pod**: Only need to wait for one deployment instead of five
- **Faster CI/CD**: CI pipeline completes faster (fewer resources to wait for)
- **Faster Tilt startup**: Local development iteration is faster

**Before (5 separate deployments):**
```
pact-broker deployment → wait
aws-mock-server deployment → wait
gcp-mock-server deployment → wait
azure-mock-server deployment → wait
mock-webhook deployment → wait
Total: 5 deployments to wait for
```

**After (1 combined deployment):**
```
pact-infrastructure deployment → wait
Total: 1 deployment to wait for
```

**2. Simplified Orchestration**

- **One resource**: Single deployment to manage
- **Single Service**: All components accessible via one service
- **Unified health checks**: All components in one pod for health monitoring
- **Simpler dependencies**: Fewer resource dependencies in Tilt

**3. Better Resource Utilization**

- **Shared volumes**: All components share the same volumes (pacts storage, ConfigMap)
- **Shared networking**: Components communicate via `localhost` (no network overhead)
- **Reduced overhead**: Single pod overhead instead of five

**4. Faster Tilt Startup**

- **One deployment**: Tilt only needs to wait for one deployment
- **Faster iteration**: Local development cycle is faster
- **Simplified resource dependencies**: Clear dependency chain

#### Trade-offs

**Single Point of Failure:**
- If the pod crashes, all components go down
- **Acceptable for testing**: Testing infrastructure doesn't need high availability
- **Restart is fast**: Pod restart is quick (all components start together)

**Resource Limits:**
- All components share pod resource limits
- **Sufficient for testing**: Testing workloads don't need high resources
- **Can be tuned**: Resource limits can be adjusted if needed

---

### Component Packaging Decisions

#### Mock Servers: Single Image

All three mock servers (AWS, GCP, Azure) are packaged in a single `pact-mock-server` image.

**Structure:**
```
pact-mock-server image:
├── aws-mock-server binary (port 1234)
├── gcp-mock-server binary (port 1235)
└── azure-mock-server binary (port 1236)
```

**Rationale:**

1. **Shared Base Image**: All servers use the same Rust/Axum stack, reducing build time
2. **Single Image**: Simplifies deployment (one image to build and push)
3. **Consistent Environment**: All servers run in the same environment
4. **Easier Maintenance**: One Dockerfile, one build process

**Implementation:**
- Each server is a separate Rust binary
- All binaries are included in the same image
- Each server runs as a separate container in the pod
- Ports are assigned uniquely (1234, 1235, 1236)

**Alternative Considered:**
- **Separate images**: Would require 3 separate builds and deployments
- **Rejected**: More complex, slower builds, more images to manage

#### Manager Sidecar

The manager is a Rust binary sidecar container that handles contract publishing.

**Responsibilities:**
1. **Broker Monitoring**: Watches for Pact broker to be ready
2. **Contract Publishing**: Publishes contracts from ConfigMap to broker
3. **ConfigMap Watching**: Watches for ConfigMap changes and re-publishes contracts
4. **Health Endpoints**: Provides `/ready`, `/liveness`, `/readiness` endpoints
5. **Signal to Mock Servers**: Writes flag file when contracts are published

**Why Sidecar Instead of Init Container?**

**Init Container Limitations:**
- **One-time execution**: Runs once at pod startup
- **No dynamic updates**: Can't handle ConfigMap changes
- **No health monitoring**: No way to check if contracts are published
- **Less reliable**: If init fails, pod fails to start

**Sidecar Benefits:**
- **Dynamic Updates**: Can handle ConfigMap changes without pod restart
- **Health Monitoring**: Provides health endpoints for Kubernetes probes
- **Better Reliability**: More robust than one-time init container execution
- **Continuous Operation**: Runs continuously, can re-publish contracts as needed

**Implementation:**
```rust
// Manager watches broker and publishes contracts
loop {
    // Wait for broker to be ready
    wait_for_broker_ready().await?;
    
    // Publish contracts from ConfigMap
    publish_contracts_from_configmap().await?;
    
    // Write flag file for mock servers
    write_published_flag().await?;
    
    // Watch for ConfigMap changes
    watch_configmap().await?;
}
```

#### Mock Webhook: Separate Image

The mock webhook is a separate image from the mock servers.

**Rationale:**
- **Different Purpose**: Webhook receiver vs API mocks (different functionality)
- **Different Codebase**: Webhook is a separate Rust binary
- **Clarity**: Separating concerns makes the architecture clearer
- **Independent Updates**: Can update webhook without rebuilding mock servers

**Note:** Could be combined with mock servers, but kept separate for clarity.

---

### Tilt Integration Rationale

The Pact infrastructure is integrated into the Tilt development environment.

#### Why Tilt?

**1. Fast Iteration**
- **Live Updates**: Code changes are reflected immediately
- **Hot Reload**: Containers restart automatically on code changes
- **Fast Feedback**: See changes in seconds, not minutes

**2. Unified Development Environment**
- **All Components**: Controller, Pact infrastructure, and dependencies in one environment
- **Consistent Setup**: Same setup for all developers
- **Easy Onboarding**: New developers can start quickly

**3. Resource Organization**
- **Labels**: Resources organized by labels (`controllers`, `infrastructure`, `pact`)
- **Parallel Streams**: Tilt shows separate streams for different resource types
- **Dependency Management**: Clear `resource_deps` ensure correct ordering

#### Tilt Configuration

**Resource Dependencies:**
```python
k8s_resource(
    'pact-infrastructure',
    labels=['pact'],
    resource_deps=['populate-pact-configmap'],  # Wait for ConfigMap
    port_forwards=[
        '9292:9292',  # Pact broker
        '1234:1234',  # AWS mock server
        '1235:1235',  # GCP mock server
        '1236:1236',  # Azure mock server
        '1237:1237',  # Mock webhook
        '1238:1238',  # Manager health endpoint
    ],
)
```

**Build Strategy:**
- **Cross-compilation**: Builds Rust binaries for Linux (musl target)
- **Image Building**: Builds Docker images with compiled binaries
- **Live Updates**: Syncs binaries and restarts containers on changes

**Port Forwarding:**
- **Local Access**: All services accessible on localhost
- **Testing**: Tests can connect to services via port forwarding
- **Development**: Developers can interact with services directly

#### Benefits for Development

**1. Fast Feedback Loop**
- Code changes → Build → Deploy → Test (all in Tilt)
- No manual steps required
- See results in seconds

**2. Isolated Environment**
- Each developer has their own Kind cluster
- No conflicts between developers
- Can test changes without affecting others

**3. Easy Debugging**
- All logs in one place (Tilt UI)
- Port forwarding for direct access
- Can inspect services directly

**4. Consistent CI/CD**
- `tilt ci` mode mirrors local development
- Same setup in CI as local
- Reduces "works on my machine" issues

---

## Summary

### Source Handling

- **FluxCD**: Uses source-controller artifact paths (reuses caching, avoids duplicate clones)
- **ArgoCD**: Direct Git cloning (no artifact paths available)
- **Hierarchical Cache**: `/tmp/smc/{type}/{namespace}/{name}/{revision}/` structure
- **Credential Handling**: FluxCD uses source-controller, ArgoCD uses controller-managed secrets

### Pact Architecture

- **Combined Deployment**: Single `pact-infrastructure` pod with all components
- **Component Packaging**: Mock servers in single image, manager as sidecar, webhook separate
- **Tilt Integration**: Fast iteration, unified environment, clear dependencies

### Design Principles

1. **Reuse Existing Infrastructure**: Leverage FluxCD source-controller when available
2. **Performance**: Hierarchical cache structures for better filesystem performance
3. **Simplicity**: Combined deployments reduce complexity
4. **Developer Experience**: Tilt integration for fast iteration
5. **Flexibility**: Support multiple GitOps tools and credential types

For detailed diagrams and sequence flows, see [Pact Testing Architecture](../testing/pact-testing/architecture.md).

