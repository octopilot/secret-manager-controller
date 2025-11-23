# Components

Detailed documentation of the Secret Manager Controller components.

## Controller Components

### 1. SecretManagerConfig CRD

The Custom Resource Definition that defines the desired state.

**Location:** `crates/controller/src/crd/`

**Key Fields:**
- `sourceRef`: Reference to GitRepository or Application
- `provider`: Cloud provider configuration
- `secrets`: Secret sync configuration
- `configs`: Config store configuration (optional)
- `otel`: OpenTelemetry configuration (optional)

**Status Fields:**
- `phase`: Current phase (Pending, Syncing, Synced, Error)
- `description`: Human-readable status message
- `conditions`: Kubernetes conditions array
- `lastSyncTime`: Timestamp of last successful sync
- `secretsCount`: Number of secrets managed

### 2. Reconciliation Loop

The core controller logic that watches and reconciles resources.

**Location:** `crates/controller/src/controller/`

**Responsibilities:**
- Watches SecretManagerConfig resources
- Polls GitOps artifacts for changes
- Coordinates secret extraction and sync
- Updates status and conditions

**Key Functions:**
- `reconcile()`: Main reconciliation logic
- `sync_secrets()`: Syncs secrets to cloud provider
- `extract_secrets()`: Extracts secrets from Git artifacts

### 3. GitOps Integration

Handles integration with FluxCD and ArgoCD.

**Location:** `crates/controller/src/gitops/`

**FluxCD Support:**
- Reads artifacts from `/tmp/flux-source-*` directories
- Watches GitRepository CRD
- Uses source-controller artifacts

**ArgoCD Support:**
- Reads from Application CRD
- Direct Git repository access
- Clones repository to temporary directory

### 4. SOPS Decryption

Decrypts SOPS-encrypted secret files.

**Location:** `crates/controller/src/sops/`

**Features:**
- GPG key management from Kubernetes Secrets
- File-level and value-level decryption
- Multiple encryption method support
- Error handling and logging

**Process:**
1. Read GPG key from Kubernetes Secret
2. Detect SOPS-encrypted files
3. Decrypt using GPG key
4. Return decrypted content

### 5. Kustomize Builder

Builds Kustomize overlays and extracts secrets.

**Location:** `crates/controller/src/kustomize/`

**Process:**
1. Run `kustomize build` on specified path
2. Parse generated YAML
3. Extract Kubernetes Secret resources
4. Return secret data

**Features:**
- Supports overlays and patches
- Handles generators
- Error handling for invalid Kustomize configs

### 6. Provider Clients

Cloud provider-specific clients.

**Location:** `crates/controller/src/providers/`

#### GCP Secret Manager Client

**Location:** `crates/controller/src/providers/gcp/`

**Features:**
- Workload Identity support
- Service account key fallback
- Secret versioning
- Batch operations

#### AWS Secrets Manager Client

**Location:** `crates/controller/src/providers/aws/`

**Features:**
- IRSA (IAM Roles for Service Accounts) support
- Access key fallback
- Secret versioning
- Tag support

#### Azure Key Vault Client

**Location:** `crates/controller/src/providers/azure/`

**Features:**
- Workload Identity support
- Service principal fallback
- Secret versioning
- Key Vault access policies

### 7. Config Store Integration

Routes `application.properties` to config stores.

**Location:** `crates/controller/src/config_store/`

**Supported Stores:**
- **AWS Parameter Store**: SSM Parameter Store
- **Azure App Configuration**: App Configuration service
- **GCP Parameter Manager**: (Future, via ESO contribution)

**Process:**
1. Detect `application.properties` files
2. Parse properties
3. Route to appropriate config store
4. Store as individual parameters

### 8. HTTP Server

Provides metrics, health checks, and probes.

**Location:** `crates/controller/src/http/`

**Endpoints:**
- `/metrics`: Prometheus metrics
- `/health`: Health check
- `/ready`: Readiness probe
- `/live`: Liveness probe

**Metrics:**
- Reconciliation count
- Secret sync duration
- Error rates
- Provider API calls

### 9. OpenTelemetry Integration

Distributed tracing support.

**Location:** `crates/controller/src/otel/`

**Exporters:**
- OTLP (OpenTelemetry Protocol)
- Datadog direct export

**Traces:**
- Reconciliation operations
- Provider API calls
- SOPS decryption
- Kustomize builds

## Data Structures

### SecretManagerConfigSpec

Main configuration structure.

```rust
pub struct SecretManagerConfigSpec {
    pub source_ref: SourceRef,
    pub provider: ProviderConfig,
    pub secrets: SecretsConfig,
    pub configs: Option<ConfigsConfig>,
    pub otel: Option<OtelConfig>,
    pub git_repository_pull_interval: String,
    pub reconcile_interval: String,
    pub diff_discovery: bool,
}
```

### SourceRef

GitOps source reference.

```rust
pub struct SourceRef {
    pub kind: String,  // "GitRepository" or "Application"
    pub name: String,
    pub namespace: String,
}
```

### ProviderConfig

Cloud provider configuration.

```rust
pub enum ProviderConfig {
    Gcp { project_id: String },
    Aws { region: String },
    Azure { vault_url: String },
}
```

### SecretsConfig

Secret sync configuration.

```rust
pub struct SecretsConfig {
    pub environment: String,
    pub kustomize_path: String,
    pub sops: Option<SopsConfig>,
}
```

## Error Handling

All components use structured error handling:

- **Result Types**: All operations return `Result<T, E>`
- **Error Propagation**: Errors bubble up with context
- **Logging**: Errors are logged with full context
- **Status Updates**: Errors are reflected in CRD status

## Testing

Components are tested at multiple levels:

- **Unit Tests**: Individual function tests
- **Integration Tests**: Component interaction tests
- **Pact Tests**: Provider API contract tests
- **E2E Tests**: Full workflow tests in Kind cluster

## Next Steps

- [Architecture Overview](./overview.md) - High-level architecture
- [API Reference](../api-reference/crd-reference.md) - CRD reference
- [Development Guide](../../contributor/development/setup.md) - Contributing
