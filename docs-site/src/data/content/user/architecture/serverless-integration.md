# Serverless Integration Architecture

Complete guide to how the Secret Manager Controller integrates with serverless systems and Crossplane.

## Overview

The Secret Manager Controller is designed to work with **both Kubernetes and serverless systems**. While the controller runs in Kubernetes, it syncs secrets to cloud provider secret managers that are consumed by serverless workloads (CloudRun, Lambda, Azure Functions) as well as Kubernetes workloads.

## Architecture Flow

### Complete End-to-End Flow

```mermaid
flowchart TB
    subgraph Git["Git Repository"]
        SECRETS[application.secrets.env<br/>application.secrets.yaml]
        PROPS[application.properties]
        CROSSPLANE[Crossplane<br/>Manifests]
    end
    
    subgraph GitOps["GitOps Tool"]
        FLUX[FluxCD<br/>GitRepository]
        ARGO[ArgoCD<br/>Application]
    end
    
    subgraph K8s["Kubernetes Cluster"]
        CRD[SecretManagerConfig<br/>CRD]
        CONTROLLER[Secret Manager<br/>Controller]
        CROSSPLANE_OP[Crossplane<br/>Operator]
    end
    
    subgraph Cloud["Cloud Providers"]
        GCP_SEC[GCP Secret<br/>Manager]
        AWS_SEC[AWS Secrets<br/>Manager]
        AZURE_KV[Azure Key<br/>Vault]
        GCP_PARAM[GCP Parameter<br/>Manager]
        AWS_PARAM[AWS Parameter<br/>Store]
        AZURE_APP[Azure App<br/>Configuration]
    end
    
    subgraph Serverless["Serverless Workloads"]
        CLOUDRUN[CloudRun<br/>Services]
        LAMBDA[Lambda<br/>Functions]
        FUNCTIONS[Azure<br/>Functions]
    end
    
    subgraph K8s_Workloads["Kubernetes Workloads"]
        PODS[Pods<br/>Deployments]
        ESO[External Secrets<br/>Operator]
    end
    
    SECRETS --> FLUX
    PROPS --> FLUX
    CROSSPLANE --> FLUX
    
    FLUX --> CRD
    ARGO --> CRD
    
    CRD --> CONTROLLER
    CONTROLLER --> GCP_SEC
    CONTROLLER --> AWS_SEC
    CONTROLLER --> AZURE_KV
    CONTROLLER --> GCP_PARAM
    CONTROLLER --> AWS_PARAM
    CONTROLLER --> AZURE_APP
    
    FLUX --> CROSSPLANE_OP
    CROSSPLANE_OP --> CLOUDRUN
    CROSSPLANE_OP --> LAMBDA
    CROSSPLANE_OP --> FUNCTIONS
    
    GCP_SEC --> CLOUDRUN
    GCP_PARAM --> CLOUDRUN
    AWS_SEC --> LAMBDA
    AWS_PARAM --> LAMBDA
    AZURE_KV --> FUNCTIONS
    AZURE_APP --> FUNCTIONS
    
    GCP_SEC --> ESO
    AWS_SEC --> ESO
    AZURE_KV --> ESO
    ESO --> PODS
    
    style CONTROLLER fill:#4a90e2,stroke:#2c5aa0,stroke-width:2px,color:#fff
    style CROSSPLANE_OP fill:#ae3ec9,stroke:#862e9c,stroke-width:2px,color:#fff
    style GCP_SEC fill:#ffd43b,stroke:#f59f00,stroke-width:2px,color:#000
    style AWS_SEC fill:#ffd43b,stroke:#f59f00,stroke-width:2px,color:#000
    style AZURE_KV fill:#ffd43b,stroke:#f59f00,stroke-width:2px,color:#000
    style CLOUDRUN fill:#51cf66,stroke:#2f9e44,stroke-width:2px,color:#fff
    style LAMBDA fill:#51cf66,stroke:#2f9e44,stroke-width:2px,color:#fff
    style FUNCTIONS fill:#51cf66,stroke:#2f9e44,stroke-width:2px,color:#fff
```

## Step-by-Step Flow

### 1. Git Repository Setup

Secrets and configuration are stored in Git:

```bash
my-service/
└── deployment-configuration/
    └── profiles/
        └── prod/
            ├── application.secrets.env    # Secrets
            ├── application.secrets.yaml   # Secrets (alternative)
            └── application.properties     # Config values
```

**Crossplane Manifests** define serverless resources:

