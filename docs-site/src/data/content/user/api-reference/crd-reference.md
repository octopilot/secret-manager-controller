# CRD Reference

Complete reference for the `SecretManagerConfig` Custom Resource Definition.

## SecretManagerConfig

### API Version

`secret-management.microscaler.io/v1beta1`

### Kind

`SecretManagerConfig` (shortname: `smc`)

### Example

```yaml
apiVersion: secret-management.microscaler.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-service-secrets
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: microscaler-system
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
        namespace: microscaler-system
        key: private.key
```

## Spec Fields

### sourceRef (required)

Reference to the GitOps source (GitRepository or Application).

```yaml
sourceRef:
  kind: GitRepository  # or "Application" for ArgoCD
  name: my-repo
  namespace: microscaler-system
```

**Fields:**
- `kind` (string, required): `"GitRepository"` or `"Application"`
- `name` (string, required): Name of the GitRepository or Application resource
- `namespace` (string, required): Namespace where the resource exists

### provider (required)

Cloud provider configuration. Specify one of: `gcp`, `aws`, or `azure`.

#### GCP Configuration

```yaml
provider:
  gcp:
    projectId: my-gcp-project
```

**Fields:**
- `projectId` (string, required): GCP project ID

#### AWS Configuration

```yaml
provider:
  aws:
    region: us-east-1
```

**Fields:**
- `region` (string, required): AWS region (e.g., `us-east-1`, `eu-west-1`)

#### Azure Configuration

```yaml
provider:
  azure:
    vaultUrl: https://my-vault.vault.azure.net/
```

**Fields:**
- `vaultUrl` (string, required): Azure Key Vault URL

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
      namespace: microscaler-system
      key: private.key
```

**Fields:**
- `environment` (string, required): Environment name (e.g., `dev`, `staging`, `prod`)
- `kustomizePath` (string, required): Path to Kustomize overlay in Git repository
- `sops` (object, optional): SOPS decryption configuration
  - `enabled` (boolean): Enable SOPS decryption (default: `false`)
  - `gpgSecretRef` (object): Reference to GPG key Kubernetes Secret
    - `name` (string): Secret name
    - `namespace` (string): Secret namespace
    - `key` (string): Key in secret containing GPG private key (default: `private.key`)

### configs (optional)

Config store configuration for routing `application.properties` to config stores.

```yaml
configs:
  enabled: true
  parameterPath: /my-service/dev  # AWS only
  appConfigEndpoint: https://my-app-config.azconfig.io  # Azure only
  store: SecretManager  # GCP: SecretManager or ParameterManager
```

**Fields:**
- `enabled` (boolean, default: `false`): Enable config store sync
- `parameterPath` (string, optional, AWS only): Parameter Store path prefix
- `appConfigEndpoint` (string, optional, Azure only): App Configuration endpoint
- `store` (string, optional, GCP only): Store type - `SecretManager` or `ParameterManager`

### otel (optional)

OpenTelemetry configuration for distributed tracing.

```yaml
otel:
  exporter: otlp  # or "datadog"
  endpoint: http://otel-collector:4317
  serviceName: secret-manager-controller
```

**Fields:**
- `exporter` (string): Exporter type - `"otlp"` or `"datadog"` (default: `"otlp"`)
- `endpoint` (string): Exporter endpoint URL
- `serviceName` (string): Service name for tracing (default: `"secret-manager-controller"`)

### gitRepositoryPullInterval (optional)

How often to check for updates from Git.

```yaml
gitRepositoryPullInterval: 5m  # Default: 5m, minimum: 1m
```

**Format:** Kubernetes duration string (e.g., `"1m"`, `"5m"`, `"1h"`)

**Recommendation:** 5 minutes or greater to avoid Git API rate limits.

### reconcileInterval (optional)

How often to reconcile secrets between Git and cloud provider.

```yaml
reconcileInterval: 1m  # Default: 1m
```

**Format:** Kubernetes duration string (e.g., `"30s"`, `"1m"`, `"5m"`)

### diffDiscovery (optional)

Enable diff discovery to detect tampering.

```yaml
diffDiscovery: true  # Default: false
```

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

**Log Levels:** `ERROR`, `WARN`, `INFO`, `DEBUG`

**Defaults:**
- `reconciliation`: `INFO`
- `secrets`: `INFO`
- `properties`: `INFO`
- `provider`: `DEBUG`
- `sops`: `DEBUG`
- `git`: `INFO`
- `kustomize`: `INFO`
- `diffDiscovery`: `WARN`

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

**Fields:**
- `fluxcd` (object, optional): FluxCD notification configuration
  - `providerRef` (object): FluxCD Provider reference
    - `name` (string): Provider name
    - `namespace` (string, optional): Provider namespace
- `argocd` (object, optional): ArgoCD notification configuration
  - `subscriptions` (array): List of notification subscriptions
    - `service` (string): Notification service (e.g., `slack`, `email`, `webhook`)
    - `channel` (string): Notification channel
    - `trigger` (string): Trigger name

### hotReload (optional)

Hot reload configuration for controller-level settings.

```yaml
hotReload:
  enabled: false  # Default: false
  configMapName: secret-manager-controller-config
  configMapNamespace: microscaler-system
```

**Fields:**
- `enabled` (boolean, default: `false`): Enable hot-reload
- `configMapName` (string, default: `"secret-manager-controller-config"`): ConfigMap to watch
- `configMapNamespace` (string, optional): ConfigMap namespace (defaults to controller namespace)

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
