# Pact Broker for Secret Manager Controller

This directory contains Kubernetes manifests for deploying an isolated Pact Broker specifically for the Secret Manager Controller.

## Overview

This Pact Broker is **completely isolated** from other submodules and uses:
- **SQLite** database for storing Pact contracts (ideal for single-controller use)
- **Direct environment variables** (no External Secrets Operator required)
- **Isolated namespace**: `secret-manager-controller-pact-broker`

## Architecture

```
SQLite Database (file-based)
    ↓
Pact Broker (pact-broker)
    ↓
Secret Manager Controller Pact Tests
```

## Why SQLite?

Since this Pact Broker is isolated to just the Secret Manager Controller and not used by any other system, SQLite is ideal:
- **Simpler**: No separate database service to manage
- **Lighter**: Lower resource usage (no PostgreSQL pod)
- **Sufficient**: Single-controller use case doesn't need concurrent database access
- **Faster**: No network latency between broker and database

## Deployment

### Local Development (k3s)

```bash
kubectl apply -k pact-broker/k8s/
```

### CI/CD

The CI workflow automatically deploys this Pact Broker to k3s clusters for testing.

## Configuration

- **Namespace**: `secret-manager-controller-pact-broker`
- **Database**: SQLite (file-based at `/pacts/pact_broker.sqlite`)
- **Credentials**: 
  - Username: `pact`
  - Password: `pact`
- **Broker URL**: `http://pact-broker.secret-manager-controller-pact-broker.svc.cluster.local:9292`
- **Storage**: Ephemeral (`emptyDir`) for CI/testing

## Port Forwarding

To access the Pact Broker locally:

```bash
kubectl port-forward -n secret-manager-controller-pact-broker service/pact-broker 9292:9292
```

Then access at: `http://localhost:9292`

## Axum Pact Mock Server

A Rust/Axum-based Pact mock server is deployed alongside the broker to serve contracts as a mock API. This allows the controller to use Pact contracts for testing without real cloud provider credentials.

- **Service**: `axum-pact-mock-server`
- **Port**: `1234`
- **Endpoint**: `http://axum-pact-mock-server.secret-manager-controller-pact-broker.svc.cluster.local:1234`

The mock server:
- Loads contracts from the Pact broker on startup
- Serves GCP Secret Manager REST API endpoints based on contracts
- Falls back to default mock responses if contracts aren't available

### Usage

The controller can be configured to use the mock server by setting:
- `PACT_MODE=true`
- `GCP_SECRET_MANAGER_ENDPOINT=http://axum-pact-mock-server.secret-manager-controller-pact-broker.svc.cluster.local:1234`

See `config/deployment/pact-env-patch.yaml` for the full configuration.

## Isolation

This Pact Broker is completely isolated from:
- Other PriceWhisperer submodules
- The shared `hack/controllers/pact-broker` directory
- Any other Pact Broker instances

Each controller submodule should have its own Pact Broker to avoid contract conflicts.

