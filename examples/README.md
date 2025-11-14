# Secret Manager Controller Examples

This directory contains example `SecretManagerConfig` resources for configuring the Secret Manager Controller to sync secrets from Flux GitRepositories to cloud secret managers (GCP Secret Manager, AWS Secrets Manager, Azure Key Vault).

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

**Example:** See `idam-dev-secret-manager-config.yaml` and `idam-prd-secret-manager-config.yaml`

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

### Backward Compatibility

The controller also supports legacy structures without the `profiles/` directory:
- `microservices/{service}/deployment-configuration/{env}/`
- `deployment-configuration/{env}/`

However, the `profiles/` structure is recommended for Skaffold compatibility.

## Cloud Provider Examples

### Google Cloud Platform (GCP)

**Files:**
- `idam-dev-secret-manager-config.yaml` - GCP with default authentication
- `idam-dev-workload-identity-secret-manager-config.yaml` - GCP with Workload Identity

### Amazon Web Services (AWS)

**File:** `idam-dev-aws-secret-manager-config.yaml`

This example syncs secrets to AWS Secrets Manager using IRSA (IAM Roles for Service Accounts):

```bash
# Apply the configuration
kubectl apply -f examples/idam-dev-aws-secret-manager-config.yaml

# Check the status
kubectl get secretmanagerconfig idam-dev-secrets-aws -n pricewhisperer
```

**Expected Secrets in AWS Secrets Manager:**
- `idam-dev-supabase-anon-key`
- `idam-dev-jwt-secret`
- `idam-dev-supabase-service-role-key`
- `idam-dev-properties`

### Microsoft Azure

**File:** `idam-dev-azure-secret-manager-config.yaml`

This example syncs secrets to Azure Key Vault using Workload Identity:

```bash
# Apply the configuration
kubectl apply -f examples/idam-dev-azure-secret-manager-config.yaml

# Check the status
kubectl get secretmanagerconfig idam-dev-secrets-azure -n pricewhisperer
```

**Expected Secrets in Azure Key Vault:**
- `idam-dev-supabase-anon-key`
- `idam-dev-jwt-secret`
- `idam-dev-supabase-service-role-key`
- `idam-dev-properties`

## IDAM Service Examples (Monolith Structure)

### Development Environment

**File:** `idam-dev-secret-manager-config.yaml`

This example syncs secrets from the IDAM service's development deployment configuration to GCP Secret Manager:

```bash
# Apply the configuration
kubectl apply -f examples/idam-dev-secret-manager-config.yaml

# Check the status
kubectl get secretmanagerconfig idam-dev-secrets -n pricewhisperer

# View detailed status
kubectl describe secretmanagerconfig idam-dev-secrets -n pricewhisperer
```

**Expected Secrets in GCP Secret Manager:**
- `idam-dev-supabase-anon-key`
- `idam-dev-jwt-secret`
- `idam-dev-supabase-service-role-key`
- `idam-dev-properties`

### Production Environment

**File:** `idam-prd-secret-manager-config.yaml`

This example syncs secrets from the IDAM service's production deployment configuration:

```bash
# Apply the configuration
kubectl apply -f examples/idam-prd-secret-manager-config.yaml

# Check the status
kubectl get secretmanagerconfig idam-prd-secrets -n pricewhisperer

# View detailed status
kubectl describe secretmanagerconfig idam-prd-secrets -n pricewhisperer
```

**Expected Secrets in GCP Secret Manager:**
- `idam-prd-supabase-anon-key`
- `idam-prd-jwt-secret`
- `idam-prd-supabase-service-role-key`
- `idam-prd-properties`

## Operation Modes

The controller supports two operation modes:

### 1. Kustomize Build Mode (Recommended)

**When to use:** When you use kustomize overlays, patches, or generators to modify secrets.

**How it works:**
- Controller runs `kustomize build` on the specified path
- Extracts secrets from generated Kubernetes Secret resources
- Supports all kustomize features (overlays, patches, generators)
- Works with any GitOps tool (FluxCD, ArgoCD, etc.)

**Example:** See `idam-dev-kustomize-secret-manager-config.yaml`

**Requirements:**
- `kustomize` binary must be available in controller container
- Path must contain `kustomization.yaml` with `secretGenerator` configuration

### 2. Raw File Mode

**When to use:** Simple setups without kustomize overlays/patches.

**How it works:**
- Controller reads `application.secrets.env` files directly
- Simpler but doesn't support kustomize overlays/patches

**Example:** See `idam-dev-secret-manager-config.yaml`

## Namespace Flexibility

**Important:** The controller watches `SecretManagerConfig` resources in **all namespaces**. You can deploy your `SecretManagerConfig` resources in any namespace where your services are deployed. The controller itself runs in the `microscaler-system` namespace (GitOps provider agnostic).

Examples show different namespaces:
- `pricewhisperer` - For PriceWhisperer services
- `default` - For general services
- Any custom namespace - Deploy where your services run

## Source Reference Examples

### FluxCD GitRepository

Most examples use FluxCD GitRepository:

