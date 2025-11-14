# Config Store Analysis for Serverless Systems

## Overview

This document analyzes how the three major cloud providers handle non-secret configuration values for both **serverless systems** and **Kubernetes workloads** (GKE, EKS, AKS), and proposes a solution for routing `application.properties` to appropriate config stores.

## Current State

The controller currently stores **all values** (secrets and properties) in secret management systems:
- **GCP**: All values → Secret Manager
- **AWS**: All values → Secrets Manager  
- **Azure**: All values → Key Vault

This is suboptimal because:
1. Non-secret configs don't need the same security/encryption overhead
2. Serverless systems have separate services for configs vs secrets
3. Costs may be higher for storing non-secrets in secret stores

## Cloud Provider Comparison

| Provider | Serverless Platform | Secret Store | Config Store | Config Consumption Method |
|----------|-------------------|--------------|--------------|---------------------------|
| **GCP** | CloudRun<br/>Cloud Functions | Secret Manager | Parameter Manager<br/>(Google Cloud Parameter Manager) | Environment variables<br/>API calls<br/>`@google-cloud/params` SDK |
| **AWS** | Lambda<br/>ECS Fargate<br/>App Runner | Secrets Manager | Systems Manager<br/>Parameter Store<br/>AppConfig | Environment variables<br/>Lambda Extension<br/>SDK calls<br/>`aws-sdk-ssm` |
| **Azure** | Azure Functions<br/>Container Apps<br/>App Service | Key Vault | App Configuration | Environment variables<br/>App Configuration SDK<br/>`@azure/app-configuration` |

## Detailed Analysis

### Kubernetes Consumption Methods

All three cloud providers support consuming config stores in Kubernetes via the **Secrets Store CSI Driver**:

#### GKE (Google Kubernetes Engine)
- **Secret Manager add-on**: Uses Secrets Store CSI Driver with Google Secret Manager provider
- **Parameter Manager**: Can be accessed via Secret Manager add-on (if supported) or API calls
- **Native alternative**: ConfigMaps for non-secret configs

#### EKS (Amazon Elastic Kubernetes Service)
- **ASCP (AWS Secrets and Configuration Provider)**: Uses Secrets Store CSI Driver
  - Supports **both** Secrets Manager AND Parameter Store
  - Can mount Parameter Store values as files in pods
  - Authentication: IRSA or Pod Identity
- **Native alternative**: ConfigMaps for non-secret configs

#### AKS (Azure Kubernetes Service)
- **Azure Key Vault Provider**: Uses Secrets Store CSI Driver
  - Primarily for Key Vault secrets
  - App Configuration may require separate integration
- **Native alternative**: ConfigMaps for non-secret configs

**Conclusion**: Kubernetes workloads CAN consume from config stores, but the primary use case is for **serverless systems** that don't have native ConfigMaps. For Kubernetes workloads, ConfigMaps are the standard approach for non-secret configs.

### Google Cloud Platform (GCP)

**Secret Store: Secret Manager**
- Purpose: Store sensitive data (API keys, passwords, certificates)
- Access: Via `secretKeyRef` in CloudRun, API calls in Cloud Functions
- Cost: Higher (designed for secrets)

**Config Store: Parameter Manager**
- Purpose: Store non-sensitive configuration values
- Access: Environment variables, API calls, SDK
- Cost: Lower (optimized for configs)
- Features: Hierarchical organization, versioning, environment-specific values

**CloudRun Integration:**
- Secrets: Use `secretKeyRef` pointing to Secret Manager
- Configs: Use environment variables or Parameter Manager API calls
- **Note**: CloudRun doesn't have native Parameter Manager integration like AWS Lambda extensions

### Amazon Web Services (AWS)

**Secret Store: Secrets Manager**
- Purpose: Store sensitive data with automatic rotation
- Access: Via Lambda extensions, SDK calls, ECS task definitions
- Cost: Higher (per secret per month)

**Config Store: Systems Manager Parameter Store**
- Purpose: Store configuration data (plaintext or encrypted)
- Access: Lambda extensions (caching), SDK calls, environment variables
- Cost: Lower (free tier available)
- Features: Hierarchical paths, versioning, Lambda extension for caching

**Lambda Integration:**
- Secrets: Use Secrets Manager Lambda extension (caching)
- Configs: Use Parameter Store Lambda extension (caching)
- Both can be accessed via environment variables or SDK

**AppConfig** (Alternative):
- Purpose: Feature flags and dynamic configuration
- Access: AppConfig Lambda extension
- Use case: More advanced config management with validation and rollouts

### Microsoft Azure

**Secret Store: Key Vault**
- Purpose: Store secrets, keys, certificates
- Access: Via managed identity, Key Vault references in app settings
- Cost: Higher (per operation pricing)

**Config Store: App Configuration**
- Purpose: Centralized application settings and feature flags
- Access: App Configuration SDK, environment variables
- Cost: Lower (per operation pricing)
- Features: Feature flags, hierarchical keys, environment-specific configs

**Azure Functions Integration:**
- Secrets: Use Key Vault references in app settings (`@Microsoft.KeyVault(...)`)
- Configs: Use App Configuration SDK or environment variables
- Both support managed identity for authentication

## Current Controller Implementation

### How Properties Are Handled

Currently, `application.properties` is:
1. Parsed as key-value pairs
2. JSON-encoded into a single string
3. Stored as a **secret** in the secret store with name `{prefix}-properties-{suffix}`

**Example:**
```yaml
# application.properties
database.host=db.example.com
database.port=5432
api.timeout=30s
```

**Current behavior:**
- Stored as: `my-service-properties-prod` in Secret Manager/Secrets Manager/Key Vault
- Value: `{"database.host":"db.example.com","database.port":"5432","api.timeout":"30s"}`
- **Problem**: Non-secret configs stored in secret stores

