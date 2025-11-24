# Design Decisions

This document outlines the key architectural decisions made during the development of the Secret Manager Controller, including the rationale behind technology choices, design patterns, and implementation approaches.

## Technology Stack

### Why Rust?

Rust was chosen as the primary language for the Secret Manager Controller for several critical reasons:

#### Performance and Resource Efficiency

- **Zero-Cost Abstractions**: Rust's abstractions compile to efficient machine code without runtime overhead
- **No Garbage Collection**: Eliminates GC pauses that can cause unpredictable latency in controllers that run 24/7
- **Memory Safety**: Compile-time guarantees prevent memory-related bugs without runtime overhead
- **Small Binaries**: Static linking with musl produces very small container images (~20MB)
- **Lower Resource Footprint**: Reduced memory and CPU usage compared to garbage-collected languages

#### Async/Await Excellence

- **Tokio Runtime**: Mature, production-ready async runtime
- **Better than Goroutines**: Rust's async/await model is more suitable for complex state machines and controller logic
- **Zero-Cost Concurrency**: Async operations have minimal overhead
- **Excellent Error Handling**: Result types and pattern matching provide compile-time error safety

#### Type System and Safety

- **Strong Type System**: Catches errors at compile time, reducing runtime failures
- **Pattern Matching**: Algebraic data types (enums) provide exhaustive pattern matching
- **Ownership Model**: Prevents data races and memory leaks at compile time
- **No Null References**: Option types eliminate null pointer exceptions

#### Controller-Specific Benefits

- **Long-Running Processes**: Controllers run continuously; Rust's memory safety prevents memory leaks over time
- **Resource Constraints**: Kubernetes controllers often run in resource-constrained environments; Rust's efficiency is critical
- **Reliability**: Compile-time guarantees reduce production incidents
- **Maintainability**: Strong types and ownership make code easier to reason about and maintain

### Why kube-rs?

The `kube-rs` crate was chosen as the Kubernetes client library:

#### Native Rust Implementation

- **No FFI Overhead**: Pure Rust implementation, no C bindings
- **Type Safety**: Strongly-typed Kubernetes resources
- **Async-First**: Built on tokio, integrates seamlessly with Rust async/await

#### CRD Support

- **Derive Macros**: `#[derive(kube::CustomResource)]` generates type-safe CRD code
- **Automatic Code Generation**: CRD schemas generate Rust types automatically
- **Version Management**: Built-in support for API versioning

#### Active Development

- **Strong Community**: Active development and responsive maintainers
- **Well Documented**: Comprehensive documentation and examples
- **Production Ready**: Used by many production Kubernetes controllers

#### Integration

- **Tokio Runtime**: Works seamlessly with tokio async runtime
- **Error Handling**: Integrates with Rust's Result types
- **Resource Watching**: Built-in support for watching Kubernetes resources

### Rust vs Go for Controllers

While Go is popular for Kubernetes controllers, Rust offers significant advantages:

| Aspect | Rust | Go |
|--------|------|-----|
| **Memory Safety** | Compile-time guarantees | Runtime checks (GC) |
| **Performance** | Zero-cost abstractions | GC overhead |
| **Binary Size** | ~20MB (musl static) | ~50-100MB |
| **Memory Usage** | Lower (no GC) | Higher (GC overhead) |
| **Concurrency** | Async/await (zero-cost) | Goroutines (stack overhead) |
| **Error Handling** | Result types (compile-time) | Error values (runtime) |
| **Type System** | Strong (ADTs, pattern matching) | Moderate (interfaces) |
| **GC Pauses** | None | Yes (unpredictable) |

For controllers that run 24/7 and handle critical infrastructure, Rust's guarantees and efficiency are invaluable.

## GCP Secret Manager Client

### Why Native REST Client?

The controller implements a native REST client for GCP Secret Manager instead of using the official `google-cloud-secret-manager` gRPC SDK.

#### OpenSSL Issues

The official `google-cloud-secret-manager` crate has OpenSSL dependencies that cause issues:

- **Build Complexity**: OpenSSL requires system libraries and complex build configuration
- **Docker Image Size**: OpenSSL adds significant size to container images
- **Cross-Compilation**: Difficult to cross-compile for different architectures
- **Security Concerns**: OpenSSL has a history of security vulnerabilities
- **Dependency Conflicts**: Can conflict with other Rust crates using different TLS implementations

#### reqwest with rustls

The controller uses `reqwest` with `rustls` for HTTP requests:

