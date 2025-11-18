# Pact Mock Server

A lightweight Rust/Axum HTTP server that serves as a mock for the GCP Secret Manager REST API.

## Purpose

This mock server is built independently from the main controller to enable:
- **Fast iteration**: Quick rebuilds without compiling the entire controller
- **Quick startup**: Lightweight binary that starts in seconds
- **Independent development**: Can be modified and tested without affecting controller builds

## Building

### Local Development

```bash
cd pact-broker/mock-server
cargo build --release
```

### Docker Build

```bash
docker build -t localhost:5000/axum-pact-mock-server:latest -f pact-broker/Dockerfile pact-broker/mock-server
```

## Running

```bash
# Set environment variables (optional)
export PACT_BROKER_URL=http://pact-broker:9292
export PACT_BROKER_USERNAME=pact
export PACT_BROKER_PASSWORD=pact
export PACT_PROVIDER=GCP-Secret-Manager
export PACT_CONSUMER=Secret-Manager-Controller
export PORT=1234

# Run the server
./target/release/pact-mock-server
```

## Environment Variables

- `PACT_BROKER_URL`: URL of the Pact broker (default: `http://pact-broker:9292`)
- `PACT_BROKER_USERNAME`: Username for broker authentication (default: `pact`)
- `PACT_BROKER_PASSWORD`: Password for broker authentication (default: `pact`)
- `PACT_PROVIDER`: Provider name in contracts (default: `GCP-Secret-Manager`)
- `PACT_CONSUMER`: Consumer name in contracts (default: `Secret-Manager-Controller`)
- `PORT`: Port to listen on (default: `1234`)

## API Endpoints

- `GET /health` - Health check
- `GET /v1/projects/{project}/secrets/{secret}/versions/{version}:access` - Get secret value
- `POST /v1/projects/{project}/secrets` - Create secret
- `POST /v1/projects/{project}/secrets/{secret}:addVersion` - Add secret version
- `DELETE /v1/projects/{project}/secrets/{secret}` - Delete secret

## Architecture

- **Framework**: Axum (async Rust web framework)
- **TLS**: rustls (pure Rust TLS implementation)
- **Logging**: tracing/tracing-subscriber
- **HTTP Client**: reqwest (for loading contracts from broker)

## Development

The mock server is intentionally kept minimal and focused. It:
- Serves mock responses for GCP Secret Manager API
- Logs all incoming requests for visibility
- Can load contracts from Pact broker (future enhancement)
- Runs as non-root user in containers

