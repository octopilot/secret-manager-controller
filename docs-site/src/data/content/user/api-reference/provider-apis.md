# Provider APIs

Comprehensive API reference for cloud provider secret management services used by the Secret Manager Controller.

## Overview

The Secret Manager Controller interacts with three cloud provider secret management services:

- **AWS Secrets Manager**: Amazon Web Services secret management service
- **GCP Secret Manager**: Google Cloud Platform secret management service
- **Azure Key Vault**: Microsoft Azure key vault secrets service

Each provider has its own API structure, authentication methods, and operational patterns. This document details the specific endpoints, operations, and configurations used by the controller.

---

## AWS Secrets Manager

### API Structure

AWS Secrets Manager uses a **single POST endpoint** (`/`) with an `x-amz-target` header to specify the operation. All requests are sent to the same path with different header values.

**Base Endpoint:**
```
POST https://secretsmanager.{region}.amazonaws.com/
```

**Request Format:**
- **Method**: `POST`
- **Path**: `/`
- **Header**: `x-amz-target: secretsmanager.{Operation}`
- **Body**: JSON request payload

### Operations Used by Controller

#### 1. CreateSecret

**Operation:** `secretsmanager.CreateSecret`  
**Purpose:** Create a new secret

**Request:**
```json
{
  "Name": "my-secret",
  "SecretString": "secret-value",
  "ClientRequestToken": "optional-uuid"
}
```

**Response:**
```json
{
  "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:my-secret-abc123",
  "Name": "my-secret",
  "VersionId": "uuid-version-id"
}
```

#### 2. PutSecretValue

**Operation:** `secretsmanager.PutSecretValue`  
**Purpose:** Add a new version to an existing secret

**Request:**
```json
{
  "SecretId": "my-secret",
  "SecretString": "new-secret-value",
  "ClientRequestToken": "optional-uuid"
}
```

**Response:**
```json
{
  "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:my-secret-abc123",
  "Name": "my-secret",
  "VersionId": "uuid-version-id",
  "VersionStages": ["AWSCURRENT"]
}
```

#### 3. GetSecretValue

**Operation:** `secretsmanager.GetSecretValue`  
**Purpose:** Retrieve the secret value

**Request:**
```json
{
  "SecretId": "my-secret",
  "VersionStage": "AWSCURRENT"
}
```

**Response:**
```json
{
  "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:my-secret-abc123",
  "Name": "my-secret",
  "SecretString": "secret-value",
  "VersionId": "uuid-version-id",
  "VersionStages": ["AWSCURRENT"]
}
```

#### 4. DescribeSecret

**Operation:** `secretsmanager.DescribeSecret`  
**Purpose:** Get secret metadata (used to check if secret exists)

**Request:**
```json
{
  "SecretId": "my-secret"
}
```

**Response:**
```json
{
  "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:my-secret-abc123",
  "Name": "my-secret",
  "Description": "Secret description",
  "LastChangedDate": 1234567890.0,
  "LastAccessedDate": 1234567890.0,
  "VersionIdsToStages": {
    "uuid-version-id": ["AWSCURRENT"]
  }
}
```

#### 5. DeleteSecret

**Operation:** `secretsmanager.DeleteSecret`  
**Purpose:** Delete a secret (with optional recovery window)

**Request:**
```json
{
  "SecretId": "my-secret",
  "RecoveryWindowInDays": 7
}
```

**Response:**
```json
{
  "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:my-secret-abc123",
  "Name": "my-secret",
  "DeletionDate": 1234567890.0
}
```

#### 6. RestoreSecret

**Operation:** `secretsmanager.RestoreSecret`  
**Purpose:** Restore a deleted secret

**Request:**
```json
{
  "SecretId": "my-secret"
}
```

**Response:**
```json
{
  "ARN": "arn:aws:secretsmanager:us-east-1:123456789012:secret:my-secret-abc123",
  "Name": "my-secret"
}
```

