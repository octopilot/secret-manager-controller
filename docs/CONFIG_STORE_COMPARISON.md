# Config Store Comparison Table

## Quick Reference: Secret Stores vs Config Stores

### Serverless Systems

| Provider | Serverless Platform | Secret Store | Config Store | Current Controller Support |
|----------|-------------------|--------------|--------------|----------------------------|
| **GCP** | CloudRun<br/>Cloud Functions | Secret Manager | Parameter Manager | ❌ Only Secret Manager |
| **AWS** | Lambda<br/>ECS Fargate<br/>App Runner | Secrets Manager | Systems Manager<br/>Parameter Store<br/>AppConfig | ❌ Only Secrets Manager |
| **Azure** | Azure Functions<br/>Container Apps<br/>App Service | Key Vault | App Configuration | ❌ Only Key Vault |

### Kubernetes Workloads (GKE, EKS, AKS)

| Provider | Kubernetes Platform | Secret Store | Config Store | Consumption Method | Current Controller Support |
|----------|-------------------|--------------|--------------|-------------------|----------------------------|
| **GCP** | GKE | Secret Manager | Parameter Manager<br/>**ConfigMaps** | Secrets Store CSI Driver<br/>Secret Manager add-on<br/>**Native ConfigMaps** | ❌ Only Secret Manager |
| **AWS** | EKS | Secrets Manager | Parameter Store<br/>**ConfigMaps** | Secrets Store CSI Driver<br/>**ASCP supports Parameter Store**<br/>**Native ConfigMaps** | ❌ Only Secrets Manager |
| **Azure** | AKS | Key Vault | App Configuration<br/>**ConfigMaps** | Secrets Store CSI Driver<br/>Azure Key Vault Provider<br/>**Native ConfigMaps** | ❌ Only Key Vault |

**Key Finding**: 
- ✅ **EKS**: ASCP (AWS Secrets and Configuration Provider) can mount Parameter Store values directly into pods
- ✅ **GKE**: Secret Manager add-on can access Parameter Manager (if supported)
- ✅ **AKS**: Azure Key Vault Provider (App Configuration support may vary)
- ✅ **All**: Native ConfigMaps are the standard Kubernetes approach for non-secret configs

## Detailed Comparison

### Google Cloud Platform (GCP)

| Aspect | Secret Manager | Parameter Manager |
|--------|---------------|------------------|
| **Purpose** | Store sensitive data (API keys, passwords, certificates) | Store non-sensitive configuration values |
| **Cost** | ~$0.06/secret/month | Lower cost (optimized for configs) |
| **Access Method** | `secretKeyRef` in CloudRun<br/>API calls in Cloud Functions | Environment variables<br/>API calls<br/>`@google-cloud/params` SDK |
| **Features** | Versioning, IAM access control, audit logging | Hierarchical organization, versioning, environment-specific values |
| **Current Controller** | ✅ Supported | ❌ Not supported |
| **Use Case** | Database passwords, API keys, TLS certificates | Database hostnames, port numbers, timeout values, feature flags |

### Amazon Web Services (AWS)

| Aspect | Secrets Manager | Parameter Store | AppConfig |
|--------|----------------|-----------------|-----------|
| **Purpose** | Store sensitive data with rotation | Store configuration data | Feature flags & dynamic config |
| **Cost** | $0.40/secret/month | Free tier, then $0.05/parameter/month | Per operation pricing |
| **Access Method** | Lambda extension (caching)<br/>SDK calls<br/>ECS task definitions | Lambda extension (caching)<br/>SDK calls<br/>Environment variables | Lambda extension<br/>SDK calls |
| **Features** | Automatic rotation, versioning | Hierarchical paths, versioning, Lambda extension caching | Validation, gradual rollouts, feature flags |
| **Current Controller** | ✅ Supported | ❌ Not supported | ❌ Not supported |
| **Use Case** | Database credentials, API keys | Database hostnames, port numbers, timeout values | Feature flags, dynamic settings, A/B testing configs |

### Microsoft Azure

| Aspect | Key Vault | App Configuration |
|--------|-----------|-------------------|
| **Purpose** | Store secrets, keys, certificates | Centralized application settings and feature flags |
| **Cost** | Per operation pricing | Per operation pricing (lower) |
| **Access Method** | Managed identity<br/>Key Vault references in app settings<br/>`@azure/keyvault-secrets` SDK | App Configuration SDK<br/>Environment variables<br/>`@azure/app-configuration` SDK |
| **Features** | Versioning, access policies, audit logging | Feature flags, hierarchical keys, environment-specific configs |
| **Current Controller** | ✅ Supported | ❌ Not supported |
| **Use Case** | Database passwords, API keys, TLS certificates | Database hostnames, port numbers, timeout values, feature flags |

