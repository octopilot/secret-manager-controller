# Secret Manager Controller

Kubernetes controller that syncs secrets from GitOps repositories (FluxCD/ArgoCD) to cloud secret managers (GCP Secret Manager, AWS Secrets Manager, Azure Key Vault).

## Overview

This controller:

1. **GitOps-Agnostic Source Support** - Supports both FluxCD GitRepository and ArgoCD Application via `sourceRef` pattern
2. **Supports two modes:**
   - **Kustomize Build Mode** (recommended): Runs `kustomize build` and extracts secrets from generated Kubernetes Secret resources. Supports overlays, patches, and generators. Works with any GitOps tool (FluxCD, ArgoCD, etc.)
   - **Raw File Mode**: Reads `application.secrets.env`, `application.secrets.yaml`, and `application.properties` directly
3. **Decrypts SOPS files** - ✅ Fully implemented! Automatically decrypts SOPS-encrypted secret files using GPG private key from Kubernetes secret
4. **GitOps-agnostic** - Works with FluxCD, ArgoCD, or any GitOps tool that provides GitRepository artifacts
5. **Multi-Cloud Support** - Syncs secrets to:
   - **Google Cloud Secret Manager** (GCP) - Default and recommended for GKE
   - **AWS Secrets Manager** - For EKS clusters
   - **Azure Key Vault** - For AKS clusters
6. **Workload Identity Support** - Uses Workload Identity/IRSA by default for secure authentication without storing credentials

## Problem Statement

### Why Do We Need This Tool?

**The Problem:**

1. **Secrets can't be stored in Git unless encrypted** - Storing plaintext secrets in version control is a security risk. Secrets must be encrypted (e.g., using SOPS) before being committed to Git repositories.

2. **Services can't use SOPS secrets directly** - While SOPS-encrypted secrets can be safely stored in Git, services running in cloud environments (GKE, EKS, AKS) cannot directly consume SOPS-encrypted files. They need secrets in a format that their cloud provider's secret management system can provide.

**The Solution:**

A Kubernetes controller that:
- Runs in Kubernetes and reconciles secrets using a GitOps loop
- Reads SOPS-encrypted secrets from Git repositories (via FluxCD or ArgoCD)
- Decrypts secrets using GPG keys stored in Kubernetes secrets
- Publishes decrypted secrets to cloud provider secret management systems (GCP Secret Manager, AWS Secrets Manager, Azure Key Vault)
- Enables services to consume secrets using standard `secretRef` patterns native to their cloud platform

**Key Benefits:**

- **Service-agnostic of CI/CD** - Services don't need to know about SOPS, GitOps, or encryption. They simply reference secrets using standard cloud provider mechanisms.
- **GitOps-driven** - Secrets are managed in Git with full version control, audit trail, and pull request workflows.
- **Cloud-native** - Services consume secrets through their cloud provider's native secret management APIs (e.g., GCP Secret Manager, AWS Secrets Manager).
- **Kubernetes-native** - Controller runs in Kubernetes, leveraging existing cluster infrastructure and RBAC.

**Trade-offs:**

- **Requires Kubernetes** - The controller must run in a Kubernetes cluster. This is a disadvantage for infrastructures that are heavily serverless-focused without Kubernetes infrastructure.

## Architecture

```mermaid
flowchart TB
    subgraph FluxCD["FluxCD Path"]
        FR[GitRepository<br/>CRD]
        SC[SourceController]
        AC[Artifact Cache<br/>/tmp/flux-source-*]
        FR --> SC
        SC --> AC
    end
    
    subgraph ArgoCD["ArgoCD Path"]
        AA[Application<br/>CRD]
        GC[Git Clone<br/>Direct Repository Access]
        AA --> GC
    end
    
    subgraph Controller["Secret Manager Controller"]
        CRD[SecretManagerConfig<br/>CRD]
        HTTP[HTTP Server<br/>Metrics & Probes]
        SOPS[SOPS Decryption]
        GIT[Git Operations]
        RECON[GitOps Reconciliation]
        
        CRD --> RECON
        RECON --> SOPS
        RECON --> GIT
        RECON --> HTTP
    end
    
    subgraph Cloud["Cloud Providers"]
        GCP[GCP Secret Manager<br/>Git is source of truth]
        AWS[AWS Secrets Manager<br/>Git is source of truth]
        Azure[Azure Key Vault<br/>Git is source of truth]
        Services[Cloud Services<br/>GKE/EKS/AKS]
        GCP --> Services
        AWS --> Services
        Azure --> Services
    end
    
    AC --> CRD
    GC --> CRD
    RECON --> GCP
    RECON --> AWS
    RECON --> Azure
    
    style CRD fill:#e1f5ff
    style GCP fill:#fff4e1
    style AWS fill:#fff4e1
    style Azure fill:#fff4e1
    style RECON fill:#e8f5e9
```

## GitOps Reconciliation

The controller implements **GitOps-style reconciliation** where Git is the source of truth:

1. **Git → GCP Sync**: When secrets change in Git, they are automatically synced to GCP Secret Manager
2. **Manual Changes Overwritten**: If a secret is manually changed in GCP Secret Manager, the controller will detect the difference and overwrite it with the Git value on the next reconciliation
3. **Version Management**: New versions are created in GCP Secret Manager when values change; old versions remain accessible but the new version becomes "latest"
4. **Continuous Reconciliation**: The controller continuously reconciles to ensure GCP Secret Manager matches Git

### Reconciliation Flow

```mermaid
sequenceDiagram
    autonumber
    actor User as User/Developer
    participant Git as Git Repository<br/>(Source of Truth)
    participant Flux as FluxCD/ArgoCD<br/>GitOps Tool
    participant K8s as Kubernetes API<br/>SecretManagerConfig CRD
    participant Controller as Secret Manager<br/>Controller
    participant SOPS as SOPS Decryption<br/>(if encrypted)
    participant Kustomize as Kustomize Build<br/>(if enabled)
    participant GCP as GCP Secret Manager
    participant CloudRun as CloudRun Services<br/>(via Upbound)
    
    Note over User,Git: Initial Setup
    User->>Git: Commit secrets to repository<br/>(application.secrets.env)
    User->>K8s: Apply SecretManagerConfig<br/>kubectl apply -f config.yaml
    K8s->>Controller: Watch event: CRD created
    
    Note over Controller: Reconciliation Triggered
    Controller->>K8s: Get SecretManagerConfig spec
    Controller->>Flux: Get Git source (FluxCD/ArgoCD)
    
    alt FluxCD Path
        Flux->>Controller: Provide artifact path<br/>(from SourceController cache)
    else ArgoCD Path
        Controller->>Git: Clone repository directly<br/>(extract from Application spec)
        Git->>Controller: Repository cloned<br/>(cached for efficiency)
    end
    
    Note over Controller: Process Secrets
    Controller->>SOPS: Decrypt if SOPS-encrypted<br/>(using GPG key from K8s secret)
    SOPS-->>Controller: Decrypted secrets
    
    alt Kustomize Build Mode
        Controller->>Kustomize: Run kustomize build<br/>(on specified path)
        Kustomize->>Controller: Generated Secret resources<br/>(with overlays/patches)
        Controller->>Controller: Extract secrets from<br/>Kubernetes Secret resources
    else Raw File Mode
        Controller->>Controller: Parse application.secrets.env<br/>(direct file reading)
    end
    
    Note over Controller: Sync to GCP
    loop For each secret
        Controller->>GCP: Create or update secret<br/>(with prefix/suffix naming)
        alt Secret value changed
            GCP->>GCP: Create new version<br/>(old versions preserved)
            GCP-->>Controller: Secret updated
        else No change
            GCP-->>Controller: Secret unchanged
        end
    end
    
    Controller->>K8s: Update SecretManagerConfig status<br/>(secrets synced, conditions)
    K8s-->>User: Status updated<br/>(Ready condition, sync count)
    
    Note over CloudRun: Consumption
    CloudRun->>GCP: Read secrets<br/>(via Upbound Provider)
    GCP-->>CloudRun: Secret values<br/>(latest version)
    
    Note over User,Git: Continuous Reconciliation
    User->>Git: Update secrets in Git
    Git->>Flux: GitOps tool detects change
    Flux->>Controller: Trigger reconciliation<br/>(via watch mechanism)
    Controller->>GCP: Sync updated secrets<br/>(Git overwrites manual changes)
```

