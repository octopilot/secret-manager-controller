# CRD Design

Comprehensive guide to the `SecretManagerConfig` Custom Resource Definition structure, field design decisions, versioning strategy, and backward compatibility.

## Overview

The `SecretManagerConfig` CRD is the primary interface for configuring the Secret Manager Controller. It defines how secrets from GitOps repositories are synced to cloud secret managers (GCP, AWS, Azure).

**API Version:** `secret-management.microscaler.io/v1beta1`  
**Kind:** `SecretManagerConfig` (shortname: `smc`)  
**Scope:** Namespaced

## CRD Structure

### High-Level Structure

```yaml
apiVersion: secret-management.microscaler.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: <resource-name>
  namespace: <namespace>
spec:
  sourceRef:          # Required - GitOps source reference
  provider:            # Required - Cloud provider configuration
  secrets:             # Required - Secrets sync configuration
  configs:             # Optional - Config store configuration
  otel:                # Optional - OpenTelemetry configuration
  gitRepositoryPullInterval:  # Optional - Git pull interval (default: "5m")
  reconcileInterval:   # Optional - Reconcile interval (default: "1m")
  diffDiscovery:       # Optional - Enable diff discovery (default: true)
  triggerUpdate:       # Optional - Enable update triggers (default: true)
  suspend:             # Optional - Suspend reconciliation (default: false)
  suspendGitPulls:     # Optional - Suspend Git pulls (default: false)
  notifications:       # Optional - Notification configuration
  logging:             # Optional - Logging configuration
  hotReload:           # Optional - Hot reload configuration
status:
  phase:               # Current reconciliation phase
  description:         # Human-readable status description
  conditions:          # Kubernetes conditions array
  observedGeneration: # Observed generation
  lastReconcileTime:  # Last reconciliation timestamp
  nextReconcileTime:  # Next scheduled reconciliation
  secretsSynced:       # Number of secrets synced
  sync:                # Sync state tracking
  decryptionStatus:    # SOPS decryption status
  # ... additional status fields
```

## Field Design Decisions

### Required Fields

#### `sourceRef` (Required)

**Type:** `SourceRef`  
**Purpose:** Reference to the GitOps source (FluxCD GitRepository or ArgoCD Application)

**Design Rationale:**
- **GitOps-agnostic**: Supports both FluxCD and ArgoCD via `kind` field
- **Namespace isolation**: Each resource can reference sources in different namespaces
- **Optional credentials**: Git credentials are optional (supports public repos)

**Structure:**
```yaml
sourceRef:
  kind: GitRepository  # or "Application" for ArgoCD
  name: my-repo
  namespace: microscaler-system
  gitCredentials:      # Optional - for ArgoCD private repos
    name: git-credentials
    namespace: my-namespace
```

**Default Values:**
- `kind`: Defaults to `"GitRepository"` if not specified

**Design Decisions:**
- **Separate `kind` field**: Allows controller to handle different source types differently
- **Optional `gitCredentials`**: Only needed for ArgoCD private repositories (FluxCD handles credentials via source-controller)
- **Namespace in sourceRef**: Allows referencing sources across namespaces

#### `provider` (Required)

**Type:** `ProviderConfig` (enum: `Gcp`, `Aws`, `Azure`)  
**Purpose:** Cloud provider configuration

**Design Rationale:**
- **Externally tagged enum**: Uses `gcp`, `aws`, `azure` as top-level fields
- **Type safety**: Rust enum ensures only one provider can be specified
- **Provider-specific configs**: Each provider has its own configuration structure

**Structure:**
```yaml
provider:
  gcp:
    projectId: my-gcp-project
  # OR
  aws:
    region: us-east-1
  # OR
  azure:
    vaultUrl: https://my-vault.vault.azure.net/
```

**Design Decisions:**
- **Externally tagged format**: Kubernetes sends `{"gcp": {...}}` or `{"aws": {...}}`
- **"type" field compatibility**: Schema allows `"type"` field for backward compatibility but ignores it during deserialization
- **OneOf validation**: Schema enforces that exactly one provider is specified
- **Provider-specific fields**: Each provider has its own configuration (projectId, region, vaultUrl, etc.)