```yaml
# cloudrun-service.yaml
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

### 2. FluxCD Applies Resources

FluxCD syncs the Git repository and applies resources:

```mermaid
sequenceDiagram
    participant Git as Git Repository
    participant Flux as FluxCD
    participant K8s as Kubernetes API
    participant Crossplane as Crossplane Operator
    participant Controller as Secret Manager Controller
    
    Git->>Flux: Repository updated
    Flux->>K8s: Apply SecretManagerConfig
    Flux->>K8s: Apply Crossplane Service
    K8s->>Controller: Watch event: CRD created
    K8s->>Crossplane: Watch event: Service created
```

**FluxCD GitRepository:**

```yaml
apiVersion: source.toolkit.fluxcd.io/v1beta2
kind: GitRepository
metadata:
  name: my-service-repo
  namespace: flux-system
spec:
  url: https://github.com/my-org/my-service
  ref:
    branch: main
  interval: 1m
```

**SecretManagerConfig:**

```yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-service-secrets
  namespace: octopilot-system
spec:
  sourceRef:
    kind: GitRepository
    name: my-service-repo
    namespace: flux-system
  provider:
    type: gcp
    gcp:
      region: us-central1
  secrets:
    environment: prod
    kustomizePath: deployment-configuration/profiles/prod
    prefix: my-service
```

### 3. Controller Syncs Secrets

The controller reconciles secrets:

```mermaid
sequenceDiagram
    participant Controller as Secret Manager Controller
    participant Flux as FluxCD Source
    participant SOPS as SOPS Decryption
    participant Parser as File Parser
    participant GCP as GCP Secret Manager
    
    Controller->>Flux: Get artifact path
    Flux-->>Controller: /tmp/flux-source-*/deployment-configuration/profiles/prod
    
    Controller->>Parser: Find application files
    Parser-->>Controller: application.secrets.env found
    
    Controller->>SOPS: Decrypt if encrypted
    SOPS-->>Controller: Decrypted content
    
    Controller->>Parser: Parse secrets
    Parser-->>Controller: {DATABASE_URL: "postgresql://..."}
    
    loop For each secret
        Controller->>GCP: Create/update secret
        Note over Controller,GCP: Secret name: my-service-database-url
        GCP-->>Controller: Success
    end
```

**Secret Storage:**

- **Secrets** (`application.secrets.*`) → Secret Manager
- **Configs** (`application.properties`) → Parameter Manager (if enabled)

### 4. Crossplane Provisions Serverless Resources

Crossplane applies serverless resource manifests:

```mermaid
sequenceDiagram
    participant Flux as FluxCD
    participant Crossplane as Crossplane Operator
    participant GCP as GCP CloudRun API
    participant AWS as AWS Lambda API
    participant Azure as Azure Functions API
    
    Flux->>Crossplane: Apply CloudRun Service manifest
    Crossplane->>GCP: Create CloudRun Service
    GCP-->>Crossplane: Service created
    Crossplane->>Crossplane: Update status
    
    Flux->>Crossplane: Apply Lambda Function manifest
    Crossplane->>AWS: Create Lambda Function
    AWS-->>Crossplane: Function created
    
    Flux->>Crossplane: Apply Azure Function manifest
    Crossplane->>Azure: Create Azure Function
    Azure-->>Crossplane: Function created
```

**Crossplane Service References Secrets:**

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
          # Reference secret from Secret Manager
          - name: DATABASE_URL
            valueFrom:
              secretKeyRef:
                name: my-service-database-url  # Matches controller prefix
                key: latest
          # Reference config from Parameter Manager
          - name: API_TIMEOUT
            value: "30s"  # From application.properties
```

### 5. Serverless Consumption

Serverless workloads consume secrets at runtime:

#### GCP CloudRun

```mermaid
flowchart LR
    A[CloudRun Service] -->|secretKeyRef| B[GCP Secret Manager]
    A -->|Environment Variables| C[GCP Parameter Manager]
    B -->|Runtime| D[Application Code]
    C -->|Runtime| D
```

**Consumption Methods:**

1. **Secret Manager** (via `secretKeyRef`):
   ```yaml
   env:
   - name: DATABASE_URL
     valueFrom:
       secretKeyRef:
         name: my-service-database-url
         key: latest
   ```

2. **Parameter Manager** (via environment variables):
   ```yaml
   env:
   - name: API_TIMEOUT
     value: "30s"  # From Parameter Manager
   ```

3. **SDK Access** (programmatic):
   ```python
   from google.cloud import secretmanager
   
   client = secretmanager.SecretManagerServiceClient()
   name = f"projects/{project_id}/secrets/my-service-database-url/versions/latest"
   response = client.access_secret_version(request={"name": name})
   secret_value = response.payload.data.decode("UTF-8")
   ```

