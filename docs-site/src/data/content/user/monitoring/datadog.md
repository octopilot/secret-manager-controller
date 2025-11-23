# Datadog APM Integration

The Secret Manager Controller provides native Datadog integration for comprehensive Application Performance Monitoring (APM) and distributed tracing.

## Overview

Datadog integration provides:

- **Automatic Instrumentation**: Automatic span creation and context propagation
- **APM Features**: Service maps, traces, profiles, and error tracking
- **Service Tags**: Automatic service, version, and environment tagging
- **Direct Export**: Traces sent directly to Datadog Agent (no collector required)
- **Trace Context**: Full support for Datadog and W3C TraceContext formats

## Configuration

### Via CRD

Configure Datadog in your `SecretManagerConfig`:

```yaml
apiVersion: secret-management.microscaler.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-config
spec:
  provider: gcp
  otel:
    type: datadog
    serviceName: secret-manager-controller
    serviceVersion: v1.0.0
    environment: production
    site: datadoghq.com
    apiKey: ${DD_API_KEY}  # Optional: can use env var instead
```

### Via Environment Variables

Configure Datadog using environment variables:

```bash
# Required: API key (indicates Datadog is configured)
DD_API_KEY=your-api-key-here

# Optional: Service information
DD_SERVICE=secret-manager-controller
DD_VERSION=v1.0.0
DD_ENV=production
DD_SITE=datadoghq.com

# Optional: Agent URL (defaults to localhost:8126)
DD_TRACE_AGENT_URL=http://datadog-agent:8126
```

### Priority

Configuration priority (highest to lowest):

1. CRD `otel.datadog` field
2. Environment variables (`DD_*`)
3. Defaults (service name, site)

## Datadog Agent Setup

### Kubernetes Deployment

Deploy the Datadog Agent to your cluster:

```yaml
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: datadog-agent
  namespace: datadog
spec:
  selector:
    matchLabels:
      app: datadog-agent
  template:
    metadata:
      labels:
        app: datadog-agent
    spec:
      containers:
      - name: agent
        image: gcr.io/datadoghq/agent:latest
        env:
        - name: DD_API_KEY
          valueFrom:
            secretKeyRef:
              name: datadog-secret
              key: api-key
        - name: DD_SITE
          value: datadoghq.com
        - name: DD_APM_ENABLED
          value: "true"
        - name: DD_APM_NON_LOCAL_TRAFFIC
          value: "true"
        ports:
        - containerPort: 8126
          name: apm
          protocol: TCP
```

### Service

Create a Service for the Datadog Agent:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: datadog-agent
  namespace: datadog
spec:
  selector:
    app: datadog-agent
  ports:
  - port: 8126
    targetPort: 8126
    protocol: TCP
    name: apm
```

### Controller Configuration

Update your controller deployment to use the Datadog Agent service:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: secret-manager-controller
spec:
  template:
    spec:
      containers:
      - name: controller
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
        - name: DD_TRACE_AGENT_URL
          value: http://datadog-agent.datadog.svc.cluster.local:8126
```

## Service Tags

The controller automatically sets service tags:

- **`service`**: Service name (default: `secret-manager-controller`)
- **`version`**: Service version (default: `{CARGO_PKG_VERSION}-{BUILD_GIT_HASH}`)
- **`env`**: Environment (e.g., `production`, `staging`, `development`)

### Custom Tags

Add custom tags via the Datadog Agent configuration or environment variables:

```yaml
env:
- name: DD_TAGS
  value: "team:platform,component:secret-management"
```

## Trace Structure

Traces are automatically created for:

- **Reconciliations**: Full reconciliation lifecycle
- **Provider Operations**: Cloud provider API calls
- **Processing Operations**: SOPS decryption, Kustomize builds, Git operations
- **HTTP Requests**: All HTTP client requests

### Example Trace

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

## APM Features

### Service Map

View the controller's service map in Datadog:

- **Service Dependencies**: See how the controller interacts with cloud providers
- **Request Flow**: Visualize request paths through the system
- **Error Rates**: Identify services with high error rates

### Traces

View individual traces in Datadog APM:

- **Trace Search**: Search by service, operation, error, duration
- **Trace Details**: View span hierarchy, attributes, and logs
- **Error Analysis**: Identify and analyze errors in traces

### Profiles

Enable continuous profiling (requires Datadog Agent configuration):

- **CPU Profiling**: Identify CPU hotspots
- **Memory Profiling**: Track memory usage
- **Allocation Profiling**: Monitor object allocations

### Error Tracking

Automatic error tracking:

- **Error Aggregation**: Group similar errors
- **Error Trends**: Track error rates over time
- **Error Context**: View full trace context for errors

## Log Correlation

Correlate logs with traces:

- **Trace IDs**: Automatically included in log output
- **Span IDs**: Link logs to specific spans
- **Service Tags**: Filter logs by service, version, environment

### Log Integration

Enable log collection in the Datadog Agent:

```yaml
env:
- name: DD_LOGS_ENABLED
  value: "true"
- name: DD_LOGS_CONFIG_CONTAINER_COLLECT_ALL
  value: "true"
```

## Metrics Integration

Datadog automatically collects Prometheus metrics:

- **Automatic Discovery**: Datadog Agent discovers Prometheus endpoints
- **Metric Conversion**: Prometheus metrics converted to Datadog format
- **Custom Metrics**: All controller metrics available in Datadog

### ServiceMonitor Annotation

Annotate your ServiceMonitor for automatic discovery:

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: secret-manager-controller
  annotations:
    ad.datadoghq.com/service.check_names: '["prometheus"]'
    ad.datadoghq.com/service.init_configs: '[{}]'
    ad.datadoghq.com/service.instances: '[{"prometheus_url": "http://%%host%%:5000/metrics"}]'
```

## Troubleshooting

### Verify Configuration

Check that Datadog is initialized:

```bash
kubectl logs -n secret-manager-controller deployment/secret-manager-controller | grep -i datadog
```

Look for:

```
INFO Initializing Datadog OpenTelemetry tracing: service=secret-manager-controller
INFO ✅ Datadog OpenTelemetry tracing initialized successfully
```

### Check Agent Connectivity

Verify the controller can reach the Datadog Agent:

```bash
kubectl exec -n secret-manager-controller deployment/secret-manager-controller -- \
  curl -v http://datadog-agent.datadog.svc.cluster.local:8126/info
```

### View Traces

1. Open Datadog APM
2. Navigate to **Traces**
3. Filter by service: `secret-manager-controller`
4. View trace details and spans

### Common Issues

**Traces not appearing**:
- Verify `DD_API_KEY` is set
- Check Datadog Agent is running and accessible
- Verify `DD_TRACE_AGENT_URL` is correct

**Service name incorrect**:
- Check `DD_SERVICE` environment variable
- Verify CRD `otel.datadog.serviceName` field

**Missing spans**:
- Check log level (should be `INFO` or higher)
- Verify OpenTelemetry initialization succeeded
- Check Datadog Agent logs for errors

## Related Documentation

- [Metrics](./metrics.md) - Prometheus metrics
- [Tracing](./tracing.md) - Structured logging and tracing
- [OpenTelemetry](./opentelemetry.md) - OpenTelemetry setup
- [Observability Guide](./observability-guide.md) - Complete observability overview