#### `secrets` (Required)

**Type:** `SecretsConfig`  
**Purpose:** Secrets sync configuration

**Design Rationale:**
- **Environment-based**: Uses `environment` field to select which profile/environment to sync
- **Flexible paths**: Supports both kustomize builds and raw file parsing
- **Naming control**: Prefix and suffix allow customization of secret names

**Structure:**
```yaml
secrets:
  environment: dev                    # Required - environment/profile name
  kustomizePath: path/to/kustomize   # Optional - kustomize build path
  basePath: microservices            # Optional - base path for file search
  prefix: my-service                 # Optional - secret name prefix
  suffix: -prod                      # Optional - secret name suffix
```

**Design Decisions:**
- **Environment field**: Required to select which profile directory to process
- **Optional kustomizePath**: Supports both kustomize builds and raw file parsing
- **Optional basePath**: Allows searching from repository root or subdirectories
- **Optional prefix/suffix**: Matches kustomize-google-secret-manager behavior for familiarity

### Optional Fields

#### `configs` (Optional)

**Type:** `ConfigsConfig`  
**Purpose:** Config store configuration for routing `application.properties` to config stores

**Design Rationale:**
- **Backward compatibility**: Defaults to `false` (disabled) to maintain existing behavior
- **Provider-specific options**: Each provider has different config store options
- **Future-proof**: Designed to support Parameter Manager (GCP) after ESO contribution

**Structure:**
```yaml
configs:
  enabled: true                       # Default: false
  parameterPath: /my-service/dev     # AWS-specific (optional)
  store: SecretManager               # GCP-specific (optional, default: SecretManager)
  appConfigEndpoint: https://...     # Azure-specific (optional)
```

**Design Decisions:**
- **Opt-in feature**: Must explicitly enable to avoid breaking existing deployments
- **Provider-specific fields**: Only apply when provider matches (e.g., `parameterPath` only for AWS)
- **Interim solution**: GCP uses Secret Manager for configs until Parameter Manager support is available

#### `otel` (Optional)

**Type:** `OtelConfig`  
**Purpose:** OpenTelemetry configuration for distributed tracing

**Design Rationale:**
- **Optional feature**: Not all users need distributed tracing
- **Multiple backends**: Supports both OTLP (OpenTelemetry Collector) and Datadog direct export
- **Per-resource configuration**: Allows different tracing configs per resource

**Structure:**
```yaml
otel:
  type: Datadog  # or "Otlp"
  serviceName: secret-manager-controller
  serviceVersion: 1.0.0
  environment: production
  # Datadog-specific:
  site: datadoghq.com
  apiKey: <secret-reference>
```

**Design Decisions:**
- **Tagged enum**: Uses `type` field to distinguish between OTLP and Datadog
- **Optional fields**: Service name, version, environment have defaults
- **Secret references**: API keys should be referenced from secrets (not inlined)

#### `gitRepositoryPullInterval` (Optional)

**Type:** `String` (Kubernetes duration)  
**Default:** `"5m"`  
**Purpose:** How often to check for updates from GitRepository source

**Design Rationale:**
- **Rate limiting**: Longer intervals avoid hitting API rate limits
- **Minimum enforcement**: Controller validates minimum interval (1 minute)
- **Flexible configuration**: Users can tune based on their update frequency needs

**Design Decisions:**
- **Default to 5 minutes**: Balances freshness with rate limit concerns
- **Minimum validation**: Prevents users from setting intervals that cause rate limiting
- **Kubernetes duration format**: Uses standard Kubernetes duration strings (`"1m"`, `"5m"`, `"1h"`)

#### `reconcileInterval` (Optional)

**Type:** `String` (Kubernetes duration)  
**Default:** `"1m"`  
**Purpose:** How often to reconcile secrets between Git and cloud providers

**Design Rationale:**
- **Separate from Git pulls**: Reconciliation can happen more frequently than Git pulls
- **Drift detection**: Frequent reconciliation detects tampering faster
- **Resource efficiency**: Users can tune based on their needs