### Authentication

The controller supports multiple AWS authentication methods:

#### 1. IRSA (IAM Roles for Service Accounts) - Recommended

**How it works:**
- Kubernetes ServiceAccount annotated with IAM role ARN
- AWS SDK automatically assumes the role using OIDC
- No credentials stored in Kubernetes

**Configuration:**
```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: secret-manager-controller
  namespace: microscaler-system
  annotations:
    eks.amazonaws.com/role-arn: arn:aws:iam::123456789012:role/secret-manager-role
```

**Benefits:**
- No credential management
- Automatic credential rotation
- Fine-grained IAM permissions
- Works with EKS clusters

#### 2. Access Keys

**Configuration:**
```yaml
spec:
  provider:
    aws:
      region: us-east-1
      auth:
        accessKey:
          accessKeyId: <key-id>
          secretAccessKey: <secret-key>
```

**Note:** Not recommended for production (credentials stored in CRD)

#### 3. Default Credential Chain

If no auth is specified, AWS SDK uses default credential chain:
- Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`)
- IAM instance profile (EC2/ECS)
- IRSA (EKS)
- Shared credentials file (`~/.aws/credentials`)

### Regions

All AWS regions are supported. Specify in `spec.provider.aws.region`:

```yaml
spec:
  provider:
    aws:
      region: us-east-1  # or eu-west-1, ap-southeast-1, etc.
```

### Error Handling

**Common Errors:**

| Error Code | Description | Controller Behavior |
|------------|-------------|---------------------|
| `ResourceNotFoundException` | Secret doesn't exist | Creates new secret |
| `InvalidParameterException` | Invalid request | Returns error, logs warning |
| `InvalidRequestException` | Invalid operation | Returns error, logs warning |
| `LimitExceededException` | Rate limit exceeded | Retries with exponential backoff |
| `AccessDeniedException` | Insufficient permissions | Returns error, logs error |
| `DecryptionFailureException` | Decryption failed | Returns error, logs error |

**Retry Logic:**
- Transient errors (rate limits, network issues) are retried
- Exponential backoff with jitter
- Maximum retry attempts: 3

### Rate Limiting

AWS Secrets Manager has rate limits:
- **API requests**: 5,000 requests per second per account
- **Burst capacity**: Up to 10,000 requests per second

The controller implements:
- Exponential backoff on rate limit errors
- Request throttling for high-volume operations
- Metrics tracking for rate limit events

---

## GCP Secret Manager

### API Structure

GCP Secret Manager uses **RESTful HTTP endpoints** with standard HTTP methods.

**Base URL:**
```
https://secretmanager.googleapis.com/v1/projects/{project}/secrets
```

**API Version:** `v1`

### Operations Used by Controller

#### 1. Create Secret

**Endpoint:** `POST /v1/projects/{project}/secrets`  
**Purpose:** Create a new secret (metadata only, no value)

**Request:**
```json
{
  "secretId": "my-secret",
  "replication": {
    "automatic": {}
  }
}
```

**Response:**
```json
{
  "name": "projects/my-project/secrets/my-secret",
  "replication": {
    "automatic": {}
  },
  "createTime": "2024-01-15T10:30:45Z"
}
```

#### 2. Add Secret Version

**Endpoint:** `POST /v1/projects/{project}/secrets/{secret}:addVersion`  
**Purpose:** Add a new version with the secret value

**Request:**
```json
{
  "payload": {
    "data": "base64-encoded-secret-value"
  }
}
```

**Response:**
```json
{
  "name": "projects/my-project/secrets/my-secret/versions/1",
  "createTime": "2024-01-15T10:30:45Z",
  "state": "ENABLED"
}
```

#### 3. Access Secret Version

**Endpoint:** `GET /v1/projects/{project}/secrets/{secret}/versions/latest:access`  
**Purpose:** Retrieve the secret value (latest version)

**Request:**
```
GET /v1/projects/my-project/secrets/my-secret/versions/latest:access
```

**Response:**
```json
{
  "name": "projects/my-project/secrets/my-secret/versions/1",
  "payload": {
    "data": "base64-encoded-secret-value"
  }
}
```

**Note:** The controller decodes the base64-encoded `data` field to get the actual secret value.

#### 4. Delete Secret

**Endpoint:** `DELETE /v1/projects/{project}/secrets/{secret}`  
**Purpose:** Delete a secret (all versions)

**Request:**
```
DELETE /v1/projects/my-project/secrets/my-secret
```

**Response:**
```
200 OK (empty body)
```

#### 5. List Secrets

**Endpoint:** `GET /v1/projects/{project}/secrets`  
**Purpose:** List all secrets in a project (used for drift detection)

**Request:**
```
GET /v1/projects/my-project/secrets?pageSize=100
```

**Response:**
```json
{
  "secrets": [
    {
      "name": "projects/my-project/secrets/my-secret",
      "replication": {
        "automatic": {}
      }
    }
  ],
  "nextPageToken": "optional-pagination-token"
}
```

### Authentication

The controller supports multiple GCP authentication methods:

#### 1. Workload Identity - Recommended

**How it works:**
- Kubernetes ServiceAccount bound to GCP Service Account
- GCP Service Account has permissions to access Secret Manager
- No credentials stored in Kubernetes

**Configuration:**
```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: secret-manager-controller
  namespace: microscaler-system
  annotations:
    iam.gke.io/gcp-service-account: secret-manager@my-project.iam.gserviceaccount.com