### Reconciliation State Machine

The controller follows a formal state machine for reconciliation. Each `SecretManagerConfig` resource transitions through states based on reconciliation results:

```mermaid
stateDiagram-v2
    [*] --> Suspended: spec.suspend = true
    
    [*] --> Pending: Resource created
    Pending --> Pending: GitRepository not found<br/>(Action await_change)
    Pending --> Pending: GitRepository reconciling<br/>(Action await_change)
    Pending --> Reconciling: GitRepository ready
    
    Reconciling --> Ready: Secrets synced successfully<br/>(Action requeue(interval))
    Reconciling --> Retrying: Transient error<br/>(Action requeue(30s))
    Reconciling --> Failed: Permanent error
    
    Retrying --> Reconciling: Retry after backoff
    Retrying --> Failed: Max retries exceeded
    
    Ready --> Reconciling: Timer-based trigger<br/>(Action requeue(interval))
    Ready --> Reconciling: Watch event trigger
    Ready --> Reconciling: Manual trigger<br/>(annotation)
    
    Failed --> Reconciling: Manual trigger<br/>(annotation)
    Failed --> Reconciling: Configuration fixed
    
    Suspended --> Pending: spec.suspend = false<br/>(Action await_change)
    
    note right of Pending
        Waiting states use Action::await_change()
        to avoid blocking the event loop
    end note
    
    note right of Ready
        Success path uses single
        Action::requeue(interval)
        based on reconcileInterval
    end note
    
    note right of Retrying
        Transient errors use
        Action::requeue(30s)
        with Fibonacci backoff
        in error_policy()
    end note
```

**State Definitions:**

- **Suspended**: Reconciliation is disabled via `spec.suspend = true`. Returns `Action::await_change()` to wait for suspend to be cleared.
- **Pending**: Waiting for dependencies (GitRepository/Application) to be ready. Uses `Action::await_change()` to avoid blocking the event loop.
- **Reconciling**: Actively processing secrets (downloading artifacts, decrypting SOPS, syncing to cloud provider).
- **Ready**: Successfully synced all secrets. Uses `Action::requeue(interval)` based on `spec.reconcileInterval` for timer-based reconciliation.
- **Retrying**: Transient error occurred (network timeout, temporary unavailability). Uses `Action::requeue(30s)` with Fibonacci backoff managed in `error_policy()`.
- **Failed**: Permanent error (validation failure, wrong key, corrupted file). Requires manual intervention or configuration fix.

**Action Types:**

- `Action::await_change()`: Used for waiting states (Suspended, Pending). Waits for watch events without blocking the timer loop.
- `Action::requeue(duration)`: Used for scheduling next reconciliation:
  - **Success path**: `Action::requeue(reconcileInterval)` - single interval-based requeue
  - **Transient errors**: `Action::requeue(30s)` - fixed delay with backoff in `error_policy()`
  - **Duration parsing errors**: `Action::requeue(backoff_duration)` - Fibonacci backoff

**Key Design Principles:**

1. **No nested requeue calls**: Each reconciliation returns exactly one `Action`.
2. **Waiting uses await_change**: All "waiting for X" paths use `Action::await_change()` to prevent deadlocks.
3. **Success path is single requeue**: Successful reconciliation uses a single `Action::requeue(interval)` based on `reconcileInterval`.
4. **Backoff in error_policy**: Fibonacci backoff is managed in the `error_policy()` layer, not in the reconcile function.

## CRD Definition

### Multi-Cloud Provider Support

The controller supports three cloud providers: GCP, AWS, and Azure. Configure the `provider` field with the appropriate provider type:

```yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: my-service-secrets
  namespace: default
spec:
  sourceRef:
    kind: GitRepository  # FluxCD GitRepository (default) or "Application" for ArgoCD
    name: my-repo
    namespace: microscaler-system
  
  # Cloud provider configuration - choose one:
  provider:
    # Option 1: Google Cloud Platform (GCP)
    type: gcp
    gcp:
      projectId: my-gcp-project
      # Optional: GCP authentication configuration
      # Defaults to Workload Identity when not specified (recommended for GKE)
      auth:
        authType: WorkloadIdentity  # or "JsonCredentials" (deprecated)
        serviceAccountEmail: secret-manager-controller@my-project.iam.gserviceaccount.com
        # OR for JSON credentials (deprecated):
        # authType: JsonCredentials
        # secretName: gcp-secret-manager-credentials
        # secretNamespace: microscaler-system
        # secretKey: key.json
    
    # Option 2: Amazon Web Services (AWS)
    # type: aws
    # aws:
    #   region: us-east-1
    #   # Optional: AWS authentication configuration
    #   # Defaults to IRSA (IAM Roles for Service Accounts) when not specified (recommended for EKS)
    #   auth:
    #     authType: Irsa  # or "AccessKeys" (deprecated)
    #     roleArn: arn:aws:iam::123456789012:role/secret-manager-role
    #     # OR for Access Keys (deprecated):
    #     # authType: AccessKeys
    #     # secretName: aws-secret-manager-credentials
    #     # secretNamespace: flux-system
    #     # accessKeyIdKey: access-key-id
    #     # secretAccessKeyKey: secret-access-key
    
    # Option 3: Microsoft Azure
    # type: azure
    # azure:
    #   vaultName: my-key-vault
    #   # Optional: Azure authentication configuration
    #   # Defaults to Workload Identity when not specified (recommended for AKS)
    #   auth:
    #     authType: WorkloadIdentity  # or "ServicePrincipal" (deprecated)
    #     clientId: 12345678-1234-1234-1234-123456789012
    #     # OR for Service Principal (deprecated):
    #     # authType: ServicePrincipal
    #     # secretName: azure-credentials
    #     # secretNamespace: flux-system
  
  secrets:
    # Environment/profile name to sync (required - must match directory name under profiles/)
    environment: dev
    # Option 1: Kustomize Build Mode (recommended - supports overlays/patches)
    kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
    # Option 2: Raw File Mode (if kustomizePath not specified)
    # basePath: microservices  # Optional - if omitted, searches from repository root
    prefix: my-service  # Optional, defaults to service name
    suffix: -prod       # Optional, matches kustomize-google-secret-manager behavior
  # Optional: OpenTelemetry configuration for distributed tracing
  # If not specified, OpenTelemetry is disabled and standard tracing is used
  # otel:
  #   type: Otlp
  #   endpoint: http://otel-collector:4317
  #   serviceName: secret-manager-controller
  #   environment: production
  # OR for Datadog:
  # otel:
  #   type: Datadog
  #   serviceName: secret-manager-controller
  #   site: datadoghq.com
  #   apiKey: <from-secret>
```