#### AWS Lambda

```mermaid
flowchart LR
    A[Lambda Function] -->|Lambda Extension| B[AWS Secrets Manager]
    A -->|Lambda Extension| C[AWS Parameter Store]
    B -->|Runtime| D[Application Code]
    C -->|Runtime| D
```

**Consumption Methods:**

1. **Lambda Extension** (automatic caching):
   ```yaml
   # Lambda function configuration
   environment:
     SECRETS_EXTENSION_HTTP_PORT: 2773
   ```
   
   ```python
   import requests
   
   # Access via Lambda extension
   response = requests.get(
       f"http://localhost:2773/secretsmanager/get?secretId=my-service-database-url"
   )
   secret = response.json()["SecretString"]
   ```

2. **SDK Access**:
   ```python
   import boto3
   
   client = boto3.client('secretsmanager')
   response = client.get_secret_value(SecretId='my-service-database-url')
   secret = response['SecretString']
   ```

3. **Parameter Store** (for configs):
   ```python
   import boto3
   
   ssm = boto3.client('ssm')
   response = ssm.get_parameter(
       Name='/my-service/prod/api-timeout',
       WithDecryption=False
   )
   value = response['Parameter']['Value']
   ```

#### Azure Functions

```mermaid
flowchart LR
    A[Azure Function] -->|Key Vault Reference| B[Azure Key Vault]
    A -->|App Config SDK| C[Azure App Configuration]
    B -->|Runtime| D[Application Code]
    C -->|Runtime| D
```

**Consumption Methods:**

1. **Key Vault References** (in app settings):
   ```json
   {
     "DATABASE_URL": "@Microsoft.KeyVault(SecretUri=https://my-vault.vault.azure.net/secrets/my-service-database-url/)"
   }
   ```

2. **App Configuration SDK**:
   ```python
   from azure.appconfiguration import AzureAppConfigurationClient
   
   client = AzureAppConfigurationClient.from_connection_string(connection_string)
   setting = client.get_configuration_setting(key="api-timeout")
   value = setting.value
   ```

## Provider-Specific Details

### GCP (CloudRun)

| Component | Service | Purpose |
|-----------|---------|---------|
| **Secret Store** | Secret Manager | Secrets (passwords, API keys) |
| **Config Store** | Parameter Manager | Non-secret configs (timeouts, URLs) |
| **Consumption** | `secretKeyRef` | Native CloudRun integration |

**Example Flow:**

1. Controller syncs `application.secrets.env` → Secret Manager
2. Controller syncs `application.properties` → Parameter Manager (if `configs.enabled=true`)
3. Crossplane creates CloudRun Service with `secretKeyRef`
4. CloudRun injects secrets at runtime

### AWS (Lambda)

| Component | Service | Purpose |
|-----------|---------|---------|
| **Secret Store** | Secrets Manager | Secrets (passwords, API keys) |
| **Config Store** | Parameter Store | Non-secret configs (timeouts, URLs) |
| **Consumption** | Lambda Extension | Automatic caching and injection |

**Example Flow:**

1. Controller syncs `application.secrets.env` → Secrets Manager
2. Controller syncs `application.properties` → Parameter Store (if `configs.enabled=true`)
3. Crossplane creates Lambda Function
4. Lambda Extension caches secrets/parameters
5. Application accesses via extension or SDK

### Azure (Functions)

| Component | Service | Purpose |
|-----------|---------|---------|
| **Secret Store** | Key Vault | Secrets (passwords, API keys) |
| **Config Store** | App Configuration | Non-secret configs (timeouts, URLs) |
| **Consumption** | Key Vault References | Native Azure integration |

**Example Flow:**

1. Controller syncs `application.secrets.env` → Key Vault
2. Controller syncs `application.properties` → App Configuration (if `configs.enabled=true`)
3. Crossplane creates Azure Function
4. Function references Key Vault secrets in app settings
5. Function accesses configs via App Configuration SDK

## Complete Example: GCP CloudRun

### 1. Git Repository Structure

```
my-service/
├── deployment-configuration/
│   └── profiles/
│       └── prod/
│           ├── application.secrets.env
│           └── application.properties
└── infrastructure/
    └── cloudrun-service.yaml
```

**application.secrets.env:**
```bash
DATABASE_URL=postgresql://user:password@host:5432/db
API_KEY=sk_live_1234567890
```

**application.properties:**
```properties
api.timeout=30s
api.retries=3
database.pool.size=10
```