```

**Benefits:**
- No credential management
- Automatic credential rotation
- Fine-grained IAM permissions
- Works with GKE clusters

#### 2. Service Account Key

**Configuration:**
```yaml
spec:
  provider:
    gcp:
      projectId: my-project
      auth:
        serviceAccountKey:
          secretRef:
            name: gcp-service-account-key
            namespace: microscaler-system
            key: key.json
```

**Note:** Service account key JSON stored in Kubernetes Secret

#### 3. Application Default Credentials (ADC)

If no auth is specified, GCP SDK uses Application Default Credentials:
- Environment variable (`GOOGLE_APPLICATION_CREDENTIALS`)
- Workload Identity (GKE)
- Compute Engine service account (GCE)
- User credentials (`gcloud auth application-default login`)

### Project Configuration

Specify GCP project ID in `spec.provider.gcp.projectId`:

```yaml
spec:
  provider:
    gcp:
      projectId: my-gcp-project
```

### Error Handling

**Common Errors:**

| HTTP Status | Error Code | Description | Controller Behavior |
|-------------|------------|-------------|---------------------|
| `404` | `NOT_FOUND` | Secret doesn't exist | Creates new secret |
| `400` | `INVALID_ARGUMENT` | Invalid request | Returns error, logs warning |
| `403` | `PERMISSION_DENIED` | Insufficient permissions | Returns error, logs error |
| `429` | `RESOURCE_EXHAUSTED` | Rate limit exceeded | Retries with exponential backoff |
| `500` | `INTERNAL` | Server error | Retries with exponential backoff |
| `503` | `UNAVAILABLE` | Service unavailable | Retries with exponential backoff |

**Retry Logic:**
- Transient errors (429, 500, 503) are retried
- Exponential backoff with jitter
- Maximum retry attempts: 3

### Rate Limiting

GCP Secret Manager has rate limits:
- **API requests**: 60 requests per minute per project (default)
- **Burst capacity**: Up to 120 requests per minute

The controller implements:
- Exponential backoff on rate limit errors
- Request throttling for high-volume operations
- Metrics tracking for rate limit events

---

## Azure Key Vault

### API Structure

Azure Key Vault uses **RESTful HTTP endpoints** with standard HTTP methods and an `api-version` query parameter.

**Base URL:**
```
https://{vault-name}.vault.azure.net/secrets
```

**API Version:** `7.4` (latest)

### Operations Used by Controller

#### 1. Set Secret

**Endpoint:** `PUT /secrets/{name}?api-version=7.4`  
**Purpose:** Create or update a secret (creates new version automatically)

**Request:**
```json
{
  "value": "secret-value",
  "contentType": "text/plain",
  "attributes": {
    "enabled": true
  }
}
```

**Response:**
```json
{
  "value": "secret-value",
  "id": "https://my-vault.vault.azure.net/secrets/my-secret/abc123",
  "attributes": {
    "enabled": true,
    "created": 1234567890,
    "updated": 1234567890,
    "recoveryLevel": "Recoverable+Purgeable"
  }
}
```

#### 2. Get Secret

**Endpoint:** `GET /secrets/{name}/?api-version=7.4`  
**Purpose:** Retrieve the latest version of a secret

**Request:**
```
GET /secrets/my-secret/?api-version=7.4
```

**Note:** Trailing slash is required for latest version

**Response:**
```json
{
  "value": "secret-value",
  "id": "https://my-vault.vault.azure.net/secrets/my-secret/abc123",
  "attributes": {
    "enabled": true,
    "created": 1234567890,
    "updated": 1234567890
  }
}
```

#### 3. Get Secret Version

**Endpoint:** `GET /secrets/{name}/{version}?api-version=7.4`  
**Purpose:** Retrieve a specific version of a secret

**Request:**
```
GET /secrets/my-secret/abc123?api-version=7.4
```

**Response:**
```json
{
  "value": "secret-value",
  "id": "https://my-vault.vault.azure.net/secrets/my-secret/abc123",
  "attributes": {
    "enabled": true,
    "created": 1234567890,
    "updated": 1234567890
  }
}
```

#### 4. Delete Secret

**Endpoint:** `DELETE /secrets/{name}?api-version=7.4`  
**Purpose:** Delete a secret (soft-delete, goes to deletedsecrets)

**Request:**
```
DELETE /secrets/my-secret?api-version=7.4
```

**Response:**
```json
{
  "recoveryId": "https://my-vault.vault.azure.net/deletedsecrets/my-secret",
  "deletedDate": 1234567890,
  "scheduledPurgeDate": 1234567890
}
```

#### 5. Update Secret Attributes

**Endpoint:** `PATCH /secrets/{name}?api-version=7.4`  
**Purpose:** Update secret attributes (enabled/disabled, tags, etc.)

**Request:**
```json
{
  "attributes": {
    "enabled": false
  }
}
```

**Response:**
```json
{
  "id": "https://my-vault.vault.azure.net/secrets/my-secret/abc123",
  "attributes": {
    "enabled": false,
    "updated": 1234567890
  }
}
```

### Authentication

The controller supports multiple Azure authentication methods:

#### 1. Workload Identity - Recommended

**How it works:**
- Kubernetes ServiceAccount bound to Azure Managed Identity
- Azure Managed Identity has permissions to access Key Vault
- No credentials stored in Kubernetes

**Configuration:**
```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: secret-manager-controller
  namespace: microscaler-system
  annotations:
    azure.workload.identity/client-id: <managed-identity-client-id>