```yaml
sourceRef:
  kind: GitRepository  # Default, can be omitted
  name: pricewhisperer-manifests
  namespace: flux-system
```

**Example:** See `idam-dev-secret-manager-config.yaml`

### ArgoCD Application

For ArgoCD users:

```yaml
sourceRef:
  kind: Application
  name: idam-app
  namespace: argocd
```

**Example:** See `idam-dev-argocd-secret-manager-config.yaml`

**Note:** ArgoCD support requires Git repository access. The controller extracts Git source information from the Application and attempts to access the repository. You may need to configure Git credentials or ensure the repository is accessible.

## Prerequisites

Before applying these examples, ensure:

1. **Source Resource exists:**

   **For FluxCD:**
   ```bash
   kubectl get gitrepository pricewhisperer-manifests -n flux-system
   ```
   
   If it doesn't exist, create it:
   ```yaml
   apiVersion: source.toolkit.fluxcd.io/v1beta2
   kind: GitRepository
   metadata:
     name: pricewhisperer-manifests
     namespace: flux-system
   spec:
     url: https://github.com/microscaler/PriceWhisperer
     interval: 5m
     ref:
       branch: main
   ```

   **For ArgoCD:**
   ```bash
   kubectl get application idam-app -n argocd
   ```
   
   Ensure your ArgoCD Application references a Git repository with the deployment configuration.

2. **Cloud Provider Configuration:**
   
   **For GCP:**
   - Replace `pricewhisperer-dev` and `pricewhisperer-prd` with your actual GCP project IDs
   - Ensure Secret Manager API is enabled in both projects
   - Ensure the controller's service account has `roles/secretmanager.admin` role
   
   **For AWS:**
   - Replace `us-east-1` with your AWS region
   - Ensure Secrets Manager API is enabled in your AWS account
   - Ensure the IAM role has `SecretsManagerReadWrite` policy attached
   
   **For Azure:**
   - Replace `my-key-vault` with your Azure Key Vault name
   - Ensure Key Vault exists and has appropriate access policies
   - Ensure the Azure AD application has "Key Vault Secrets Officer" role

3. **SOPS Private Key:**
   - The controller needs access to the SOPS private key to decrypt `application.secrets.env` files
   - The key should be stored in a Kubernetes secret in the `microscaler-system` namespace
   - See the main [README.md](../README.md) for details on SOPS key configuration

## Single Service Example

**File:** `single-service-secret-manager-config.yaml`

This example shows how to configure the controller for a single service repository:

```bash
# Apply the configuration
kubectl apply -f examples/single-service-secret-manager-config.yaml

# Check the status
kubectl get secretmanagerconfig my-service-secrets -n default
```

**Key Differences:**
- `secrets.environment: dev` - **Required** - Explicitly specifies which environment/profile to sync
- `secrets.basePath: "."` - Indicates root of repository
- `secrets.prefix: my-service` - **Required** for single service (used as service name)

**Directory Structure:**
```
deployment-configuration/
  profiles/
    dev/
      ├── application.properties
      └── application.secrets.env
    prd/
      ├── application.properties
      └── application.secrets.env
```

## Directory Structure Reference

### Monolith Structure

```
microservices/
  {service-name}/
    deployment-configuration/
      profiles/
        {env}/
          ├── application.properties      # Non-sensitive config
          └── application.secrets.env     # SOPS-encrypted secrets
```

**Configuration:**
- `secrets.basePath: microservices` (optional - specify if services are under a subdirectory)
- `secrets.prefix: {service-name}` (optional, defaults to service name from path)

### Single Service Structure

```
deployment-configuration/
  profiles/
    {env}/
      ├── application.properties      # Non-sensitive config
      └── application.secrets.env     # SOPS-encrypted secrets
```

**Configuration:**
- `secrets.basePath:` (optional - omit for root, or use `"."` to explicitly indicate root)
- `secrets.prefix: {service-name}` (**required** - used as service name)

## Secret Naming Convention

Secrets in GCP Secret Manager follow the same naming pattern as `kustomize-google-secret-manager` for drop-in replacement compatibility:

- `{prefix}-{key}-{suffix}` if both prefix and suffix are specified
- `{prefix}-{key}` if only prefix is specified
- `{key}-{suffix}` if only suffix is specified
- `{key}` if neither is specified

Invalid characters (`.`, `/`, spaces) are automatically sanitized to `_` to comply with cloud provider naming requirements:
- **GCP Secret Manager**: Names must be 1-255 characters, can contain letters, numbers, hyphens, and underscores
- **AWS Secrets Manager**: Names must be 1-512 characters, can contain letters, numbers, `/`, `_`, `+`, `=`, `.`, `@`, `-`
- **Azure Key Vault**: Names must be 1-127 characters, can contain letters, numbers, and hyphens

Where `prefix` is either:
- The value specified in `spec.secrets.prefix`
- Or derived from the service name in the path (e.g., `idam`)

And `suffix` is:
- The value specified in `spec.secrets.suffix` (optional)
- Commonly used for environment identifiers (e.g., `-prod`, `-dev-cf`) or tags (e.g., `-be-gcw1`)

