# Provider APIs

API details for each cloud provider.

## AWS Secrets Manager

### Endpoints

- `GetSecretValue` - Retrieve secret value
- `DescribeSecret` - Get secret metadata
- `ListSecrets` - List available secrets

### Authentication

- IAM roles (recommended)
- Access keys
- Temporary credentials

### Regions

All AWS regions are supported. Specify in `spec.region`.

## Azure Key Vault

### Endpoints

- `Get Secret` - Retrieve secret value
- `List Secrets` - List available secrets

### Authentication

- Managed Identity (recommended)
- Service Principal
- Client certificates

### Vault URL Format

`https://<vault-name>.vault.azure.net/`

## GCP Secret Manager

### Endpoints

- `projects.secrets.versions.access` - Retrieve secret value
- `projects.secrets.list` - List available secrets

### Authentication

- Workload Identity (recommended)
- Service Account keys
- Application Default Credentials

### Project Format

Specify GCP project ID in `spec.project`.

## Error Handling

All providers return standardized errors:
- `AuthenticationError` - Credential issues
- `NotFoundError` - Secret doesn't exist
- `PermissionError` - Insufficient permissions
- `NetworkError` - Connection issues

## Rate Limiting

The controller implements rate limiting and retry logic for all providers.

