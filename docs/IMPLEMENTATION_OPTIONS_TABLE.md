# Implementation Options Comparison Table

## Quick Reference: Implementation Options by Provider

| Provider | Config Store Option | SDK Status | K8s Consumption | Serverless Consumption | Cost | Recommendation |
|----------|-------------------|------------|-----------------|----------------------|------|----------------|
| **AWS** | Parameter Store | ✅ `aws-sdk-ssm` | ✅ ASCP (mounts as files) | ✅ Lambda Extension | Low | **Implement** |
| **GCP** | Secret Manager | ✅ Already in use | ✅ External Secrets Operator | ✅ `secretKeyRef` | Medium | **Default** |
| **GCP** | Parameter Manager | ⚠️ Research needed | ⚠️ API calls | ✅ Environment vars/API | Low | **Optional** |
| **Azure** | App Configuration | ⚠️ Research needed | ⚠️ SDK calls | ✅ App Config SDK | Low | **Implement** |

## Detailed Comparison Tables

### AWS Implementation Options

| Aspect | Parameter Store | Secrets Manager (Current) |
|--------|----------------|--------------------------|
| **Purpose** | Non-secret configs | Secrets |
| **Rust SDK** | ✅ `aws-sdk-ssm` | ✅ `aws-sdk-secretsmanager` |
| **Parameter Format** | `/service/env/key` (hierarchical) | ARN-based |
| **EKS Consumption** | ✅ ASCP mounts as files | ✅ ASCP mounts as files |
| **Lambda Consumption** | ✅ Lambda Extension (caching) | ✅ Lambda Extension (caching) |
| **Cost** | Free tier, then $0.05/param/month | $0.40/secret/month |
| **Implementation** | ✅ Ready to implement | ✅ Already implemented |
| **Recommendation** | **Use for configs** | **Use for secrets** |

### GCP Implementation Options

| Aspect | Secret Manager (Default) | Parameter Manager (Optional) |
|--------|------------------------|------------------------------|
| **Purpose** | Secrets + Configs (default) | Configs only |
| **Rust SDK** | ✅ `google-cloud-secretmanager-v1` | ⚠️ Research needed |
| **GKE Consumption** | ✅ External Secrets Operator (industry standard) | ⚠️ API calls (no native mounting) |
| **CloudRun Consumption** | ✅ `secretKeyRef` | ✅ Environment variables/API |
| **Cost** | ~$0.06/secret/month | Lower (optimized for configs) |
| **Implementation** | ✅ Ready (current behavior) | ⚠️ Research API availability |
| **Recommendation** | **Default for GKE** | **Optional for serverless** |

**GCP Decision Matrix**:

| Use Case | Recommended Store | Reason |
|----------|------------------|--------|
| GKE workloads | Secret Manager | External Secrets Operator support |
| Serverless-heavy | Parameter Manager | Lower cost |
| Mixed workloads | Secret Manager | Better GKE integration |

### Azure Implementation Options

| Aspect | Key Vault (Current) | App Configuration (Proposed) |
|--------|-------------------|----------------------------|
| **Purpose** | Secrets only | Configs only |
| **Rust SDK** | ✅ `azure_security_keyvault_secrets` | ⚠️ Research `azure-app-configuration` |
| **AKS Consumption** | ✅ Secrets Store CSI Driver | ⚠️ SDK calls (no native mounting) |
| **Functions Consumption** | ✅ Key Vault references | ✅ App Configuration SDK |
| **Cost** | Higher (per operation) | Lower (per operation) |
| **Implementation** | ✅ Already implemented | ⚠️ Research SDK availability |
| **Recommendation** | **Use for secrets** | **Use for configs** |

## File Routing Matrix

### Current Behavior (All → Secret Stores)

| File Type | AWS | GCP | Azure |
|-----------|-----|-----|-------|
| `application.secrets.env` | Secrets Manager ✅ | Secret Manager ✅ | Key Vault ✅ |
| `application.secrets.yaml` | Secrets Manager ✅ | Secret Manager ✅ | Key Vault ✅ |
| `application.properties` | Secrets Manager ❌ | Secret Manager ❌ | Key Vault ❌ |

### Proposed Behavior (Configs → Config Stores)

| File Type | AWS | GCP (Default) | GCP (Optional) | Azure |
|-----------|-----|---------------|----------------|-------|
| `application.secrets.env` | Secrets Manager ✅ | Secret Manager ✅ | Secret Manager ✅ | Key Vault ✅ |
| `application.secrets.yaml` | Secrets Manager ✅ | Secret Manager ✅ | Secret Manager ✅ | Key Vault ✅ |
| `application.properties` | **Parameter Store** ✅ | **Secret Manager** ✅ | **Parameter Manager** ✅ | **App Configuration** ✅ |
| `application.config.env` | **Parameter Store** ✅ | **Secret Manager** ✅ | **Parameter Manager** ✅ | **App Configuration** ✅ |
| `application.config.yaml` | **Parameter Store** ✅ | **Secret Manager** ✅ | **Parameter Manager** ✅ | **App Configuration** ✅ |

## Consumption Methods Matrix

### Serverless Systems