**Design Decisions:**
- **Default to 1 minute**: Frequent enough to detect drift, not too frequent to waste resources
- **Independent of Git pulls**: Reconciliation uses cached Git state, doesn't require new pull

#### `diffDiscovery` (Optional)

**Type:** `bool`  
**Default:** `true`  
**Purpose:** Enable detection of secrets tampered with in cloud providers

**Design Rationale:**
- **Security feature**: Detects unauthorized changes to secrets
- **Logging only**: Warnings are logged, but secrets are not automatically corrected
- **Performance consideration**: Adds comparison overhead, but minimal

**Design Decisions:**
- **Default enabled**: Security feature should be on by default
- **Warning only**: Doesn't auto-correct to avoid disrupting legitimate manual changes
- **Optional disable**: Users can disable if they have other drift detection mechanisms

#### `triggerUpdate` (Optional)

**Type:** `bool`  
**Default:** `true`  
**Purpose:** Automatically update cloud provider secrets when Git values change

**Design Rationale:**
- **Git as source of truth**: Ensures cloud providers match Git state
- **Automatic sync**: Reduces manual intervention
- **Optional disable**: Users can disable for manual control

**Design Decisions:**
- **Default enabled**: Git should be source of truth by default
- **Automatic updates**: Ensures consistency without manual steps
- **Optional disable**: Allows manual control when needed

#### `suspend` (Optional)

**Type:** `bool`  
**Default:** `false`  
**Purpose:** Suspend reconciliation for this resource

**Design Rationale:**
- **Troubleshooting**: Allows pausing reconciliation without deleting resource
- **CI/CD transitions**: Useful during complex deployment scenarios
- **Manual reconciliation blocked**: `msmctl reconcile` also respects suspend

**Design Decisions:**
- **Default disabled**: Resources should reconcile by default
- **Blocks all reconciliation**: Both automatic and manual reconciliation are blocked
- **Status preserved**: Resource status is preserved when suspended

#### `suspendGitPulls` (Optional)

**Type:** `bool`  
**Default:** `false`  
**Purpose:** Suspend Git pulls but continue reconciliation with last pulled commit

**Design Rationale:**
- **Freeze Git state**: Useful when you want to freeze Git but keep syncing
- **Separate from suspend**: Different use case than full suspension
- **Automatic patching**: Controller patches GitRepository resource automatically

**Design Decisions:**
- **Default disabled**: Git pulls should happen by default
- **Separate concern**: Different from full suspension (reconciliation continues)
- **Controller-managed**: Controller handles GitRepository patching automatically

#### `notifications` (Optional)

**Type:** `NotificationConfig`  
**Purpose:** Notification configuration for drift detection alerts

**Design Rationale:**
- **Drift alerts**: Notifies when secrets are tampered with
- **GitOps integration**: Supports both FluxCD (via Provider) and ArgoCD (via annotations)
- **Optional feature**: Not all users need notifications

**Structure:**
```yaml
notifications:
  fluxcd:
    providerRef:
      name: slack-provider
      namespace: flux-system
  # OR
  argocd:
    applicationAnnotations:
      notifications.argoproj.io/subscribe.on-sync-succeeded.slack: my-channel
```

**Design Decisions:**
- **GitOps-agnostic**: Supports both FluxCD and ArgoCD notification mechanisms
- **Provider reference**: FluxCD uses Provider CRD reference
- **Application annotations**: ArgoCD uses Application annotations

#### `logging` (Optional)

**Type:** `LoggingConfig`  
**Purpose:** Fine-grained control over log verbosity

**Design Rationale:**
- **Per-operation control**: Different log levels for different operations
- **Reduced noise**: Users can reduce verbosity for specific operations
- **Debugging support**: Allows detailed logging for troubleshooting

**Structure:**
```yaml
logging:
  secrets: INFO          # Log level for secret operations
  properties: INFO       # Log level for property operations
  reconciliation: INFO   # Log level for reconciliation
  diffDiscovery: WARN    # Log level for diff discovery
  sops: DEBUG            # Log level for SOPS operations
  git: INFO              # Log level for Git operations
  provider: DEBUG        # Log level for provider operations
  kustomize: INFO        # Log level for Kustomize operations
```