## Proposed Solution

### Option 1: File-Based Routing (Recommended)

Distinguish between secrets and configs based on file naming:

| File Name | Purpose | Destination |
|-----------|---------|-------------|
| `application.secrets.env` | Secrets | Secret Store |
| `application.secrets.yaml` | Secrets | Secret Store |
| `application.properties` | Configs | Config Store |
| `application.config.env` | Configs | Config Store (new) |
| `application.config.yaml` | Configs | Config Store (new) |

**Implementation:**
- Add config store providers (Parameter Manager, Parameter Store, App Configuration)
- Route based on file type:
  - `*.secrets.*` → Secret Store
  - `*.properties` → Config Store
  - `*.config.*` → Config Store

### Option 2: Explicit Configuration

Add CRD fields to specify which files go to which store:

```yaml
spec:
  secrets:
    # Secrets go to secret store
    secretFiles:
      - application.secrets.env
      - application.secrets.yaml
  configs:
    # Configs go to config store
    configFiles:
      - application.properties
      - application.config.env
```

### Option 3: Key-Based Routing

Allow prefix/suffix patterns to route to different stores:

```yaml
spec:
  secrets:
    # Keys matching pattern go to secret store
    secretPatterns:
      - "DATABASE_*"
      - "*_PASSWORD"
      - "*_KEY"
  configs:
    # All other keys go to config store
    configPatterns:
      - "*"
```

## Use Cases: Serverless vs Kubernetes

### Serverless Systems (Primary Use Case)
- **CloudRun, Lambda, Azure Functions**: Don't have native ConfigMaps
- **Need config stores**: Parameter Manager, Parameter Store, App Configuration
- **Benefit**: Centralized config management outside Kubernetes

### Kubernetes Workloads (Secondary Use Case)
- **GKE, EKS, AKS**: Have native ConfigMaps
- **Can use config stores**: Via Secrets Store CSI Driver (EKS ASCP supports Parameter Store)
- **Alternative**: Sync to ConfigMaps instead of config stores
- **Benefit**: Native Kubernetes integration, no external dependencies

**Recommendation**: 
- **Primary focus**: Config stores for serverless systems (CloudRun, Lambda, Functions)
- **Optional enhancement**: ConfigMap sync for Kubernetes workloads (can use existing ConfigMap tools)

## Recommended Approach: Option 1 (File-Based Routing)

### Benefits:
1. **Clear separation**: File naming makes intent obvious
2. **Backward compatible**: Existing `application.properties` → config store
3. **Simple implementation**: No CRD changes needed initially
4. **Matches conventions**: `.secrets.*` vs `.config.*` is intuitive

### Implementation Plan:

1. **Add Config Store Providers:**
   - `gcp_parameter_manager.rs` - Google Cloud Parameter Manager client
   - `aws_parameter_store.rs` - AWS Systems Manager Parameter Store client
   - `azure_app_config.rs` - Azure App Configuration client

2. **Update Parser:**
   - Detect file type (secrets vs configs)
   - Parse config files separately

3. **Update Reconciler:**
   - Route secrets → Secret Store providers
   - Route configs → Config Store providers

4. **Update CRD (Optional):**
   - Add `configStore` field to specify config store settings
   - Keep backward compatibility with existing behavior

### File Structure:

```
profiles/{env}/
├── application.secrets.env      → Secret Store
├── application.secrets.yaml      → Secret Store
├── application.properties       → Config Store
├── application.config.env        → Config Store (new)
└── application.config.yaml       → Config Store (new)
```

### Config Store Providers Needed:

#### GCP Parameter Manager
- Service: `google-cloud-params` (if available) or Parameter Manager API
- Client: Similar to Secret Manager client
- Storage: Individual parameters or hierarchical paths

#### AWS Parameter Store
- Service: AWS Systems Manager Parameter Store
- Client: `aws-sdk-ssm`
- Storage: `/service-name/env/key` format
- Features: Lambda extension support, caching

#### Azure App Configuration
- Service: Azure App Configuration
- Client: `@azure/app-configuration` SDK
- Storage: Key-value pairs with hierarchical keys
- Features: Feature flags, environment-specific configs

## Migration Path

### Phase 1: Add Config Store Support (Non-Breaking)
- Add config store providers
- Keep existing behavior (properties → secret store)
- Add feature flag to enable config store routing

### Phase 2: Default to Config Store
- Change default: `application.properties` → config store
- Add migration guide
- Support both behaviors during transition

### Phase 3: Remove Secret Store for Properties
- Remove properties → secret store code
- Update documentation
- Clean up deprecated code

## Cost Considerations

| Provider | Secret Store Cost | Config Store Cost | Savings |
|----------|------------------|-------------------|---------|
| **GCP** | Secret Manager: ~$0.06/secret/month | Parameter Manager: Lower cost | Significant for many configs |
| **AWS** | Secrets Manager: $0.40/secret/month | Parameter Store: Free tier, then $0.05/parameter/month | Significant |
| **Azure** | Key Vault: Per operation | App Configuration: Per operation | Moderate |

**Recommendation**: Use config stores for non-secret values to reduce costs and improve separation of concerns.

## Next Steps

1. **Research SDKs**: Verify available SDKs for Parameter Manager, Parameter Store, App Configuration
2. **Design CRD**: Determine if CRD changes are needed or file-based routing is sufficient
3. **Implement Providers**: Create config store provider implementations
4. **Update Parser**: Add config file detection and parsing
5. **Update Reconciler**: Route configs to config stores
6. **Add Tests**: Test config store integration
7. **Update Documentation**: Document new file naming conventions