```

**Benefits:**
- No credential management
- Automatic credential rotation
- Fine-grained RBAC permissions
- Works with AKS clusters

#### 2. Service Principal

**Configuration:**
```yaml
spec:
  provider:
    azure:
      vaultUrl: https://my-vault.vault.azure.net/
      auth:
        servicePrincipal:
          clientId: <client-id>
          clientSecret:
            secretRef:
              name: azure-service-principal
              namespace: microscaler-system
              key: client-secret
          tenantId: <tenant-id>
```

**Note:** Client secret stored in Kubernetes Secret

#### 3. Managed Identity

**Configuration:**
```yaml
spec:
  provider:
    azure:
      vaultUrl: https://my-vault.vault.azure.net/
      auth:
        managedIdentity:
          clientId: <managed-identity-client-id>
```

### Vault URL Format

Specify vault URL in `spec.provider.azure.vaultUrl`:

```yaml
spec:
  provider:
    azure:
      vaultUrl: https://my-vault.vault.azure.net/
```

**Format:** `https://{vault-name}.vault.azure.net/`

### Error Handling

**Common Errors:**

| HTTP Status | Error Code | Description | Controller Behavior |
|-------------|------------|-------------|---------------------|
| `404` | `SecretNotFound` | Secret doesn't exist | Creates new secret |
| `400` | `BadParameter` | Invalid request | Returns error, logs warning |
| `401` | `Unauthorized` | Authentication failed | Returns error, logs error |
| `403` | `Forbidden` | Insufficient permissions | Returns error, logs error |
| `429` | `TooManyRequests` | Rate limit exceeded | Retries with exponential backoff |
| `500` | `InternalServerError` | Server error | Retries with exponential backoff |
| `503` | `ServiceUnavailable` | Service unavailable | Retries with exponential backoff |

