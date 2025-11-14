# Contributing to External Secrets Operator

## Strategic Decision

Instead of building a new "External Parameter Manager" controller, we should **contribute to External Secrets Operator (ESO)** to add support for:
1. **GCP Parameter Manager** - Currently missing
2. **Azure App Configuration** - Currently missing (Azure has separate provider, but ESO integration would be better)

## Why Contribute to ESO?

### Benefits

1. **Leverage Existing Infrastructure**
   - ESO is mature, well-maintained, and widely adopted
   - Established patterns and architecture
   - Active community and maintainers

2. **Benefit Entire Kubernetes Community**
   - Our contribution helps all Kubernetes users
   - Reduces fragmentation in the ecosystem
   - Standardizes config management approach

3. **Consistent Experience**
   - Single operator for all cloud providers
   - Unified CRD interface
   - Consistent behavior across providers

4. **Avoid Duplication**
   - No need to build and maintain separate operator
   - Reuse existing ESO infrastructure
   - Focus on provider implementation, not operator logic

5. **Long-term Sustainability**
   - ESO is maintained by CNCF
   - Active development and support
   - Better than maintaining our own operator

## Current ESO Support

| Provider | Secret Store | Config Store | ESO Support |
|----------|-------------|--------------|-------------|
| **AWS** | Secrets Manager ✅ | Parameter Store ✅ | ✅ Both supported |
| **GCP** | Secret Manager ✅ | Parameter Manager ❌ | ⚠️ Secrets only |
| **Azure** | Key Vault ✅ | App Configuration ❌ | ⚠️ Secrets only |

## What Needs to Be Contributed

### 1. GCP Parameter Manager Provider

**Current State**: ESO has GCP Secret Manager provider, but not Parameter Manager

**What to Add**:
- New provider: `gcp-parameter-manager`
- Similar to existing `gcp-secrets-manager` provider
- Uses GCP Parameter Manager API
- Creates ConfigMaps (not Secrets) from parameters

**Implementation Approach**:
- Follow existing GCP Secret Manager provider pattern
- Use GCP Parameter Manager API (similar to Secret Manager API)
- Support Workload Identity authentication (already in ESO)
- Create ConfigMaps instead of Secrets

**Reference**: 
- ESO GCP Secret Manager provider: https://github.com/external-secrets/external-secrets/tree/main/pkg/provider/gcp
- GCP Parameter Manager API: https://cloud.google.com/secret-manager/parameter-manager/docs

### 2. Azure App Configuration Provider

**Current State**: ESO has Azure Key Vault provider, but not App Configuration

**What to Add**:
- New provider: `azure-app-configuration`
- Similar to existing `azure-keyvault` provider
- Uses Azure App Configuration API
- Creates ConfigMaps from App Configuration key-values

**Implementation Approach**:
- Follow existing Azure Key Vault provider pattern
- Use Azure App Configuration SDK
- Support Managed Identity authentication (already in ESO)
- Create ConfigMaps instead of Secrets

**Reference**:
- ESO Azure Key Vault provider: https://github.com/external-secrets/external-secrets/tree/main/pkg/provider/azure
- Azure App Configuration API: https://learn.microsoft.com/en-us/azure/azure-app-configuration/rest-api-reference

## Contribution Process

### Step 1: Research ESO Architecture

1. **Study Existing Providers**
   - Review GCP Secret Manager provider implementation
   - Review Azure Key Vault provider implementation
   - Understand provider interface and patterns

2. **Understand ESO Architecture**
   - How providers are registered
   - How authentication works
   - How ConfigMaps vs Secrets are created
   - Testing patterns

### Step 2: Design Provider Interface

1. **GCP Parameter Manager Provider**
   ```go
   // Similar to existing GCP provider
   type GCPParameterManagerProvider struct {
       client *parameterManagerClient
       projectID string
   }
   
   func (p *GCPParameterManagerProvider) GetSecret(ctx context.Context, ref ExternalSecretRef) ([]byte, error)
   func (p *GCPParameterManagerProvider) GetSecretMap(ctx context.Context, ref ExternalSecretRef) (map[string][]byte, error)
   ```

2. **Azure App Configuration Provider**
   ```go
   // Similar to existing Azure provider
   type AzureAppConfigurationProvider struct {
       client *appConfigClient
       endpoint string
   }
   
   func (p *AzureAppConfigurationProvider) GetSecret(ctx context.Context, ref ExternalSecretRef) ([]byte, error)
   func (p *AzureAppConfigurationProvider) GetSecretMap(ctx context.Context, ref ExternalSecretRef) (map[string][]byte, error)
   ```