**Design Decisions:**
- **Per-operation levels**: Allows fine-grained control
- **Sensible defaults**: INFO for most operations, WARN for diff discovery, DEBUG for SOPS/provider
- **Hierarchical levels**: DEBUG includes INFO/WARN/ERROR, INFO includes WARN/ERROR, etc.

#### `hotReload` (Optional)

**Type:** `HotReloadConfig`  
**Purpose:** Controller-level hot reload configuration

**Design Rationale:**
- **No pod restart**: Allows configuration changes without pod restart
- **Controller-level**: Applies to entire controller, not per-resource
- **Optional feature**: Most users rely on pod restarts via Reloader

**Structure:**
```yaml
hotReload:
  enabled: true
  configMapName: secret-manager-controller-config
  configMapNamespace: microscaler-system
```

**Design Decisions:**
- **Controller-level**: Only one resource needs to specify this (others are ignored)
- **ConfigMap-based**: Uses ConfigMap for configuration storage
- **Optional feature**: Default disabled, most users don't need it

## Status Fields

### Phase

**Type:** `String` (optional)  
**Values:** `Pending`, `Started`, `Cloning`, `Updating`, `Failed`, `Ready`

**Purpose:** Current reconciliation phase

**Design Rationale:**
- **State machine**: Tracks progression through reconciliation states
- **User visibility**: Visible in `kubectl get` output via print column
- **Debugging**: Helps identify where reconciliation is stuck

### Description

**Type:** `String` (optional)  
**Purpose:** Human-readable description of current state

**Design Rationale:**
- **User-friendly**: Provides context about what's happening
- **Error messages**: Includes error details when reconciliation fails
- **Visible in kubectl**: Shown in `kubectl get` output

### Conditions

**Type:** `Array<Condition>`  
**Purpose:** Kubernetes-standard conditions array

**Structure:**
```yaml
conditions:
  - type: Ready
    status: "True"  # or "False", "Unknown"
    lastTransitionTime: "2024-01-15T10:30:45Z"
    reason: ReconciliationSucceeded
    message: "All secrets synced successfully"
```

**Design Rationale:**
- **Kubernetes standard**: Follows Kubernetes condition pattern
- **Tool compatibility**: Works with tools that expect conditions
- **Status tracking**: Tracks Ready condition for health checks

### Observed Generation

**Type:** `i64` (optional)  
**Purpose:** Tracks which generation of the resource has been processed

**Design Rationale:**
- **Change detection**: Helps detect when spec changes haven't been processed
- **Kubernetes pattern**: Standard Kubernetes status field
- **Debugging**: Identifies if controller is processing latest spec

### Sync State

**Type:** `SyncStatus` (optional)  
**Purpose:** Tracks which secrets and properties have been synced

**Structure:**
```yaml
sync:
  secrets:
    my-secret:
      exists: true
      updateCount: 3
  properties:
    database.host:
      exists: true
      updateCount: 1
```

**Design Rationale:**
- **Sync tracking**: Tracks which resources have been successfully pushed
- **Update counting**: Tracks how many times each resource has been updated
- **Drift detection**: Helps identify resources that were deleted externally

### SOPS Status Fields

**Purpose:** Track SOPS decryption status and key availability

**Fields:**
- `decryptionStatus`: `Success`, `TransientFailure`, `PermanentFailure`, `NotApplicable`
- `lastDecryptionAttempt`: Timestamp of last attempt
- `lastDecryptionError`: Error message if decryption failed
- `sopsKeyAvailable`: Whether key is available in namespace
- `sopsKeySecretName`: Name of the key secret found
- `sopsKeyNamespace`: Namespace where key was found
- `sopsKeyLastChecked`: Timestamp of last key check

**Design Rationale:**
- **Decryption tracking**: Tracks SOPS decryption success/failure
- **Key availability**: Tracks whether SOPS key is available (avoids redundant API calls)
- **Error reporting**: Provides error details for troubleshooting