## Verification

After applying a `SecretManagerConfig`, verify secrets are synced:

```bash
# Check controller logs
kubectl logs -n pricewhisperer -l app=secret-manager-controller --tail=50

# Check GCP Secret Manager (requires gcloud CLI)
gcloud secrets list --project=pricewhisperer-dev --filter="name:idam-dev-*"

# View a specific secret
gcloud secrets versions access latest --secret=idam-dev-jwt-secret --project=pricewhisperer-dev
```

## Troubleshooting

### SecretManagerConfig Not Ready

If the status shows `Ready=False`, check:

1. **GitRepository Status:**
   ```bash
   kubectl get gitrepository pricewhisperer-manifests -n flux-system -o yaml
   ```
   Ensure the GitRepository has an artifact in its status.

2. **Controller Logs:**
   ```bash
   kubectl logs -n pricewhisperer -l app=secret-manager-controller --tail=100
   ```

3. **GCP Authentication:**
   ```bash
   # Check service account
   kubectl get secret -n pricewhisperer -l app=secret-manager-controller
   ```

### Secrets Not Appearing in GCP

1. Verify the controller has permissions:
   ```bash
   gcloud projects get-iam-policy pricewhisperer-dev \
     --flatten="bindings[].members" \
     --filter="bindings.members:*secret-manager-controller*"
   ```

2. Check if SOPS decryption is working:
   ```bash
   kubectl logs -n pricewhisperer -l app=secret-manager-controller | grep -i sops
   ```

3. Verify the file paths exist in the Git repository:
   ```bash
   # If you have access to the repository
   ls -la microservices/idam/deployment-configuration/profiles/dev/
   ```

## Environment Configuration

**Important:** The `secrets.environment` field is **required** and must exactly match the directory name under `profiles/`.

### Standard Environment Names
- `dev` - Development environment
- `staging` - Staging environment
- `prod` or `prd` - Production environment

### Custom Environment Names (Skaffold)
Projects using Skaffold may use custom environment names:
- `dev-cf` - Development Cloud Foundry
- `pp-cf` - Pre-production Cloud Foundry
- `prod-cf` - Production Cloud Foundry
- `dev-k8s` - Development Kubernetes
- `prod-k8s` - Production Kubernetes

**Example:** See `sam-activity-example.yaml` for a service with custom environment names.

**Note:** You need separate `SecretManagerConfig` resources for each environment you want to sync.

## Customization

To create your own `SecretManagerConfig`:

1. Copy one of the example files
2. Update `metadata.name` and `metadata.namespace`
3. Update `spec.sourceRef` to reference your source:
   - FluxCD: `kind: GitRepository`, `name`, `namespace`
   - ArgoCD: `kind: Application`, `name`, `namespace`
4. Update `spec.gcp.projectId` to your GCP project ID
5. **Set `spec.secrets.environment`** - Must match the directory name under `profiles/`:
   - Standard: `dev`, `staging`, `prod`
   - Custom: `dev-cf`, `pp-cf`, `prod-cf`, etc.
6. **Optionally** set `spec.secrets.basePath` based on your structure:
   - Monolith: `microservices`, `services`, `apps`, etc. (only if services are under a subdirectory)
   - Single service: Omit (searches from root) or `"."` to explicitly indicate root
   - If omitted, searches from repository root
7. Set `spec.secrets.prefix` to control secret naming:
   - Monolith: Optional (defaults to service name from path)
   - Single service: **Required** (used as service name)

### Example: Monolith Service

```yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: billing-service-secrets
  namespace: pricewhisperer
spec:
  sourceRef:
    kind: GitRepository  # FluxCD GitRepository
    name: pricewhisperer-manifests
    namespace: flux-system
  gcp:
    projectId: pricewhisperer-dev
  secrets:
    environment: dev  # Required - must match directory name under profiles/
    basePath: microservices  # Optional - only needed if services are under a subdirectory
    prefix: billing-service  # Optional
```

### Example: Single Service

```yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: my-service-secrets
  namespace: default
spec:
  sourceRef:
    kind: GitRepository  # FluxCD GitRepository
    name: my-service-repo
    namespace: flux-system
  gcp:
    projectId: my-gcp-project-dev
  secrets:
    environment: dev  # Required - must match directory name under profiles/
    # basePath omitted - searches from repository root
    prefix: my-service  # Required for single service
```

### Example: Custom Environment Names (Skaffold)

```yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: sam-activity-dev-cf-secrets
  namespace: default
spec:
  sourceRef:
    kind: GitRepository  # FluxCD GitRepository
    name: sam-activity-repo
    namespace: flux-system
  gcp:
    projectId: sam-activity-dev
  secrets:
    environment: dev-cf  # Custom environment name - must match directory name
    # basePath omitted - searches from repository root
    prefix: sam-activity-dev-cf
```

**Note:** For services with multiple custom environments (e.g., `dev-cf`, `pp-cf`, `prod-cf`), create separate `SecretManagerConfig` resources for each environment.