- **Pure Rust**: No C dependencies, no OpenSSL
- **Smaller Binaries**: rustls is more lightweight than OpenSSL
- **Better Security**: rustls is written in Rust with memory safety guarantees
- **Easier Cross-Compilation**: Pure Rust compiles to any target
- **Consistent Dependencies**: Same TLS stack across the entire codebase

#### GCP API v1 Completeness

The GCP Secret Manager REST API v1 is complete and well-documented:

- **All Operations**: Create, read, update, delete, list secrets
- **Full Feature Support**: All Secret Manager features available via REST
- **OAuth2 Support**: Standard OAuth2 authentication works with REST
- **Workload Identity**: Works seamlessly with GKE Workload Identity

#### Pact Integration

The native REST client integrates better with Pact testing:

- **HTTP Mock Servers**: Pact mock servers are HTTP-based
- **Direct Testing**: No gRPC-to-HTTP translation needed
- **Simpler Setup**: HTTP endpoints are easier to mock and test
- **Better Debugging**: HTTP requests are easier to inspect and debug

### Implementation Details

```rust
// Uses reqwest with rustls (configured in Cargo.toml)
use reqwest::Client;

// HTTP client with rustls (no OpenSSL)
let http_client = Client::builder()
    .build()
    .context("Failed to create HTTP client")?;
```

The REST client:
- Handles OAuth2 authentication (Workload Identity or service account JSON)
- Supports Pact mode via `GCP_SECRET_MANAGER_ENDPOINT` environment variable
- Provides full Secret Manager API coverage
- Includes comprehensive error handling

## Azure and AWS Pact API Replication

### Why Replicate Provider APIs?

The Pact mock servers replicate the actual Azure and AWS APIs rather than using simplified test interfaces.

#### Testing and Assurance

- **Real API Contracts**: Tests verify against actual API contracts, not simplified mocks
- **Integration Confidence**: Ensures the controller works with real provider APIs
- **Contract Evolution**: API changes are caught by contract tests
- **Provider Compatibility**: Validates compatibility with actual provider behavior

#### Contract-Driven Development

- **Consumer-Driven**: Controller defines expected API behavior via Pact contracts
- **Provider Verification**: Contracts can be verified against real providers
- **Documentation**: Contracts serve as executable API documentation
- **Regression Prevention**: Prevents breaking changes in provider integration

#### Realistic Testing

- **API Fidelity**: Mock servers match real API behavior, including error responses
- **Edge Cases**: Can test error conditions, rate limiting, and edge cases
- **Authentication**: Tests include authentication flows (OAuth2, AWS SigV4, etc.)
- **Request/Response Formats**: Validates request/response serialization

### Implementation

Each mock server (AWS, Azure, GCP) implements the actual provider API:

- **AWS Secrets Manager**: Replicates AWS Secrets Manager REST API
- **Azure Key Vault**: Replicates Azure Key Vault REST API
- **GCP Secret Manager**: Replicates GCP Secret Manager REST API v1

Contracts define:
- Request formats (headers, body, query parameters)
- Response formats (status codes, body structure)
- Error responses (error codes, error messages)
- Authentication requirements

## Source Handling Architecture

### FluxCD: Source-Controller Artifact Paths

The controller uses FluxCD's source-controller artifact paths for FluxCD GitRepository resources.

#### Why Source-Controller?

FluxCD's source-controller provides:

- **Artifact Caching**: Clones repos and caches artifacts as tar.gz files
- **HTTP Artifact Server**: Exposes artifacts via HTTP endpoints
- **Git Credential Handling**: Manages Git authentication automatically
- **Artifact URLs**: Provides predictable artifact URLs

#### Implementation

```rust
// Download artifact from source-controller
let artifact_url = format!(
    "http://source-controller.{}.svc.cluster.local/gitrepository/{}/{}/latest.tar.gz",
    namespace, namespace, name
);
```

Benefits:
- **Reuses Caching**: Leverages source-controller's artifact caching
- **Avoids Duplicate Clones**: No need to clone repos ourselves
- **Credential Management**: Uses source-controller's Git credential handling
- **Performance**: Artifacts are pre-cloned and cached

#### Artifact Path Structure

Artifacts are downloaded and extracted to:
```
/tmp/flux-source-{namespace}-{name}-{hash}/
```

This structure:
- Isolates artifacts by namespace and name
- Uses hash for cache invalidation
- Provides predictable paths for processing

### ArgoCD: Direct Git Cloning