| Provider | Service | Secret Store | Config Store | Secret Consumption | Config Consumption |
|----------|---------|-------------|--------------|------------------|-------------------|
| **AWS Lambda** | Lambda | Secrets Manager | Parameter Store | Lambda Extension | Lambda Extension |
| **GCP CloudRun** | CloudRun | Secret Manager | Secret Manager / Parameter Manager | `secretKeyRef` | `secretKeyRef` / Env vars |
| **Azure Functions** | Functions | Key Vault | App Configuration | Key Vault refs | App Config SDK |

### Kubernetes Workloads

| Provider | Service | Secret Store | Config Store | Secret Consumption | Config Consumption |
|----------|---------|-------------|--------------|------------------|-------------------|
| **AWS EKS** | EKS | Secrets Manager | Parameter Store | ASCP (CSI Driver) | ASCP (CSI Driver) |
| **GCP GKE** | GKE | Secret Manager | Secret Manager | External Secrets Operator | External Secrets Operator |
| **GCP GKE** | GKE | Secret Manager | Parameter Manager | External Secrets Operator | API calls |
| **Azure AKS** | AKS | Key Vault | App Configuration | CSI Driver | SDK calls |

## CRD Configuration Examples

### AWS Configuration

```yaml
spec:
  provider:
    type: aws
    aws:
      region: us-east-1
      auth:
        authType: Irsa
        roleArn: arn:aws:iam::123456789012:role/secret-manager-role
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/profiles/dev
  configs:
    enabled: true
    parameterPath: /my-service/dev  # Optional
```

**Result**:
- Secrets → Secrets Manager
- Configs → Parameter Store at `/my-service/dev/*`

### GCP Configuration (Default: Secret Manager)

```yaml
spec:
  provider:
    type: gcp
    gcp:
      projectId: my-project
      auth:
        authType: WorkloadIdentity
        serviceAccountEmail: secret-manager@my-project.iam.gserviceaccount.com
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/profiles/dev
  configs:
    enabled: true
    store: SecretManager  # Default
```

**Result**:
- Secrets → Secret Manager
- Configs → Secret Manager (consumed via External Secrets Operator in GKE)

### GCP Configuration (Optional: Parameter Manager)

```yaml
spec:
  provider:
    type: gcp
    gcp:
      projectId: my-project
      auth:
        authType: WorkloadIdentity
        serviceAccountEmail: secret-manager@my-project.iam.gserviceaccount.com
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/profiles/dev
  configs:
    enabled: true
    store: ParameterManager  # Optional
```

**Result**:
- Secrets → Secret Manager
- Configs → Parameter Manager (lower cost, but GKE needs API calls)

### Azure Configuration

```yaml
spec:
  provider:
    type: azure
    azure:
      vaultName: my-vault
      auth:
        authType: WorkloadIdentity
        clientId: "12345678-1234-1234-1234-123456789012"
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/profiles/dev
  configs:
    enabled: true
    appConfigEndpoint: https://my-app-config.azconfig.io  # Optional
```

**Result**:
- Secrets → Key Vault
- Configs → App Configuration

## Implementation Priority

| Phase | Provider | Feature | Priority | Effort | Dependencies |
|-------|----------|---------|----------|--------|--------------|
| **1** | AWS | Parameter Store | High | 2-3 days | None (SDK available) |
| **2** | GCP | Secret Manager (configs) | High | 1-2 days | None (already using) |
| **3** | Azure | App Configuration | Medium | 2-3 days | SDK research |
| **4** | GCP | Parameter Manager | Low | 2-3 days | API research |

## Cost Comparison (Example: 50 Config Values)

| Provider | Current (Secret Store) | Proposed (Config Store) | Monthly Savings |
|----------|----------------------|------------------------|-----------------|
| **AWS** | 50 × $0.40 = $20.00 | 50 × $0.00 (free tier) = $0.00 | **$20.00** |
| **GCP (Secret Manager)** | 50 × $0.06 = $3.00 | 50 × $0.06 = $3.00 | $0.00 (but better integration) |
| **GCP (Parameter Manager)** | 50 × $0.06 = $3.00 | 50 × ~$0.01 = $0.50 | **$2.50** |
| **Azure** | 50 × ~$0.10 = $5.00 | 50 × ~$0.02 = $1.00 | **$4.00** |

## Decision Tree

### For AWS Users
```
application.properties → Parameter Store ✅
Reason: Best EKS integration, lower cost, SDK available
```

### For GCP Users
```
GKE workloads?
  Yes → Secret Manager (External Secrets Operator) ✅
  No → Parameter Manager (lower cost) ✅
  
Default: Secret Manager (better GKE integration)
```

### For Azure Users
```
application.properties → App Configuration ✅
Reason: Purpose-built for configs, lower cost
```

## Summary Recommendations

1. **AWS**: ✅ Implement Parameter Store support (high priority, SDK available)
2. **GCP**: ✅ Default to Secret Manager for configs (better GKE integration), optionally support Parameter Manager
3. **Azure**: ✅ Implement App Configuration support (research SDK first)
4. **All**: Maintain backward compatibility (`configs.enabled: false` by default)

