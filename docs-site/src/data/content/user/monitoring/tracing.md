# Structured Logging and Tracing

The Secret Manager Controller provides comprehensive structured logging and distributed tracing for observability and debugging.

## Structured Logging

The controller uses the `tracing` crate for structured, contextual logging with support for:

- **Log Levels**: `ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE`
- **Structured Fields**: Key-value pairs for context
- **Span Context**: Automatic propagation of trace context
- **Environment Filtering**: Configurable via `RUST_LOG` environment variable

### Log Format

Logs are emitted in a structured format with contextual information:

```
2024-01-15T10:30:45.123456Z  INFO secret_manager_controller::controller::reconciler Starting reconciliation
  secret_manager_config.name=my-config
  secret_manager_config.namespace=default
  trace_id=abc123
  span_id=def456
```

### Log Levels

**ERROR**: Critical errors that require immediate attention
- Reconciliation failures
- Provider API errors
- Configuration errors

**WARN**: Warning conditions that may indicate issues
- Retry attempts
- Configuration drift detected
- Non-critical failures

**INFO**: Informational messages about normal operations
- Reconciliation start/end
- Secret sync operations
- Configuration changes

**DEBUG**: Detailed diagnostic information
- HTTP request/response details
- Provider API calls
- Internal state transitions

**TRACE**: Very detailed diagnostic information
- Function entry/exit
- Internal calculations
- Low-level operations

### Environment Configuration

Configure log levels via the `RUST_LOG` environment variable:

```bash
# Set default log level
RUST_LOG=info

# Set log level for specific modules
RUST_LOG=secret_manager_controller=debug,provider::gcp=trace

# Multiple modules with different levels
RUST_LOG=secret_manager_controller=info,provider=debug,kube=warn
```

### Log Fields

Common log fields include:

- **`secret_manager_config.name`**: Name of the SecretManagerConfig resource
- **`secret_manager_config.namespace`**: Namespace of the resource
- **`provider`**: Cloud provider (gcp, aws, azure)
- **`operation`**: Operation being performed (sync, update, delete)
- **`secret_name`**: Name of the secret being processed
- **`duration_ms`**: Operation duration in milliseconds
- **`error`**: Error message (when present)

## Distributed Tracing

The controller supports distributed tracing via OpenTelemetry, enabling:

- **Trace Context Propagation**: Automatic propagation across service boundaries
- **Span Creation**: Automatic span creation for operations
- **Span Attributes**: Rich metadata attached to spans
- **Trace Sampling**: Configurable sampling rates

### Trace Structure

Traces are organized hierarchically:

```
Reconciliation (root span)
├── Git Clone (child span)
│   └── Artifact Download (nested span)
├── SOPS Decryption (child span)
├── Kustomize Build (child span)
└── Provider Sync (child span)
    ├── List Secrets (nested span)
    ├── Create Secret (nested span)
    └── Update Secret (nested span)
```

### Span Attributes

Spans include rich metadata:

- **`secret_manager_config.name`**: Resource name
- **`secret_manager_config.namespace`**: Resource namespace
- **`provider`**: Cloud provider
- **`operation`**: Operation type
- **`secret_count`**: Number of secrets processed
- **`duration_ms`**: Operation duration
- **`error`**: Error information (if any)

### Trace Context Propagation

The controller automatically propagates trace context:

- **HTTP Headers**: `traceparent`, `tracestate` (W3C TraceContext)
- **Datadog Headers**: `x-datadog-trace-id`, `x-datadog-parent-id` (when using Datadog)
- **Kubernetes Events**: Trace IDs included in event metadata

## Integration with Observability Platforms

### OpenTelemetry

The controller integrates with OpenTelemetry for vendor-neutral tracing:

- **OTLP Exporter**: Export traces to OpenTelemetry Collector
- **Standard Format**: W3C TraceContext format
- **Vendor Agnostic**: Works with any OpenTelemetry-compatible backend

See [OpenTelemetry Setup](./opentelemetry.md) for configuration details.

### Datadog

The controller provides native Datadog integration:

- **Direct Export**: Traces sent directly to Datadog Agent
- **APM Integration**: Full APM features (service maps, traces, profiles)
- **Automatic Instrumentation**: Automatic span creation and context propagation
- **Service Tags**: Automatic service, version, and environment tagging

See [Datadog Integration](./datadog.md) for configuration details.

## Logging Best Practices

### Development

```bash
# Verbose logging for debugging
RUST_LOG=secret_manager_controller=debug,provider=trace
```

### Production

```bash
# Production-appropriate logging
RUST_LOG=secret_manager_controller=info,warn
```

### Troubleshooting

```bash
# Focus on errors and warnings
RUST_LOG=secret_manager_controller=warn,error
```

## Example Log Output

### Successful Reconciliation

```
2024-01-15T10:30:45.123456Z  INFO secret_manager_controller::controller::reconciler Starting reconciliation
  secret_manager_config.name=my-config
  secret_manager_config.namespace=default
  
2024-01-15T10:30:45.234567Z  INFO secret_manager_controller::provider::gcp Syncing secrets to GCP
  provider=gcp
  secret_count=5
  
2024-01-15T10:30:45.345678Z  INFO secret_manager_controller::controller::reconciler Reconciliation completed
  duration_ms=222
  secrets_synced=5
```

### Error Scenario

```
2024-01-15T10:30:45.123456Z  INFO secret_manager_controller::controller::reconciler Starting reconciliation
  secret_manager_config.name=my-config
  
2024-01-15T10:30:45.234567Z  ERROR secret_manager_controller::provider::gcp Failed to sync secret
  provider=gcp
  secret_name=my-secret
  error="Permission denied: insufficient IAM permissions"
  
2024-01-15T10:30:45.345678Z  WARN secret_manager_controller::controller::reconciler Reconciliation failed, will retry
  duration_ms=222
  error="Provider operation failed"
```

## Kubernetes Log Collection

### kubectl logs

```bash
# View controller logs
kubectl logs -n secret-manager-controller deployment/secret-manager-controller

# Follow logs
kubectl logs -n secret-manager-controller deployment/secret-manager-controller -f

# Filter by log level
kubectl logs -n secret-manager-controller deployment/secret-manager-controller | grep ERROR
```

### Log Aggregation

For production deployments, use log aggregation tools:

- **Fluentd/Fluent Bit**: Collect logs from Kubernetes pods
- **Loki**: Grafana's log aggregation system
- **Datadog Logs**: Automatic log collection with Datadog Agent
- **Cloud Logging**: GCP Cloud Logging, AWS CloudWatch, Azure Monitor

## Related Documentation

- [Metrics](./metrics.md) - Prometheus metrics
- [OpenTelemetry](./opentelemetry.md) - OpenTelemetry setup
- [Datadog](./datadog.md) - Datadog APM integration
- [Observability Guide](./observability-guide.md) - Complete observability overview

