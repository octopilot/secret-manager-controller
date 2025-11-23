# Logging Guidelines

Comprehensive guide to logging patterns and best practices in the Secret Manager Controller.

## Overview

The controller uses the `tracing` crate for structured, contextual logging with support for:

- **Log Levels**: `ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE`
- **Structured Fields**: Key-value pairs for context
- **Span Context**: Automatic propagation of trace context
- **Environment Filtering**: Configurable via `RUST_LOG` environment variable
- **OpenTelemetry Integration**: Distributed tracing support

## Log Levels

### `error!` - Errors Requiring Attention

**Use for:**
- Reconciliation failures that cannot be recovered automatically
- API errors that indicate a configuration problem
- Validation failures
- Critical system errors
- Authentication failures (permanent)

**Examples:**
```rust
error!("Validation error for {}: {}", name, e);
error!("Failed to get FluxCD GitRepository: {}/{} - {}", namespace, name, e);
error!("Reconciliation failed with error: {}", e);
error!("CRD is not queryable; {:?}. Is the CRD installed?", e);
```

**When to use:**
- The error requires human intervention
- The error indicates a misconfiguration
- The error prevents the controller from functioning correctly
- The error is permanent (not transient)

**Structured logging:**
```rust
error!(
    resource.name = name,
    resource.namespace = namespace,
    error = %e,
    "reconciliation.error"
);
```

---

### `warn!` - Recoverable Issues

**Use for:**
- Expected conditions that may resolve automatically
- Retryable errors (404 Not Found, 429 Too Many Requests)
- Degraded functionality
- Watch stream errors that trigger automatic recovery
- Transient authentication issues

**Examples:**
```rust
warn!("GitRepository {}/{} not found yet, will retry in 30s", namespace, name);
warn!("Watch resource version expired (410) - this is normal during pod restarts");
warn!("API server storage reinitializing (429), backing off for {}ms", backoff);
warn!("Transient SOPS error: {}", error);
```

**When to use:**
- The condition is expected and will be retried
- The error is transient and will likely resolve
- The controller can continue operating despite the issue
- The condition is part of normal operation (e.g., resource not found yet)

**Structured logging:**
```rust
warn!(
    resource.name = name,
    resource.namespace = namespace,
    retry_after_secs = 30,
    "resource.not_found"
);
```

---

### `info!` - Important State Changes

**Use for:**
- Reconciliation start/completion
- Resource status updates
- Important configuration details
- Successful operations
- Controller initialization
- Startup/shutdown events

**Examples:**
```rust
info!("Reconciling SecretManagerConfig: {}", name);
info!("Creating new GCP secret: {}", secret_name);
info!("Secret value changed, creating new version for: {}", secret_name);
info!("✅ Reconciliation complete for {}: {} secrets synced", name, secret_count);
info!("Starting Secret Manager Controller v2");
info!("Controller initialized, starting watch loop...");
```

**When to use:**
- The event represents a significant state change
- The information is useful for understanding controller behavior
- The event should be visible in normal operation logs
- The event marks a milestone in operation

**Structured logging:**
```rust
info!(
    resource.name = name,
    resource.namespace = namespace,
    secrets.synced = count,
    "reconciliation.complete"
);
```

---

### `debug!` - Detailed Debugging Information

**Use for:**
- Detailed step-by-step execution flow
- File parsing details
- SOPS decryption process
- Kustomize build output parsing
- Internal state information
- Provider API interactions
- HTTP request/response details

**Examples:**
```rust
debug!("Parsing secrets from: {}", path.display());
debug!("Detected SOPS-encrypted file: {}", path.display());
debug!("Attempting SOPS decryption with provided GPG private key");
debug!("Using temporary GPG home: {:?}", gpg_home_path);
debug!("Kustomize build succeeded, parsing output...");
```

**When to use:**
- The information is only needed for debugging
- The information is verbose and would clutter normal logs
- The information helps diagnose issues but isn't critical for operations
- The information is about internal implementation details

**Structured logging:**
```rust
debug!(
    file.path = %path.display(),
    file.size = size,
    "file.parsing"
);
```

---

### `trace!` - Very Detailed Diagnostic Information

**Use for:**
- Function entry/exit
- Internal calculations
- Low-level operations
- Loop iterations
- Detailed state transitions

**Examples:**
```rust
trace!("Entering reconcile function");
trace!("Processing secret: {}", secret_name);
trace!("Loop iteration: {}", i);
```

**When to use:**
- Very detailed diagnostic information
- Function-level tracing
- Performance analysis
- Deep debugging scenarios