## Current Controller Behavior

### What Gets Stored Where

| File Type | Current Destination | Should Be |
|-----------|-------------------|-----------|
| `application.secrets.env` | Secret Store ✅ | Secret Store ✅ |
| `application.secrets.yaml` | Secret Store ✅ | Secret Store ✅ |
| `application.properties` | Secret Store ❌ | Config Store ✅ |

### Example: application.properties

**Current behavior:**
```properties
# application.properties
database.host=db.example.com
database.port=5432
api.timeout=30s
```

**Stored as:**
- **GCP**: `my-service-properties-prod` in Secret Manager (JSON encoded)
- **AWS**: `my-service-properties-prod` in Secrets Manager (JSON encoded)
- **Azure**: `my-service-properties-prod` in Key Vault (JSON encoded)

**Should be stored as:**
- **GCP**: Individual parameters in Parameter Manager
- **AWS**: Individual parameters in Parameter Store (`/my-service/prod/database.host`, etc.)
- **Azure**: Key-value pairs in App Configuration (`my-service:prod:database.host`, etc.)

## Recommended Solution: File-Based Routing

### Proposed File Naming Convention

| File Name | Purpose | Destination Store |
|-----------|---------|-------------------|
| `application.secrets.env` | Secrets | Secret Store ✅ |
| `application.secrets.yaml` | Secrets | Secret Store ✅ |
| `application.properties` | Configs | Config Store ✅ |
| `application.config.env` | Configs | Config Store ✅ (new) |
| `application.config.yaml` | Configs | Config Store ✅ (new) |

### Implementation Benefits

1. **Clear Intent**: File naming makes purpose obvious
2. **Backward Compatible**: Existing `application.properties` → config store
3. **Cost Savings**: Configs stored in cheaper config stores
4. **Better Integration**: Native serverless support (Lambda extensions, App Config SDK)
5. **Separation of Concerns**: Secrets vs configs clearly separated

### Migration Path

1. **Phase 1**: Add config store providers (non-breaking)
2. **Phase 2**: Route `application.properties` → config stores by default
3. **Phase 3**: Remove properties → secret store code

## Cost Impact Example

Assuming 50 config values per service:

| Provider | Current (Secret Store) | Proposed (Config Store) | Monthly Savings |
|----------|----------------------|----------------------|-----------------|
| **GCP** | 50 × $0.06 = $3.00 | 50 × ~$0.01 = $0.50 | $2.50 |
| **AWS** | 50 × $0.40 = $20.00 | 50 × $0.00 (free tier) = $0.00 | $20.00 |
| **Azure** | 50 × ~$0.10 = $5.00 | 50 × ~$0.02 = $1.00 | $4.00 |

**Note**: Actual costs vary by usage patterns and provider pricing.

## Kubernetes Consumption Summary

### Can Kubernetes Workloads Consume Config Stores?

**Yes**, but with important considerations:

1. **EKS (AWS)**: ✅ **Best Support**
   - ASCP (AWS Secrets and Configuration Provider) supports Parameter Store directly
   - Can mount Parameter Store values as files in pods via Secrets Store CSI Driver
   - Authentication: IRSA or Pod Identity

2. **GKE (GCP)**: ⚠️ **Partial Support**
   - Secret Manager add-on can access Parameter Manager (if supported)
   - May require API calls instead of direct mounting
   - Native ConfigMaps are preferred for non-secrets

3. **AKS (Azure)**: ⚠️ **Limited Support**
   - Azure Key Vault Provider primarily for secrets
   - App Configuration may require separate SDK integration
   - Native ConfigMaps are preferred for non-secrets

### Recommendation for Kubernetes Workloads

**Primary Use Case**: Config stores are most valuable for **serverless systems** (CloudRun, Lambda, Functions) that don't have native ConfigMaps.

**For Kubernetes Workloads**:
- **Option 1**: Use native ConfigMaps (standard Kubernetes approach)
- **Option 2**: Use config stores via Secrets Store CSI Driver (EKS has best support)
- **Option 3**: Controller could sync to ConfigMaps instead of config stores for K8s workloads

**Conclusion**: Config store support is **primarily for serverless**, but Kubernetes workloads CAN consume them (especially EKS with Parameter Store).

## Next Steps

1. Review [CONFIG_STORE_ANALYSIS.md](CONFIG_STORE_ANALYSIS.md) for detailed implementation plan
2. Verify SDK availability for each config store
3. Design config store provider interfaces
4. Implement file-based routing (focus on serverless use cases)
5. Consider ConfigMap sync option for Kubernetes workloads
6. Add tests and documentation

