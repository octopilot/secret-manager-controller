# Next Steps Implementation Plan

## 📊 Current Status Summary

**Last Updated**: Current  
**Overall Progress**: All Phases Complete ✅

### ✅ Completed Phases
- **Phase 1: AWS Parameter Store** - Fully implemented and tested
- **Phase 2: GCP Secret Manager Config Routing** - Fully implemented
- **Phase 3: Azure App Configuration** - Fully implemented and tested ✅

### 🎉 All Implementation Complete!
All three phases of config store routing are now complete:
- ✅ AWS Parameter Store (6 Pact tests)
- ✅ GCP Secret Manager Config Routing (uses existing Secret Manager tests)
- ✅ Azure App Configuration (6 Pact tests)

### 🎯 Next Steps
1. ✅ Research Azure App Configuration Rust SDK availability - Complete
2. ✅ Enable Azure provider - Complete
3. ✅ Implement Azure App Configuration provider - Complete
4. ✅ Update reconciler with Azure routing logic - Complete
5. ✅ Add Pact tests for Azure App Configuration - Complete

---

## Current State ✅

### Completed
1. ✅ **Pact Contract Testing** - Comprehensive API coverage (51 tests)
   - GCP Secret Manager: 12 tests ✅
   - AWS Secrets Manager: 13 tests ✅
   - AWS Parameter Store: 6 tests ✅
   - Azure Key Vault: 14 tests ✅
   - Azure App Configuration: 6 tests ✅
   - All tests passing and publishing to Pact broker

2. ✅ **Controller Infrastructure**
   - Controller deploying and running in Kubernetes
   - SOPS GPG key reading from Kubernetes secrets
   - FluxCD GitRepository integration working
   - ArgoCD Application integration working
   - Kustomize build mode working
   - Raw file mode working

3. ✅ **Parser Implementation**
   - Parses `application.secrets.env` → secrets
   - Parses `application.secrets.yaml` → secrets
   - Parses `application.properties` → properties
   - ✅ SOPS decryption fully implemented (using sops binary with GPG key import)

4. ✅ **GCP Secret Manager Provider**
   - Full CRUD operations implemented
   - Version management
   - Secret lifecycle management

### Current Gap ✅ RESOLVED

**Properties routing to config stores** (IMPLEMENTED):
- ✅ AWS: Properties route to Parameter Store as individual parameters when `configs.enabled = true`
- ✅ GCP: Properties route to Secret Manager as individual secrets when `configs.enabled = true` (interim solution)
- ✅ Backward compatibility: Properties stored as JSON blob when `configs.enabled = false` (default)

## Next Implementation Phase: Config Store Routing

### Goal
Route `application.properties` to cloud config stores instead of secret stores, storing individual properties as separate entries.

### Implementation Priority

#### Phase 1: AWS Parameter Store (High Priority) ✅ COMPLETE
**Status**: ✅ Implemented  
**Effort**: Completed  
**Why First**: 
- AWS SDK already available (`aws-sdk-ssm`)
- Best EKS integration (ASCP mounts as files)
- Lower cost than Secrets Manager
- ESO already supports Parameter Store

**Tasks**:
1. ✅ Enable AWS provider (currently disabled)
2. ✅ Add `aws-sdk-ssm` dependency
3. ✅ Create `ConfigStoreProvider` trait
4. ✅ Implement `AwsParameterStore` provider
5. ✅ Update CRD schema with `configs` field
6. ✅ Update reconciler to route properties → Parameter Store
7. ✅ Store individual properties (not JSON blob)
8. ✅ Add Pact tests for Parameter Store API (6 tests)

**Storage Format**:
```
/my-service/prod/database.host = db.example.com
/my-service/prod/database.port = 5432
/my-service/prod/api.timeout = 30s
```

#### Phase 2: GCP Secret Manager Config Routing ✅ COMPLETE
**Status**: ✅ Implemented  
**Effort**: Completed  
**Why Second**: 
- Uses existing Secret Manager provider
- Quick win (no new SDK needed)
- ESO already supports Secret Manager
- Interim solution until Parameter Manager contribution

**Tasks**:
1. ✅ Update reconciler to route properties → Secret Manager
2. ✅ Store individual properties as separate secrets
3. ✅ Update CRD schema with `configs.enabled` field
4. ✅ Implementation verified (uses existing Secret Manager provider and tests)