**Note:** `trace!` is rarely used in the controller codebase. Most detailed logging uses `debug!`.

---

## Structured Logging

### Tracing Spans

Spans provide context for a group of operations. All logs within a span automatically include span context.

**Creating spans:**
```rust
let span = tracing::span!(
    tracing::Level::INFO,
    "reconcile",
    resource.name = name,
    resource.namespace = namespace,
    resource.kind = "SecretManagerConfig",
    resource.provider = provider_type
);
let _guard = span.enter();
```

**Span levels:**
- `tracing::span!` - Creates a span at the specified level
- `tracing::info_span!` - Creates an INFO-level span
- `tracing::debug_span!` - Creates a DEBUG-level span
- `tracing::warn_span!` - Creates a WARN-level span
- `tracing::error_span!` - Creates an ERROR-level span

**Span attributes:**
- Use structured fields for context
- Fields are automatically included in all logs within the span
- Fields can be strings, numbers, or other types

**Example:**
```rust
let span = tracing::span!(
    tracing::Level::INFO,
    "reconcile",
    resource.name = name,
    resource.namespace = namespace,
    resource.kind = "SecretManagerConfig",
    resource.provider = provider_type
);
let _guard = span.enter();

// All logs within this span automatically include:
// - resource.name
// - resource.namespace
// - resource.kind
// - resource.provider

info!("Starting reconciliation");
// Log output includes all span fields
```

### Instrumentation

Use `#[instrument]` attribute to automatically create spans for functions:

```rust
use tracing::instrument;

#[instrument]
async fn reconcile(config: Arc<SecretManagerConfig>, ctx: Arc<Reconciler>) -> Result<Action> {
    // Function automatically creates a span with function name and parameters
    // All logs within this function are within the span
}
```

**Instrumentation with fields:**
```rust
#[instrument(
    skip(ctx),
    fields(
        resource.name = %config.metadata.name.as_deref().unwrap_or("unknown"),
        resource.namespace = %config.metadata.namespace.as_deref().unwrap_or("default"),
    )
)]
async fn reconcile(config: Arc<SecretManagerConfig>, ctx: Arc<Reconciler>) -> Result<Action> {
    // Span includes specified fields
}
```

### Structured Fields

Always include structured fields in log messages:

```rust
// Good: Structured fields
info!(
    resource.name = name,
    resource.namespace = namespace,
    secrets.synced = count,
    "reconciliation.complete"
);

// Bad: String interpolation only
info!("Reconciliation complete for {}: {} secrets synced", name, count);
```

**Field naming conventions:**
- Use dot notation for hierarchical fields: `resource.name`, `resource.namespace`
- Use snake_case for field names: `secret_name`, `provider_type`
- Group related fields: `resource.*`, `secrets.*`, `provider.*`

**Common field patterns:**
- `resource.name` - Resource name
- `resource.namespace` - Resource namespace
- `resource.kind` - Resource kind
- `resource.provider` - Provider type (gcp, aws, azure)
- `operation.type` - Operation type (create, update, delete)
- `error.message` - Error message
- `error.code` - Error code

---

## Logging Configuration

### Environment Variables

**RUST_LOG:**
```bash
# Set default log level
RUST_LOG=info

# Set log level for specific modules
RUST_LOG=secret_manager_controller=debug,provider::gcp=trace

# Multiple modules with different levels
RUST_LOG=secret_manager_controller=info,provider=debug,kube=warn

# Enable all logs
RUST_LOG=trace
```

**Log level hierarchy:**
- `trace` - All logs
- `debug` - DEBUG, INFO, WARN, ERROR
- `info` - INFO, WARN, ERROR (default)
- `warn` - WARN, ERROR
- `error` - ERROR only

### Per-Resource Logging Configuration

The controller supports fine-grained logging configuration per resource via the CRD:

```yaml
apiVersion: secret-manager.microscaler.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-config
spec:
  logging:
    secrets: INFO          # Log level for secret operations
    properties: INFO       # Log level for property operations
    reconciliation: INFO   # Log level for reconciliation
    diff_discovery: WARN   # Log level for diff discovery
    sops: DEBUG            # Log level for SOPS operations
    git: INFO              # Log level for Git operations
    provider: DEBUG        # Log level for provider operations
    kustomize: INFO        # Log level for Kustomize operations
```

**Log level hierarchy:**
- `DEBUG` includes INFO, WARN, ERROR
- `INFO` includes WARN, ERROR
- `WARN` includes ERROR
- `ERROR` only

### Controller-Wide Logging

Configure controller-wide logging via ConfigMap:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: secret-manager-controller-config
  namespace: microscaler-system