**cloudrun-service.yaml:**
```yaml
apiVersion: cloudrun.gcp.upbound.io/v1beta1
kind: Service
metadata:
  name: my-service
spec:
  forProvider:
    location: us-central1
    template:
      spec:
        containers:
        - image: gcr.io/my-project/my-service:latest
          env:
          # Secret from Secret Manager
          - name: DATABASE_URL
            valueFrom:
              secretKeyRef:
                name: my-service-database-url
                key: latest
          - name: API_KEY
            valueFrom:
              secretKeyRef:
                name: my-service-api-key
                key: latest
          # Config from Parameter Manager (or environment variables)
          - name: API_TIMEOUT
            value: "30s"
          - name: API_RETRIES
            value: "3"
```

### 2. FluxCD Configuration

**GitRepository:**
```yaml
apiVersion: source.toolkit.fluxcd.io/v1beta2
kind: GitRepository
metadata:
  name: my-service-repo
  namespace: flux-system
spec:
  url: https://github.com/my-org/my-service
  ref:
    branch: main
  interval: 1m
```

**SecretManagerConfig:**
```yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-service-secrets
  namespace: octopilot-system
spec:
  sourceRef:
    kind: GitRepository
    name: my-service-repo
    namespace: flux-system
  provider:
    type: gcp
    gcp:
      region: us-central1
  secrets:
    environment: prod
    kustomizePath: deployment-configuration/profiles/prod
    prefix: my-service
  configs:
    enabled: true  # Route properties to Parameter Manager
```

**Kustomization (applies Crossplane manifests):**
```yaml
apiVersion: kustomize.toolkit.fluxcd.io/v1
kind: Kustomization
metadata:
  name: my-service-infra
  namespace: flux-system
spec:
  interval: 5m
  path: ./infrastructure
  sourceRef:
    kind: GitRepository
    name: my-service-repo
  prune: true
```

### 3. Controller Sync Process

1. **Controller watches** `SecretManagerConfig`
2. **Reads** Git repository via FluxCD artifact
3. **Decrypts** SOPS-encrypted files (if applicable)
4. **Parses** `application.secrets.env` and `application.properties`
5. **Syncs secrets** to GCP Secret Manager:
   - `my-service-database-url` = `postgresql://user:password@host:5432/db`
   - `my-service-api-key` = `sk_live_1234567890`
6. **Syncs configs** to GCP Parameter Manager:
   - `my-service-prod/api.timeout` = `30s`
   - `my-service-prod/api.retries` = `3`
   - `my-service-prod/database.pool.size` = `10`

### 4. Crossplane Provisioning

1. **FluxCD applies** `cloudrun-service.yaml`
2. **Crossplane operator** reconciles the Service resource
3. **Crossplane creates** CloudRun Service in GCP
4. **CloudRun Service** references secrets via `secretKeyRef`:
   - `my-service-database-url` → Injected as `DATABASE_URL`
   - `my-service-api-key` → Injected as `API_KEY`

### 5. Runtime Consumption

**Application code** (no changes needed):

```python
import os

# Secrets from Secret Manager (injected via secretKeyRef)
database_url = os.environ["DATABASE_URL"]
api_key = os.environ["API_KEY"]

# Configs from Parameter Manager (injected as env vars)
api_timeout = os.environ["API_TIMEOUT"]  # "30s"
api_retries = int(os.environ["API_RETRIES"])  # 3
```

## Key Benefits

1. **GitOps-Driven**: All secrets and configs in Git
2. **Automatic Sync**: Controller syncs to cloud providers
3. **Serverless-Native**: Works with CloudRun, Lambda, Functions
4. **Crossplane Integration**: Infrastructure as Code for serverless
5. **Separation of Concerns**: Secrets vs configs routed appropriately
6. **No Manual Steps**: Fully automated from Git to runtime

## Comparison: Kubernetes vs Serverless

| Aspect | Kubernetes | Serverless |
|--------|------------|------------|
| **Secret Consumption** | External Secrets Operator → ConfigMaps/Secrets | Native cloud provider integration |
| **Config Consumption** | ConfigMaps (native) or Config Store operators | Config Store SDKs or environment variables |
| **Infrastructure** | Kubernetes manifests | Crossplane manifests |
| **Deployment** | `kubectl apply` or GitOps | Crossplane operator |
| **Runtime Access** | Volume mounts or env vars | Environment variables or SDK calls |

## Next Steps

- [Application Files Guide](../guides/application-files.md) - Learn about file formats
- [GitOps Integration](../guides/gitops-integration.md) - Set up GitOps workflow
- [Configuration Reference](../getting-started/configuration.md) - Complete configuration guide
- [Provider Setup](../guides/aws-setup.md) - Provider-specific setup

