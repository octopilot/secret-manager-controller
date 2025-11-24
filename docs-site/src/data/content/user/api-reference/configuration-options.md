# Configuration Options

Complete reference for all configuration options available in the Secret Manager Controller.

## Overview

The Secret Manager Controller supports two levels of configuration:

1. **Controller-Level Configuration**: Global settings that apply to the entire controller (via environment variables/ConfigMap)
2. **Resource-Level Configuration**: Per-resource settings in `SecretManagerConfig` CRD

---

## Controller-Level Configuration

Controller-level settings are configured via environment variables, which are typically populated from a ConfigMap using `envFrom` in the deployment.

### Configuration via ConfigMap

Create a ConfigMap in the controller namespace:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: secret-manager-controller-config
  namespace: microscaler-system
data:
  metrics_port: "5000"
  log_level: "INFO"
  # ... other settings ...
```

Reference it in the deployment:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: secret-manager-controller
spec:
  template:
    spec:
      containers:
      - name: controller
        envFrom:
        - configMapRef:
            name: secret-manager-controller-config
            optional: true
```

### Server Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `METRICS_PORT` | `5000` | HTTP server port for metrics and health probes |
| `SERVER_STARTUP_TIMEOUT_SECS` | `10` | How long to wait for server to be ready (seconds) |
| `SERVER_POLL_INTERVAL_MS` | `50` | How often to check if server is ready (milliseconds) |

### Reconciliation Behavior

| Variable | Default | Description |
|----------|---------|-------------|
| `RECONCILIATION_ERROR_REQUEUE_SECS` | `60` | How long to wait before retrying a failed reconciliation (seconds) |
| `BACKOFF_START_MS` | `1000` | Exponential backoff starting value (milliseconds) |
| `BACKOFF_MAX_MS` | `30000` | Exponential backoff maximum value (milliseconds) |

### Watch Stream Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `WATCH_RESTART_DELAY_SECS` | `5` | How long to wait before restarting watch stream after unknown errors (seconds) |
| `WATCH_RESTART_DELAY_AFTER_END_SECS` | `1` | How long to wait before restarting watch stream after it ends normally (seconds) |

### Validation Minimums

| Variable | Default | Description |
|----------|---------|-------------|
| `MIN_GITREPOSITORY_PULL_INTERVAL_SECS` | `60` | Minimum GitRepository pull interval (seconds) - enforced to prevent API rate limiting |
| `MIN_RECONCILE_INTERVAL_SECS` | `60` | Minimum reconcile interval (seconds) - enforced to prevent API rate limiting |

### SOPS Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `SOPS_PRIVATE_KEY_SECRET_NAME` | `sops-private-key` | Name of the Kubernetes secret containing the SOPS GPG private key |
| `SOPS_KEY_WATCH_ENABLED` | `true` | Enable SOPS key watch for hot-reload |

### Global Logging Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `LOG_LEVEL` | `INFO` | Global log level (`ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE`) - separate from per-resource CRD logging |
| `LOG_FORMAT` | `json` | Log format (`json`, `text`) |
| `LOG_ENABLE_COLOR` | `false` | Enable color in text format logs |

### Feature Flags

| Variable | Default | Description |
|----------|---------|-------------|
| `ENABLE_METRICS` | `true` | Enable metrics collection |
| `ENABLE_TRACING` | `true` | Enable distributed tracing |

### Resource Limits

| Variable | Default | Description |
|----------|---------|-------------|
| `MAX_CONCURRENT_RECONCILIATIONS` | `10` | Maximum concurrent reconciliations - limits how many resources can be reconciled simultaneously |
| `MAX_SECRETS_PER_RESOURCE` | `1000` | Maximum secrets per resource - prevents resource exhaustion from overly large secret lists |
| `MAX_SECRET_SIZE_BYTES` | `65536` | Maximum secret size in bytes - enforced by validation (64KB default) |

### Hot Reload