data:
  LOG_LEVEL: "INFO"        # ERROR, WARN, INFO, DEBUG, TRACE
  LOG_FORMAT: "json"       # json, text
  LOG_ENABLE_COLOR: "false" # Only applies to text format
```

---

## Logging Patterns

### Resource Identification

Always include resource name and namespace in log messages:

```rust
// Good:
info!("Reconciling SecretManagerConfig: {} in namespace {}", name, namespace);

// Better: Structured fields
info!(
    resource.name = name,
    resource.namespace = namespace,
    "reconciliation.start"
);

// Bad:
info!("Reconciling resource");
```

### Error Context

Include context about what operation failed:

```rust
// Good:
error!("Failed to create GCP secret {}: {}", secret_name, e);

// Better: Structured fields
error!(
    provider = "gcp",
    operation = "create_secret",
    secret.name = secret_name,
    error = %e,
    "provider.operation.failed"
);

// Bad:
error!("Operation failed: {}", e);
```

### Status Updates

Log status phase changes:

```rust
info!(
    resource.name = name,
    status.phase = new_phase,
    status.previous_phase = old_phase,
    "status.updated"
);
```

### Operation Milestones

Log important milestones in operations:

```rust
info!("🔄 Reconciling SecretManagerConfig: {} (trigger source: {})", name, trigger_source);
info!("✅ Reconciliation complete for {}: {} secrets synced", name, secret_count);
info!("📋 Status updated: {} -> {} ({})", old_phase, new_phase, description);
```

---

## OpenTelemetry Integration

### Distributed Tracing

The controller supports OpenTelemetry for distributed tracing:

**Datadog Integration:**
```rust
// Spans are automatically exported to Datadog when configured
let span = tracing::span!(
    tracing::Level::INFO,
    "reconcile",
    resource.name = name,
    resource.namespace = namespace,
);
let _guard = span.enter();

// All logs within this span are part of the trace
info!("Starting reconciliation");
```

**Trace Context Propagation:**
- Trace context is automatically propagated via spans
- All logs within a span are part of the same trace
- Spans can be nested for hierarchical tracing

**Span Attributes:**
- Span attributes are exported to OpenTelemetry
- Attributes are searchable in tracing backends
- Use structured fields for better observability

### Instrumentation with OpenTelemetry

When OpenTelemetry is configured, spans are automatically exported:

```rust
// Create a span
let span = tracing::span!(
    tracing::Level::INFO,
    "reconcile",
    resource.name = name,
    resource.namespace = namespace,
    resource.provider = provider_type
);
let _guard = span.enter();

// All operations within this span are part of the trace
// Spans are automatically exported to Datadog/OTel when configured
```

---

## Best Practices

### ✅ DO

1. **Use structured logging**
   ```rust
   info!(
       resource.name = name,
       resource.namespace = namespace,
       "operation.complete"
   );
   ```

2. **Include context in error messages**
   ```rust
   error!(
       operation = "create_secret",
       secret.name = secret_name,
       error = %e,
       "operation.failed"
   );
   ```

3. **Use appropriate log levels**
   - `error!` for permanent failures
   - `warn!` for transient/retryable errors
   - `info!` for important state changes
   - `debug!` for detailed debugging

4. **Create spans for operations**
   ```rust
   let span = tracing::span!(
       tracing::Level::INFO,
       "reconcile",
       resource.name = name,
   );
   let _guard = span.enter();
   ```

5. **Use consistent field naming**
   - `resource.name`, `resource.namespace`
   - `operation.type`, `operation.status`
   - `error.message`, `error.code`

6. **Log milestones and state changes**
   ```rust
   info!("✅ Reconciliation complete for {}: {} secrets synced", name, count);
   ```

### ❌ DON'T

1. **Don't log sensitive data**
   ```rust
   // Bad: Logs secret value
   debug!("Secret value: {}", secret_value);
   
   // Good: Log secret name only
   debug!("Processing secret: {}", secret_name);
   ```

2. **Don't use wrong log levels**
   ```rust
   // Bad: Using error! for expected condition
   error!("Resource not found");
   
   // Good: Using warn! for retryable condition
   warn!("Resource not found, will retry");
   ```

3. **Don't lose context**
   ```rust
   // Bad: No context
   error!("Operation failed");
   
   // Good: Includes context
   error!(
       operation = "create_secret",
       secret.name = secret_name,
       error = %e,
       "operation.failed"
   );
   ```

4. **Don't use string interpolation for structured data**
   ```rust
   // Bad: String interpolation
   info!("Reconciliation complete for {}: {} secrets", name, count);
   
   // Good: Structured fields
   info!(
       resource.name = name,
       secrets.synced = count,
       "reconciliation.complete"
   );
   ```

5. **Don't log at inappropriate levels**
   ```rust
   // Bad: Debug info at info level
   info!("Loop iteration: {}", i);
   
   // Good: Debug info at debug level
   debug!("Loop iteration: {}", i);
   ```

---

## Examples

### Example 1: Reconciliation Logging

```rust
// Create span for reconciliation
let span = tracing::span!(
    tracing::Level::INFO,
    "reconcile",
    resource.name = name,
    resource.namespace = namespace,
    resource.kind = "SecretManagerConfig",
    resource.provider = provider_type
);
let _guard = span.enter();

