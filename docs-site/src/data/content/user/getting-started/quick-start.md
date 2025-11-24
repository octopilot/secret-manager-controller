# Quick Start

Get the Secret Manager Controller up and running in minutes with this quick start guide.

## Prerequisites

- Kubernetes cluster with kubectl access
- GitOps tool (FluxCD or ArgoCD) installed
- Cloud provider credentials configured (see [Installation](./installation.md))

## Step 1: Create a GitRepository (FluxCD)

If you're using FluxCD, create a GitRepository resource:

```yaml
apiVersion: source.toolkit.fluxcd.io/v1
kind: GitRepository
metadata:
  name: my-secrets-repo
  namespace: microscaler-system
spec:
  url: https://github.com/your-org/your-secrets-repo
  interval: 5m
  ref:
    branch: main
```

Apply it:

```bash
kubectl apply -f gitrepository.yaml
```

## Step 2: Create a SecretManagerConfig

Create a `SecretManagerConfig` resource that references your GitRepository:

```yaml
apiVersion: secret-management.microscaler.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-service-secrets
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-secrets-repo
    namespace: microscaler-system
  provider:
    gcp:
      projectId: my-gcp-project
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
```

**Key fields:**
- `sourceRef`: References your GitRepository or ArgoCD Application
- `provider`: Your cloud provider configuration (GCP, AWS, or Azure)
- `secrets.environment`: The environment name (e.g., `dev`, `staging`, `prod`)
- `secrets.kustomizePath`: Path to your Kustomize overlay in the Git repository

## Step 3: Apply the Configuration

```bash
kubectl apply -f secretmanagerconfig.yaml
```

## Step 4: Verify Sync

Check the status of your SecretManagerConfig:

```bash
kubectl get secretmanagerconfig my-service-secrets -n default
```

You should see output like:

```
NAME                  PHASE      DESCRIPTION                    READY
my-service-secrets    Synced    Successfully synced 5 secrets  True
```

Check the detailed status:

```bash
kubectl describe secretmanagerconfig my-service-secrets -n default
```

## Step 5: Verify Secrets in Cloud Provider

### GCP Secret Manager

```bash
gcloud secrets list --project=my-gcp-project
```

### AWS Secrets Manager

```bash
aws secretsmanager list-secrets --region us-east-1
```

### Azure Key Vault

```bash
az keyvault secret list --vault-name my-vault
```

## Example: SOPS-Encrypted Secrets

If your secrets are encrypted with SOPS, the controller will automatically decrypt them. Make sure you have:

1. **SOPS-encrypted files** in your Git repository
2. **GPG private key** stored in a Kubernetes Secret:

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

3. **Reference the key** in your SecretManagerConfig:

```yaml
spec:
  secrets:
    sops:
      enabled: true
      gpgSecretRef:
        name: sops-gpg-key
        namespace: microscaler-system
        key: private.key
```

## Troubleshooting

### Controller Not Syncing

Check the controller logs:

```bash
kubectl logs -n microscaler-system -l app=secret-manager-controller --tail=100
```

Common issues:
- **GitRepository not found**: Verify the `sourceRef` name and namespace
- **Authentication errors**: Check cloud provider credentials
- **SOPS decryption failures**: Verify GPG key is correct

### Secrets Not Appearing in Cloud Provider

1. Check the SecretManagerConfig status for errors
2. Verify the `kustomizePath` is correct
3. Ensure secrets are properly formatted in your Git repository

## Next Steps

- [Configuration Guide](./configuration.md) - Learn about all configuration options
- [Provider Setup Guides](../guides/aws-setup.md) - Detailed provider configuration
- [Architecture Overview](../architecture/overview.md) - Understand the system architecture
