# Azure Setup Guide

Configure the Secret Manager Controller to work with Azure Key Vault.

## Prerequisites

- Azure subscription
- Azure Key Vault created
- Service principal or managed identity
- Kubernetes cluster with controller installed

## Authentication Methods

### Method 1: Managed Identity (Recommended)

If running on AKS, use managed identity:

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: secret-manager-controller
  namespace: microscaler-system
  annotations:
    azure.workload.identity/client-id: <client-id>
```

### Method 2: Service Principal

Create a Kubernetes Secret with Azure credentials:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: azure-credentials
  namespace: microscaler-system
type: Opaque
stringData:
  AZURE_CLIENT_ID: <client-id>
  AZURE_CLIENT_SECRET: <client-secret>
  AZURE_TENANT_ID: <tenant-id>
```

## Configuration Example

```yaml
apiVersion: secret-management.microscaler.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: azure-secrets
  namespace: production
spec:
  provider: azure
  vaultUrl: https://myvault.vault.azure.net/
  secrets:
    - name: db-password
      key: database-password
    - name: api-key
      key: api-key
```

## Required Permissions

Your service principal needs:
- `Key Vault Secrets User` role
- Or `Get` and `List` permissions on secrets

## Troubleshooting

See [AWS Setup Guide](./aws-setup.md) for common troubleshooting steps.