**Important:** 
- The `secrets.environment` field is **required** and must exactly match the directory name under `profiles/`. This allows the controller to explicitly sync a specific environment rather than scanning all environments. This is especially useful for projects using Skaffold with custom environment names like `dev`, `pp`, `prod`.
- **Kustomize Build Mode** (when `secrets.kustomizePath` is specified): The controller runs `kustomize build` and extracts secrets from the generated Kubernetes Secret resources. This ensures overlays, patches, and generator modifications are included. Works with any GitOps tool (FluxCD, ArgoCD, etc.).
- **Raw File Mode** (when `secrets.kustomizePath` is not specified): The controller reads `application.secrets.env` files directly. Simpler but doesn't support kustomize overlays/patches.

### Directory Structure

The controller expects the following structure (matching Flux kustomize):

```mermaid
graph TD
    Root[Git Repository Root]
    Root --> Microservices[microservices/]
    Microservices --> Service["{service-name}/"]
    Service --> Config[deployment-configuration/]
    Config --> Profiles[profiles/]
    Profiles --> Env["{environment}/"]
    Env --> SecretsEnv[application.secrets.env<br/>ENV format secrets]
    Env --> SecretsYaml[application.secrets.yaml<br/>YAML format secrets]
    Env --> Properties[application.properties<br/>Properties file]
    Env --> Kustomize[kustomization.yaml<br/>Kustomize config]
    
    style Env fill:#e8f5e9
    style SecretsEnv fill:#fff4e1
    style SecretsYaml fill:#fff4e1
    style Properties fill:#e1f5ff
```

### Secret Naming

Secrets in cloud secret managers are named using the same convention as `kustomize-google-secret-manager` for drop-in replacement compatibility:

- `{secretPrefix}-{key}-{secretSuffix}` if both prefix and suffix are specified
- `{secretPrefix}-{key}` if only prefix is specified
- `{key}-{secretSuffix}` if only suffix is specified
- `{key}` if neither is specified

Invalid characters (`.`, `/`, spaces) are automatically sanitized to `_` to comply with cloud provider naming requirements:
- **GCP Secret Manager**: Names must be 1-255 characters, can contain letters, numbers, hyphens, and underscores
- **AWS Secrets Manager**: Names must be 1-512 characters, can contain letters, numbers, `/`, `_`, `+`, `=`, `.`, `@`, `-`
- **Azure Key Vault**: Names must be 1-127 characters, can contain letters, numbers, and hyphens

The controller automatically sanitizes secret names to ensure compliance with all providers.

Examples:
- With prefix only: `my-service-database-url`, `my-service-api-key`
- With prefix and suffix: `my-service-database-url-prod`, `my-service-api-key-prod`
- Properties secret: `my-service-properties` or `my-service-properties-prod`

## Configuration

### Environment Variables

- `GOOGLE_APPLICATION_CREDENTIALS` - Path to GCP service account JSON (only needed for JSON credentials, not Workload Identity)
- `AWS_ACCESS_KEY_ID` - AWS access key (only needed for Access Keys, not IRSA)
- `AWS_SECRET_ACCESS_KEY` - AWS secret access key (only needed for Access Keys, not IRSA)
- `AWS_DEFAULT_REGION` - AWS region (default: `us-east-1`)
- `AZURE_CLIENT_ID` - Azure client ID (only needed for Service Principal, not Workload Identity)
- `AZURE_CLIENT_SECRET` - Azure client secret (only needed for Service Principal, not Workload Identity)
- `AZURE_TENANT_ID` - Azure tenant ID (only needed for Service Principal, not Workload Identity)
- `RUST_LOG` - Logging level (default: `info`)
- `METRICS_PORT` - Port for metrics and probe endpoints (default: `5000`)

### Cloud Provider Authentication

The controller supports multiple authentication methods for each cloud provider. **Workload Identity (or equivalent) is the default and recommended method** for all providers, as it eliminates the need to manage credentials.

#### GCP Authentication

The controller supports two authentication methods for GCP:

#### 1. Workload Identity (Recommended for GKE)

Workload Identity is the recommended authentication method for GKE clusters. It eliminates the need to manage service account keys.

**Setup:**

1. **Create GCP Service Account:**
   ```bash
   gcloud iam service-accounts create secret-manager-controller \
     --display-name="Secret Manager Controller" \
     --project=YOUR_PROJECT_ID
   ```

2. **Grant Secret Manager Admin role:**
   ```bash
   gcloud projects add-iam-policy-binding YOUR_PROJECT_ID \
     --member="serviceAccount:secret-manager-controller@YOUR_PROJECT_ID.iam.gserviceaccount.com" \
     --role="roles/secretmanager.admin"
   ```

3. **Bind Workload Identity:**
   ```bash
   gcloud iam service-accounts add-iam-policy-binding \
     secret-manager-controller@YOUR_PROJECT_ID.iam.gserviceaccount.com \
     --role roles/iam.workloadIdentityUser \
     --member "serviceAccount:YOUR_PROJECT_ID.svc.id.goog[microscaler-system/secret-manager-controller]"
   ```

4. **Annotate Kubernetes Service Account:**
   Update `config/rbac/serviceaccount.yaml`:
   ```yaml
   metadata:
     annotations:
       iam.gke.io/gcp-service-account: secret-manager-controller@YOUR_PROJECT_ID.iam.gserviceaccount.com
   ```

5. **Configure SecretManagerConfig:**
   ```yaml
   spec:
     gcp:
       projectId: YOUR_PROJECT_ID
       auth:
         type: WorkloadIdentity
         serviceAccountEmail: secret-manager-controller@YOUR_PROJECT_ID.iam.gserviceaccount.com
   ```

#### 2. JSON Credentials

For non-GKE clusters or when Workload Identity is not available:

1. **Create service account key:**
   ```bash
   gcloud iam service-accounts keys create key.json \
     --iam-account=secret-manager-controller@YOUR_PROJECT_ID.iam.gserviceaccount.com
   ```

2. **Create Kubernetes secret:**
   ```bash
   kubectl create secret generic gcp-secret-manager-credentials \
     --from-file=key.json=key.json \
     --namespace=flux-system
   ```

3. **Update deployment:**
   Uncomment the volume mount and environment variable in `config/deployment/deployment.yaml`:
   ```yaml
   env:
   - name: GOOGLE_APPLICATION_CREDENTIALS
     value: /var/secrets/google/key.json
   volumeMounts:
   - name: gcp-credentials
     mountPath: /var/secrets/google
     readOnly: true
   volumes:
   - name: gcp-credentials
     secret:
       secretName: gcp-secret-manager-credentials
   ```