**Retry Logic:**
- Transient errors (429, 500, 503) are retried
- Exponential backoff with jitter
- Maximum retry attempts: 3

### Rate Limiting

Azure Key Vault has rate limits:
- **API requests**: Varies by SKU (Standard: 2,000 requests per 10 seconds)
- **Burst capacity**: Up to 4,000 requests per 10 seconds

The controller implements:
- Exponential backoff on rate limit errors
- Request throttling for high-volume operations
- Metrics tracking for rate limit events

---

## Common Patterns

### Secret Versioning

All three providers support secret versioning:

- **AWS**: Automatic versioning with staging labels (`AWSCURRENT`, `AWSPENDING`, etc.)
- **GCP**: Explicit version numbers (1, 2, 3, ...)
- **Azure**: Automatic versioning with version IDs (UUIDs)

The controller:
- Always uses the latest version when reading secrets
- Creates new versions when updating secrets
- Tracks version changes for drift detection

### Secret Naming

**AWS:**
- Alphanumeric characters, hyphens, underscores
- Length: 1-512 characters
- Case-sensitive

**GCP:**
- Alphanumeric characters, hyphens, underscores
- Length: 1-255 characters
- Case-sensitive

**Azure:**
- Alphanumeric characters, hyphens
- Length: 1-127 characters
- Case-insensitive (stored as lowercase)

The controller sanitizes secret names to comply with provider requirements.

### Error Classification

The controller classifies errors as:

1. **Transient Errors**: Retryable (rate limits, network issues, server errors)
2. **Permanent Errors**: Non-retryable (authentication failures, invalid requests, not found)

Transient errors are retried with exponential backoff. Permanent errors are logged and returned immediately.

### Metrics

All provider operations are tracked with metrics:

- **Operation duration**: Time taken for each operation
- **Operation count**: Total operations by provider and type
- **Error count**: Errors by provider and type
- **Rate limit events**: Rate limit errors by provider

See [Metrics Documentation](../monitoring/metrics.md) for details.

---

## Summary

| Provider | API Style | Authentication | Rate Limits | Versioning |
|----------|-----------|----------------|-------------|------------|
| **AWS** | Single POST endpoint with headers | IRSA, Access Keys, Default Chain | 5,000 req/s | Automatic with stages |
| **GCP** | RESTful HTTP endpoints | Workload Identity, Service Account Key, ADC | 60 req/min | Explicit versions |
| **Azure** | RESTful HTTP endpoints | Workload Identity, Service Principal, Managed Identity | 2,000 req/10s | Automatic with UUIDs |

All providers support:
- Create/update secrets
- Retrieve secret values
- Delete secrets
- Error handling and retries
- Rate limiting and throttling
- Metrics and observability