// Log start
info!(
    resource.name = name,
    trigger.source = trigger_source.as_str(),
    "reconciliation.start"
);

// Log validation errors
if let Err(e) = validate_config(&config) {
    error!(
        resource.name = name,
        error = %e,
        "validation.failed"
    );
    return Err(ReconcilerError::ReconciliationFailed(e));
}

// Log completion
info!(
    resource.name = name,
    secrets.synced = count,
    duration.seconds = duration.as_secs(),
    "reconciliation.complete"
);
```

### Example 2: Provider Operation Logging

```rust
// Create debug span for provider operation
let span = tracing::debug_span!(
    "provider.create_secret",
    provider = "gcp",
    secret.name = secret_name,
    project.id = project_id
);
let _guard = span.enter();

debug!("Creating secret in GCP Secret Manager");

match provider.create_secret(secret_name, secret_value).await {
    Ok(_) => {
        info!(
            provider = "gcp",
            secret.name = secret_name,
            "secret.created"
        );
    }
    Err(e) => {
        error!(
            provider = "gcp",
            secret.name = secret_name,
            error = %e,
            "secret.creation.failed"
        );
    }
}
```

### Example 3: Error Handling Logging

```rust
// Transient error
if is_transient_error(&e) {
    warn!(
        resource.name = name,
        error = %e,
        retry.after_secs = 30,
        "transient.error"
    );
    return Ok(Action::requeue(Duration::from_secs(30)));
}

// Permanent error
error!(
    resource.name = name,
    error = %e,
    "permanent.error"
);
return Err(ReconcilerError::ReconciliationFailed(e));
```

### Example 4: Structured Logging with Spans

```rust
// Create parent span
let reconcile_span = tracing::span!(
    tracing::Level::INFO,
    "reconcile",
    resource.name = name,
    resource.namespace = namespace,
);
let _reconcile_guard = reconcile_span.enter();

info!("Starting reconciliation");

// Create child span for file processing
let file_span = tracing::debug_span!(
    "process_files",
    file.count = files.len(),
);
let _file_guard = file_span.enter();

debug!("Processing {} files", files.len());

// All logs within file_span include both span contexts
for file in files {
    debug!("Processing file: {}", file.display());
}
```

---

## Log Format

### Text Format

```
2024-01-15T10:30:45.123456Z  INFO secret_manager_controller::controller::reconciler Starting reconciliation
  resource.name=my-config
  resource.namespace=default
  trace_id=abc123
  span_id=def456
```

### JSON Format

```json
{
  "timestamp": "2024-01-15T10:30:45.123456Z",
  "level": "INFO",
  "target": "secret_manager_controller::controller::reconciler",
  "message": "Starting reconciliation",
  "fields": {
    "resource.name": "my-config",
    "resource.namespace": "default"
  },
  "span": {
    "trace_id": "abc123",
    "span_id": "def456"
  }
}
```

---

## Testing Logging

### Unit Tests

```rust
#[tokio::test]
async fn test_logging() {
    // Use tracing test subscriber
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .try_init();

    // Your test code
    info!("Test log message");
    
    // Logs are captured by test subscriber
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_reconciliation_logging() {
    // Initialize tracing for tests
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init();

    // Run reconciliation
    let result = reconcile(config, reconciler).await;
    
    // Check logs are emitted correctly
    assert!(result.is_ok());
}
```

---

## Summary

- **Log Levels**: Use appropriate levels (error, warn, info, debug, trace)
- **Structured Logging**: Use structured fields instead of string interpolation
- **Spans**: Create spans for operations to provide context
- **Context**: Always include resource identification and operation context
- **Sensitive Data**: Never log secret values or credentials
- **OpenTelemetry**: Spans are automatically exported when configured
- **Configuration**: Use `RUST_LOG` for environment filtering
- **Best Practices**: Follow consistent patterns across the codebase

This structured approach provides comprehensive observability while maintaining clarity and avoiding sensitive data exposure.

