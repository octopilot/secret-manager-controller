# Prometheus Metrics

The Secret Manager Controller exposes comprehensive Prometheus metrics for monitoring controller operations, provider interactions, and processing tasks.

## Metrics Endpoint

The controller exposes metrics at the `/metrics` endpoint on port `5000` by default (configurable via `METRICS_PORT` environment variable).

```bash
# Query metrics endpoint
curl http://localhost:5000/metrics
```

## Controller Metrics

### Reconciliation Metrics

**`secret_manager_reconciliations_total`** (Counter)
- Total number of reconciliation attempts
- Use to track controller activity and workload

**`secret_manager_reconciliation_errors_total`** (Counter)
- Total number of reconciliation errors
- Monitor for controller health and configuration issues

**`secret_manager_reconciliation_duration_seconds`** (Histogram)
- Duration of reconciliation operations in seconds
- Buckets: `0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0`
- Track reconciliation performance and identify slow operations

### Secrets Management Metrics

**`secret_manager_secrets_synced_total`** (Counter)
- Total number of secrets synced to cloud providers
- Tracks successful secret synchronization operations

**`secret_manager_secrets_updated_total`** (Counter)
- Total number of secrets updated (overwritten from Git)
- Indicates when Git changes trigger secret updates

**`secret_manager_secrets_managed`** (Gauge)
- Current number of secrets being managed
- Real-time count of active secrets

**`secret_manager_requeues_total`** (CounterVec)
- Total number of reconciliation requeues
- Labels: `reason` (e.g., "error", "retry", "backoff")
- Tracks why reconciliations are requeued

## Provider Metrics

### Generic Provider Metrics

**`secret_manager_provider_operations_total`** (CounterVec)
- Total number of provider operations by provider type
- Labels: `provider` (e.g., "gcp", "aws", "azure")
- Track operations across all providers

**`secret_manager_provider_operation_duration_seconds`** (HistogramVec)
- Duration of provider operations in seconds
- Labels: `provider`
- Buckets: `0.1, 0.5, 1.0, 2.0, 5.0, 10.0`
- Monitor provider API performance

**`secret_manager_provider_operation_errors_total`** (CounterVec)
- Total number of provider operation errors
- Labels: `provider`
- Track provider-specific failures

### Secret Publishing Metrics

**`secret_manager_secrets_published_total`** (CounterVec)
- Total number of secrets published to providers
- Labels: `provider`
- Track successful secret publications

**`secret_manager_secrets_skipped_total`** (CounterVec)
- Total number of secrets skipped (no changes or errors)
- Labels: `provider`, `reason`
- Understand why secrets are not published

**`secret_manager_secrets_diff_detected_total`** (CounterVec)
- Total number of secrets where differences were detected between Git and cloud provider
- Labels: `provider`
- Track configuration drift

### GCP-Specific Metrics (Backward Compatibility)

**`secret_manager_gcp_operations_total`** (Counter)
- Total number of GCP Secret Manager operations
- Maintained for backward compatibility

**`secret_manager_gcp_operation_duration_seconds`** (Histogram)
- Duration of GCP Secret Manager operations in seconds
- Buckets: `0.1, 0.5, 1.0, 2.0, 5.0`

## Processing Metrics

### SOPS Decryption Metrics

**`secret_manager_sops_decryption_total`** (Counter)
- Total number of SOPS decryption operations (attempts)

**`secret_manager_sops_decrypt_success_total`** (Counter)
- Total number of successful SOPS decryption operations

**`secret_manager_sops_decrypt_duration_seconds`** (Histogram)
- Duration of SOPS decryption operations in seconds
- Buckets: `0.1, 0.5, 1.0, 2.0, 5.0`

**`secret_manager_sops_decryption_errors_total`** (Counter)
- Total number of SOPS decryption errors

**`secret_manager_sops_decryption_errors_total_by_reason`** (CounterVec)
- Total number of SOPS decryption errors by failure reason
- Labels: `reason` (e.g., "key_not_found", "invalid_format", "permission_denied")

### Kustomize Build Metrics

**`secret_manager_kustomize_build_total`** (Counter)
- Total number of kustomize build operations

