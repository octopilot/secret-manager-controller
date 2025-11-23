# OpenTelemetry Setup

The Secret Manager Controller supports OpenTelemetry for vendor-neutral distributed tracing and observability.

## Overview

OpenTelemetry provides a standardized approach to observability:

- **Vendor Neutral**: Works with any OpenTelemetry-compatible backend
- **Standard Format**: Uses W3C TraceContext for trace propagation
- **Flexible Export**: Export to OpenTelemetry Collector or directly to backends
- **Rich Metadata**: Automatic span creation with contextual information

## Current Status

**OTLP Exporter**: Currently pending implementation. The controller detects OTLP configuration and logs it, but does not yet export traces via OTLP.

**Datadog Direct Export**: Fully implemented and available. See [Datadog Integration](./datadog.md) for details.

## Configuration

### Via CRD

Configure OpenTelemetry in your `SecretManagerConfig`:

```yaml
apiVersion: secret-management.microscaler.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-config
spec:
  provider: gcp
  otel:
    type: otlp
    endpoint: http://otel-collector:4317
    serviceName: secret-manager-controller
    serviceVersion: v1.0.0
    environment: production
```

### Via Environment Variables

Configure OpenTelemetry using environment variables:

```bash
# OTLP endpoint
OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317

# Service information
OTEL_SERVICE_NAME=secret-manager-controller
OTEL_SERVICE_VERSION=v1.0.0
OTEL_RESOURCE_ATTRIBUTES=environment=production
```

## OTLP Configuration

### Endpoint

The OTLP endpoint should point to your OpenTelemetry Collector:

```yaml
otel:
  type: otlp
  endpoint: http://otel-collector:4317  # gRPC endpoint
  # or
  endpoint: http://otel-collector:4318  # HTTP endpoint
```

### Service Information

Configure service metadata for trace identification:

```yaml
otel:
  type: otlp
  serviceName: secret-manager-controller
  serviceVersion: v1.0.0
  environment: production
```

## OpenTelemetry Collector Setup

### Basic Collector Configuration

Deploy an OpenTelemetry Collector to receive traces:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: otel-collector-config
data:
  config.yaml: |
    receivers:
      otlp:
        protocols:
          grpc:
            endpoint: 0.0.0.0:4317
          http:
            endpoint: 0.0.0.0:4318
    
    processors:
      batch:
    
    exporters:
      logging:
        loglevel: debug
      # Add your backend exporter here
      # jaeger:
      #   endpoint: jaeger:14250
      # prometheus:
      #   endpoint: prometheus:9090
    
    service:
      pipelines:
        traces:
          receivers: [otlp]
          processors: [batch]
          exporters: [logging]
```

### Collector Deployment

Deploy the OpenTelemetry Collector to your cluster:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: otel-collector
spec:
  replicas: 1
  selector:
    matchLabels:
      app: otel-collector
  template:
    metadata:
      labels:
        app: otel-collector
    spec:
      containers:
      - name: otel-collector
        image: otel/opentelemetry-collector:latest
        ports:
        - containerPort: 4317
          name: otlp-grpc
        - containerPort: 4318
          name: otlp-http
        volumeMounts:
        - name: config
          mountPath: /etc/otelcol
      volumes:
      - name: config
        configMap:
          name: otel-collector-config
```

## Backend Integration

### Jaeger

Export traces to Jaeger:

```yaml
exporters:
  jaeger:
    endpoint: jaeger:14250
    tls:
      insecure: true
```

### Prometheus

Export metrics to Prometheus (note: traces are converted to metrics):

```yaml
exporters:
  prometheus:
    endpoint: prometheus:9090
```

### Grafana Tempo

Export traces to Grafana Tempo:

```yaml
exporters:
  tempo:
    endpoint: tempo:4317
    tls:
      insecure: true
```

### Datadog

Export traces to Datadog via the collector:

```yaml
exporters:
  datadog:
    api:
      key: ${DD_API_KEY}
      site: datadoghq.com
```

## Trace Context Propagation

The controller automatically propagates trace context:

- **W3C TraceContext**: Standard `traceparent` and `tracestate` headers
- **HTTP Requests**: Trace context included in provider API calls
- **Kubernetes Events**: Trace IDs included in event metadata

## Span Attributes

Spans include rich metadata:

- **`secret_manager_config.name`**: Resource name
- **`secret_manager_config.namespace`**: Resource namespace
- **`provider`**: Cloud provider (gcp, aws, azure)
- **`operation`**: Operation type (sync, update, delete)
- **`secret_count`**: Number of secrets processed
- **`duration_ms`**: Operation duration

## Future Implementation

The OTLP exporter implementation is planned for a future release. When implemented, it will:

- Export traces to OpenTelemetry Collector via OTLP
- Support both gRPC and HTTP protocols
- Include full span metadata and context
- Support trace sampling and filtering

## Workaround: Use Datadog

Until OTLP is fully implemented, you can use Datadog's direct export:

```yaml
otel:
  type: datadog
  serviceName: secret-manager-controller
  serviceVersion: v1.0.0
  environment: production
  site: datadoghq.com
  apiKey: ${DD_API_KEY}
```

See [Datadog Integration](./datadog.md) for details.

## Related Documentation

- [Metrics](./metrics.md) - Prometheus metrics
- [Tracing](./tracing.md) - Structured logging and tracing
- [Datadog](./datadog.md) - Datadog APM integration
- [Observability Guide](./observability-guide.md) - Complete observability overview

