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
  namespace: octopilot-system
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
  namespace: octopilot-system
type: Opaque
stringData:
  AZURE_CLIENT_ID: <client-id>
  AZURE_CLIENT_SECRET: <client-secret>
  AZURE_TENANT_ID: <tenant-id>
```

## Configuration Example

```yaml
apiVersion: secret-management.octopilot.io/v1beta1
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

Your Azure service principal or managed identity needs permissions to create, read, update, and delete secrets in Key Vault.

### Minimum Required Permissions

The identity needs the following Key Vault access policies:

- **Get** - Read secret values
- **List** - List secrets in the vault
- **Set** - Create and update secrets
- **Delete** - Delete secrets (optional, for cleanup)

### Using Azure RBAC Roles

#### Option 1: Key Vault Secrets Officer (Recommended)

```bash
az role assignment create \
  --role "Key Vault Secrets Officer" \
  --assignee <service-principal-id> \
  --scope /subscriptions/<subscription-id>/resourceGroups/<resource-group>/providers/Microsoft.KeyVault/vaults/<vault-name>
```

This role provides full access to secrets, including:
- Creating secrets
- Reading secret values
- Updating secrets
- Deleting secrets
- Listing secrets

#### Option 2: Key Vault Secrets User (Read-Only)

If you only need read access:

```bash
az role assignment create \
  --role "Key Vault Secrets User" \
  --assignee <service-principal-id> \
  --scope /subscriptions/<subscription-id>/resourceGroups/<resource-group>/providers/Microsoft.KeyVault/vaults/<vault-name>
```

**Note:** This role is read-only and does not allow creating or updating secrets.

### Using Key Vault Access Policies (Legacy)

If using access policies instead of RBAC:

```bash
az keyvault set-policy \
  --name <vault-name> \
  --spn <service-principal-id> \
  --secret-permissions get list set delete
```

### Managed Identity Permissions

If using managed identity with AKS:

1. **Enable managed identity** on your AKS cluster
2. **Assign the identity** to the Key Vault:

```bash
az role assignment create \
  --role "Key Vault Secrets Officer" \
  --assignee <managed-identity-client-id> \
  --scope /subscriptions/<subscription-id>/resourceGroups/<resource-group>/providers/Microsoft.KeyVault/vaults/<vault-name>
```

### Verify Permissions

Test that your service principal has the required permissions:

```bash
# List secrets (requires List permission)
az keyvault secret list --vault-name <vault-name>

# Get a secret (requires Get permission)
az keyvault secret show --vault-name <vault-name> --name <secret-name>

# Set a secret (requires Set permission)
az keyvault secret set --vault-name <vault-name> --name test-secret --value test-value
```

## Troubleshooting

See [AWS Setup Guide](./aws-setup.md) for common troubleshooting steps.