4. **Configure SecretManagerConfig (optional, defaults to above):**
   ```yaml
   spec:
     gcp:
       projectId: YOUR_PROJECT_ID
       auth:
         type: JsonCredentials
         secretName: gcp-secret-manager-credentials
         secretNamespace: flux-system
         secretKey: key.json
   ```

### SOPS Private Key

The controller automatically loads the SOPS private key from a Kubernetes secret in the `microscaler-system` namespace (or the namespace specified by `POD_NAMESPACE` environment variable). It looks for secrets named:
- `sops-private-key`
- `sops-gpg-key`
- `gpg-key`

The secret should contain the private key in one of these data keys:
- `private-key`
- `key`
- `gpg-key`

**Example Secret:**

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: sops-private-key
  namespace: flux-system
type: Opaque
stringData:
  private-key: |
    -----BEGIN PGP PRIVATE KEY BLOCK-----
    ...
    -----END PGP PRIVATE KEY BLOCK-----
```

**Note:** This key should be the same GPG key used by the GitHub SOPS Bot and should be added to `.sops.yaml` and re-encrypted so the controller can decrypt files.

### Source Reference Support

The controller supports multiple GitOps tools via the `sourceRef` pattern:

**FluxCD GitRepository:**
```yaml
sourceRef:
  kind: GitRepository  # Default, can be omitted
  name: my-repo
  namespace: flux-system
```
- Uses FluxCD SourceController artifacts
- Artifact path extracted from GitRepository status
- Fully supported

**ArgoCD Application:**
```yaml
sourceRef:
  kind: Application
  name: my-app
  namespace: argocd
```
- Extracts Git source from Application spec
- **Fully supported**: Controller clones the Git repository directly
- Supports both HTTPS and SSH Git URLs
- Caches repositories for efficiency (reuses if revision matches)
- **Git Authentication**: Supports SSH keys and HTTPS tokens from Kubernetes secrets
- See examples for ArgoCD configuration

### RBAC

The controller uses a `ClusterRole` to watch resources across all namespaces:
- `get`, `list`, `watch` on `secretmanagerconfigs.secret-management.microscaler.io` (all namespaces)
- `update`, `patch` on `secretmanagerconfigs.secret-management.microscaler.io/status` (all namespaces)
- `get`, `list`, `watch` on `gitrepositories.source.toolkit.fluxcd.io` (all namespaces) - FluxCD
- `get`, `list`, `watch` on `applications.argoproj.io` (all namespaces) - ArgoCD
- `get` on `secrets` in `microscaler-system` namespace (for SOPS private key)

**Namespace Flexibility:**
- Controller deploys to `microscaler-system` namespace
- `SecretManagerConfig` resources can be deployed in **any namespace**
- Controller automatically watches and reconciles resources in all namespaces

### HTTP Endpoints

- `GET /metrics` - Prometheus metrics endpoint
- `GET /healthz` - Kubernetes liveness probe
- `GET /readyz` - Kubernetes readiness probe

### Prometheus Metrics

The controller exposes the following metrics:

- `secret_manager_reconciliations_total` - Total number of reconciliations
- `secret_manager_reconciliation_errors_total` - Total number of reconciliation errors
- `secret_manager_reconciliation_duration_seconds` - Duration of reconciliations
- `secret_manager_secrets_synced_total` - Total number of secrets synced to GCP
- `secret_manager_secrets_updated_total` - Total number of secrets updated (overwritten from git)
- `secret_manager_secrets_managed` - Current number of secrets being managed
- `secret_manager_gcp_operations_total` - Total number of GCP Secret Manager operations
- `secret_manager_gcp_operation_duration_seconds` - Duration of GCP operations

## Quick Start

### Quick SOPS Testing

Want to test SOPS decryption without setting up Git/Flux? Use our quick test scripts:

```bash
# Complete setup with all steps and instructions
python3 scripts/test-sops-complete.py --env dev

# Or just copy files quickly
python3 scripts/test-sops-quick.py --env dev
```

This copies SOPS-encrypted files directly to the path where the controller expects them, bypassing Git/Flux setup. See `docs/QUICK_TEST_SOPS.md` for details.

### SOPS Troubleshooting

The controller provides detailed status fields and metrics to help diagnose SOPS decryption issues.

#### Status Fields

Check the `SecretManagerConfig` status for SOPS decryption information:

```bash
kubectl get secretmanagerconfig <name> -n <namespace> -o yaml
```

**Key Status Fields:**

- `status.decryptionStatus`: Current decryption status (`"Success"`, `"TransientFailure"`, `"PermanentFailure"`)
- `status.lastDecryptionAttempt`: Timestamp of last decryption attempt (RFC3339)
- `status.lastDecryptionError`: Error message from last failed decryption attempt
- `status.sopsKeyAvailable`: Whether SOPS private key is available in the resource namespace
- `status.sopsKeySecretName`: Name of the SOPS key secret found
- `status.sopsKeyNamespace`: Namespace where the SOPS key was found

#### Common Errors and Solutions

| Error Reason | Status | Metrics | Solution |
|-------------|--------|---------|----------|
| **Key Not Found** | `decryptionStatus: "PermanentFailure"`<br>`sopsKeyAvailable: false` | `sops_decryption_errors_total_by_reason{reason="key_not_found"}` | Create SOPS private key secret in the resource namespace:<br>`kubectl create secret generic sops-private-key -n <namespace> --from-file=private-key=<key-file>` |
| **Wrong Key** | `decryptionStatus: "PermanentFailure"`<br>`lastDecryptionError: "wrong key"` | `sops_decryption_errors_total_by_reason{reason="wrong_key"}` | Verify the SOPS private key matches the encryption key used in `.sops.yaml`. Check the key fingerprint in the file metadata. |
| **Invalid Key Format** | `decryptionStatus: "PermanentFailure"` | `sops_decryption_errors_total_by_reason{reason="invalid_key_format"}` | Ensure the SOPS private key is in ASCII-armored GPG format:<br>`-----BEGIN PGP PRIVATE KEY BLOCK-----`<br>`...`<br>`-----END PGP PRIVATE KEY BLOCK-----` |
| **Network Timeout** | `decryptionStatus: "TransientFailure"` | `sops_decryption_errors_total_by_reason{reason="network_timeout"}` | Usually transient - controller will retry automatically. Check GPG service availability. |
| **Provider Unavailable** | `decryptionStatus: "TransientFailure"` | `sops_decryption_errors_total_by_reason{reason="provider_unavailable"}` | Check if GPG service is running and accessible. Usually transient - controller will retry. |
| **Permission Denied** | `decryptionStatus: "TransientFailure"` | `sops_decryption_errors_total_by_reason{reason="permission_denied"}` | Verify RBAC is configured correctly. ServiceAccount needs `get` permission for the SOPS key secret. |

#### Metrics

Monitor SOPS decryption health using Prometheus metrics:

```promql
# Total SOPS decryption attempts
secret_manager_sops_decryption_total

