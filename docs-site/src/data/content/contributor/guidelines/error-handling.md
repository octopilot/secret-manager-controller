# Error Handling Guidelines

Comprehensive guide to error handling patterns and best practices in the Secret Manager Controller.

## Overview

The codebase uses a hybrid approach combining `anyhow` for general error handling and `thiserror` for domain-specific errors. This provides flexibility while maintaining type safety and clear error boundaries.

## Error Handling Strategy

### 1. Domain Errors: Use `thiserror::Error`

**When to use:**
- Errors specific to the controller's domain
- Errors that need to be matched/classified by callers
- Errors that are part of the public API
- Errors that need structured information (e.g., transient vs permanent)

**Example:**
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReconcilerError {
    #[error("Reconciliation failed: {0}")]
    ReconciliationFailed(#[from] anyhow::Error),
}
```

**Benefits:**
- Type-safe error matching
- Clear error boundaries
- Easy to extend with new error variants
- Can be converted to/from `anyhow::Error` via `#[from]`
- Supports structured error information

**Location:** `crates/controller/src/controller/reconciler/types.rs`

### 2. General Errors: Use `anyhow::Result`

**When to use:**
- Internal function errors
- Errors from external libraries
- Errors that don't need specific handling
- Errors that will be wrapped in domain errors

**Example:**
```rust
use anyhow::{Context, Result};

pub async fn get_secret_value(&self, secret_name: &str) -> Result<Option<String>> {
    self.client
        .access_secret_version()
        .with_request(request)
        .send()
        .await
        .context(format!("Failed to get secret: {secret_name}"))?;
    // ...
}
```

**Benefits:**
- Concise error handling with `?` operator
- Rich error context with `.context()`
- Automatic error chaining
- Easy to convert to domain errors

**Locations:** Most provider modules, parser, kustomize

### 3. Error Propagation Pattern

**Pattern:** Convert `anyhow::Error` to domain errors at boundaries

```rust
// Internal function uses anyhow::Result
async fn reconcile_internal(...) -> anyhow::Result<Action> {
    // Use anyhow::Result internally
    let value = some_operation()
        .context("Failed to do something")?;
    
    Ok(Action::requeue(...))
}

// Public API uses domain error
pub async fn reconcile(...) -> Result<Action, ReconcilerError> {
    reconcile_internal(...)
        .await
        .map_err(ReconcilerError::ReconciliationFailed)
}
```

## Error Types

### ReconcilerError

**Location:** `crates/controller/src/controller/reconciler/types.rs`

**Definition:**
```rust
#[derive(Debug, Error)]
pub enum ReconcilerError {
    #[error("Reconciliation failed: {0}")]
    ReconciliationFailed(#[from] anyhow::Error),
}
```

**Usage:**
- Returned from public reconciliation functions
- Wraps anyhow errors from internal operations
- Automatically converts via `#[from]` attribute

### SopsDecryptionError

**Location:** `crates/controller/src/controller/parser/sops/error.rs`

**Definition:**
```rust
#[derive(Debug, Error)]
#[error("SOPS decryption failed: {reason:?} - {message}")]
pub struct SopsDecryptionError {
    pub reason: SopsDecryptionFailureReason,
    pub message: String,
    pub is_transient: bool,
}
```

**Features:**
- Classifies errors as transient or permanent
- Provides remediation guidance
- Supports structured error handling

**Failure Reasons:**
- **Permanent**: `KeyNotFound`, `WrongKey`, `InvalidKeyFormat`, `UnsupportedFormat`, `CorruptedFile`
- **Transient**: `NetworkTimeout`, `ProviderUnavailable`, `PermissionDenied`, `Unknown`

## Error Classification

### Transient Errors (Retryable)

**Pattern:** Log as `warn!`, return `Action::requeue()`

```rust
if is_404 {
    warn!("Resource not found, will retry in 30s");
    return Ok(Action::requeue(Duration::from_secs(30)));
}
```

**Examples:**
- **404 Not Found**: Resource doesn't exist yet (GitRepository, Application)
- **429 Too Many Requests**: Rate limiting from provider or API server
- **410 Gone**: Resource version expired (watch restart)
- **Network Timeouts**: Temporary connectivity issues
- **Provider Unavailable**: Temporary provider outages

**Handling:**
- Use Fibonacci backoff for retries
- Track error count per resource
- Log as warning (not error)
- Return `Action::requeue()` with backoff duration

### Permanent Errors (Non-Retryable)

**Pattern:** Log as `error!`, return domain error

```rust
error!("Validation failed: {}", e);
return Err(ReconcilerError::ReconciliationFailed(e));
```

**Examples:**
- **Validation Failures**: Invalid configuration, missing required fields
- **Configuration Errors**: Invalid provider settings, missing credentials
- **Authentication Failures**: Invalid credentials (not transient RBAC issues)
- **Invalid Resource State**: Resource in invalid state that won't self-correct

**Handling:**
- Log as error
- Return domain error immediately
- Don't retry (will be retried by watch loop if resource changes)

## Error Context

### Adding Context

Always add context to errors to make debugging easier:

```rust
// Good: Adds context
let secret = provider
    .get_secret("my-secret")
    .await
    .context("Failed to get secret from GCP")?;

// Better: Adds specific context
let secret = provider
    .get_secret("my-secret")
    .await
    .context(format!("Failed to get secret '{}' from GCP project '{}'", secret_name, project))?;
```

### Context Chain

Errors automatically chain context:

```rust
// Error chain: "Failed to create secret" -> "Failed to authenticate" -> "Invalid credentials"
let secret = provider
    .authenticate()
    .await
    .context("Failed to authenticate")?
    .create_secret(name, value)
    .await
    .context("Failed to create secret")?;
```

## Error Handling Guidelines

### ✅ DO

1. **Use `anyhow::Result` for internal functions**
   ```rust
   fn parse_secrets(path: &Path) -> anyhow::Result<HashMap<String, String>> {
       // Implementation
   }
   ```

2. **Use `thiserror::Error` for public APIs**
   ```rust
   #[derive(Debug, Error)]
   pub enum ProviderError {
       #[error("Provider initialization failed: {0}")]
       InitializationFailed(#[from] anyhow::Error),
   }
   ```

3. **Add context to errors**
   ```rust
   .context(format!("Failed to create secret: {secret_name}"))
   ```

4. **Convert at boundaries**
   ```rust
   Err(ReconcilerError::ReconciliationFailed(e.into()))
   ```

5. **Use `#[from]` for automatic conversion**
   ```rust
   #[error("Reconciliation failed: {0}")]
   ReconciliationFailed(#[from] anyhow::Error),
   ```

6. **Classify errors as transient or permanent**
   ```rust
   if error.is_transient() {
       warn!("Transient error, will retry: {}", error);
       return Ok(Action::requeue(Duration::from_secs(30)));
   } else {
       error!("Permanent error: {}", error);
       return Err(ReconcilerError::ReconciliationFailed(error.into()));
   }
   ```

7. **Use structured error types for complex errors**
   ```rust
   #[derive(Debug, Error)]
   pub struct SopsDecryptionError {
       pub reason: SopsDecryptionFailureReason,
       pub message: String,
       pub is_transient: bool,
   }
   ```

### ❌ DON'T

1. **Don't mix error types unnecessarily**
   ```rust
   // Bad: Mixing anyhow and thiserror without conversion
   fn bad_function() -> Result<(), ReconcilerError> {
       let result: anyhow::Result<()> = some_op()?; // Error: can't use ? on anyhow::Result
       Ok(())
   }
   ```

2. **Don't use `unwrap()` or `expect()` for recoverable errors**
   ```rust
   // Bad: Panics on error
   let value = operation().unwrap();
   
   // Good: Handles error gracefully
   let value = operation()
       .context("Operation failed")?;
   ```

3. **Don't lose error context**
   ```rust
   // Bad: Loses context
   match operation() {
       Ok(v) => v,
       Err(_) => return Err(ReconcilerError::ReconciliationFailed(
           anyhow::anyhow!("Operation failed")
       )),
   }
   
   // Good: Preserves context
   operation()
       .context("Operation failed")
       .map_err(ReconcilerError::ReconciliationFailed)?;
   ```

4. **Don't ignore errors silently**
   ```rust
   // Bad: Ignores error
   let _ = operation();
   
   // Good: Handles or logs error
   if let Err(e) = operation() {
       warn!("Operation failed (non-critical): {}", e);
   }
   ```

5. **Don't use string matching for error classification**
   ```rust
   // Bad: Fragile string matching
   if error.to_string().contains("404") {
       // Handle 404
   }
   
   // Good: Use structured error types
   match error {
       ReconcilerError::ResourceNotFound => {
           // Handle 404
       }
   }
   ```

## Error Policy

### Reconciliation Errors

**Location:** `crates/controller/src/runtime/error_policy.rs`

**Handling:**
- Uses Fibonacci backoff (1 minute min, 10 minutes max)
- Tracks error count per resource
- Prevents blocking watch/timer paths
- Logs error with structured span

**Example:**
```rust
pub fn handle_reconciliation_error(
    obj: Arc<SecretManagerConfig>,
    error: &ReconcilerError,
    ctx: Arc<Reconciler>,
) -> Action {
    // Log error
    error!("Reconciliation error for {}: {:?}", name, error);
    
    // Calculate backoff
    let backoff_seconds = calculate_fibonacci_backoff(resource_key, &ctx);
    
    // Return requeue action
    Action::requeue(Duration::from_secs(backoff_seconds))
}
```

### Watch Stream Errors

**Handling:**
- Classifies errors (401, 404, 410, 429)
- Applies appropriate backoff
- Filters errors to allow restart
- Provides diagnostic guidance

**Error Classifications:**
- **401 Unauthorized**: RBAC revoked, token expired → Wait and restart
- **404 Not Found**: Resource deleted, CRD missing → Continue (expected)
- **410 Gone**: Resource version expired → Restart watch
- **429 Too Many Requests**: API server reinitializing → Exponential backoff

**Example:**
```rust
pub async fn handle_watch_stream_error(
    error_string: &str,
    backoff: &Arc<AtomicU64>,
    max_backoff_ms: u64,
    watch_restart_delay_secs: u64,
) -> Option<()> {
    if is_401 {
        error!("Watch authentication failed (401)");
        // Provide diagnostic guidance
        // Wait and restart
        None // Filter out to allow restart
    } else if is_410 {
        warn!("Watch resource version expired (410)");
        None // Filter out to allow restart
    } else if is_429 {
        // Exponential backoff
        let current_backoff = backoff.load(Ordering::Relaxed);
        let new_backoff = std::cmp::min(current_backoff * 2, max_backoff_ms);
        backoff.store(new_backoff, Ordering::Relaxed);
        None // Filter out to allow restart
    } else if is_not_found {
        warn!("Resource not found (404) - may be normal");
        Some(()) // Continue
    } else {
        error!("Unknown watch error: {}", error_string);
        None // Filter out to allow restart
    }
}
```

## Backoff Strategy

### Fibonacci Backoff

**Location:** `crates/controller/src/controller/backoff.rs`

**Configuration:**
- Minimum: 1 minute
- Maximum: 10 minutes
- Per-resource tracking

**Benefits:**
- Prevents thundering herd
- Gradual retry increase
- Per-resource isolation
- Prevents blocking watch paths

**Usage:**
```rust
let mut backoff_state = BackoffState::new();
backoff_state.increment_error();
let backoff_seconds = backoff_state.backoff.next_backoff_seconds();
Action::requeue(Duration::from_secs(backoff_seconds))
```

## Error Logging

### Log Levels

**Error**: Permanent failures, authentication errors
```rust
error!("Reconciliation failed: {}", error);
```

**Warn**: Transient errors, retryable failures
```rust
warn!("Resource not found, will retry: {}", error);
```

**Debug**: Detailed error information
```rust
debug!("Error details: {:?}", error);
```

### Structured Logging

Use tracing spans for structured error logging:

```rust
let error_span = tracing::span!(
    tracing::Level::ERROR,
    "controller.watch.reconciliation_error",
    resource.name = name,
    resource.namespace = namespace,
    error = %error
);
let _error_guard = error_span.enter();
error!("Reconciliation error: {}", error);
```

## Error Metrics

### Recording Errors

```rust
use crate::observability;

// Increment error counter
observability::metrics::increment_reconciliation_errors();

// Record operation error
observability::metrics::record_provider_operation_error("gcp", "create_secret");
```

### Error Metrics

- `secret_manager_reconciliations_errors_total`: Total reconciliation errors
- `secret_manager_provider_operations_errors_total`: Provider operation errors by provider
- `secret_manager_requeues_total`: Requeue count by trigger source

## Examples

### Example 1: Provider Error Handling

```rust
// Provider trait uses anyhow::Result
#[async_trait]
pub trait SecretManagerProvider {
    async fn create_or_update_secret(
        &self,
        secret_name: &str,
        secret_value: &str,
    ) -> anyhow::Result<bool>;
}

// Implementation uses anyhow::Result internally
impl SecretManagerProvider for SecretManager {
    async fn create_or_update_secret(
        &self,
        secret_name: &str,
        secret_value: &str,
    ) -> anyhow::Result<bool> {
        self.client
            .create_secret()
            .send()
            .await
            .context(format!("Failed to create secret: {secret_name}"))?;
        Ok(true)
    }
}

// Reconciler converts to domain error
async fn sync_secret(...) -> Result<Action, ReconcilerError> {
    provider
        .create_or_update_secret(name, value)
        .await
        .map_err(ReconcilerError::ReconciliationFailed)?;
    Ok(Action::requeue(...))
}
```

### Example 2: SOPS Error Handling

```rust
// SOPS error with classification
let sops_error = SopsDecryptionError::new(
    SopsDecryptionFailureReason::KeyNotFound,
    "SOPS key not found in namespace".to_string(),
);

// Check if transient
if sops_error.is_transient {
    warn!("Transient SOPS error: {}", sops_error);
    return Ok(Action::requeue(Duration::from_secs(30)));
} else {
    error!("Permanent SOPS error: {}", sops_error);
    error!("Remediation: {}", sops_error.remediation());
    return Err(ReconcilerError::ReconciliationFailed(sops_error.into()));
}
```

### Example 3: Validation Error Handling

```rust
// Validation uses anyhow::Result
fn validate_config(config: &Config) -> anyhow::Result<()> {
    if config.name.is_empty() {
        return Err(anyhow::anyhow!("Name cannot be empty"));
    }
    Ok(())
}

// Reconciler converts to domain error
async fn reconcile(...) -> Result<Action, ReconcilerError> {
    validate_config(&config)
        .context("Configuration validation failed")
        .map_err(ReconcilerError::ReconciliationFailed)?;
    // ...
}
```

### Example 4: Transient Error Handling

```rust
// Check for transient errors
if let Err(e) = operation().await {
    // Check if error is transient
    if is_transient_error(&e) {
        warn!("Transient error, will retry: {}", e);
        return Ok(Action::requeue(Duration::from_secs(30)));
    } else {
        error!("Permanent error: {}", e);
        return Err(ReconcilerError::ReconciliationFailed(e));
    }
}
```

## Error Handling by Module

| Module | Error Type | Pattern |
|--------|-----------|---------|
| `reconciler.rs` | `ReconcilerError` (thiserror) | Domain error wrapping anyhow |
| `provider/*` | `anyhow::Result` | General error handling |
| `parser.rs` | `anyhow::Result`, `SopsDecryptionError` | General + structured SOPS errors |
| `kustomize.rs` | `anyhow::Result` | General error handling |
| `metrics.rs` | `anyhow::Result` | General error handling |
| `server.rs` | `anyhow::Error` | Direct error return |
| `runtime/error_policy.rs` | Error classification | Transient vs permanent |

## Best Practices

### 1. Error Context

Always add meaningful context:

```rust
// Good
.context(format!("Failed to create secret '{}' in project '{}'", name, project))

// Better: Include operation details
.context(format!(
    "Failed to create secret '{}' in GCP project '{}' (region: {})",
    name, project, region
))
```

### 2. Error Classification

Classify errors early:

```rust
// Classify at error creation
let error = SopsDecryptionError::new(
    classify_sops_error(&error_msg, exit_code),
    error_msg,
);

// Use classification for handling
if error.is_transient {
    // Retry
} else {
    // Fail immediately
}
```

### 3. Error Propagation

Convert at module boundaries:

```rust
// Internal: anyhow::Result
async fn internal_operation() -> anyhow::Result<()> {
    // ...
}

// Public: Domain error
pub async fn public_operation() -> Result<(), ReconcilerError> {
    internal_operation()
        .await
        .map_err(ReconcilerError::ReconciliationFailed)
}
```

### 4. Structured Errors

Use structured errors for complex scenarios:

```rust
#[derive(Debug, Error)]
pub struct ProviderError {
    pub provider: String,
    pub operation: String,
    pub reason: ProviderErrorReason,
    pub is_transient: bool,
}
```

### 5. Error Recovery

Provide recovery guidance:

```rust
impl SopsDecryptionError {
    pub fn remediation(&self) -> String {
        match self.reason {
            SopsDecryptionFailureReason::KeyNotFound => {
                "Create SOPS private key secret in the resource namespace".to_string()
            }
            // ...
        }
    }
}
```

## Testing Error Handling

### Unit Tests

```rust
#[tokio::test]
async fn test_error_handling() {
    // Test error propagation
    let result = operation_that_fails().await;
    assert!(result.is_err());
    
    // Test error context
    let error = result.unwrap_err();
    assert!(error.to_string().contains("Expected context"));
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_transient_error_retry() {
    // Simulate transient error
    mock_server.set_error_response(429, "Too Many Requests");
    
    // Trigger reconciliation
    let result = reconcile(config, reconciler).await;
    
    // Verify retry action
    assert!(matches!(result, Ok(Action::Requeue(_))));
}
```

## Summary

- **Domain errors**: Use `thiserror::Error` for public APIs
- **General errors**: Use `anyhow::Result` for internal functions
- **Conversion**: Convert at module boundaries using `#[from]` or `.map_err()`
- **Context**: Always add context with `.context()` or error messages
- **Classification**: Distinguish between transient (retryable) and permanent errors
- **Backoff**: Use Fibonacci backoff for transient errors
- **Logging**: Use appropriate log levels (error for permanent, warn for transient)
- **Metrics**: Record errors for observability
- **Structured**: Use structured error types for complex scenarios

This hybrid approach provides flexibility while maintaining type safety and clear error boundaries.

