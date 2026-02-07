# GCP Setup Guide

Configure the Secret Manager Controller to work with GCP Secret Manager.

## Prerequisites

- GCP project with Secret Manager API enabled
- Service account with appropriate permissions
- Kubernetes cluster with controller installed

## Authentication Methods

### Method 1: Workload Identity (Recommended)

If running on GKE, use Workload Identity:

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: secret-manager-controller
  namespace: octopilot-system
  annotations:
    iam.gke.io/gcp-service-account: secret-manager@PROJECT_ID.iam.gserviceaccount.com
```

### Method 2: Service Account Key

Create a Kubernetes Secret with GCP credentials:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: gcp-credentials
  namespace: octopilot-system
type: Opaque
stringData:
  GOOGLE_APPLICATION_CREDENTIALS_JSON: |
    {
      "type": "service_account",
      "project_id": "...",
      ...
    }
```

## Configuration Example

```yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: gcp-secrets
  namespace: production
spec:
  provider: gcp
  project: my-gcp-project
  secrets:
    - name: db-password
      key: database-password
    - name: api-key
      key: api-key
```

## Required Permissions

Your GCP service account needs permissions to create, read, update, and delete secrets in Secret Manager.

### Minimum Required Permissions

The service account needs the following IAM role:

- **`roles/secretmanager.admin`** - Full access to Secret Manager (recommended for controller)

Or grant specific permissions:

- `secretmanager.secrets.create`
- `secretmanager.secrets.get`
- `secretmanager.secrets.update`
- `secretmanager.secrets.delete`
- `secretmanager.secrets.list`

### Using Predefined Roles

#### Option 1: Secret Manager Admin (Recommended)

```bash
gcloud projects add-iam-policy-binding PROJECT_ID \
  --member="serviceAccount:secret-manager@PROJECT_ID.iam.gserviceaccount.com" \
  --role="roles/secretmanager.admin"
```

This role provides full access to Secret Manager, including:
- Creating secrets
- Reading secret values
- Updating secrets
- Deleting secrets
- Listing secrets

#### Option 2: Secret Manager Secret Accessor (Read-Only)

If you only need read access:

```bash
gcloud projects add-iam-policy-binding PROJECT_ID \
  --member="serviceAccount:secret-manager@PROJECT_ID.iam.gserviceaccount.com" \
  --role="roles/secretmanager.secretAccessor"
```

**Note:** This role is read-only and does not allow creating or updating secrets.

### Custom IAM Policy

For fine-grained control, create a custom IAM policy:

```json
{
  "bindings": [
    {
      "role": "roles/secretmanager.admin",
      "members": [
        "serviceAccount:secret-manager@PROJECT_ID.iam.gserviceaccount.com"
      ]
    }
  ]
}
```

### Workload Identity Permissions

If using Workload Identity, ensure the Kubernetes ServiceAccount is bound to the GCP ServiceAccount:

```bash
gcloud iam service-accounts add-iam-policy-binding \
  secret-manager@PROJECT_ID.iam.gserviceaccount.com \
  --role roles/iam.workloadIdentityUser \
  --member "serviceAccount:PROJECT_ID.svc.id.goog[octopilot-system/secret-manager-controller]"
```

### Verify Permissions

Test that your service account has the required permissions:

```bash
# List secrets (requires secretmanager.secrets.list)
gcloud secrets list --project=PROJECT_ID

# Create a test secret (requires secretmanager.secrets.create)
gcloud secrets create test-secret --data-file=- --project=PROJECT_ID
```

## Troubleshooting

See [AWS Setup Guide](./aws-setup.md) for common troubleshooting steps.