# Successful decryptions
secret_manager_sops_decrypt_success_total

# Decryption errors by reason
secret_manager_sops_decryption_errors_total_by_reason{reason="key_not_found"}
secret_manager_sops_decryption_errors_total_by_reason{reason="wrong_key"}
secret_manager_sops_decryption_errors_total_by_reason{reason="network_timeout"}

# Decryption duration
secret_manager_sops_decrypt_duration_seconds
```

#### Debugging Steps

1. **Check SOPS Key Availability:**
   ```bash
   kubectl get secretmanagerconfig <name> -n <namespace> -o jsonpath='{.status.sopsKeyAvailable}'
   ```

2. **Verify Key Secret Exists:**
   ```bash
   kubectl get secret sops-private-key -n <namespace>
   # Or check alternative names:
   kubectl get secret sops-gpg-key -n <namespace>
   kubectl get secret gpg-key -n <namespace>
   ```

3. **Check Controller Logs:**
   ```bash
   kubectl logs -n microscaler-system -l app=secret-manager-controller | grep -i sops
   ```

4. **Verify Key Format:**
   ```bash
   kubectl get secret sops-private-key -n <namespace> -o jsonpath='{.data.private-key}' | base64 -d | head -1
   # Should output: -----BEGIN PGP PRIVATE KEY BLOCK-----
   ```

5. **Test Decryption Locally:**
   ```bash
   # Export key from Kubernetes secret
   kubectl get secret sops-private-key -n <namespace> -o jsonpath='{.data.private-key}' | base64 -d > /tmp/key.asc
   
   # Import into GPG
   gpg --import /tmp/key.asc
   
   # Test decryption
   sops -d <encrypted-file>
   ```

6. **Check RBAC Permissions:**
   ```bash
   kubectl auth can-i get secrets -n <namespace> --as=system:serviceaccount:microscaler-system:secret-manager-controller
   ```

For more detailed SOPS setup and configuration, see [`docs/SOPS_DECRYPTION.md`](docs/SOPS_DECRYPTION.md).

### Prerequisites

- Kubernetes cluster with FluxCD or ArgoCD installed (for production)
- Cloud provider credentials configured (GCP/AWS/Azure)
- `kubectl` - Kubernetes CLI
- **For Kustomize Build Mode**: `kustomize` binary must be available in the controller container (v5.0+)
- **For ArgoCD support**: `git` binary must be available in the controller container (v2.0+)

### Deploy to Kubernetes

The controller deploys to the `microscaler-system` namespace (GitOps provider agnostic) and works with both FluxCD and ArgoCD.

```bash
# Apply CRD
kubectl apply -f config/crd/

# Deploy controller to microscaler-system namespace
kubectl apply -k config/
```

**Note:** The controller watches `SecretManagerConfig` resources in **all namespaces**, so you can deploy your `SecretManagerConfig` resources in any namespace where your services are deployed.

## Usage with Cloud Services

After secrets are synced to cloud secret managers, reference them in your cloud services:

### Google Cloud Platform (GCP) - CloudRun

Use the [Upbound GCP CloudRun Provider](https://marketplace.upbound.io/providers/upbound/provider-gcp-cloudrun/v2.2.0) to reference secrets:

```yaml
apiVersion: cloudrun.gcp.upbound.io/v1beta1
kind: Service
metadata:
  name: my-service
spec:
  forProvider:
    template:
      spec:
        containers:
        - image: gcr.io/my-project/my-service:latest
          env:
          - name: DATABASE_URL
            valueFrom:
              secretKeyRef:
                name: my-service-database-url
                key: latest
```

**Note on GCP Parameter Manager:**

CloudRun supports both Secret Manager (for secrets) and Parameter Manager (for non-secret configuration values). Currently, this controller **only syncs to Secret Manager**. All values from `application.secrets.env`, `application.secrets.yaml`, and `application.properties` are stored in Secret Manager.

If you need to use Parameter Manager for non-secret configs:
- **Current limitation**: The controller does not support Parameter Manager. All values are synced to Secret Manager.
- **Workaround**: You can manually create Parameter Manager parameters and reference them separately in your CloudRun service configuration.
- **Future enhancement**: Support for Parameter Manager could be added to distinguish between secrets (Secret Manager) and configs (Parameter Manager), potentially based on file naming conventions (e.g., `application.config.env` → Parameter Manager, `application.secrets.env` → Secret Manager).

**Note on Config Stores for Serverless Systems:**

All three cloud providers offer separate services for non-secret configuration values:

| Provider | Secret Store | Config Store | Current Controller Support |
|----------|-------------|--------------|----------------------------|
| **GCP** | Secret Manager | Parameter Manager | ❌ Only Secret Manager |
| **AWS** | Secrets Manager | Systems Manager Parameter Store / AppConfig | ❌ Only Secrets Manager |
| **Azure** | Key Vault | App Configuration | ❌ Only Key Vault |

**Current Limitation**: The controller stores **all values** (including `application.properties`) in secret stores. Non-secret configs should ideally be stored in config stores for:
- Lower costs
- Better separation of concerns
- Native serverless integration (Lambda extensions, App Configuration SDK)

**Future Enhancement**: Support for config stores could be added using file-based routing:
- `application.secrets.*` → Secret Store
- `application.properties` → Config Store
- `application.config.*` → Config Store (new)

See [CONFIG_STORE_ANALYSIS.md](docs/CONFIG_STORE_ANALYSIS.md) for detailed analysis and implementation plan.

### Amazon Web Services (AWS) - ECS/EKS/Lambda

Reference secrets from AWS Secrets Manager in your ECS tasks, EKS pods, or Lambda functions:

```yaml
# ECS Task Definition
apiVersion: ecs.aws.upbound.io/v1beta1
kind: TaskDefinition
metadata:
  name: my-service
spec:
  forProvider:
    containerDefinitions: |
      [
        {
          "name": "my-service",
          "image": "my-registry/my-service:latest",
          "secrets": [
            {
              "name": "DATABASE_URL",
              "valueFrom": "arn:aws:secretsmanager:us-east-1:123456789012:secret:my-service-database-url"
            }
          ]
        }
      ]
```

**Note on AWS Config Stores:**

- **Systems Manager Parameter Store**: For non-secret configs, use Parameter Store instead of Secrets Manager. Lambda functions can use the Parameter Store Lambda extension for caching.
- **AppConfig**: For advanced config management with feature flags and validation, use AWS AppConfig.
- **Current limitation**: Controller only syncs to Secrets Manager. Configs from `application.properties` are stored as secrets.

### Microsoft Azure - AKS/Container Apps/Azure Functions

Reference secrets from Azure Key Vault in your AKS pods, Container Apps, or Azure Functions:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: my-service
spec:
  containers:
  - name: my-service
    image: my-registry/my-service:latest
    env:
    - name: DATABASE_URL
      valueFrom:
        secretKeyRef:
          name: azure-keyvault-secret-my-service-database-url
          key: value
```

**Note:** For Azure, you'll typically use the [External Secrets Operator](https://external-secrets.io/) or [Secrets Store CSI Driver](https://secrets-store-csi-driver.sigs.k8s.io/) to sync Azure Key Vault secrets to Kubernetes secrets.