**`secret_manager_kustomize_build_duration_seconds`** (Histogram)
- Duration of kustomize build operations in seconds
- Buckets: `0.5, 1.0, 2.0, 5.0, 10.0, 30.0`

**`secret_manager_kustomize_build_errors_total`** (Counter)
- Total number of kustomize build errors

### Git Operations Metrics

**`secret_manager_git_clone_total`** (Counter)
- Total number of git clone operations

**`secret_manager_git_clone_duration_seconds`** (Histogram)
- Duration of git clone operations in seconds
- Buckets: `1.0, 2.0, 5.0, 10.0, 30.0, 60.0`

**`secret_manager_git_clone_errors_total`** (Counter)
- Total number of git clone errors

### Artifact Metrics

**`secret_manager_artifact_downloads_total`** (Counter)
- Total number of artifact downloads (FluxCD/ArgoCD)

**`secret_manager_artifact_download_duration_seconds`** (Histogram)
- Duration of artifact downloads in seconds
- Buckets: `0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0`

**`secret_manager_artifact_download_errors_total`** (Counter)
- Total number of artifact download errors

**`secret_manager_artifact_extractions_total`** (Counter)
- Total number of artifact extractions

**`secret_manager_artifact_extraction_duration_seconds`** (Histogram)
- Duration of artifact extractions in seconds
- Buckets: `0.1, 0.5, 1.0, 2.0, 5.0, 10.0`

**`secret_manager_artifact_extraction_errors_total`** (Counter)
- Total number of artifact extraction errors

### Configuration Metrics

**`secret_manager_duration_parsing_errors_total`** (Counter)
- Total number of duration parsing errors (reconcileInterval parsing failures)

## Example Queries

### Reconciliation Rate

```promql
rate(secret_manager_reconciliations_total[5m])
```

### Error Rate

```promql
rate(secret_manager_reconciliation_errors_total[5m])
```

### Average Reconciliation Duration

```promql
histogram_quantile(0.95, secret_manager_reconciliation_duration_seconds_bucket)
```

### Secrets Managed by Provider

```promql
secret_manager_secrets_published_total
```

### Provider Operation Error Rate

```promql
rate(secret_manager_provider_operation_errors_total[5m])
```

### SOPS Decryption Success Rate

```promql
rate(secret_manager_sops_decrypt_success_total[5m]) / rate(secret_manager_sops_decryption_total[5m])
```

### Secrets with Configuration Drift

```promql
secret_manager_secrets_diff_detected_total
```

## Prometheus ServiceMonitor

For Kubernetes deployments, create a ServiceMonitor to automatically scrape metrics:

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: secret-manager-controller
  namespace: secret-manager-controller
spec:
  selector:
    matchLabels:
      app: secret-manager-controller
  endpoints:
  - port: metrics
    interval: 30s
    path: /metrics
```

## Grafana Dashboards

Use these metrics to create Grafana dashboards for:

- **Controller Health**: Reconciliation rates, error rates, duration
- **Provider Performance**: Operation counts, durations, error rates by provider
- **Secret Management**: Secrets synced, updated, managed, skipped
- **Processing Operations**: SOPS decryption, Kustomize builds, Git operations
- **Configuration Drift**: Secrets with detected differences

## Alerting Rules

Example Prometheus alerting rules:

```yaml
groups:
- name: secret_manager_controller
  rules:
  - alert: HighReconciliationErrorRate
    expr: rate(secret_manager_reconciliation_errors_total[5m]) > 0.1
    for: 5m
    annotations:
      summary: "High reconciliation error rate"
      
  - alert: SlowReconciliations
    expr: histogram_quantile(0.95, secret_manager_reconciliation_duration_seconds_bucket) > 10
    for: 5m
    annotations:
      summary: "Slow reconciliation operations"
      
  - alert: ProviderOperationErrors
    expr: rate(secret_manager_provider_operation_errors_total[5m]) > 0.05
    for: 5m
    annotations:
      summary: "Provider operation errors detected"
```

## Related Documentation

- [Tracing](./tracing.md) - Structured logging and distributed tracing
- [OpenTelemetry](./opentelemetry.md) - OpenTelemetry integration
- [Datadog](./datadog.md) - Datadog APM integration
- [Observability Guide](./observability-guide.md) - Complete observability overview

