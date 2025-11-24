# Configuration

Complete guide to configuring the Secret Manager Controller.

## SecretManagerConfig Spec

The `SecretManagerConfig` CRD is the main configuration resource. Here's the complete spec:

```yaml
apiVersion: secret-management.microscaler.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-config
  namespace: default
spec:
  # Source reference (required)
  sourceRef:
    kind: GitRepository  # or Application for ArgoCD
    name: my-repo
    namespace: microscaler-system
  
  # Provider configuration (required)
  provider:
    gcp:
      projectId: my-project
    # OR
    aws:
      region: us-east-1
    # OR
    azure:
      vaultUrl: https://my-vault.vault.azure.net/
  
  # Secrets configuration (required)
  secrets:
    environment: dev
    kustomizePath: path/to/kustomize/overlay
    sops:
      enabled: true
      gpgSecretRef:
        name: sops-gpg-key
        namespace: microscaler-system
        key: private.key
  
  # Config store configuration (optional)
  configs:
    enabled: true
    parameterPath: /my-service/dev  # AWS only
    appConfigEndpoint: https://my-app-config.azconfig.io  # Azure only
    store: SecretManager  # GCP: SecretManager or ParameterManager
  
  # OpenTelemetry configuration (optional)
  otel:
    exporter: otlp
    endpoint: http://otel-collector:4317
    serviceName: secret-manager-controller
  
  # Timing configuration (optional)
  gitRepositoryPullInterval: 5m  # Default: 5m
  reconcileInterval: 1m  # Default: 1m
  
  # Feature flags (optional)
  diffDiscovery: true  # Default: false
```

## Source Reference

The `sourceRef` field references your GitOps source. It supports:

### FluxCD GitRepository

```yaml
sourceRef:
  kind: GitRepository
  name: my-repo
  namespace: microscaler-system
```

### ArgoCD Application

```yaml
sourceRef:
  kind: Application
  name: my-app
  namespace: argocd
```

## Provider Configuration

### GCP Secret Manager

```yaml
provider:
  gcp:
    projectId: my-gcp-project
```

**Authentication:**
- Uses Workload Identity by default (recommended)
- Or service account key via Kubernetes Secret

### AWS Secrets Manager

```yaml
provider:
  aws:
    region: us-east-1
```

**Authentication:**
- Uses IRSA (IAM Roles for Service Accounts) by default (recommended)
- Or access keys via Kubernetes Secret

### Azure Key Vault

```yaml
provider:
  azure:
    vaultUrl: https://my-vault.vault.azure.net/
```

**Authentication:**
- Uses Workload Identity by default (recommended)
- Or service principal via Kubernetes Secret

## Secrets Configuration

### Basic Configuration

```yaml
secrets:
  environment: dev
  kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
```

- `environment`: Environment name (e.g., `dev`, `staging`, `prod`)
- `kustomizePath`: Path to Kustomize overlay in Git repository

### SOPS Decryption

Enable SOPS decryption:

```yaml
secrets:
  sops:
    enabled: true
    gpgSecretRef:
      name: sops-gpg-key
      namespace: microscaler-system
      key: private.key
```

**GPG Key Secret Format:**

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: sops-gpg-key
  namespace: microscaler-system
type: Opaque
data:
  private.key: <base64-encoded-gpg-private-key>
```

## Config Store Configuration

Route `application.properties` files to config stores instead of secret stores:

```yaml
configs:
  enabled: true
  # AWS: Parameter Store path prefix
  parameterPath: /my-service/dev
  # Azure: App Configuration endpoint
  appConfigEndpoint: https://my-app-config.azconfig.io
  # GCP: Store type (SecretManager or ParameterManager)
  store: SecretManager
```

## OpenTelemetry Configuration

Enable distributed tracing:

```yaml
otel:
  exporter: otlp  # or "datadog"
  endpoint: http://otel-collector:4317
  serviceName: secret-manager-controller
```

**Supported exporters:**
- `otlp`: OpenTelemetry Protocol (default)
- `datadog`: Direct Datadog export

## Timing Configuration

### Git Repository Pull Interval

How often to check for updates from Git:

```yaml
gitRepositoryPullInterval: 5m  # Default: 5m, minimum: 1m
```

**Recommendation:** 5m or greater to avoid Git API rate limits.

### Reconcile Interval

How often to reconcile secrets between Git and cloud provider:

```yaml
reconcileInterval: 1m  # Default: 1m
```

## Feature Flags

### Diff Discovery

Detect if secrets have been tampered with in cloud provider:

```yaml
diffDiscovery: true  # Default: false
```

When enabled, logs warnings when differences are found between Git (source of truth) and cloud provider.

## Environment Variables

The controller can also be configured via environment variables:

- `RUST_LOG`: Log level (e.g., `info`, `debug`, `trace`)
- `METRICS_PORT`: Metrics server port (default: `8080`)
- `HEALTH_PORT`: Health check port (default: `8081`)

## Validation

The controller validates your configuration:

- **Required fields**: `sourceRef`, `provider`, `secrets`
- **Provider-specific**: Required fields vary by provider
- **Intervals**: Must be valid Kubernetes duration strings

Check validation errors:

```bash
kubectl describe secretmanagerconfig my-config
```

## Examples

See the [Examples](../tutorials/basic-usage.md) section for complete working examples.

## Next Steps

- [API Reference](../api-reference/crd-reference.md) - Complete CRD reference
- [Provider Setup Guides](../guides/aws-setup.md) - Detailed provider configuration
- [Tutorials](../tutorials/basic-usage.md) - Step-by-step tutorials
