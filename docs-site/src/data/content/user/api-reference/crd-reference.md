# CRD Reference

Complete reference for the `SecretManagerConfig` Custom Resource Definition.

## SecretManagerConfig

### API Version

`secret-management.octopilot.io/v1beta1`

### Kind

`SecretManagerConfig` (shortname: `smc`)

### Example

```yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-service-secrets
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
    sops:
      enabled: true
      gpgSecretRef:
        name: sops-gpg-key
        namespace: octopilot-system
        key: private.key
```

## Spec Fields

### sourceRef (required)

Reference to the GitOps source (GitRepository or Application).

```yaml
sourceRef:
  kind: GitRepository  # or "Application" for ArgoCD
  name: my-repo
  namespace: octopilot-system
```

| Field | Type | Description | Required | Default |
|-------|------|-------------|----------|---------|
| `kind` | string | GitOps source type: `"GitRepository"` (FluxCD) or `"Application"` (ArgoCD) | ✓ | - |
| `name` | string | Name of the GitRepository or Application resource | ✓ | - |
| `namespace` | string | Namespace where the GitRepository or Application exists | ✓ | - |

### provider (required)

Cloud provider configuration. Specify one of: `gcp`, `aws`, or `azure`.

#### GCP Configuration

```yaml
provider:
  gcp:
    projectId: my-gcp-project
```

| Field | Type | Description | Required | Default |
|-------|------|-------------|----------|---------|
| `projectId` | string | GCP project ID where secrets will be stored | ✓ | - |

#### AWS Configuration

```yaml
provider:
  aws:
    region: us-east-1
```

| Field | Type | Description | Required | Default |
|-------|------|-------------|----------|---------|
| `region` | string | AWS region where secrets will be stored (e.g., `us-east-1`, `eu-west-1`) | ✓ | - |

#### Azure Configuration

```yaml
provider:
  azure:
    vaultUrl: https://my-vault.vault.azure.net/
```

| Field | Type | Description | Required | Default |
|-------|------|-------------|----------|---------|
| `vaultUrl` | string | Azure Key Vault URL (format: `https://<vault-name>.vault.azure.net/`) | ✓ | - |

### secrets (required)

Secret sync configuration.

```yaml
secrets:
  environment: dev
  kustomizePath: path/to/kustomize/overlay
  sops:
    enabled: true
    gpgSecretRef:
      name: sops-gpg-key
      namespace: octopilot-system
      key: private.key
```

| Field | Type | Description | Required | Default |
|-------|------|-------------|----------|---------|
| `environment` | string | Environment name (e.g., `dev`, `staging`, `prod`) | ✓ | - |
| `kustomizePath` | string | Path to Kustomize overlay in Git repository | ✓ | - |
| `sops` | object | SOPS decryption configuration | ✗ | `enabled: false` |
| `sops.enabled` | boolean | Enable SOPS decryption | ✗ | `false` |
| `sops.gpgSecretRef` | object | Reference to GPG key Kubernetes Secret | ✗ | - |
| `sops.gpgSecretRef.name` | string | Name of the Kubernetes Secret containing the GPG key | ✗ | - |
| `sops.gpgSecretRef.namespace` | string | Namespace where the GPG key secret exists | ✗ | - |
| `sops.gpgSecretRef.key` | string | Key in secret containing GPG private key | ✗ | `private.key` |

### configs (optional)

Config store configuration for routing `application.properties` to config stores.

```yaml
configs:
  enabled: true
  parameterPath: /my-service/dev  # AWS only
  appConfigEndpoint: https://my-app-config.azconfig.io  # Azure only
  store: SecretManager  # GCP: SecretManager or ParameterManager
```

| Field | Type | Description | Required | Default |
|-------|------|-------------|----------|---------|
| `enabled` | boolean | Enable config store sync for `application.properties` files | ✗ | `false` |
| `parameterPath` | string | Parameter Store path prefix (AWS only) | ✗ | - |
| `appConfigEndpoint` | string | App Configuration endpoint URL (Azure only) | ✗ | - |
| `store` | string | Store type: `SecretManager` or `ParameterManager` (GCP only) | ✗ | `SecretManager` |

### otel (optional)

OpenTelemetry configuration for distributed tracing.

```yaml
otel:
  exporter: otlp  # or "datadog"
  endpoint: http://otel-collector:4317
  serviceName: secret-manager-controller
```

| Field | Type | Description | Required | Default |
|-------|------|-------------|----------|---------|
| `exporter` | string | Exporter type: `"otlp"` or `"datadog"` | ✗ | `"otlp"` |
| `endpoint` | string | Exporter endpoint URL (e.g., `http://otel-collector:4317`) | ✗ | - |
| `serviceName` | string | Service name for tracing | ✗ | `"secret-manager-controller"` |

### gitRepositoryPullInterval (optional)

How often to check for updates from Git.

```yaml
gitRepositoryPullInterval: 5m  # Default: 5m, minimum: 1m
```

| Field | Type | Description | Required | Default |
|-------|------|-------------|----------|---------|
| `gitRepositoryPullInterval` | string | How often to check for updates from Git (Kubernetes duration format) | ✗ | `"5m"` |

**Format:** Kubernetes duration string (e.g., `"1m"`, `"5m"`, `"1h"`)

**Recommendation:** 5 minutes or greater to avoid Git API rate limits. Minimum: `1m`.

### reconcileInterval (optional)

How often to reconcile secrets between Git and cloud provider.

