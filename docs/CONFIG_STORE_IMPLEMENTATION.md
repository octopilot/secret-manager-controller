# Config Store Implementation - Focused Plan

## Goal

**Primary Objective**: Sync `application.properties` (and other config files) from Git → Cloud Config Stores

**Focus**: Get configs into cloud stores. Consumption layer (ESO contributions) comes later.

## Implementation Priority

### Phase 1: AWS Parameter Store ✅ (Ready)

**Status**: Ready to implement  
**SDK**: `aws-sdk-ssm` (already available)  
**Effort**: 2-3 days  
**Priority**: High

### Phase 2: GCP Secret Manager ✅ (Interim Solution)

**Status**: Ready to implement  
**SDK**: `google-cloud-secretmanager-v1` (already in use)  
**Effort**: 1-2 days  
**Priority**: High  
**Note**: Using Secret Manager as interim solution (ESO already supports). Parameter Manager contribution comes later.

### Phase 3: Azure App Configuration ⚠️ (Research Needed)

**Status**: Research SDK availability  
**SDK**: Need to verify `azure-app-configuration` crate  
**Effort**: 2-3 days (after SDK research)  
**Priority**: Medium

## File Routing Logic

### Current Behavior

```rust
// All files → Secret Store
application.secrets.env → Secret Store
application.secrets.yaml → Secret Store
application.properties → Secret Store (as JSON blob) ❌
```

### Target Behavior

```rust
// File-based routing
if file_name.contains("secrets") {
    → Secret Store
} else if file_name == "application.properties" || file_name.contains("config") {
    if configs.enabled {
        → Config Store (based on provider)
    } else {
        → Secret Store (backward compatibility)
    }
}
```

### File Patterns

| Pattern | Route To | Notes |
|---------|----------|-------|
| `application.secrets.env` | Secret Store | Always |
| `application.secrets.yaml` | Secret Store | Always |
| `application.properties` | Config Store (if enabled) | Default: Secret Store (backward compat) |
| `application.config.env` | Config Store (if enabled) | New file type |
| `application.config.yaml` | Config Store (if enabled) | New file type |

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
    # ... provider-specific config
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

## Implementation Details

### Phase 1: AWS Parameter Store

#### Tasks

1. **Add SDK Dependency**
   ```toml
   # Cargo.toml
   aws-sdk-ssm = "1"  # Systems Manager (includes Parameter Store)
   ```

2. **Create Provider Module**
   - File: `src/aws_parameter_store.rs`
   - Implement `ConfigStoreProvider` trait
   - Similar to `SecretManagerProvider` trait

3. **Update Reconciler**
   - Add config store routing logic
   - Route `application.properties` → Parameter Store
   - Store individual properties as separate parameters (not JSON blob)

4. **Update CRD**
   - Add `configs` field to `SecretManagerConfig`
   - Add `parameterPath` field for AWS

5. **Add Tests**
   - Unit tests for Parameter Store provider
   - Integration tests (or mocks)

#### Parameter Storage Format

**Current (Secret Store)**:
```
my-service-properties-prod = {"database.host":"db.example.com","database.port":"5432"}
```

**Target (Parameter Store)**:
```
/my-service/prod/database.host = db.example.com
/my-service/prod/database.port = 5432
/my-service/prod/api.timeout = 30s
```

#### Implementation Steps

1. Create `ConfigStoreProvider` trait (similar to `SecretManagerProvider`)
2. Implement `AwsParameterStore` provider
3. Update `reconciler.rs` to route configs to config store
4. Update CRD schema
5. Add tests

### Phase 2: GCP Secret Manager (Interim)

#### Tasks

1. **Update Reconciler**
   - Route `application.properties` → Secret Manager
   - Store individual properties as separate secrets (not JSON blob)
   - Use existing `SecretManager` provider

2. **Update CRD**
   - Add `configs.enabled` field
   - Add `configs.store: SecretManager` (default)

3. **Storage Format**

**Current (JSON blob)**:
```
my-service-properties-prod = {"database.host":"db.example.com","database.port":"5432"}
```

**Target (Individual secrets)**:
```
my-service-database-host-prod = db.example.com
my-service-database-port-prod = 5432
my-service-api-timeout-prod = 30s
```

#### Implementation Steps

1. Update `reconciler.rs` to route configs to Secret Manager
2. Store individual properties as separate secrets
3. Update CRD schema
4. Add tests

**Note**: This uses existing Secret Manager provider - no new provider needed.

### Phase 3: Azure App Configuration

#### Tasks

1. **Research SDK**
   - Verify `azure-app-configuration` Rust crate availability
   - Check API compatibility

2. **Create Provider Module**
   - File: `src/azure_app_config.rs`
   - Implement `ConfigStoreProvider` trait
   - Similar to Azure Key Vault provider

