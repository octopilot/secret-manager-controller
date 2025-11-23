# Observability Guide

Complete guide to monitoring, logging, and tracing the Secret Manager Controller.

## Overview

The Secret Manager Controller provides comprehensive observability through:

- **Prometheus Metrics**: Quantitative metrics for monitoring and alerting
- **Structured Logging**: Contextual logs with trace correlation
- **Distributed Tracing**: End-to-end request tracing across services
- **Health Probes**: Kubernetes liveness and readiness probes

## Quick Start

### Enable Metrics

Metrics are enabled by default on port `5000`:

```bash
# Query metrics
curl http://localhost:5000/metrics
```

### Enable Logging

Configure log levels via environment variable:

```bash
RUST_LOG=secret_manager_controller=info
```

### Enable Tracing

Configure OpenTelemetry (Datadog recommended):

```bash
DD_API_KEY=your-api-key
DD_SERVICE=secret-manager-controller
DD_ENV=production
```

## Observability Stack

### Recommended Stack

**Metrics**: Prometheus + Grafana
- Prometheus for metrics collection
- Grafana for visualization and dashboards

**Logging**: Loki + Grafana
- Loki for log aggregation
- Grafana for log querying and visualization

**Tracing**: Datadog APM
- Native Datadog integration
- Full APM features (service maps, traces, profiles)

### Alternative Stack

**Metrics**: Datadog
- Automatic Prometheus metric collection
- Built-in dashboards and alerting

**Logging**: Datadog Logs
- Automatic log collection
- Log correlation with traces

**Tracing**: OpenTelemetry Collector
- Vendor-neutral tracing
- Export to multiple backends

## Metrics

### Key Metrics

**Controller Health**:
- `secret_manager_reconciliations_total`: Reconciliation count
- `secret_manager_reconciliation_errors_total`: Error count
- `secret_manager_reconciliation_duration_seconds`: Performance

**Provider Operations**:
- `secret_manager_provider_operations_total`: Operation count by provider
- `secret_manager_provider_operation_errors_total`: Error count by provider
- `secret_manager_provider_operation_duration_seconds`: Performance by provider

**Secret Management**:
- `secret_manager_secrets_managed`: Current secret count
- `secret_manager_secrets_synced_total`: Sync count
- `secret_manager_secrets_updated_total`: Update count

### Dashboards

Create Grafana dashboards for:

- **Controller Overview**: Reconciliation rates, errors, duration
- **Provider Performance**: Operations, errors, latency by provider
- **Secret Management**: Secrets managed, synced, updated, skipped
- **Processing Operations**: SOPS, Kustomize, Git operations

See [Metrics Documentation](./metrics.md) for details.

## Logging

### Log Levels

- **ERROR**: Critical errors requiring attention
- **WARN**: Warning conditions
- **INFO**: Normal operations (default)
- **DEBUG**: Detailed diagnostic information
- **TRACE**: Very detailed diagnostic information

### Structured Fields

Logs include structured fields:

- `secret_manager_config.name`: Resource name
- `secret_manager_config.namespace`: Resource namespace
- `provider`: Cloud provider
- `operation`: Operation type
- `duration_ms`: Operation duration
- `error`: Error message (when present)

### Log Aggregation

For production, use log aggregation:

- **Fluentd/Fluent Bit**: Collect from Kubernetes
- **Loki**: Grafana's log aggregation
- **Datadog Logs**: Automatic collection
- **Cloud Logging**: GCP, AWS, Azure native logging

See [Tracing Documentation](./tracing.md) for details.

## Tracing

### Distributed Tracing

The controller supports distributed tracing via OpenTelemetry:

- **Automatic Instrumentation**: Spans created automatically
- **Context Propagation**: Trace context propagated across services
- **Rich Metadata**: Spans include operation context

### Trace Structure

Traces are hierarchical:

```
Reconciliation (root)
├── Git Operations
├── SOPS Decryption
├── Kustomize Build
└── Provider Sync
    ├── List Secrets
    ├── Create/Update Secrets
    └── Error Handling
```

### Backend Options

**Datadog** (Recommended):
- Native integration
- Full APM features
- Direct export (no collector)

**OpenTelemetry Collector**:
- Vendor-neutral
- Export to multiple backends
- Flexible configuration

See [Datadog Integration](./datadog.md) and [OpenTelemetry Setup](./opentelemetry.md) for details.

## Health Probes

### Liveness Probe

The controller exposes a liveness probe at `/healthz`:

```yaml
livenessProbe:
  httpGet:
    path: /healthz
    port: 5000
  initialDelaySeconds: 30
  periodSeconds: 10
```

### Readiness Probe

The controller exposes a readiness probe at `/readyz`:

```yaml
readinessProbe:
  httpGet:
    path: /readyz
    port: 5000
  initialDelaySeconds: 5
  periodSeconds: 5
```

## Alerting

### Key Alerts

**High Error Rate**:
```yaml
- alert: HighReconciliationErrorRate
  expr: rate(secret_manager_reconciliation_errors_total[5m]) > 0.1
  for: 5m
```

**Slow Reconciliations**:
```yaml
- alert: SlowReconciliations
  expr: histogram_quantile(0.95, secret_manager_reconciliation_duration_seconds_bucket) > 10
  for: 5m
```

**Provider Errors**:
```yaml
- alert: ProviderOperationErrors
  expr: rate(secret_manager_provider_operation_errors_total[5m]) > 0.05
  for: 5m
```

**Configuration Drift**:
```yaml
- alert: ConfigurationDrift
  expr: increase(secret_manager_secrets_diff_detected_total[1h]) > 0
  for: 5m
```

## Best Practices

### Production

1. **Metrics**: Enable Prometheus scraping
2. **Logging**: Set appropriate log levels (`INFO` or `WARN`)
3. **Tracing**: Enable Datadog or OpenTelemetry
4. **Alerting**: Configure alerts for error rates and performance
5. **Dashboards**: Create dashboards for key metrics

### Development

1. **Logging**: Use `DEBUG` or `TRACE` for detailed diagnostics
2. **Tracing**: Enable to understand request flows
3. **Metrics**: Monitor during testing

### Troubleshooting

1. **Check Logs**: Review controller logs for errors
2. **View Traces**: Use APM to trace request flows
3. **Query Metrics**: Check metrics for anomalies
4. **Health Probes**: Verify controller health

## Integration Examples

### Prometheus + Grafana

```yaml
# ServiceMonitor
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: secret-manager-controller
spec:
  selector:
    matchLabels:
      app: secret-manager-controller
  endpoints:
  - port: metrics
    path: /metrics
```

### Datadog

```yaml
# Environment variables
env:
- name: DD_API_KEY
  valueFrom:
    secretKeyRef:
      name: datadog-secret
      key: api-key
- name: DD_SERVICE
  value: secret-manager-controller
- name: DD_ENV
  value: production
```

### OpenTelemetry Collector

```yaml
# CRD configuration
spec:
  otel:
    type: otlp
    endpoint: http://otel-collector:4317
    serviceName: secret-manager-controller
```

## Related Documentation

- [Metrics](./metrics.md) - Detailed metrics documentation
- [Tracing](./tracing.md) - Logging and tracing details
- [OpenTelemetry](./opentelemetry.md) - OpenTelemetry setup
- [Datadog](./datadog.md) - Datadog APM integration