**Note on Azure Config Stores:**

- **App Configuration**: For non-secret configs, use Azure App Configuration instead of Key Vault. Azure Functions can use the App Configuration SDK for dynamic configuration.
- **Current limitation**: Controller only syncs to Key Vault. Configs from `application.properties` are stored as secrets.

## MSMCTL CLI

`msmctl` (Microscaler Secret Manager Controller) is a command-line tool for interacting with the Secret Manager Controller running in Kubernetes. Similar to `fluxctl`, it provides commands to trigger reconciliations and view the status of SecretManagerConfig resources.

### Installation

#### Build from Source

```bash
cd hack/controllers/secret-manager-controller
cargo build --bin msmctl
```

The binary will be available at:
- Debug build: `target/debug/msmctl`
- Release build: `target/release/msmctl` (run `cargo build --release --bin msmctl`)

The build scripts (`scripts/tilt/build_binaries.py`) automatically build `msmctl` alongside the controller binaries.

#### Prerequisites

- Kubernetes cluster with Secret Manager Controller deployed
- `kubectl` configured with access to the cluster
- RBAC permissions to read/update SecretManagerConfig resources

### Authentication

`msmctl` uses Kubernetes authentication primitives:

- **kubeconfig**: Uses the default kubeconfig (`~/.kube/config`) or `KUBECONFIG` environment variable
- **Service Account**: When running in-cluster, uses the pod's service account token
- **Client Certificates**: Supports client certificate authentication from kubeconfig

No additional authentication is required - `msmctl` leverages Kubernetes' built-in security mechanisms.

### Commands

#### `msmctl reconcile`

Trigger a manual reconciliation for a SecretManagerConfig resource.

**Usage:**
```bash
msmctl reconcile secretmanagerconfig <name> [--namespace <namespace>] [--force]
```

**Arguments:**
- `secretmanagerconfig` (or `smc`): Resource type (required)
- `<name>`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- `--namespace, -n`: Namespace of the resource (defaults to current context namespace)
- `--force`: Force reconciliation by deleting and waiting for GitOps to recreate the resource (useful when resources get stuck)

**Example:**
```bash
# Trigger reconciliation in default namespace
msmctl reconcile secretmanagerconfig idam-dev-secrets

# Trigger reconciliation in specific namespace
msmctl reconcile secretmanagerconfig idam-dev-secrets --namespace pricewhisperer

# Using short form 'smc'
msmctl reconcile smc idam-dev-secrets

# Force reconciliation (delete and wait for GitOps recreation)
msmctl reconcile secretmanagerconfig idam-dev-secrets --namespace pricewhisperer --force
```

**How it works:**
- **Normal mode**: Updates the `secret-management.microscaler.io/reconcile` annotation with a timestamp. The controller watches for annotation changes and triggers reconciliation. This is a Kubernetes-native approach that doesn't require HTTP endpoints.
- **Force mode (`--force`)**: 
  1. Deletes the SecretManagerConfig resource
  2. Waits for GitOps (Flux/ArgoCD) to recreate it (up to 5 minutes)
  3. Shows progress logs during the wait
  4. Once recreated, triggers reconciliation
  5. Provides command to view reconciliation logs

**Force mode output:**
```
🔄 Force reconciliation mode enabled
   Resource: pricewhisperer/idam-dev-secrets

🗑️  Deleting SecretManagerConfig 'pricewhisperer/idam-dev-secrets'...
   ✅ Resource deleted

⏳ Waiting for GitOps to recreate resource...
   (This may take a few moments depending on GitOps sync interval)
   ⏳ Still waiting... (10s elapsed)
   ⏳ Still waiting... (20s elapsed)
   ✅ Resource recreated (generation: 1)

⏳ Waiting for resource to stabilize...

🔄 Triggering reconciliation for SecretManagerConfig 'pricewhisperer/idam-dev-secrets'...
✅ Reconciliation triggered successfully
   Resource: pricewhisperer/idam-dev-secrets
   Timestamp: 1702567890

📊 Watching reconciliation logs...
   (Use 'kubectl logs -n microscaler-system -l app=secret-manager-controller --tail=50 -f' to see detailed logs)
```

#### `msmctl list`

List all SecretManagerConfig resources.

**Usage:**
```bash
msmctl list secretmanagerconfig [--namespace <namespace>]
```

**Arguments:**
- `secretmanagerconfig` (or `smc`): Resource type (required)

**Options:**
- `--namespace, -n`: Namespace to list resources in (defaults to all namespaces)

**Example:**
```bash
# List all resources in all namespaces
msmctl list secretmanagerconfig

# List resources in specific namespace
msmctl list secretmanagerconfig --namespace pricewhisperer

# Using short form 'smc'
msmctl list smc
```

**Output:**
```
NAME                           NAMESPACE            SUSPEND      READY           SECRETS SYNCED 
--------------------------------------------------------------------------------------------
test-sops-config               default              No           False           -              
test-sops-config-prod          default              No           False           -              
test-sops-config-stage         default              No           False           -              
```

**Note:** The `SUSPEND` column shows whether reconciliation is paused for each resource.

#### `msmctl status`

Show detailed status of a SecretManagerConfig resource.

**Usage:**
```bash
msmctl status secretmanagerconfig <name> [--namespace <namespace>]
```

**Arguments:**
- `secretmanagerconfig` (or `smc`): Resource type (required)
- `<name>`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- `--namespace, -n`: Namespace of the resource (defaults to current context namespace)

**Example:**
```bash
msmctl status secretmanagerconfig idam-dev-secrets --namespace pricewhisperer

# Using short form 'smc'
msmctl status smc idam-dev-secrets
```

**Output:**
```
Status for SecretManagerConfig 'default/test-sops-config':

Metadata:
  Name: test-sops-config
  Namespace: default
  Generation: 1

Spec:
  Provider: GCP
  GCP Project ID: my-project-id
  Environment: dev
  Source: GitRepository/test-repo
  Kustomize Path: deployment-configuration/profiles/dev
  Base Path: 
  Secret Prefix: test-service
  Secret Suffix: 
  Suspend: false (active)

Status:
  Observed Generation: 1
  Last Reconcile Time: 2024-01-15T10:30:00Z
  Secrets Synced: 5

Conditions:
  Ready: True
    Reason: ReconciliationSucceeded
    Message: Synced 5 secrets
    Last Transition: 2024-01-15T10:30:00Z
```

#### `msmctl suspend`

Suspend reconciliation for a SecretManagerConfig resource. Useful for troubleshooting or during intricate CI/CD transitions where secrets need to be carefully managed.

**Usage:**
```bash
msmctl suspend secretmanagerconfig <name> [--namespace <namespace>]
```

**Arguments:**
- `secretmanagerconfig` (or `smc`): Resource type (required)
- `<name>`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- `--namespace, -n`: Namespace of the resource (defaults to current context namespace)

**Example:**
```bash
msmctl suspend secretmanagerconfig test-sops-config --namespace default

# Using short form 'smc'
msmctl suspend smc test-sops-config
```