## Versioning Strategy

### Current Version: `v1beta1`

**Status:** Beta  
**Rationale:**
- **Feature complete**: Core functionality is stable
- **API stability**: API structure is stable but may have minor changes
- **Production ready**: Suitable for production use with understanding that minor changes may occur

### Version Naming

Following Kubernetes conventions:
- **`v1alpha1`**: Alpha - experimental, may change
- **`v1beta1`**: Beta - stable API, minor changes possible
- **`v1`**: Stable - no breaking changes

### Versioning Rules

1. **Breaking changes**: Require new API version
2. **Non-breaking additions**: Can be added to existing version
3. **Field deprecation**: Fields can be deprecated but must remain supported
4. **Default value changes**: Considered non-breaking if old behavior can be achieved

### Future Versions

**Potential `v1` (Stable):**
- All features proven in production
- No planned breaking changes
- Long-term support commitment

**Migration Path:**
- Support multiple versions during transition
- Provide migration guide
- Deprecation period before removing old version

## Backward Compatibility

### Default Values

All optional fields have sensible defaults that maintain backward compatibility:

```rust
// Defaults maintain existing behavior
gitRepositoryPullInterval: "5m"    // Avoids rate limiting
reconcileInterval: "1m"             // Frequent enough for drift detection
diffDiscovery: true                 // Security feature enabled
triggerUpdate: true                 // Git as source of truth
suspend: false                      // Reconciliation enabled
suspendGitPulls: false              // Git pulls enabled
configs.enabled: false             // Backward compatible (old behavior)
```

### Field Additions

**Non-breaking additions:**
- New optional fields can be added without breaking existing resources
- Default values ensure existing resources continue to work
- Validation ensures new fields don't break old resources

**Example:**
```yaml
# Old resource (still works)
spec:
  sourceRef: {...}
  provider: {...}
  secrets: {...}

# New resource (with new optional field)
spec:
  sourceRef: {...}
  provider: {...}
  secrets: {...}
  configs:              # New optional field
    enabled: true
```

### Field Changes

**Breaking changes require new version:**
- Removing required fields
- Changing field types
- Changing default behavior in incompatible ways

**Non-breaking changes:**
- Adding optional fields
- Adding new enum variants (if handled gracefully)
- Changing default values (if old behavior can be achieved)

### Provider Configuration Compatibility

**"type" field compatibility:**
- Schema allows `"type"` field for backward compatibility
- Field is ignored during deserialization
- Users should use `gcp`/`aws`/`azure` fields instead

**Rationale:**
- Some tools may generate YAML with `"type"` field
- Maintaining compatibility avoids breaking existing configurations
- Clear migration path to new format

### Validation

**Backward compatibility in validation:**
- Validation allows old field formats
- Warnings for deprecated fields
- Clear error messages for invalid configurations

## Field Naming Conventions

### camelCase

All fields use `camelCase` (Rust convention):

```yaml
sourceRef:           # Not source_ref
gitRepositoryPullInterval:  # Not git_repository_pull_interval
```

**Rationale:**
- **Kubernetes standard**: Kubernetes resources use camelCase
- **JSON compatibility**: Matches JSON/YAML conventions
- **Rust serde**: `#[serde(rename_all = "camelCase")]` handles conversion

### Field Organization

Fields are organized logically:

1. **Core configuration** (required): `sourceRef`, `provider`, `secrets`
2. **Optional features**: `configs`, `otel`, `notifications`
3. **Behavior tuning**: `gitRepositoryPullInterval`, `reconcileInterval`, `diffDiscovery`, `triggerUpdate`
4. **Control flags**: `suspend`, `suspendGitPulls`
5. **Observability**: `logging`, `hotReload`

## Validation Rules

### Required Fields

- `sourceRef`: Must be specified
- `provider`: Must specify exactly one provider (`gcp`, `aws`, or `azure`)
- `secrets`: Must be specified
- `secrets.environment`: Must be specified

### Minimum Values

