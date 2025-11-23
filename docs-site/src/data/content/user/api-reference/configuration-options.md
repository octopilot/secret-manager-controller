# Configuration Options

Complete list of configuration options for Secret Manager Controller.

## Controller Configuration

### Environment Variables

- `LOG_LEVEL` - Logging level (`debug`, `info`, `warn`, `error`)
- `METRICS_PORT` - Port for metrics endpoint (default: `9090`)
- `RECONCILE_INTERVAL` - Reconciliation interval in seconds (default: `300`)

### ConfigMap Options

Create a ConfigMap in `microscaler-system` namespace:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: secret-manager-controller-config
  namespace: microscaler-system
data:
  log-level: "info"
  metrics-port: "9090"
  reconcile-interval: "300"
```

## SecretManagerConfig Options

### Provider-Specific Options

#### AWS

- `region` - AWS region (required)
- `endpoint` - Custom endpoint (optional, for testing)

#### Azure

- `vaultUrl` - Key Vault URL (required)
- `tenantId` - Azure tenant ID (optional, from credentials)

#### GCP

- `project` - GCP project ID (required)
- `location` - Secret location (optional, default: `global`)

### Secret Options

Each secret in `spec.secrets` can have:
- `name` - Kubernetes Secret name (required)
- `key` - Provider secret key/path (required)
- `version` - Specific version (optional)
- `encoding` - Value encoding (`base64`, `plain`) (optional)

### Update Policy

- `Always` - Always update Kubernetes Secrets
- `OnChange` - Only update when provider value changes

## Advanced Options

### Git Repository Integration

- `gitRepository.name` - FluxCD GitRepository name
- `gitRepository.namespace` - GitRepository namespace
- `gitRepository.path` - Path within repository

### SOPS Configuration

- `sops.enabled` - Enable SOPS decryption
- `sops.keySecret.name` - Secret containing SOPS key
- `sops.keySecret.namespace` - Secret namespace