3. **Update Reconciler**
   - Route `application.properties` → App Configuration
   - Store individual properties as key-value pairs

4. **Update CRD**
   - Add `appConfigEndpoint` field

5. **Add Tests**

#### Storage Format

**Current (Key Vault)**:
```
my-service-properties-prod = {"database.host":"db.example.com","database.port":"5432"}
```

**Target (App Configuration)**:
```
my-service:prod:database.host = db.example.com
my-service:prod:database.port = 5432
my-service:prod:api.timeout = 30s
```

## Code Structure

### New Trait: ConfigStoreProvider

```rust
#[async_trait]
pub trait ConfigStoreProvider: Send + Sync {
    /// Create or update a config value
    /// Returns true if the value was updated (changed), false if it was created new
    async fn create_or_update_config(
        &self,
        key: &str,
        value: &str,
    ) -> Result<bool>;
    
    /// Get a config value
    async fn get_config_value(&self, key: &str) -> Result<Option<String>>;
    
    /// Delete a config value
    async fn delete_config(&self, key: &str) -> Result<()>;
}
```

### Provider Implementations

1. **AwsParameterStore** - Implements `ConfigStoreProvider`
2. **GcpSecretManager** - Reuse existing provider (for configs)
3. **AzureAppConfiguration** - Implements `ConfigStoreProvider`

### Reconciler Updates

```rust
// In reconciler.rs
if config.spec.configs.enabled {
    // Route configs to config store
    let config_provider: Box<dyn ConfigStoreProvider> = match &config.spec.provider {
        ProviderConfig::Aws(_) => Box::new(AwsParameterStore::new(...)?),
        ProviderConfig::Gcp(gcp_config) => {
            match config.spec.configs.store {
                Some(ConfigStoreType::SecretManager) => {
                    // Reuse SecretManager provider
                    Box::new(GcpSecretManager::new(...)?)
                }
                Some(ConfigStoreType::ParameterManager) => {
                    // Future: Parameter Manager provider
                    todo!("Parameter Manager support coming after ESO contribution")
                }
                None => Box::new(GcpSecretManager::new(...)?), // Default
            }
        }
        ProviderConfig::Azure(_) => Box::new(AzureAppConfiguration::new(...)?),
    };
    
    // Store configs
    for (key, value) in properties {
        config_provider.create_or_update_config(&key, &value).await?;
    }
}
```

## Testing Strategy

### Unit Tests
- Config store provider implementations
- File routing logic
- Parameter/key name construction

### Integration Tests
- AWS Parameter Store sync
- GCP Secret Manager config sync
- Azure App Configuration sync

### E2E Tests (Pact Tests)
- Full reconciliation flow with config stores
- Verify configs are stored correctly
- Verify backward compatibility

## Migration Path

### Backward Compatibility

- **Default**: `configs.enabled: false` → all files go to secret stores (current behavior)
- **Opt-in**: `configs.enabled: true` → configs go to config stores

### Migration Steps

1. **No action required**: Existing CRDs continue to work
2. **Opt-in**: Add `configs.enabled: true` to enable config store routing
3. **Verify**: Check that configs are synced to correct stores
4. **Update consumption**: Update serverless/K8s configs to consume from config stores

## Success Criteria

1. ✅ `application.properties` routes to config stores (not secret stores)
2. ✅ Individual properties stored as separate entries (not JSON blob)
3. ✅ Backward compatibility maintained (`configs.enabled: false` by default)
4. ✅ All three providers supported (AWS, GCP, Azure)
5. ✅ Clear CRD configuration for routing decisions
6. ✅ Tests passing
7. ✅ Documentation updated

## Next Steps

1. ✅ Design `ConfigStoreProvider` trait
2. 🔄 Implement AWS Parameter Store provider
3. 🔄 Update reconciler for config routing
4. 🔄 Update CRD schema
5. 🔄 Add tests
6. ⏳ Research Azure App Configuration SDK
7. ⏳ Implement Azure App Configuration provider
8. ⏳ Update documentation

## Focus Areas

### Immediate Focus

1. **AWS Parameter Store** - Ready to implement, highest priority
2. **GCP Secret Manager** - Quick win, uses existing provider
3. **Azure App Configuration** - After SDK research

### Future Enhancements

1. **GCP Parameter Manager** - After ESO contribution
2. **Enhanced CRD options** - Based on user feedback
3. **Config validation** - Validate config values before storing

## Key Principle

> **Get configs into cloud stores first. Consumption layer (ESO contributions) comes later.**

This approach allows us to:
- Deliver value quickly
- Test the sync mechanism
- Validate the approach
- Then work on consumption layer improvements

