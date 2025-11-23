# Advanced Scenarios

Advanced usage patterns and scenarios.

## Multiple Providers

You can use multiple providers in the same cluster:

```yaml
# AWS secrets
apiVersion: secret-management.microscaler.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: aws-secrets
spec:
  provider: aws
  region: us-east-1
  secrets: [...]
---
# Azure secrets
apiVersion: secret-management.microscaler.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: azure-secrets
spec:
  provider: azure
  vaultUrl: https://myvault.vault.azure.net/
  secrets: [...]
```

## Namespace Isolation

Create separate configurations per namespace:

```yaml
apiVersion: secret-management.microscaler.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: prod-secrets
  namespace: production
spec:
  provider: aws
  region: us-east-1
  secrets: [...]
---
apiVersion: secret-management.microscaler.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: dev-secrets
  namespace: development
spec:
  provider: aws
  region: us-east-1
  secrets: [...]
```

## GitOps with SOPS

Store encrypted secrets in Git:

1. Encrypt secrets with SOPS
2. Commit encrypted files to Git
3. Configure controller to decrypt:

```yaml
apiVersion: secret-management.microscaler.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: gitops-secrets
spec:
  provider: aws
  region: us-east-1
  gitRepository:
    name: my-repo
    namespace: flux-system
  sops:
    enabled: true
    keySecret:
      name: sops-key
  secrets:
    - name: config
      key: /myapp/config
      sopsFile: config.enc.yaml
```

## Version Pinning

Pin to specific secret versions:

```yaml
secrets:
  - name: stable-secret
    key: /myapp/secret
    version: "12345678-1234-1234-1234-123456789012"
```

## Update Policies

Control when secrets are updated:

```yaml
spec:
  updatePolicy: OnChange  # Only update when provider value changes
  secrets: [...]
```

## Learn More

- [Troubleshooting](./troubleshooting.md)
- [API Reference](../api-reference/)