### Step 3: Implementation

1. **Create Provider Code**
   - Implement provider interface
   - Handle authentication (Workload Identity for GCP, Managed Identity for Azure)
   - Implement API calls to Parameter Manager/App Configuration
   - Handle errors and edge cases

2. **Add Tests**
   - Unit tests for provider logic
   - Integration tests with real APIs (or mocks)
   - Follow ESO testing patterns

3. **Add Documentation**
   - Provider documentation
   - Usage examples
   - Authentication setup guide

### Step 4: Contribute to ESO

1. **Fork ESO Repository**
   - Fork: https://github.com/external-secrets/external-secrets

2. **Create Feature Branch**
   - Branch: `feature/gcp-parameter-manager-provider`
   - Branch: `feature/azure-app-configuration-provider`

3. **Submit Pull Request**
   - Follow ESO contribution guidelines
   - Include tests and documentation
   - Address review feedback

4. **Work with Maintainers**
   - Engage with ESO community
   - Address feedback
   - Iterate on implementation

## Implementation Timeline

### Phase 1: GCP Parameter Manager Provider

**Estimated Effort**: 2-3 weeks

1. **Week 1**: Research and design
   - Study ESO architecture
   - Design provider interface
   - Set up development environment

2. **Week 2**: Implementation
   - Implement GCP Parameter Manager provider
   - Add tests
   - Write documentation

3. **Week 3**: Contribution
   - Submit PR to ESO
   - Address review feedback
   - Get merged

### Phase 2: Azure App Configuration Provider

**Estimated Effort**: 2-3 weeks

1. **Week 1**: Research and design
   - Study Azure App Configuration API
   - Design provider interface
   - Review Azure Key Vault provider

2. **Week 2**: Implementation
   - Implement Azure App Configuration provider
   - Add tests
   - Write documentation

3. **Week 3**: Contribution
   - Submit PR to ESO
   - Address review feedback
   - Get merged

## Benefits After Contribution

### For Our Controller

1. **Complete Consumption Story**
   - All config stores supported by ESO
   - Consistent experience across providers
   - No gaps in Kubernetes consumption

2. **Reduced Maintenance**
   - No need to maintain separate operator
   - ESO handles all consumption logic
   - Focus on Git → Cloud Store sync

3. **Better User Experience**
   - Single operator for all providers
   - Consistent CRD interface
   - Standard patterns

### For Kubernetes Community

1. **Standardized Approach**
   - Single operator for all config stores
   - Consistent patterns
   - Reduced fragmentation

2. **Better Ecosystem**
   - More providers supported
   - Better integration
   - Active development

## Alternative: Use Existing Solutions

### GCP: Use Secret Manager (Interim)

- ✅ Already supported by ESO
- ✅ Works for both secrets and configs
- ❌ Higher cost than Parameter Manager
- **Use while contributing**: Can use Secret Manager while contributing Parameter Manager support

### Azure: Use Azure App Config Provider (Interim)

- ✅ Official Azure solution
- ✅ Already available
- ⚠️ Separate from ESO (but works)
- **Use while contributing**: Can use Azure provider while contributing ESO support

## Recommendation

⭐ **Contribute to External Secrets Operator** - This is the most prudent route:

1. **Short-term**: Use interim solutions (Secret Manager for GCP, Azure App Config Provider for Azure)
2. **Long-term**: Contribute to ESO to add missing providers
3. **Result**: Complete, standardized solution for all providers

## Next Steps

1. ✅ Research ESO architecture and contribution process
2. 🔄 Design GCP Parameter Manager provider
3. 🔄 Design Azure App Configuration provider
4. ⏳ Implement GCP Parameter Manager provider
5. ⏳ Implement Azure App Configuration provider
6. ⏳ Submit PRs to ESO
7. ⏳ Work with ESO maintainers to get merged

## Resources

- **ESO Repository**: https://github.com/external-secrets/external-secrets
- **ESO Documentation**: https://external-secrets.io/
- **ESO Contributing Guide**: https://github.com/external-secrets/external-secrets/blob/main/CONTRIBUTING.md
- **GCP Parameter Manager API**: https://cloud.google.com/secret-manager/parameter-manager/docs
- **Azure App Configuration API**: https://learn.microsoft.com/en-us/azure/azure-app-configuration/rest-api-reference

