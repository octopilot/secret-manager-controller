# Config Store Implementation Summary

## Overview

This document provides a high-level summary of the config store implementation plan. For detailed information, see:
- [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) - Detailed implementation plan
- [IMPLEMENTATION_OPTIONS_TABLE.md](IMPLEMENTATION_OPTIONS_TABLE.md) - Comparison tables
- [CONFIG_STORE_ANALYSIS.md](CONFIG_STORE_ANALYSIS.md) - Analysis of current state
- [CONFIG_STORE_COMPARISON.md](CONFIG_STORE_COMPARISON.md) - Provider comparison

## Key Principle

> **Secrets/configs are stored in a single canonical way in application repos, then consumed optimally per cloud platform.**

## Implementation Strategy

### AWS (EKS) - Parameter Store ✅

**Status**: Ready to implement  
**SDK**: `aws-sdk-ssm` (available)  
**Consumption**: 
- EKS: ASCP (Secrets Store CSI Driver) - mounts as files ✅
- Lambda: Lambda Extension (caching) ✅

**Implementation**: High priority, 2-3 days

### GCP - Hybrid Solution

**Option 1: Secret Manager (Interim Default)** ✅
- **Status**: Ready to implement
- **SDK**: Already in use
- **Consumption**: 
  - GKE: External Secrets Operator (industry standard) ✅
  - CloudRun: `secretKeyRef` ✅
- **Implementation**: High priority, 1-2 days

**Option 2: Parameter Manager (After ESO Contribution)** ⭐
- **Status**: Contribute to External Secrets Operator
- **SDK**: Need to verify API availability
- **Consumption**:
  - GKE: External Secrets Operator (after contribution) ⭐
  - CloudRun: Environment variables/API ✅
- **Implementation**: Contribute to ESO (2-3 weeks), then use

**Recommendation**: ⭐ **Contribute GCP Parameter Manager support to External Secrets Operator**
- **Interim**: Use Secret Manager (already supported by ESO)
- **Long-term**: Contribute Parameter Manager support to ESO for complete solution

### Azure - App Configuration ⭐

**Status**: Contribute to External Secrets Operator  
**SDK**: Need to verify `azure-app-configuration` crate  
**Consumption**:
- AKS: External Secrets Operator (after contribution) ⭐
- Functions: App Configuration SDK ✅

**Implementation**: ⭐ **Contribute Azure App Configuration support to External Secrets Operator**
- **Interim**: Use Azure App Configuration Kubernetes Provider (official Azure solution)
- **Long-term**: Contribute App Configuration support to ESO for unified experience

## CRD Design

```yaml
spec:
  # Existing fields
  provider:
    type: aws  # aws | gcp | azure
    # ... provider config
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/profiles/dev
  
  # NEW: Config store configuration
  configs:
    enabled: true  # Default: false (backward compatible)
    
    # GCP-specific: Choose store type
    store: SecretManager  # SecretManager (default) | ParameterManager
    
    # AWS-specific: Parameter path prefix
    parameterPath: /my-service/dev  # Optional
    
    # Azure-specific: App Configuration endpoint
    appConfigEndpoint: https://my-app-config.azconfig.io  # Optional
```

## File Routing

| File Pattern | Current | Proposed |
|--------------|---------|----------|
| `application.secrets.*` | Secret Store ✅ | Secret Store ✅ |
| `application.properties` | Secret Store ❌ | **Config Store** ✅ |
| `application.config.*` | N/A | **Config Store** ✅ |

## Implementation Phases

1. **Phase 1**: AWS Parameter Store (2-3 days) - High priority
2. **Phase 2**: GCP Secret Manager config routing (1-2 days) - High priority
3. **Phase 3**: Azure App Configuration (2-3 days) - Medium priority
4. **Phase 4**: GCP Parameter Manager (2-3 days) - Low priority (if API available)

## Backward Compatibility

✅ **No breaking changes**: `configs.enabled: false` by default maintains current behavior

## Consumption Summary

### Serverless Systems