**Output:**
```
⏸️  Suspending reconciliation for SecretManagerConfig 'default/test-sops-config'...
✅ Reconciliation suspended successfully
   Resource: default/test-sops-config
   Status: Suspended (no secrets will be synced)

To resume reconciliation, run:
   msmctl resume secretmanagerconfig test-sops-config --namespace default
```

**Note:** When suspended, the controller will skip reconciliation entirely, even for manual triggers. The resource status will show "Suspended" phase.

#### `msmctl resume`

Resume reconciliation for a suspended SecretManagerConfig resource.

**Usage:**
```bash
msmctl resume secretmanagerconfig <name> [--namespace <namespace>]
```

**Arguments:**
- `secretmanagerconfig` (or `smc`): Resource type (required)
- `<name>`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- `--namespace, -n`: Namespace of the resource (defaults to current context namespace)

**Example:**
```bash
msmctl resume secretmanagerconfig test-sops-config --namespace default

# Using short form 'smc'
msmctl resume smc test-sops-config
```

**Output:**
```
▶️  Resuming reconciliation for SecretManagerConfig 'default/test-sops-config'...
✅ Reconciliation resumed successfully
   Resource: default/test-sops-config
   Status: Active (reconciliation will proceed)

The controller will reconcile this resource shortly.
```

#### `msmctl suspend-git-pulls`

Suspend Git pulls for a SecretManagerConfig resource. This suspends GitRepository pulls but continues reconciliation with the last pulled commit. Useful when you want to freeze the Git state but keep syncing secrets from the current commit.

**Usage:**
```bash
msmctl suspend-git-pulls secretmanagerconfig <name> [--namespace <namespace>]
```

**Arguments:**
- `secretmanagerconfig` (or `smc`): Resource type (required)
- `<name>`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- `--namespace, -n`: Namespace of the resource (defaults to current context namespace)

**Example:**
```bash
msmctl suspend-git-pulls secretmanagerconfig test-sops-config --namespace default

# Using short form 'smc'
msmctl suspend-git-pulls smc test-sops-config
```

**Output:**
```
⏸️  Suspending Git pulls for SecretManagerConfig 'default/test-sops-config'...
✅ Git pulls suspended successfully
   Resource: default/test-sops-config
   Status: Git pulls paused (reconciliation continues with last commit)

To resume Git pulls, run:
   msmctl resume-git-pulls secretmanagerconfig test-sops-config --namespace default
```

**Note:** When Git pulls are suspended, FluxCD stops fetching new commits but the last artifact remains available. The controller continues reconciling secrets from the last pulled commit.

#### `msmctl resume-git-pulls`

Resume Git pulls for a SecretManagerConfig resource. This resumes GitRepository pulls and allows new commits to be fetched.

**Usage:**
```bash
msmctl resume-git-pulls secretmanagerconfig <name> [--namespace <namespace>]
```

**Arguments:**
- `secretmanagerconfig` (or `smc`): Resource type (required)
- `<name>`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- `--namespace, -n`: Namespace of the resource (defaults to current context namespace)

**Example:**
```bash
msmctl resume-git-pulls secretmanagerconfig test-sops-config --namespace default

# Using short form 'smc'
msmctl resume-git-pulls smc test-sops-config
```

**Output:**
```
▶️  Resuming Git pulls for SecretManagerConfig 'default/test-sops-config'...
✅ Git pulls resumed successfully
   Resource: default/test-sops-config
   Status: Git pulls enabled (will fetch new commits)

The controller will resume pulling from Git shortly.
```

### Security

#### RBAC Requirements

The user/service account running `msmctl` needs the following permissions:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: msmctl-user
rules:
- apiGroups: ["secret-management.microscaler.io"]
  resources: ["secretmanagerconfigs"]
  verbs: ["get", "list", "patch"]
- apiGroups: ["secret-management.microscaler.io"]
  resources: ["secretmanagerconfigs/status"]
  verbs: ["get"]
```

#### Authentication Methods

1. **kubeconfig** (default): Uses credentials from `~/.kube/config`
2. **Service Account**: When running in-cluster, automatically uses pod service account
3. **Environment Variables**: Can use `KUBECONFIG` to specify custom config file

### Comparison with fluxctl

| Feature | fluxctl | msmctl |
|---------|---------|--------|
| **Trigger Reconciliation** | ✅ | ✅ |
| **List Resources** | ✅ | ✅ |
| **Show Status** | ✅ | ✅ |
| **Suspend/Resume** | ❌ | ✅ |
| **Suspend/Resume Git Pulls** | ❌ | ✅ |
| **Authentication** | kubeconfig | kubeconfig |
| **Method** | HTTP endpoint | Annotation-based |

#### Advantages of Annotation-Based Approach

- **Kubernetes-native**: Uses standard Kubernetes watch mechanisms
- **No HTTP endpoint required**: Controller doesn't need to expose HTTP API
- **Audit trail**: Annotation changes are logged in Kubernetes audit logs
- **RBAC integration**: Uses standard Kubernetes RBAC for authorization

### Troubleshooting

#### "Failed to create Kubernetes client"

**Cause:** kubeconfig not configured or invalid

**Solution:**
```bash
# Verify kubeconfig
kubectl cluster-info

# Set KUBECONFIG if needed
export KUBECONFIG=/path/to/kubeconfig
```

#### "Failed to trigger reconciliation"

**Cause:** Insufficient RBAC permissions

**Solution:** Ensure your user/service account has `patch` permission on SecretManagerConfig resources.

#### "Resource not found"

**Cause:** SecretManagerConfig doesn't exist or wrong namespace

**Solution:**
```bash
# List resources to verify name and namespace
msmctl list secretmanagerconfig

# Use correct namespace
msmctl reconcile secretmanagerconfig <name> --namespace <correct-namespace>
```

### Examples

#### CI/CD Integration

Trigger reconciliation after deploying secrets:

```bash
#!/bin/bash
# deploy-secrets.sh

# Apply SecretManagerConfig
kubectl apply -f secret-manager-config.yaml

# Trigger immediate reconciliation
msmctl reconcile secretmanagerconfig my-secrets --namespace default

# Wait for reconciliation to complete
kubectl wait --for=condition=Ready \
  secretmanagerconfig/my-secrets \
  --timeout=60s
```

#### Monitoring Script

Check status of all resources:

```bash
#!/bin/bash
# check-status.sh

for config in $(msmctl list secretmanagerconfig --namespace pricewhisperer | awk 'NR>2 {print $1}'); do
  echo "Checking $config..."
  msmctl status secretmanagerconfig "$config" --namespace pricewhisperer
  echo ""