```yaml
reconcileInterval: 1m  # Default: 1m
```

| Field | Type | Description | Required | Default |
|-------|------|-------------|----------|---------|
| `reconcileInterval` | string | How often to reconcile secrets between Git and cloud provider (Kubernetes duration format) | ✗ | `"1m"` |

**Format:** Kubernetes duration string (e.g., `"30s"`, `"1m"`, `"5m"`)

### diffDiscovery (optional)

Enable diff discovery to detect tampering.

```yaml
diffDiscovery: true  # Default: false
```

| Field | Type | Description | Required | Default |
|-------|------|-------------|----------|---------|
| `diffDiscovery` | boolean | Enable diff discovery to detect tampering between Git and cloud provider | ✗ | `false` |

When enabled, logs warnings when differences are found between Git (source of truth) and cloud provider.

### logging (optional)

Fine-grained logging configuration.

```yaml
logging:
  reconciliation: INFO
  secrets: INFO
  properties: INFO
  provider: DEBUG
  sops: DEBUG
  git: INFO
  kustomize: INFO
  diffDiscovery: WARN
```

| Field | Type | Description | Required | Default |
|-------|------|-------------|----------|---------|
| `logging` | object | Fine-grained logging configuration | ✗ | See defaults below |
| `logging.reconciliation` | string | Log level for reconciliation events | ✗ | `INFO` |
| `logging.secrets` | string | Log level for secret operations | ✗ | `INFO` |
| `logging.properties` | string | Log level for properties file operations | ✗ | `INFO` |
| `logging.provider` | string | Log level for provider operations | ✗ | `DEBUG` |
| `logging.sops` | string | Log level for SOPS decryption | ✗ | `DEBUG` |
| `logging.git` | string | Log level for Git operations | ✗ | `INFO` |
| `logging.kustomize` | string | Log level for Kustomize operations | ✗ | `INFO` |
| `logging.diffDiscovery` | string | Log level for diff discovery | ✗ | `WARN` |

**Log Levels:** `ERROR`, `WARN`, `INFO`, `DEBUG`

### notifications (optional)

Notification configuration for drift detection alerts.

```yaml
notifications:
  fluxcd:
    providerRef:
      name: my-provider
      namespace: flux-system
  argocd:
    subscriptions:
      - service: slack
        channel: "#secrets-alerts"
        trigger: drift-detected
```

| Field | Type | Description | Required | Default |
|-------|------|-------------|----------|---------|
| `notifications` | object | Notification configuration for drift detection alerts | ✗ | - |
| `notifications.fluxcd` | object | FluxCD notification configuration | ✗ | - |
| `notifications.fluxcd.providerRef` | object | FluxCD Provider reference | ✗ | - |
| `notifications.fluxcd.providerRef.name` | string | Name of the FluxCD Provider | ✗ | - |
| `notifications.fluxcd.providerRef.namespace` | string | Namespace where the Provider exists | ✗ | - |
| `notifications.argocd` | object | ArgoCD notification configuration | ✗ | - |
| `notifications.argocd.subscriptions` | array | List of notification subscriptions | ✗ | - |
| `notifications.argocd.subscriptions[].service` | string | Notification service: `slack`, `email`, `webhook` | ✗ | - |
| `notifications.argocd.subscriptions[].channel` | string | Notification channel (e.g., `#secrets-alerts`) | ✗ | - |
| `notifications.argocd.subscriptions[].trigger` | string | Trigger name (e.g., `drift-detected`) | ✗ | - |

### hotReload (optional)

Hot reload configuration for controller-level settings.

```yaml
hotReload:
  enabled: false  # Default: false
  configMapName: secret-manager-controller-config
  configMapNamespace: octopilot-system
```

| Field | Type | Description | Required | Default |
|-------|------|-------------|----------|---------|
| `hotReload` | object | Hot reload configuration for controller-level settings | ✗ | - |
| `hotReload.enabled` | boolean | Enable hot-reload functionality | ✗ | `false` |
| `hotReload.configMapName` | string | Name of the ConfigMap to watch for changes | ✗ | `"secret-manager-controller-config"` |
| `hotReload.configMapNamespace` | string | Namespace where the ConfigMap exists (defaults to controller namespace) | ✗ | - |

## Status Fields

The controller updates the status with:

### phase (string)

Current phase: `Pending`, `Syncing`, `Synced`, `Error`

### description (string)

Human-readable status message.

### conditions (array)

Kubernetes conditions array with:
- `type`: Condition type (e.g., `Ready`, `Synced`)
- `status`: `True`, `False`, or `Unknown`
- `reason`: Reason code
- `message`: Human-readable message
- `lastTransitionTime`: Timestamp

### lastSyncTime (string)

Timestamp of last successful sync (RFC3339 format).

### secretsCount (integer)

Number of secrets currently managed.

## Printer Columns

The CRD includes additional printer columns:

- `PHASE`: Current phase
- `DESCRIPTION`: Status description
- `READY`: Ready condition status

View with:

```bash
kubectl get secretmanagerconfig
```

## Validation

The CRD schema validates:
- Required fields are present
- Provider-specific required fields
- Duration strings are valid
- Enum values are correct

## Examples

See the [Examples](../tutorials/basic-usage.md) section for complete working examples.

## Learn More

- [Configuration Guide](../getting-started/configuration.md) - Detailed configuration guide
- [Provider APIs](./provider-apis.md) - Provider-specific API details
- [Configuration Options](./configuration-options.md) - All configuration options