- `gitRepositoryPullInterval`: Minimum 1 minute (enforced by controller)
- `reconcileInterval`: No minimum (but recommended >= 30s)

### Format Validation

- **Duration strings**: Must be valid Kubernetes duration format (`"1m"`, `"5m"`, `"1h"`)
- **URLs**: Provider URLs must be valid (vaultUrl, appConfigEndpoint)
- **Namespaces**: Must be valid Kubernetes namespace names

### Provider-Specific Validation

- **GCP**: `projectId` must be specified
- **AWS**: `region` must be specified
- **Azure**: `vaultUrl` must be specified

## Print Columns

The CRD defines print columns for `kubectl get`:

```yaml
printColumns:
  - name: Phase
    type: string
    jsonPath: .status.phase
  - name: Description
    type: string
    jsonPath: .status.description
  - name: Ready
    type: string
    jsonPath: .status.conditions[?(@.type=="Ready")].status
```

**Design Rationale:**
- **User visibility**: Shows key status information in `kubectl get`
- **Standard fields**: Phase, Description, Ready are most important
- **Condition extraction**: Uses JSONPath to extract Ready condition status

## Short Name

**Shortname:** `smc` (Secret Manager Config)

**Usage:**
```bash
kubectl get smc  # Instead of kubectl get secretmanagerconfig
```

**Design Rationale:**
- **Convenience**: Shorter command for common operations
- **Standard practice**: Kubernetes resources often have shortnames
- **Memorable**: `smc` is easy to remember

## Examples

### Minimal Configuration

```yaml
apiVersion: secret-management.microscaler.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-secrets
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: flux-system
  provider:
    gcp:
      projectId: my-project
  secrets:
    environment: dev
```

### Full Configuration

```yaml
apiVersion: secret-management.microscaler.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-secrets
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: flux-system
  provider:
    gcp:
      projectId: my-project
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
    prefix: my-service
    suffix: -prod
  configs:
    enabled: true
    store: SecretManager
  otel:
    type: Datadog
    serviceName: secret-manager-controller
    environment: production
  gitRepositoryPullInterval: "5m"
  reconcileInterval: "1m"
  diffDiscovery: true
  triggerUpdate: true
  suspend: false
  suspendGitPulls: false
  notifications:
    fluxcd:
      providerRef:
        name: slack-provider
        namespace: flux-system
  logging:
    secrets: INFO
    provider: DEBUG
    sops: DEBUG
```

## Migration Guide

### From v1alpha1 to v1beta1

If a previous alpha version existed:

1. **Update API version**: Change `apiVersion` to `v1beta1`
2. **Review field changes**: Check for any field renames or removals
3. **Update provider format**: Use `gcp`/`aws`/`azure` fields instead of `type` field
4. **Test migration**: Validate resources work with new version

### Adding New Fields

When adding new optional fields:

1. **Default values**: Ensure defaults maintain backward compatibility
2. **Validation**: Add validation that doesn't break existing resources
3. **Documentation**: Update documentation with new field
4. **Examples**: Add examples showing new field usage

## Best Practices

### Field Organization

1. **Required fields first**: List required fields at the top
2. **Logical grouping**: Group related fields together
3. **Optional fields last**: List optional fields after required

### Default Values

1. **Sensible defaults**: Choose defaults that work for most users
2. **Security first**: Default security features to enabled
3. **Performance balanced**: Balance performance with functionality

### Validation

1. **Fail fast**: Validate early to catch errors quickly
2. **Clear errors**: Provide clear error messages for validation failures
3. **Backward compatible**: Don't break existing valid configurations

## Summary

The `SecretManagerConfig` CRD is designed with:

- **Flexibility**: Supports multiple GitOps tools and cloud providers
- **Backward compatibility**: Default values and optional fields maintain compatibility
- **Extensibility**: New fields can be added without breaking changes
- **User-friendly**: Clear structure, sensible defaults, good error messages
- **Production-ready**: Beta version suitable for production use

The design follows Kubernetes CRD best practices while providing the flexibility needed for a GitOps secret management controller.