Hot reload allows configuration changes without pod restart. See [Hot Reload Configuration](#hot-reload-configuration) below.

---

## SecretManagerConfig CRD Options

The `SecretManagerConfig` CRD provides per-resource configuration options.

### Required Fields

#### `sourceRef` (Required)

Reference to the GitOps source (FluxCD GitRepository or ArgoCD Application).

```yaml
sourceRef:
  kind: GitRepository  # or "Application" for ArgoCD
  name: my-repo
  namespace: microscaler-system
  gitCredentials:  # Optional - for ArgoCD private repos
    name: git-credentials
    namespace: my-namespace
```

**Fields:**
- `kind` (string, default: `"GitRepository"`): `"GitRepository"` or `"Application"`
- `name` (string, required): Name of the GitRepository or Application resource
- `namespace` (string, required): Namespace where the resource exists
- `gitCredentials` (object, optional): Git credentials for ArgoCD private repos
  - `name` (string, required): Secret name containing git credentials
  - `namespace` (string, optional): Secret namespace (defaults to `sourceRef.namespace`)

#### `provider` (Required)

Cloud provider configuration. Specify one of: `gcp`, `aws`, or `azure`.

**GCP:**
```yaml
provider:
  gcp:
    projectId: my-gcp-project
    auth:  # Optional - defaults to Workload Identity
      authType: workloadIdentity
      serviceAccountEmail: secret-manager@my-project.iam.gserviceaccount.com
```

**AWS:**
```yaml
provider:
  aws:
    region: us-east-1
    auth:  # Optional - defaults to IRSA
      authType: irsa
      roleArn: arn:aws:iam::123456789012:role/secret-manager-role
```

**Azure:**
```yaml
provider:
  azure:
    vaultName: my-vault  # or vaultUrl: https://my-vault.vault.azure.net/
    auth:  # Optional - defaults to Workload Identity
      authType: workloadIdentity
      clientId: <managed-identity-client-id>
```

#### `secrets` (Required)

Secret sync configuration.

```yaml
secrets:
  environment: dev                    # Required - environment/profile name
  kustomizePath: path/to/kustomize   # Optional - kustomize build path
  basePath: microservices            # Optional - base path for file search
  prefix: my-service                 # Optional - secret name prefix
  suffix: -prod                      # Optional - secret name suffix
```

**Fields:**
- `environment` (string, required): Environment/profile name to sync (e.g., "dev", "prod-cf")
- `kustomizePath` (string, optional): Path to kustomization.yaml file (relative to GitRepository root)
- `basePath` (string, optional): Base path for application files (used only if `kustomizePath` is not specified)
- `prefix` (string, optional): Secret name prefix (default: repository name)
- `suffix` (string, optional): Secret name suffix

### Optional Fields

#### `configs` (Optional)

Config store configuration for routing `application.properties` to config stores.

```yaml
configs:
  enabled: true                       # Default: false
  parameterPath: /my-service/dev     # AWS-specific (optional)
  store: SecretManager               # GCP-specific (optional, default: SecretManager)
  appConfigEndpoint: https://...     # Azure-specific (optional)
```

**Fields:**
- `enabled` (boolean, default: `false`): Enable config store sync
- `parameterPath` (string, optional, AWS only): Parameter path prefix (defaults to `/{prefix}/{environment}`)
- `store` (string, optional, GCP only): Store type (`SecretManager` or `ParameterManager`)
- `appConfigEndpoint` (string, optional, Azure only): App Configuration endpoint (auto-detected if not specified)

#### `otel` (Optional)

OpenTelemetry configuration for distributed tracing.

**OTLP Exporter:**
```yaml
otel:
  type: Otlp
  endpoint: http://otel-collector:4317
  serviceName: secret-manager-controller  # Optional
  serviceVersion: 1.0.0                   # Optional
  environment: production                  # Optional
```

**Datadog Direct Export:**
```yaml
otel:
  type: Datadog
  serviceName: secret-manager-controller  # Optional
  serviceVersion: 1.0.0                   # Optional
  environment: production                  # Optional
  site: datadoghq.com                     # Optional
  apiKey: <secret-reference>              # Optional (uses DD_API_KEY env var if not specified)
```

**Fields:**
- `type` (string, required): `"Otlp"` or `"Datadog"`
- `endpoint` (string, required for OTLP): OTLP endpoint URL
- `serviceName` (string, optional): Service name for traces (default: `"secret-manager-controller"`)
- `serviceVersion` (string, optional): Service version (defaults to Cargo package version)
- `environment` (string, optional): Deployment environment (e.g., `"dev"`, `"prod"`)
- `site` (string, optional, Datadog only): Datadog site (e.g., `"datadoghq.com"`, `"us3.datadoghq.com"`)
- `apiKey` (string, optional, Datadog only): Datadog API key (uses `DD_API_KEY` env var if not specified)

#### `gitRepositoryPullInterval` (Optional)

How often to check for updates from GitRepository source.

```yaml
gitRepositoryPullInterval: "5m"  # Default: "5m"
```

**Format:** Kubernetes duration string (e.g., `"1m"`, `"5m"`, `"1h"`)  
**Minimum:** `1m` (60 seconds) - shorter intervals may hit API rate limits  
**Default:** `"5m"` (5 minutes)  
**Recommended:** `5m` or greater to avoid rate limiting

#### `reconcileInterval` (Optional)

How often to reconcile secrets between Git and cloud providers.

```yaml
reconcileInterval: "1m"  # Default: "1m"
```

**Format:** Kubernetes duration string (e.g., `"1m"`, `"30s"`, `"5m"`)  
**Default:** `"1m"` (1 minute)

#### `diffDiscovery` (Optional)

Enable detection of secrets tampered with in cloud providers.

```yaml
diffDiscovery: true  # Default: true
```

**Default:** `true` (enabled)  
**Behavior:** Logs warnings when differences are found between Git (source of truth) and cloud provider

#### `triggerUpdate` (Optional)

Automatically update cloud provider secrets when Git values change.

```yaml
triggerUpdate: true  # Default: true
```

**Default:** `true` (enabled)  
**Behavior:** Ensures Git remains the source of truth by automatically syncing changes

#### `suspend` (Optional)

Suspend reconciliation for this resource.

```yaml
suspend: false  # Default: false
```

**Default:** `false` (reconciliation enabled)  
**Behavior:** When `true`, the controller skips reconciliation. Manual reconciliation via `msmctl` is also blocked.

#### `suspendGitPulls` (Optional)

Suspend Git pulls but continue reconciliation with last pulled commit.

```yaml
suspendGitPulls: false  # Default: false
```

**Default:** `false` (Git pulls enabled)  
**Behavior:** When `true`, suspends Git pulls but continues reconciliation with the last pulled commit. The controller automatically patches the GitRepository resource.

#### `notifications` (Optional)

Notification configuration for drift detection alerts.

**FluxCD:**
```yaml
notifications:
  fluxcd:
    providerRef:
      name: slack-provider
      namespace: flux-system  # Optional
```

**ArgoCD:**
```yaml
notifications:
  argocd:
    subscriptions:
      - trigger: drift-detected
        service: slack
        channel: "#secrets-alerts"
```

**Fields:**
- `fluxcd` (object, optional): FluxCD notification configuration
  - `providerRef` (object, required): FluxCD Provider reference
    - `name` (string, required): Provider name
    - `namespace` (string, optional): Provider namespace (defaults to SecretManagerConfig namespace)
- `argocd` (object, optional): ArgoCD notification configuration
  - `subscriptions` (array, required): List of notification subscriptions
    - `trigger` (string, required): Notification trigger name
    - `service` (string, required): Notification service (e.g., `"slack"`, `"email"`, `"webhook"`)
    - `channel` (string, required): Notification channel

#### `logging` (Optional)

Fine-grained control over log verbosity.

```yaml
logging:
  secrets: INFO          # Default: INFO
  properties: INFO       # Default: INFO
  reconciliation: INFO   # Default: INFO
  diffDiscovery: WARN    # Default: WARN
  sops: DEBUG            # Default: DEBUG
  git: INFO              # Default: INFO
  provider: DEBUG        # Default: DEBUG
  kustomize: INFO        # Default: INFO
```

**Fields:**
- `secrets` (string, default: `INFO`): Log level for secret operations
- `properties` (string, default: `INFO`): Log level for property operations
- `reconciliation` (string, default: `INFO`): Log level for reconciliation operations
- `diffDiscovery` (string, default: `WARN`): Log level for diff discovery operations
- `sops` (string, default: `DEBUG`): Log level for SOPS operations
- `git` (string, default: `INFO`): Log level for Git operations
- `provider` (string, default: `DEBUG`): Log level for provider operations
- `kustomize` (string, default: `INFO`): Log level for Kustomize operations

**Log Levels:** `ERROR`, `WARN`, `INFO`, `DEBUG` (hierarchical: DEBUG includes INFO/WARN/ERROR)

#### `hotReload` (Optional)

Hot reload configuration for controller-level settings.

```yaml
hotReload:
  enabled: true  # Default: false
  configMapName: secret-manager-controller-config  # Default: "secret-manager-controller-config"
  configMapNamespace: microscaler-system  # Optional (defaults to controller namespace)
```

**Fields:**
- `enabled` (boolean, default: `false`): Enable hot-reload of controller configuration
- `configMapName` (string, default: `"secret-manager-controller-config"`): ConfigMap name to watch
- `configMapNamespace` (string, optional): ConfigMap namespace (defaults to controller namespace)

**Note:** Only one resource needs to specify this (others are ignored). Most users rely on pod restarts via Reloader.

---

## Provider-Specific Configuration

### AWS Configuration

```yaml
provider:
  aws:
    region: us-east-1  # Required
    auth:  # Optional - defaults to IRSA
      authType: irsa
      roleArn: arn:aws:iam::123456789012:role/secret-manager-role
```

**Fields:**
- `region` (string, required): AWS region (e.g., `"us-east-1"`, `"eu-west-1"`)
- `auth` (object, optional): Authentication configuration
  - `authType` (string, required): `"irsa"` (IAM Roles for Service Accounts)
  - `roleArn` (string, required): AWS IAM role ARN to assume

**Authentication:**
- **IRSA (Recommended)**: Uses Kubernetes ServiceAccount annotation with IAM role ARN
- **Default Credential Chain**: If `auth` is not specified, AWS SDK uses default credential chain

### GCP Configuration

```yaml
provider:
  gcp:
    projectId: my-gcp-project  # Required
    auth:  # Optional - defaults to Workload Identity
      authType: workloadIdentity
      serviceAccountEmail: secret-manager@my-project.iam.gserviceaccount.com
```

**Fields:**
- `projectId` (string, required): GCP project ID
- `auth` (object, optional): Authentication configuration
  - `authType` (string, required): `"workloadIdentity"`
  - `serviceAccountEmail` (string, required): GCP service account email

**Authentication:**
- **Workload Identity (Recommended)**: Uses Kubernetes ServiceAccount bound to GCP Service Account
- **Application Default Credentials**: If `auth` is not specified, GCP SDK uses ADC

### Azure Configuration

```yaml
provider:
  azure:
    vaultName: my-vault  # Required (or vaultUrl)
    # OR
    vaultUrl: https://my-vault.vault.azure.net/  # Required (or vaultName)
    auth:  # Optional - defaults to Workload Identity
      authType: workloadIdentity
      clientId: <managed-identity-client-id>
```

**Fields:**
- `vaultName` (string, required if `vaultUrl` not specified): Azure Key Vault name
- `vaultUrl` (string, required if `vaultName` not specified): Full vault URL
- `auth` (object, optional): Authentication configuration
  - `authType` (string, required): `"workloadIdentity"`
  - `clientId` (string, required): Azure service principal client ID

**Authentication:**
- **Workload Identity (Recommended)**: Uses Kubernetes ServiceAccount bound to Azure Managed Identity
- **Default Credential Chain**: If `auth` is not specified, Azure SDK uses default credential chain

---

## Source Configuration

### FluxCD GitRepository

```yaml
sourceRef:
  kind: GitRepository
  name: my-repo
  namespace: flux-system
```

**Requirements:**
- FluxCD source-controller installed
- `GitRepository` resource created
- Git credentials managed by source-controller (via `GitRepository.spec.secretRef`)

### ArgoCD Application

```yaml
sourceRef:
  kind: Application
  name: my-app
  namespace: argocd
  gitCredentials:  # Optional - for private repos
    name: git-credentials
    namespace: my-namespace
```

**Requirements:**
- ArgoCD installed
- `Application` resource created
- Git credentials specified if using private repositories

**Git Credentials Secret:**
```yaml
apiVersion: v1
kind: Secret
metadata:
  name: git-credentials
type: Opaque
stringData:
  # For HTTPS:
  username: git
  password: <token-or-password>
  # OR for SSH:
  identity: |
    -----BEGIN OPENSSH PRIVATE KEY-----
    ...
    -----END OPENSSH PRIVATE KEY-----
```

---

## Secrets Configuration

### Kustomize Path

```yaml
secrets:
  environment: dev
  kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
```

**Behavior:**
- Controller runs `kustomize build` on the specified path
- Extracts secrets from generated Kubernetes Secret resources
- Supports kustomize overlays, patches, and generators

### Base Path (Raw Files)

```yaml
secrets:
  environment: dev
  basePath: microservices  # Optional - searches from root if not specified
```

**Behavior:**
- Controller reads raw `application.secrets.env` files directly
- Searches from `basePath` (or repository root if not specified)
- No kustomize processing

### Secret Naming

```yaml
secrets:
  environment: dev
  prefix: my-service
  suffix: -prod
```

**Result:** Secret names like `my-service-database-password-prod`

**Defaults:**
- `prefix`: Repository name (if not specified)
- `suffix`: None (if not specified)

---

## Config Store Configuration

### Enable Config Store Sync

```yaml
configs:
  enabled: true
```

**Behavior:**
- When `enabled: true`, `application.properties` files are routed to config stores
- When `enabled: false`, properties are stored as a JSON blob in secret stores (default)

### AWS Parameter Store

```yaml
configs:
  enabled: true
  parameterPath: /my-service/dev  # Optional - defaults to /{prefix}/{environment}
```

**Behavior:**
- Properties are stored as individual parameters in AWS Systems Manager Parameter Store
- Parameter path: `/{parameterPath}/{property-name}`

### GCP Parameter Manager

```yaml
configs:
  enabled: true
  store: ParameterManager  # Future - requires ESO contribution
```

**Current Status:**
- `SecretManager` (default): Stores configs as individual secrets in Secret Manager (interim solution)
- `ParameterManager`: Future support (requires ESO contribution)

### Azure App Configuration

```yaml
configs:
  enabled: true
  appConfigEndpoint: https://my-app-config.azconfig.io  # Optional - auto-detected if not specified
```

**Behavior:**
- Properties are stored as key-values in Azure App Configuration
- Endpoint is auto-detected from vault region if not specified

---

## OpenTelemetry Configuration

### OTLP Exporter

```yaml
otel:
  type: Otlp
  endpoint: http://otel-collector:4317
  serviceName: secret-manager-controller
  serviceVersion: 1.0.0
  environment: production
```

**Behavior:**
- Sends traces to OpenTelemetry Collector
- Collector can forward to various backends (Jaeger, Zipkin, etc.)

### Datadog Direct Export

```yaml
otel:
  type: Datadog
  serviceName: secret-manager-controller
  serviceVersion: 1.0.0
  environment: production
  site: datadoghq.com
  apiKey: <secret-reference>  # Optional - uses DD_API_KEY env var
```

**Behavior:**
- Sends traces directly to Datadog
- Uses `datadog-opentelemetry` exporter
- API key can be specified or use `DD_API_KEY` environment variable

---

## Logging Configuration

### Per-Operation Log Levels

```yaml
logging:
  secrets: INFO          # Secret operations (create, update, delete)
  properties: INFO       # Property operations (create, update, delete)
  reconciliation: INFO   # Reconciliation operations (start, complete, errors)
  diffDiscovery: WARN    # Diff discovery (only log when differences found)
  sops: DEBUG            # SOPS decryption operations
  git: INFO              # Git/artifact operations (clone, pull, resolve)
  provider: DEBUG        # Provider operations (authentication, API calls)
  kustomize: INFO        # Kustomize operations
```

**Log Level Hierarchy:**
- `DEBUG`: Includes INFO, WARN, ERROR
- `INFO`: Includes WARN, ERROR
- `WARN`: Includes ERROR
- `ERROR`: Only errors

---

## Hot Reload Configuration

### Enable Hot Reload

```yaml
hotReload:
  enabled: true
  configMapName: secret-manager-controller-config
  configMapNamespace: microscaler-system
```

**Behavior:**
- Controller watches the specified ConfigMap for changes
- When ConfigMap changes, controller reads it directly and updates internal state
- No pod restart required
- Configuration takes effect immediately

### Disable Hot Reload (Default)

```yaml
hotReload:
  enabled: false  # Default
```

**Behavior:**
- Configuration only loads at startup from environment variables
- Changes to ConfigMap require pod restart to take effect
- Most users rely on pod restarts via Reloader or manual updates

---

## Notification Configuration

### FluxCD Notifications

```yaml
notifications:
  fluxcd:
    providerRef:
      name: slack-provider
      namespace: flux-system
```

**Behavior:**
- Creates a FluxCD Alert CRD that watches this SecretManagerConfig
- Sends notifications via the specified Provider when drift is detected
- Requires FluxCD notification-controller installed

### ArgoCD Notifications

```yaml
notifications:
  argocd:
    subscriptions:
      - trigger: drift-detected
        service: slack
        channel: "#secrets-alerts"
```

**Behavior:**
- Adds annotations to the ArgoCD Application resource
- Triggers notifications when drift is detected
- Requires ArgoCD notifications configured

---

## Configuration Examples

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
      auth:
        authType: workloadIdentity
        serviceAccountEmail: secret-manager@my-project.iam.gserviceaccount.com
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
  hotReload:
    enabled: false
```

---

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

---

## Summary

| Configuration Type | Location | Scope | Hot Reload |
|-------------------|----------|-------|------------|
| **Controller-Level** | ConfigMap/Env Vars | Global | Optional (via CRD) |
| **Resource-Level** | SecretManagerConfig CRD | Per-resource | N/A |

**Key Principles:**
- **Sensible Defaults**: All optional fields have defaults
- **Backward Compatible**: New fields don't break existing resources
- **Flexible**: Supports multiple GitOps tools and cloud providers
- **Observable**: Comprehensive logging and metrics

For detailed API reference, see [CRD Reference](./crd-reference.md).
