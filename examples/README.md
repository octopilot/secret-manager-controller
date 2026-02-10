# Secret Manager Controller Examples

This directory contains example `SecretManagerConfig` resources for configuring the Secret Manager Controller to sync secrets from Flux GitRepositories to cloud secret managers (GCP Secret Manager, AWS Secrets Manager, Azure Key Vault).

## Main Examples

**For the standard setup, see:**
- `../gitops/cluster/env/` - Complete GitOps setup with GitRepository and SecretManagerConfig for each environment
- `../deployment-configuration/` - Example deployment configuration structure with SOPS-encrypted secrets

## Project Structure Support

The controller supports two project structures:

### 1. Monolith Structure (Multiple Services)

**Base Path:** Optional - specify if services are under a subdirectory (e.g., `microservices`, `services`, `apps`)

**Directory Structure:**
```
microservices/
  {service-name}/
    deployment-configuration/
      profiles/
        {env}/
          ├── application.properties
          └── application.secrets.env
```

**Examples:** See `idam-dev-secret-manager-config.yaml` and `idam-prd-secret-manager-config.yaml`

### 2. Single Service Structure

**Base Path:** Optional - omit for root, or use `"."` to explicitly indicate root

**Directory Structure:**
```
deployment-configuration/
  profiles/
    {env}/
      ├── application.properties
      └── application.secrets.env
```

**Examples:** 
- `single-service-secret-manager-config.yaml` - Shows explicit `basePath: "."`
- `single-service-no-basepath.yaml` - Shows omitting `basePath` (recommended for root)

**Note:** The `profiles/` directory structure is Skaffold-compliant. See [Skaffold documentation](https://skaffold.dev/) for more information.

## Cloud Provider Examples

### Google Cloud Platform (GCP)

**Files:**
- `idam-dev-secret-manager-config.yaml` - GCP with default authentication
- `idam-dev-workload-identity-secret-manager-config.yaml` - GCP with Workload Identity

### Amazon Web Services (AWS)

**File:** `idam-dev-aws-secret-manager-config.yaml`

This example syncs secrets to AWS Secrets Manager using IRSA (IAM Roles for Service Accounts).

### Microsoft Azure

**File:** `idam-dev-azure-secret-manager-config.yaml`

This example syncs secrets to Azure Key Vault using Workload Identity.

## Operation Modes

### 1. Kustomize Build Mode (Recommended)

**When to use:** When you use kustomize overlays, patches, or generators to modify secrets.

**How it works:**
- Controller runs `kustomize build` on the specified path
- Extracts secrets from generated Kubernetes Secret resources
- Supports all kustomize features (overlays, patches, generators)
- Works with any GitOps tool (FluxCD, ArgoCD, etc.)

**Example:** See `idam-dev-kustomize-secret-manager-config.yaml`

### 2. Raw File Mode

**When to use:** Simple setups without kustomize overlays/patches.

**How it works:**
- Controller reads `application.secrets.env` files directly
- Simpler but doesn't support kustomize overlays/patches

**Example:** See `idam-dev-secret-manager-config.yaml`

## Source Reference Examples

### FluxCD GitRepository

Most examples use FluxCD GitRepository. For complete GitOps setup examples, see `../gitops/cluster/env/`.

### ArgoCD Application

For ArgoCD users:

**Example:** See `idam-dev-argocd-secret-manager-config.yaml`

**Note:** ArgoCD support requires Git repository access. The controller extracts Git source information from the Application and attempts to access the repository. You may need to configure Git credentials or ensure the repository is accessible.

## Custom Environment Names

Projects using Skaffold may use custom environment names:
- `dev-cf` - Development Cloud Foundry
- `pp-cf` - Pre-production Cloud Foundry
- `prod-cf` - Production Cloud Foundry

**Example:** See `sam-activity-example.yaml` for a service with custom environment names.

**Note:** You need separate `SecretManagerConfig` resources for each environment you want to sync.

## Quick Start

1. **Set up deployment configuration** (see `../deployment-configuration/README.md`)
2. **Create GitRepository** (see `../gitops/cluster/env/` for examples)
3. **Create SecretManagerConfig** using one of the examples in this directory
4. **Apply to cluster:**
   ```bash
   kubectl apply -f examples/idam-dev-secret-manager-config.yaml
   ```

## Prerequisites

Before applying these examples, ensure:

1. **Source Resource exists:**
   - For FluxCD: Create GitRepository (see `../gitops/cluster/env/`)
   - For ArgoCD: Ensure Application references a Git repository

2. **Cloud Provider Configuration:**
   - GCP: Replace project IDs, ensure Secret Manager API is enabled
   - AWS: Replace region, ensure Secrets Manager API is enabled
   - Azure: Replace vault name, ensure Key Vault exists

3. **SOPS Private Key:**
   - The controller needs access to the SOPS private key to decrypt `application.secrets.env` files
   - See the main [README.md](../README.md) for details on SOPS key configuration

## Verification

After applying a `SecretManagerConfig`, verify secrets are synced:

```bash
# Check controller logs
kubectl logs -n octopilot-system -l app=secret-manager-controller --tail=50

# Check GCP Secret Manager (requires gcloud CLI)
gcloud secrets list --project=your-project --filter="name:your-prefix-*"
```

## Troubleshooting

### SecretManagerConfig Not Ready

If the status shows `Ready=False`, check:

1. **GitRepository Status:**
   ```bash
   kubectl get gitrepository <name> -n flux-system -o yaml
   ```
   Ensure the GitRepository has an artifact in its status.

2. **Controller Logs:**
   ```bash
   kubectl logs -n octopilot-system -l app=secret-manager-controller --tail=100
   ```

3. **Cloud Provider Authentication:**
   Check service account permissions and authentication setup.

## File Reference

- `idam-dev-secret-manager-config.yaml` - Basic GCP example (monolith structure)
- `idam-prd-secret-manager-config.yaml` - Production GCP example
- `idam-dev-workload-identity-secret-manager-config.yaml` - GCP with Workload Identity
- `idam-dev-aws-secret-manager-config.yaml` - AWS Secrets Manager example
- `idam-dev-azure-secret-manager-config.yaml` - Azure Key Vault example
- `idam-dev-kustomize-secret-manager-config.yaml` - Kustomize build mode example
- `idam-dev-argocd-secret-manager-config.yaml` - ArgoCD Application source example
- `single-service-secret-manager-config.yaml` - Single service with explicit basePath
- `single-service-no-basepath.yaml` - Single service without basePath
- `sam-activity-example.yaml` - Custom environment names example