**Storage Format**:
```
my-service-database-host-prod = db.example.com
my-service-database-port-prod = 5432
my-service-api-timeout-prod = 30s
```

**Note**: This is an interim solution. Long-term goal is to contribute GCP Parameter Manager support to External Secrets Operator.

#### Phase 3: Azure App Configuration ✅ COMPLETE
**Status**: ✅ Fully implemented and tested  
**Effort**: Completed  
**Why Third**: 
- Need to verify SDK availability
- Lower priority than AWS/GCP (but now that Phases 1 & 2 are complete, this was next)

**Tasks**:
1. ✅ Research `azure-app-configuration` Rust SDK availability
   - No official Rust SDK found - using REST API directly
   - Using `reqwest` with `azure_identity` for authentication
   - Similar pattern to Azure Key Vault
2. ✅ Enable Azure provider
   - Enabled `pub mod azure;` in `src/provider/mod.rs`
   - Refactored Azure Key Vault into separate module
3. ✅ Create `AzureAppConfiguration` provider
   - File: `src/provider/azure/app_configuration.rs`
   - Implements `ConfigStoreProvider` trait
   - Supports Workload Identity and Managed Identity authentication
   - Uses REST API: PUT/GET/DELETE `/kv` endpoints
4. ✅ Update reconciler
   - Removed placeholder error
   - Added routing logic for Azure App Configuration
   - Supports `app_config_endpoint` configuration
5. ✅ Add Pact tests
   - Created `tests/pact_azure_app_configuration.rs`
   - 6 tests covering CRUD operations
   - All tests passing

**Storage Format**:
```
my-service:prod:database.host = db.example.com
my-service:prod:database.port = 5432
my-service:prod:api.timeout = 30s
```

## CRD Design

```yaml
apiVersion: secret-manager.microscaler.io/v1alpha1
kind: SecretManagerConfig
metadata:
  name: my-service-config
spec:
  # Existing fields
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: flux-system
  provider:
    type: aws  # aws | gcp | azure
    aws:
      region: us-east-1
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/profiles/dev
    prefix: my-service
    suffix: -prod
  
  # NEW: Config store configuration
  configs:
    # Enable config store sync (default: false for backward compatibility)
    enabled: true
    
    # AWS-specific: Parameter path prefix
    # Only applies when provider.type == aws
    parameterPath: /my-service/dev  # Optional: defaults to /{prefix}/{environment}
    
    # GCP-specific: Store type (default: SecretManager)
    # Only applies when provider.type == gcp
    store: SecretManager  # SecretManager (default) | ParameterManager (future)
    
    # Azure-specific: App Configuration endpoint
    # Only applies when provider.type == azure
    appConfigEndpoint: https://my-app-config.azconfig.io  # Optional: auto-detect
```

## File Routing Logic

### Current Behavior ❌
```rust
// All files → Secret Store
application.secrets.env → Secret Store ✅
application.secrets.yaml → Secret Store ✅
application.properties → Secret Store (as JSON blob) ❌
```

### Target Behavior ✅
```rust
// File-based routing
if file_name.contains("secrets") {
    → Secret Store ✅
} else if file_name == "application.properties" || file_name.contains("config") {
    if configs.enabled {
        → Config Store (based on provider) ✅
    } else {
        → Secret Store (backward compatibility) ✅
    }
}
```

## Implementation Steps

### Step 1: Enable AWS Provider
- [ ] Uncomment AWS provider code
- [ ] Fix any rustls/crypto provider issues
- [ ] Verify AWS provider works with existing secret sync

### Step 2: Create ConfigStoreProvider Trait
- [ ] Define trait in `src/provider/mod.rs`
- [ ] Methods: `create_or_update_config`, `get_config_value`, `delete_config`
- [ ] Similar to `SecretManagerProvider` trait

### Step 3: Implement AWS Parameter Store Provider
- [ ] Create `src/provider/aws/parameter_store.rs`
- [ ] Implement `ConfigStoreProvider` trait
- [ ] Use `aws-sdk-ssm` for Parameter Store operations
- [ ] Handle parameter path construction
- [ ] Add error handling

### Step 4: Update CRD Schema
- [ ] Add `configs` field to `SecretManagerConfig` spec
- [ ] Add `ConfigsSpec` struct with fields:
  - `enabled: bool` (default: false)
  - `parameter_path: Option<String>` (AWS)
  - `store: Option<ConfigStoreType>` (GCP)
  - `app_config_endpoint: Option<String>` (Azure)