The controller clones Git repositories directly for ArgoCD Application resources.

#### Why Direct Cloning?

ArgoCD doesn't provide artifact paths like FluxCD:

- **No Artifact Server**: ArgoCD doesn't expose artifacts via HTTP
- **Application Spec**: ArgoCD Applications reference Git repos but don't provide artifact URLs
- **Full Control**: Direct cloning gives us full control over the process

#### Implementation

```rust
// Extract Git source from ArgoCD Application
let git_source = extract_git_source_from_application(&app)?;

// Clone repository
let repo_path = clone_repository(&git_source, &cache_path)?;
```

Benefits:
- **Works with Any Git Source**: Not limited to FluxCD-managed repos
- **Private Repo Support**: Handles private repos via credentials
- **Full Control**: Complete control over clone process and caching

#### Cache Structure

Repositories are cloned to:
```
/tmp/smc/argocd-repo/{namespace}/{name}/{hash}/
```

This hierarchical structure:
- Prevents conflicts between namespaces
- Isolates by Application name
- Uses hash for cache invalidation
- Avoids performance issues from flat directory structures

## Pact Testing Architecture

### Combined Deployment

All Pact infrastructure components are deployed in a single `pact-infrastructure` pod:

- **Pact Broker**: Main service (port 9292)
- **Manager Sidecar**: Contract publishing and ConfigMap watching (port 8080)
- **Mock Servers**: AWS, GCP, Azure (ports 1234, 1235, 1236)
- **Mock Webhook**: Webhook receiver (port 8080)

#### Why Combined Deployment?

**Reduced Startup Time**:
- Single pod vs multiple deployments
- Faster CI/CD pipeline execution
- Fewer resources to wait for

**Simplified Orchestration**:
- One resource to manage
- Single Service for all components
- Unified health checks

**Better Resource Utilization**:
- Shared volumes (pacts storage, ConfigMap)
- Shared networking (localhost communication)
- Reduced overhead

**Faster Tilt Startup**:
- One deployment to wait for
- Faster local development iteration
- Simplified resource dependencies

### Component Packaging

#### Mock Servers: Single Image

All three mock servers (AWS, GCP, Azure) are packaged in a single `pact-mock-server` image:

- **Shared Base Image**: Reduces build time
- **Single Image**: Simplifies deployment
- **Rust Binaries**: All servers are Rust/Axum binaries
- **Port Assignment**: Each server uses a unique port (1234, 1235, 1236)

#### Manager Sidecar

The manager is a Rust binary that:

- **Watches ConfigMap**: Monitors `pact-contracts` ConfigMap for changes
- **Publishes Contracts**: Publishes contracts to Pact Broker when ready
- **Health Endpoints**: Provides `/liveness`, `/readiness`, and `/ready` endpoints
- **Re-publishes on Changes**: Re-publishes contracts when ConfigMap changes

Benefits over init containers:
- **Dynamic Updates**: Can handle ConfigMap changes without pod restart
- **Health Monitoring**: Provides health endpoints for Kubernetes probes
- **Better Reliability**: More robust than one-time init container execution

### Port Assignment

Each component uses a unique port:

- **Pact Broker**: 9292 (HTTP)
- **Manager**: 8080 (HTTP health endpoints)
- **AWS Mock Server**: 1234
- **GCP Mock Server**: 1235
- **Azure Mock Server**: 1236
- **Mock Webhook**: 8080 (different container)

This assignment:
- Prevents port conflicts
- Allows localhost communication
- Simplifies service configuration

### ConfigMap Watching

The manager watches the `pact-contracts` ConfigMap:

- **Initial Publish**: Publishes contracts when broker is ready
- **Change Detection**: Detects ConfigMap changes
- **Re-publishing**: Re-publishes contracts on changes
- **Mock Server Notification**: Mock servers wait for manager to confirm contracts are published

Benefits:
- **Dynamic Updates**: Contracts can be updated without pod restart
- **Reliability**: Manager ensures contracts are published before mock servers start
- **Observability**: Manager provides health endpoints to track publishing status

## Summary

These design decisions prioritize:

1. **Performance**: Rust's zero-cost abstractions and no GC
2. **Reliability**: Compile-time safety and strong types
3. **Maintainability**: Clear architecture and well-documented decisions
4. **Testing**: Comprehensive Pact-based contract testing
5. **Efficiency**: Optimized resource usage and startup times

Each decision was made with careful consideration of the controller's requirements: long-running processes, resource constraints, reliability, and comprehensive testing.