done
```

## Development

### Prerequisites

- Rust 1.75+
- Docker and Docker Compose
- `kind` (Kubernetes in Docker) - for local cluster
- `kubectl` - Kubernetes CLI
- `tilt` - for local development (optional but recommended)
- Kubernetes cluster with FluxCD or ArgoCD installed (for production)
- Cloud provider credentials configured (GCP/AWS/Azure)
- **For Kustomize Build Mode**: `kustomize` binary must be available in the controller container (v5.0+)
- **For ArgoCD support**: `git` binary must be available in the controller container (v2.0+)
- **For cross-compilation to Linux musl**: `cargo-zigbuild` (recommended) or OpenSSL development libraries configured manually
  - Install with: `cargo install cargo-zigbuild`

### Local Development with Kind Cluster

The controller includes an independent kind cluster configuration for standalone development and testing. The project uses [`just`](https://github.com/casey/just) as a command runner for common development tasks.

#### Quick Start

1. **Check dependencies:**
   ```bash
   just check-deps
   ```

2. **Setup Kind cluster:**
   ```bash
   just setup-kind
   ```

   This will:
   - Create a local Docker registry (`kind-registry`) if it doesn't exist
   - Create a kind cluster named `secret-manager-controller`
   - Configure the cluster to use the local registry
   - Configure pod and service IP ranges (pods get their own IPs, no host port conflicts)

3. **Start development environment:**
   ```bash
   just dev-up
   ```

   This will:
   - Create the kind cluster (if not already created)
   - Start Tilt for hot-reload development
   - Generate the CRD when Rust code changes
   - Build the Rust binary
   - Build the Docker image and push to local registry
   - Deploy to the kind cluster
   - Watch for changes and automatically rebuild/redeploy

4. **Access the Tilt UI:**
   - Open http://localhost:10350 in your browser
   - Monitor build logs, deployment status, and resource health

5. **Access controller metrics:**
   ```bash
   # Port forward to access metrics endpoint (in a separate terminal)
   just port-forward
   # OR manually:
   kubectl port-forward -n microscaler-system svc/secret-manager-controller-metrics 5000:5000
   ```
   Then access:
   - Metrics: http://localhost:5000/metrics
   - Health: http://localhost:5000/healthz
   - Ready: http://localhost:5000/readyz

   **Note:** Kubernetes pods get their own IP addresses (configured via `podSubnet`), so there are no host port conflicts. Access is via `kubectl port-forward` or Service types (ClusterIP/NodePort/LoadBalancer).

#### Common Commands

View all available commands:
```bash
just --list
```

**Development:**
- `just dev-up` - Start development environment (Kind + Tilt)
- `just dev-down` - Stop development environment
- `just up` - Start Tilt (assumes cluster is running)
- `just down` - Stop Tilt

**Building:**
- `just build` - Build Rust binary and Docker image
- `just build-rust` - Build Rust binary (debug)
- `just build-release` - Build Rust binary (release)
- `just build-linux` - Build for Linux (musl target) - Uses `cargo-zigbuild` for OpenSSL cross-compilation
- `just build-linux-release` - Build for Linux (musl target, release)
- `just generate-crd` - Generate CRD from Rust code

**Testing:**
- `just test` - Run all tests (unit + pact)
- `just test-unit` - Run unit tests
- `just test-pact` - Run Pact contract tests
- `just test-pact-gcp` - Run GCP Pact tests
- `just test-pact-aws` - Run AWS Pact tests
- `just test-pact-azure` - Run Azure Pact tests

**Code Quality:**
- `just fmt` - Format code
- `just lint` - Lint code
- `just check` - Check code (compile without building)
- `just validate` - Run all validations (fmt, lint, check, tests)

**Deployment:**
- `just deploy` - Deploy to Kubernetes
- `just deploy-crd` - Deploy CRD only
- `just status` - Show cluster and controller status
- `just logs` - Show controller logs

**CLI Tool:**
- `just build-cli` - Build msmctl CLI tool
- `just install-cli` - Install msmctl to ~/.local/bin
- `just cli <args>` - Run CLI tool (development mode)

#### Manual Setup (without just)

If you prefer not to use `just`, you can use the scripts directly:

1. **Create the kind cluster:**
   ```bash
   python3 scripts/setup_kind.py
   ```

2. **Verify the cluster is running:**
   ```bash
   kubectl cluster-info --context kind-secret-manager-controller
   ```

3. **Start Tilt:**
   ```bash
   tilt up
   ```

#### Cleanup

To stop and clean up everything:
```bash
just dev-down
```

Or manually:
```bash
# Stop Tilt
tilt down

# Delete kind cluster
just teardown-kind
# OR
kind delete cluster --name secret-manager-controller

# Remove local registry (optional)
docker stop kind-registry
docker rm kind-registry
```

### Build

#### Native Build

```bash
cargo build --release
```

#### Cross-Compilation for Linux (musl)

The controller uses the Google Cloud SDK crates which depend on `reqwest` with OpenSSL support. For cross-compilation to `x86_64-unknown-linux-musl`, you need to handle OpenSSL dependencies.

**Recommended: Use `cargo-zigbuild`**

The easiest way to cross-compile is using `cargo-zigbuild`, which automatically handles OpenSSL compilation for musl targets:

```bash
# Install cargo-zigbuild (if not already installed)
cargo install cargo-zigbuild

# Build for Linux musl target
cargo zigbuild --release --target x86_64-unknown-linux-musl

# Or use the justfile command
just build-linux-release
```

**Alternative: Manual OpenSSL Configuration**

If you prefer not to use `cargo-zigbuild`, you can configure OpenSSL manually:

1. **Install OpenSSL development libraries:**
   - **macOS (Homebrew):**
     ```bash
     brew install openssl@3
     export OPENSSL_DIR=$(brew --prefix openssl@3)
     ```
   - **Ubuntu/Debian:**
     ```bash
     sudo apt-get install libssl-dev pkg-config
     ```
   - **Fedora:**
     ```bash
     sudo dnf install openssl-devel pkg-config
     ```

2. **Set environment variables for cross-compilation:**
   ```bash
   # macOS example
   export OPENSSL_DIR=/opt/homebrew/opt/openssl@3
   export PKG_CONFIG_ALLOW_CROSS=1
   
   # Build
   cargo build --release --target x86_64-unknown-linux-musl
   ```

**Note:** The Google Cloud SDK crates use `reqwest` with default features (OpenSSL), so we cannot force `rustls` without forking those crates. This is documented in `Cargo.toml`.

### Run Locally

```bash
# For GCP:
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json

# For AWS:
export AWS_ACCESS_KEY_ID=your-access-key
export AWS_SECRET_ACCESS_KEY=your-secret-key
export AWS_DEFAULT_REGION=us-east-1

# For Azure:
export AZURE_CLIENT_ID=your-client-id
export AZURE_CLIENT_SECRET=your-client-secret
export AZURE_TENANT_ID=your-tenant-id

# Run controller
cargo run
```

## Testing

### Quick SOPS Test

Test SOPS decryption without full Git/Flux setup:

```bash
# Complete test setup
python3 scripts/test-sops-complete.py --env dev

# Quick file copy only
python3 scripts/test-sops-quick.py --env dev
```

See `docs/QUICK_TEST_SOPS.md` for complete testing guide.

### Example Configurations

- `gitops/cluster/env/tilt/` - Complete GitOps setup for tilt environment (GitRepository + SecretManagerConfig)
- `deployment-configuration/` - Example deployment configuration structure with SOPS-encrypted secrets
- `examples/` - Additional use-case examples (monolith, single-service, different cloud providers)

## Related Components

- **Flux SourceController** - Provides GitRepository artifacts
- **GitHub SOPS Bot** - Manages GPG keys for secret encryption
- **Upbound GCP CloudRun Provider** - Consumes secrets from Secret Manager
