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
  namespace: microscaler-system
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
  namespace: microscaler-system
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
apiVersion: secret-management.microscaler.io/v1beta1
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

Your service account needs:
- `Secret Manager Secret Accessor` role
- Or `secretmanager.secrets.get` permission

## Troubleshooting

See [AWS Setup Guide](./aws-setup.md) for common troubleshooting steps.