| Provider | Config Store | Consumption Method |
|----------|-------------|-------------------|
| AWS Lambda | Parameter Store | Lambda Extension |
| GCP CloudRun | Secret Manager / Parameter Manager | `secretKeyRef` / Env vars |
| Azure Functions | App Configuration | App Config SDK |

### Kubernetes Workloads

| Provider | Config Store | Consumption Method |
|----------|-------------|-------------------|
| AWS EKS | Parameter Store | ASCP (mounts as files) ✅ |
| GCP GKE | Secret Manager | External Secrets Operator ✅ |
| Azure AKS | App Configuration | SDK calls ⚠️ |

## Focus: Get Configs into Cloud Stores

⭐ **Primary Goal**: Sync `application.properties` from Git → Cloud Config Stores

**Consumption layer (ESO contributions) comes later** - focus on getting configs into stores first.

## Implementation Phases

### Phase 1: AWS Parameter Store ✅ (Ready)

**Status**: Ready to implement  
**SDK**: `aws-sdk-ssm` (available)  
**Effort**: 2-3 days  
**Priority**: High

**Tasks**:
1. Create `ConfigStoreProvider` trait
2. Implement `AwsParameterStore` provider
3. Update reconciler to route configs
4. Store individual properties (not JSON blob)

### Phase 2: GCP Secret Manager ✅ (Interim)

**Status**: Ready to implement  
**SDK**: Already in use  
**Effort**: 1-2 days  
**Priority**: High

**Tasks**:
1. Update reconciler to route configs to Secret Manager
2. Store individual properties as separate secrets
3. Reuse existing `SecretManager` provider

**Note**: Using Secret Manager as interim solution. Parameter Manager contribution comes later.

### Phase 3: Azure App Configuration ⚠️ (Research)

**Status**: Research SDK availability  
**SDK**: Need to verify `azure-app-configuration` crate  
**Effort**: 2-3 days (after SDK research)  
**Priority**: Medium

**Tasks**:
1. Research SDK availability
2. Implement `AzureAppConfiguration` provider
3. Update reconciler

## Next Steps

1. ✅ Design `ConfigStoreProvider` trait
2. 🔄 Implement Phase 1: AWS Parameter Store
3. 🔄 Implement Phase 2: GCP Secret Manager config routing
4. ⏳ Research Azure App Configuration SDK
5. ⏳ Implement Phase 3: Azure App Configuration
6. ⏳ Future: Contribute to ESO for consumption layer

See [Config Store Implementation](CONFIG_STORE_IMPLEMENTATION.md) for detailed implementation plan.

## Key Decisions

1. **AWS**: ✅ Use Parameter Store for configs (best EKS integration, lower cost, ESO already supports)
2. **GCP**: ⭐ **Contribute Parameter Manager support to External Secrets Operator** (most prudent route)
   - **Interim**: Use Secret Manager (ESO already supports)
   - **Long-term**: Contribute Parameter Manager support to ESO
3. **Azure**: ⭐ **Contribute App Configuration support to External Secrets Operator** (unified experience)
   - **Interim**: Use Azure App Configuration Kubernetes Provider (official Azure solution)
   - **Long-term**: Contribute App Configuration support to ESO
4. **All**: ✅ Maintain backward compatibility (`configs.enabled: false` by default)

## Strategic Approach: Contribute to External Secrets Operator

⭐ **Most Prudent Route**: Contribute missing providers to External Secrets Operator

**Why**:
- Leverages existing, well-maintained infrastructure
- Benefits entire Kubernetes community
- Consistent experience across providers
- Avoids duplicating effort
- Industry standard approach

**What to Contribute**:
1. **GCP Parameter Manager Provider** (2-3 weeks)
2. **Azure App Configuration Provider** (2-3 weeks)

**Interim Solutions**:
- GCP: Use Secret Manager (ESO already supports)
- Azure: Use Azure App Configuration Kubernetes Provider (official Azure solution)

See [Contributing to External Secrets Operator](CONTRIBUTING_TO_ESO.md) for detailed contribution plan.