- [ ] Regenerate CRD using `crdgen`

### Step 5: Update Reconciler
- [ ] Add config store routing logic
- [ ] Route `application.properties` → config store (if enabled)
- [ ] Store individual properties (not JSON blob)
- [ ] Maintain backward compatibility (`configs.enabled: false`)

### Step 6: Add Tests
- [ ] Unit tests for Parameter Store provider
- [ ] Integration tests for config routing
- [ ] Pact tests for Parameter Store API
- [ ] Test backward compatibility

### Step 7: GCP Secret Manager Config Routing
- [ ] Update reconciler to route properties → Secret Manager
- [ ] Store individual properties as separate secrets
- [ ] Update CRD with `configs.store` field
- [ ] Add tests

### Step 8: Azure App Configuration (After Research)
- [ ] Research SDK availability
- [ ] Enable Azure provider
- [ ] Implement `AzureAppConfiguration` provider
- [ ] Update reconciler
- [ ] Add tests

## Code Structure

### New Files
```
src/provider/
  mod.rs                    # Export ConfigStoreProvider trait
  aws/
    mod.rs                  # Export ParameterStore
    parameter_store.rs      # AWS Parameter Store implementation
  gcp/
    mod.rs                  # (No changes - reuse SecretManager)
  azure/
    mod.rs                  # Export AppConfiguration
    app_configuration.rs    # Azure App Configuration implementation
```

### Updated Files
```
src/lib.rs                  # Add ConfigStoreProvider trait
src/controller/reconciler.rs # Add config routing logic
src/controller/parser.rs    # (No changes - already parses properties)
```

## Testing Strategy

### Unit Tests
- Config store provider implementations
- File routing logic
- Parameter/key name construction
- CRD validation

### Integration Tests
- AWS Parameter Store sync
- GCP Secret Manager config sync
- Azure App Configuration sync
- Backward compatibility

### Pact Tests
- Add Parameter Store API contracts
- Add App Configuration API contracts
- Verify config store operations

## Backward Compatibility

✅ **No breaking changes**:
- `configs.enabled: false` by default
- Existing CRDs continue to work
- Properties still go to secret stores unless explicitly enabled

## Success Criteria ✅ ALL MET

1. ✅ `application.properties` routes to config stores (when enabled)
2. ✅ Individual properties stored as separate entries (not JSON blob)
3. ✅ Backward compatibility maintained (`configs.enabled: false` by default)
4. ✅ All three providers supported (AWS, GCP, Azure)
   - ✅ AWS: Parameter Store (6 Pact tests)
   - ✅ GCP: Secret Manager config routing (uses existing tests)
   - ✅ Azure: App Configuration (6 Pact tests)
5. ✅ Clear CRD configuration for routing decisions
   - ✅ `configs.enabled` field
   - ✅ `configs.parameter_path` (AWS)
   - ✅ `configs.store` (GCP)
   - ✅ `configs.app_config_endpoint` (Azure)
6. ✅ Tests passing (unit, integration, Pact)
   - ✅ 51 Pact tests total (all passing)
   - ✅ Unit tests for providers
   - ✅ Integration tests for routing logic
7. ✅ Documentation updated

## Timeline Estimate ✅ COMPLETE

- **Phase 1 (AWS Parameter Store)**: ✅ Completed
- **Phase 2 (GCP Secret Manager)**: ✅ Completed
- **Phase 3 (Azure App Configuration)**: ✅ Completed

**Total**: All phases completed successfully

## Next Immediate Actions

### ✅ All Phases Complete!

All implementation tasks are complete. The following enhancements are optional future work:

### Future Enhancements (Optional)

1. **GCP Parameter Manager Support** - After ESO contribution
   - Contribute GCP Parameter Manager provider to External Secrets Operator
   - Update controller to use Parameter Manager instead of Secret Manager for configs

2. **Azure App Configuration ESO Provider** - Contribute to External Secrets Operator
   - Add Azure App Configuration provider to ESO
   - Enables ConfigMap creation from App Configuration

3. **Config Validation** - Validate config values before storing
   - Add schema validation for config values
   - Support validation rules in CRD

4. **Config Versioning** - Track config changes over time
   - Add version tracking for config changes
   - Support config rollback functionality

5. **Multi-environment Configs** - Better handling of environment-specific configs
   - Enhanced environment variable handling
   - Better support for config inheritance


