const e=`# Development Setup

Guide for setting up a local development environment for the Secret Manager Controller.

## Prerequisites

- **Rust**: 1.70+ (install via [rustup](https://rustup.rs/))
- **Docker**: For building container images
- **kubectl**: For Kubernetes cluster access
- **Kind**: For local Kubernetes cluster (optional, for integration tests)
- **Tilt**: For local development (recommended)

## Quick Start

### 1. Clone the Repository

\`\`\`bash
git clone https://github.com/microscaler/secret-manager-controller.git
cd secret-manager-controller
\`\`\`

### 2. Install Dependencies

\`\`\`bash
# Install Rust toolchain
rustup install stable

# Install required Rust targets
rustup target add x86_64-unknown-linux-musl

# Install musl tools (for cross-compilation)
# macOS
brew install musl-cross

# Linux
sudo apt-get install musl-tools
\`\`\`

### 3. Build the Project

\`\`\`bash
# Build the controller binary
cargo build

# Build for Linux (for Docker)
cargo build --target x86_64-unknown-linux-musl --release
\`\`\`

### 4. Run Tests

\`\`\`bash
# Run unit tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
\`\`\`

## Development with Tilt

Tilt provides the best development experience with live code updates.

### Setup

1. **Install Tilt:**

\`\`\`bash
# macOS
brew install tilt-dev/tap/tilt

# Linux
curl -fsSL https://raw.githubusercontent.com/tilt-dev/tilt/master/scripts/install.sh | bash
\`\`\`

2. **Start Tilt:**

\`\`\`bash
tilt up
\`\`\`

This will:
- Set up a Kind cluster
- Build and deploy the controller
- Set up GitOps components (FluxCD/ArgoCD)
- Set up Pact infrastructure for testing
- Enable live code updates

### Live Updates

Tilt watches for code changes and automatically:
- Rebuilds the Rust binary
- Syncs it into the running container
- Restarts the controller (SIGHUP)

### Tilt Resources

Key resources in Tilt:
- \`secret-manager-controller\`: Main controller deployment
- \`build-all-binaries\`: Builds all Rust binaries
- \`pact-infrastructure\`: Pact broker and mock servers
- \`apply-gitops-cluster\`: GitOps components

See [Tilt Integration](./tilt-integration.md) for details.

## Manual Development Setup

If you prefer not to use Tilt:

### 1. Set Up Kind Cluster

\`\`\`bash
# Create Kind cluster
python3 scripts/setup_kind.py

# Or manually
kind create cluster --name secret-manager-controller
\`\`\`

### 2. Install GitOps Components

\`\`\`bash
# Install FluxCD
python3 scripts/tilt/install_fluxcd.py

# Install ArgoCD CRDs
python3 scripts/tilt/install_argocd.py
\`\`\`

### 3. Build and Deploy Controller

\`\`\`bash
# Build binary
cargo build --target x86_64-unknown-linux-musl --release

# Build Docker image
docker build -t secret-manager-controller:dev -f dockerfiles/Dockerfile.controller .

# Load into Kind
kind load docker-image secret-manager-controller:dev --name secret-manager-controller

# Apply manifests
kubectl apply -k config/
\`\`\`

### 4. Update Controller

\`\`\`bash
# Rebuild
cargo build --target x86_64-unknown-linux-musl --release

# Rebuild image
docker build -t secret-manager-controller:dev -f dockerfiles/Dockerfile.controller .

# Restart controller
kubectl rollout restart deployment/secret-manager-controller -n microscaler-system
\`\`\`

## Project Structure

\`\`\`
secret-manager-controller/
├── crates/
│   ├── controller/          # Main controller crate
│   ├── providers/           # Cloud provider clients
│   ├── gitops/              # GitOps integration
│   ├── sops/                # SOPS decryption
│   ├── kustomize/           # Kustomize builder
│   └── ...
├── config/                  # Kubernetes manifests
├── scripts/                 # Automation scripts
├── tests/                   # Integration tests
└── docs/                    # Documentation
\`\`\`

## Code Organization

### Controller Logic

- **Location**: \`crates/controller/src/controller/\`
- **Main entry**: \`main.rs\`
- **Reconciliation**: \`reconcile.rs\`

### CRD Definitions

- **Location**: \`crates/controller/src/crd/\`
- **Spec**: \`spec.rs\`
- **Status**: \`status.rs\`
- **Source**: \`source.rs\`
- **Provider**: \`provider.rs\`

### Provider Clients

- **Location**: \`crates/controller/src/providers/\`
- **GCP**: \`gcp/\`
- **AWS**: \`aws/\`
- **Azure**: \`azure/\`

## Development Workflow

### 1. Make Changes

Edit code in the appropriate crate.

### 2. Test Locally

\`\`\`bash
# Run unit tests
cargo test

# Run with specific features
cargo test --features gcp,aws,azure
\`\`\`

### 3. Test in Cluster

\`\`\`bash
# With Tilt (automatic)
tilt up

# Or manually
# Rebuild, redeploy, check logs
kubectl logs -n microscaler-system -l app=secret-manager-controller -f
\`\`\`

### 4. Run Integration Tests

\`\`\`bash
# Set up Kind cluster
python3 scripts/setup_kind.py

# Run integration tests
cargo test --test integration
\`\`\`

## Debugging

### View Logs

\`\`\`bash
# Controller logs
kubectl logs -n microscaler-system -l app=secret-manager-controller -f

# With previous logs
kubectl logs -n microscaler-system -l app=secret-manager-controller --previous
\`\`\`

### Enable Debug Logging

Set \`RUST_LOG\` environment variable:

\`\`\`yaml
# In deployment
env:
  - name: RUST_LOG
    value: debug
\`\`\`

Or via ConfigMap (if hot-reload enabled):

\`\`\`yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: secret-manager-controller-config
  namespace: microscaler-system
data:
  RUST_LOG: debug
\`\`\`

### Debug in IDE

1. Install Rust analyzer extension
2. Set breakpoints
3. Use \`lldb\` or \`gdb\` for debugging

## Code Generation

### CRD Generation

The CRD is auto-generated from Rust types:

\`\`\`bash
# Generate CRD
cargo run -p controller --bin crdgen

# Output: config/crd/secretmanagerconfig.yaml
\`\`\`

**Note:** Don't edit the CRD YAML directly - modify the Rust types instead.

## Next Steps

- [Tilt Integration](./tilt-integration.md) - Tilt development workflow
- [Kind Cluster Setup](./kind-cluster-setup.md) - Local cluster setup
- [Testing Guide](../testing/testing-guide.md) - Testing strategies

`,b=Object.freeze(Object.defineProperty({__proto__:null,default:e},Symbol.toStringTag,{value:"Module"})),n=`# Tilt Integration

Complete guide to using Tilt for local development of the Secret Manager Controller.

## Overview

Tilt provides a unified development environment with:
- Automatic builds and deployments
- Live code updates (hot reload)
- Integrated testing infrastructure
- GitOps component management

## Quick Start

### Start Tilt

\`\`\`bash
tilt up
\`\`\`

This will:
1. Create a Kind cluster (if needed)
2. Build all Rust binaries
3. Deploy the controller
4. Set up GitOps components (FluxCD/ArgoCD)
5. Deploy Pact infrastructure for testing

### Stop Tilt

\`\`\`bash
tilt down
\`\`\`

Or press \`Ctrl+C\` in the Tilt UI.

## Tilt Resources

### Core Resources

#### \`build-all-binaries\`

Builds all Rust binaries for the project.

**Triggers:**
- Changes to Rust source code
- Changes to \`Cargo.toml\` or \`Cargo.lock\`

**Outputs:**
- Controller binary
- CRD generator binary
- Mock server binaries
- Manager binary

#### \`secret-manager-controller\`

Main controller deployment.

**Dependencies:**
- \`build-all-binaries\`
- CRD application
- GitOps components

**Live Updates:**
- Binary changes are synced into container
- Controller restarted with SIGHUP

#### \`pact-infrastructure\`

Pact broker and mock servers for contract testing.

**Components:**
- Pact broker (port 9292)
- Mock webhook server (port 1237)
- AWS mock server (port 1234)
- GCP mock server (port 1235)
- Azure mock server (port 1236)
- Manager sidecar (port 1238)

### GitOps Resources

#### \`apply-gitops-cluster\`

Applies GitOps cluster configuration.

**Includes:**
- Namespaces
- GitRepository resources
- Application resources

#### \`fluxcd-install\`

Installs FluxCD components.

**Installs:**
- source-controller
- GitRepository CRD

#### \`argocd-install\`

Installs ArgoCD CRDs.

**Installs:**
- Application CRD
- ApplicationSet CRD

## Development Workflow

### 1. Make Code Changes

Edit Rust source files in \`crates/\`.

### 2. Automatic Rebuild

Tilt detects changes and:
1. Rebuilds the binary
2. Syncs it into the container
3. Restarts the controller

**Watch the Tilt UI** to see build progress.

### 3. Check Logs

View controller logs in Tilt UI or:

\`\`\`bash
kubectl logs -n microscaler-system -l app=secret-manager-controller -f
\`\`\`

### 4. Test Changes

Create or update a SecretManagerConfig:

\`\`\`bash
kubectl apply -f examples/secretmanagerconfig.yaml
\`\`\`

Watch reconciliation in logs.

## Live Updates

Tilt uses \`sync\` to update the binary without full container rebuilds:

\`\`\`python
sync('./target/x86_64-unknown-linux-musl/debug/secret-manager-controller', '/app/secret-manager-controller')
\`\`\`

When the binary changes:
1. Tilt syncs it into the container
2. Sends SIGHUP to the controller process
3. Controller restarts with new binary

**Benefits:**
- Fast iteration (seconds vs minutes)
- No container rebuilds
- Preserves container state

## CRD Generation

The CRD is auto-generated when Rust types change:

**Process:**
1. \`build-all-binaries\` builds \`crdgen\` binary
2. Runs \`cargo run -p controller --bin crdgen\`
3. Generates \`config/crd/secretmanagerconfig.yaml\`
4. Applies CRD to cluster

**Manual trigger:**

\`\`\`bash
tilt trigger build-all-binaries
\`\`\`

## Testing with Tilt

### Pact Tests

Pact infrastructure is automatically deployed:

\`\`\`bash
# Run Pact tests
python3 scripts/pact_tests.py
\`\`\`

### Integration Tests

\`\`\`bash
# Run integration tests
cargo test --test integration
\`\`\`

### Unit Tests

\`\`\`bash
# Run unit tests (outside cluster)
cargo test
\`\`\`

## Troubleshooting

### Controller Not Starting

**Check:**
1. Binary build succeeded
2. CRD is applied
3. GitOps components are ready

**View logs:**
\`\`\`bash
kubectl logs -n microscaler-system -l app=secret-manager-controller
\`\`\`

### Live Updates Not Working

**Check:**
1. Binary path is correct
2. Container has write permissions
3. Process can receive SIGHUP

**Manual sync:**
\`\`\`bash
kubectl cp target/x86_64-unknown-linux-musl/debug/secret-manager-controller \\
  microscaler-system/secret-manager-controller-xxx:/app/secret-manager-controller
\`\`\`

### Build Failures

**Check:**
1. Rust toolchain is installed
2. musl target is installed
3. Dependencies are up to date

**Clean build:**
\`\`\`bash
cargo clean
tilt trigger build-all-binaries
\`\`\`

## Tiltfile Structure

The \`Tiltfile\` is organized into sections:

1. **Configuration**: Registry, context, settings
2. **Binary Builds**: Rust compilation
3. **Docker Builds**: Container images
4. **Kubernetes Resources**: Deployments, services
5. **GitOps Setup**: FluxCD, ArgoCD
6. **Pact Infrastructure**: Testing setup

## Best Practices

1. **Use Tilt for Development**: Fastest iteration cycle
2. **Watch Tilt UI**: See what's happening
3. **Check Logs Early**: Catch errors quickly
4. **Test in Cluster**: Integration tests catch real issues
5. **Clean Builds**: When things get weird, clean and rebuild

## Next Steps

- [Kind Cluster Setup](./kind-cluster-setup.md) - Cluster configuration
- [Testing Guide](../testing/testing-guide.md) - Testing strategies
- [Development Setup](./setup.md) - General development guide

`,A=Object.freeze(Object.defineProperty({__proto__:null,default:n},Symbol.toStringTag,{value:"Module"})),t=`# Configuration Options

Complete list of configuration options for Secret Manager Controller.

## Controller Configuration

### Environment Variables

- \`LOG_LEVEL\` - Logging level (\`debug\`, \`info\`, \`warn\`, \`error\`)
- \`METRICS_PORT\` - Port for metrics endpoint (default: \`9090\`)
- \`RECONCILE_INTERVAL\` - Reconciliation interval in seconds (default: \`300\`)

### ConfigMap Options

Create a ConfigMap in \`microscaler-system\` namespace:

\`\`\`yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: secret-manager-controller-config
  namespace: microscaler-system
data:
  log-level: "info"
  metrics-port: "9090"
  reconcile-interval: "300"
\`\`\`

## SecretManagerConfig Options

### Provider-Specific Options

#### AWS

- \`region\` - AWS region (required)
- \`endpoint\` - Custom endpoint (optional, for testing)

#### Azure

- \`vaultUrl\` - Key Vault URL (required)
- \`tenantId\` - Azure tenant ID (optional, from credentials)

#### GCP

- \`project\` - GCP project ID (required)
- \`location\` - Secret location (optional, default: \`global\`)

### Secret Options

Each secret in \`spec.secrets\` can have:
- \`name\` - Kubernetes Secret name (required)
- \`key\` - Provider secret key/path (required)
- \`version\` - Specific version (optional)
- \`encoding\` - Value encoding (\`base64\`, \`plain\`) (optional)

### Update Policy

- \`Always\` - Always update Kubernetes Secrets
- \`OnChange\` - Only update when provider value changes

## Advanced Options

### Git Repository Integration

- \`gitRepository.name\` - FluxCD GitRepository name
- \`gitRepository.namespace\` - GitRepository namespace
- \`gitRepository.path\` - Path within repository

### SOPS Configuration

- \`sops.enabled\` - Enable SOPS decryption
- \`sops.keySecret.name\` - Secret containing SOPS key
- \`sops.keySecret.namespace\` - Secret namespace

`,k=Object.freeze(Object.defineProperty({__proto__:null,default:t},Symbol.toStringTag,{value:"Module"})),r=`# CRD Reference

Complete reference for the \`SecretManagerConfig\` Custom Resource Definition.

## SecretManagerConfig

### API Version

\`secret-management.microscaler.io/v1\`

### Kind

\`SecretManagerConfig\` (shortname: \`smc\`)

### Example

\`\`\`yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: my-service-secrets
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: microscaler-system
  provider:
    gcp:
      projectId: my-gcp-project
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
    sops:
      enabled: true
      gpgSecretRef:
        name: sops-gpg-key
        namespace: microscaler-system
        key: private.key
\`\`\`

## Spec Fields

### sourceRef (required)

Reference to the GitOps source (GitRepository or Application).

\`\`\`yaml
sourceRef:
  kind: GitRepository  # or "Application" for ArgoCD
  name: my-repo
  namespace: microscaler-system
\`\`\`

**Fields:**
- \`kind\` (string, required): \`"GitRepository"\` or \`"Application"\`
- \`name\` (string, required): Name of the GitRepository or Application resource
- \`namespace\` (string, required): Namespace where the resource exists

### provider (required)

Cloud provider configuration. Specify one of: \`gcp\`, \`aws\`, or \`azure\`.

#### GCP Configuration

\`\`\`yaml
provider:
  gcp:
    projectId: my-gcp-project
\`\`\`

**Fields:**
- \`projectId\` (string, required): GCP project ID

#### AWS Configuration

\`\`\`yaml
provider:
  aws:
    region: us-east-1
\`\`\`

**Fields:**
- \`region\` (string, required): AWS region (e.g., \`us-east-1\`, \`eu-west-1\`)

#### Azure Configuration

\`\`\`yaml
provider:
  azure:
    vaultUrl: https://my-vault.vault.azure.net/
\`\`\`

**Fields:**
- \`vaultUrl\` (string, required): Azure Key Vault URL

### secrets (required)

Secret sync configuration.

\`\`\`yaml
secrets:
  environment: dev
  kustomizePath: path/to/kustomize/overlay
  sops:
    enabled: true
    gpgSecretRef:
      name: sops-gpg-key
      namespace: microscaler-system
      key: private.key
\`\`\`

**Fields:**
- \`environment\` (string, required): Environment name (e.g., \`dev\`, \`staging\`, \`prod\`)
- \`kustomizePath\` (string, required): Path to Kustomize overlay in Git repository
- \`sops\` (object, optional): SOPS decryption configuration
  - \`enabled\` (boolean): Enable SOPS decryption (default: \`false\`)
  - \`gpgSecretRef\` (object): Reference to GPG key Kubernetes Secret
    - \`name\` (string): Secret name
    - \`namespace\` (string): Secret namespace
    - \`key\` (string): Key in secret containing GPG private key (default: \`private.key\`)

### configs (optional)

Config store configuration for routing \`application.properties\` to config stores.

\`\`\`yaml
configs:
  enabled: true
  parameterPath: /my-service/dev  # AWS only
  appConfigEndpoint: https://my-app-config.azconfig.io  # Azure only
  store: SecretManager  # GCP: SecretManager or ParameterManager
\`\`\`

**Fields:**
- \`enabled\` (boolean, default: \`false\`): Enable config store sync
- \`parameterPath\` (string, optional, AWS only): Parameter Store path prefix
- \`appConfigEndpoint\` (string, optional, Azure only): App Configuration endpoint
- \`store\` (string, optional, GCP only): Store type - \`SecretManager\` or \`ParameterManager\`

### otel (optional)

OpenTelemetry configuration for distributed tracing.

\`\`\`yaml
otel:
  exporter: otlp  # or "datadog"
  endpoint: http://otel-collector:4317
  serviceName: secret-manager-controller
\`\`\`

**Fields:**
- \`exporter\` (string): Exporter type - \`"otlp"\` or \`"datadog"\` (default: \`"otlp"\`)
- \`endpoint\` (string): Exporter endpoint URL
- \`serviceName\` (string): Service name for tracing (default: \`"secret-manager-controller"\`)

### gitRepositoryPullInterval (optional)

How often to check for updates from Git.

\`\`\`yaml
gitRepositoryPullInterval: 5m  # Default: 5m, minimum: 1m
\`\`\`

**Format:** Kubernetes duration string (e.g., \`"1m"\`, \`"5m"\`, \`"1h"\`)

**Recommendation:** 5 minutes or greater to avoid Git API rate limits.

### reconcileInterval (optional)

How often to reconcile secrets between Git and cloud provider.

\`\`\`yaml
reconcileInterval: 1m  # Default: 1m
\`\`\`

**Format:** Kubernetes duration string (e.g., \`"30s"\`, \`"1m"\`, \`"5m"\`)

### diffDiscovery (optional)

Enable diff discovery to detect tampering.

\`\`\`yaml
diffDiscovery: true  # Default: false
\`\`\`

When enabled, logs warnings when differences are found between Git (source of truth) and cloud provider.

### logging (optional)

Fine-grained logging configuration.

\`\`\`yaml
logging:
  reconciliation: INFO
  secrets: INFO
  properties: INFO
  provider: DEBUG
  sops: DEBUG
  git: INFO
  kustomize: INFO
  diffDiscovery: WARN
\`\`\`

**Log Levels:** \`ERROR\`, \`WARN\`, \`INFO\`, \`DEBUG\`

**Defaults:**
- \`reconciliation\`: \`INFO\`
- \`secrets\`: \`INFO\`
- \`properties\`: \`INFO\`
- \`provider\`: \`DEBUG\`
- \`sops\`: \`DEBUG\`
- \`git\`: \`INFO\`
- \`kustomize\`: \`INFO\`
- \`diffDiscovery\`: \`WARN\`

### notifications (optional)

Notification configuration for drift detection alerts.

\`\`\`yaml
notifications:
  fluxcd:
    providerRef:
      name: my-provider
      namespace: flux-system
  argocd:
    subscriptions:
      - service: slack
        channel: "#secrets-alerts"
        trigger: drift-detected
\`\`\`

**Fields:**
- \`fluxcd\` (object, optional): FluxCD notification configuration
  - \`providerRef\` (object): FluxCD Provider reference
    - \`name\` (string): Provider name
    - \`namespace\` (string, optional): Provider namespace
- \`argocd\` (object, optional): ArgoCD notification configuration
  - \`subscriptions\` (array): List of notification subscriptions
    - \`service\` (string): Notification service (e.g., \`slack\`, \`email\`, \`webhook\`)
    - \`channel\` (string): Notification channel
    - \`trigger\` (string): Trigger name

### hotReload (optional)

Hot reload configuration for controller-level settings.

\`\`\`yaml
hotReload:
  enabled: false  # Default: false
  configMapName: secret-manager-controller-config
  configMapNamespace: microscaler-system
\`\`\`

**Fields:**
- \`enabled\` (boolean, default: \`false\`): Enable hot-reload
- \`configMapName\` (string, default: \`"secret-manager-controller-config"\`): ConfigMap to watch
- \`configMapNamespace\` (string, optional): ConfigMap namespace (defaults to controller namespace)

## Status Fields

The controller updates the status with:

### phase (string)

Current phase: \`Pending\`, \`Syncing\`, \`Synced\`, \`Error\`

### description (string)

Human-readable status message.

### conditions (array)

Kubernetes conditions array with:
- \`type\`: Condition type (e.g., \`Ready\`, \`Synced\`)
- \`status\`: \`True\`, \`False\`, or \`Unknown\`
- \`reason\`: Reason code
- \`message\`: Human-readable message
- \`lastTransitionTime\`: Timestamp

### lastSyncTime (string)

Timestamp of last successful sync (RFC3339 format).

### secretsCount (integer)

Number of secrets currently managed.

## Printer Columns

The CRD includes additional printer columns:

- \`PHASE\`: Current phase
- \`DESCRIPTION\`: Status description
- \`READY\`: Ready condition status

View with:

\`\`\`bash
kubectl get secretmanagerconfig
\`\`\`

## Validation

The CRD schema validates:
- Required fields are present
- Provider-specific required fields
- Duration strings are valid
- Enum values are correct

## Examples

See the [Examples](../tutorials/basic-usage.md) section for complete working examples.

## Learn More

- [Configuration Guide](../getting-started/configuration.md) - Detailed configuration guide
- [Provider APIs](./provider-apis.md) - Provider-specific API details
- [Configuration Options](./configuration-options.md) - All configuration options
`,P=Object.freeze(Object.defineProperty({__proto__:null,default:r},Symbol.toStringTag,{value:"Module"})),s=`# Provider APIs

API details for each cloud provider.

## AWS Secrets Manager

### Endpoints

- \`GetSecretValue\` - Retrieve secret value
- \`DescribeSecret\` - Get secret metadata
- \`ListSecrets\` - List available secrets

### Authentication

- IAM roles (recommended)
- Access keys
- Temporary credentials

### Regions

All AWS regions are supported. Specify in \`spec.region\`.

## Azure Key Vault

### Endpoints

- \`Get Secret\` - Retrieve secret value
- \`List Secrets\` - List available secrets

### Authentication

- Managed Identity (recommended)
- Service Principal
- Client certificates

### Vault URL Format

\`https://<vault-name>.vault.azure.net/\`

## GCP Secret Manager

### Endpoints

- \`projects.secrets.versions.access\` - Retrieve secret value
- \`projects.secrets.list\` - List available secrets

### Authentication

- Workload Identity (recommended)
- Service Account keys
- Application Default Credentials

### Project Format

Specify GCP project ID in \`spec.project\`.

## Error Handling

All providers return standardized errors:
- \`AuthenticationError\` - Credential issues
- \`NotFoundError\` - Secret doesn't exist
- \`PermissionError\` - Insufficient permissions
- \`NetworkError\` - Connection issues

## Rate Limiting

The controller implements rate limiting and retry logic for all providers.

`,R=Object.freeze(Object.defineProperty({__proto__:null,default:s},Symbol.toStringTag,{value:"Module"})),o=`# Components

Detailed documentation of the Secret Manager Controller components.

## Controller Components

### 1. SecretManagerConfig CRD

The Custom Resource Definition that defines the desired state.

**Location:** \`crates/controller/src/crd/\`

**Key Fields:**
- \`sourceRef\`: Reference to GitRepository or Application
- \`provider\`: Cloud provider configuration
- \`secrets\`: Secret sync configuration
- \`configs\`: Config store configuration (optional)
- \`otel\`: OpenTelemetry configuration (optional)

**Status Fields:**
- \`phase\`: Current phase (Pending, Syncing, Synced, Error)
- \`description\`: Human-readable status message
- \`conditions\`: Kubernetes conditions array
- \`lastSyncTime\`: Timestamp of last successful sync
- \`secretsCount\`: Number of secrets managed

### 2. Reconciliation Loop

The core controller logic that watches and reconciles resources.

**Location:** \`crates/controller/src/controller/\`

**Responsibilities:**
- Watches SecretManagerConfig resources
- Polls GitOps artifacts for changes
- Coordinates secret extraction and sync
- Updates status and conditions

**Key Functions:**
- \`reconcile()\`: Main reconciliation logic
- \`sync_secrets()\`: Syncs secrets to cloud provider
- \`extract_secrets()\`: Extracts secrets from Git artifacts

### 3. GitOps Integration

Handles integration with FluxCD and ArgoCD.

**Location:** \`crates/controller/src/gitops/\`

**FluxCD Support:**
- Reads artifacts from \`/tmp/flux-source-*\` directories
- Watches GitRepository CRD
- Uses source-controller artifacts

**ArgoCD Support:**
- Reads from Application CRD
- Direct Git repository access
- Clones repository to temporary directory

### 4. SOPS Decryption

Decrypts SOPS-encrypted secret files.

**Location:** \`crates/controller/src/sops/\`

**Features:**
- GPG key management from Kubernetes Secrets
- File-level and value-level decryption
- Multiple encryption method support
- Error handling and logging

**Process:**
1. Read GPG key from Kubernetes Secret
2. Detect SOPS-encrypted files
3. Decrypt using GPG key
4. Return decrypted content

### 5. Kustomize Builder

Builds Kustomize overlays and extracts secrets.

**Location:** \`crates/controller/src/kustomize/\`

**Process:**
1. Run \`kustomize build\` on specified path
2. Parse generated YAML
3. Extract Kubernetes Secret resources
4. Return secret data

**Features:**
- Supports overlays and patches
- Handles generators
- Error handling for invalid Kustomize configs

### 6. Provider Clients

Cloud provider-specific clients.

**Location:** \`crates/controller/src/providers/\`

#### GCP Secret Manager Client

**Location:** \`crates/controller/src/providers/gcp/\`

**Features:**
- Workload Identity support
- Service account key fallback
- Secret versioning
- Batch operations

#### AWS Secrets Manager Client

**Location:** \`crates/controller/src/providers/aws/\`

**Features:**
- IRSA (IAM Roles for Service Accounts) support
- Access key fallback
- Secret versioning
- Tag support

#### Azure Key Vault Client

**Location:** \`crates/controller/src/providers/azure/\`

**Features:**
- Workload Identity support
- Service principal fallback
- Secret versioning
- Key Vault access policies

### 7. Config Store Integration

Routes \`application.properties\` to config stores.

**Location:** \`crates/controller/src/config_store/\`

**Supported Stores:**
- **AWS Parameter Store**: SSM Parameter Store
- **Azure App Configuration**: App Configuration service
- **GCP Parameter Manager**: (Future, via ESO contribution)

**Process:**
1. Detect \`application.properties\` files
2. Parse properties
3. Route to appropriate config store
4. Store as individual parameters

### 8. HTTP Server

Provides metrics, health checks, and probes.

**Location:** \`crates/controller/src/http/\`

**Endpoints:**
- \`/metrics\`: Prometheus metrics
- \`/health\`: Health check
- \`/ready\`: Readiness probe
- \`/live\`: Liveness probe

**Metrics:**
- Reconciliation count
- Secret sync duration
- Error rates
- Provider API calls

### 9. OpenTelemetry Integration

Distributed tracing support.

**Location:** \`crates/controller/src/otel/\`

**Exporters:**
- OTLP (OpenTelemetry Protocol)
- Datadog direct export

**Traces:**
- Reconciliation operations
- Provider API calls
- SOPS decryption
- Kustomize builds

## Data Structures

### SecretManagerConfigSpec

Main configuration structure.

\`\`\`rust
pub struct SecretManagerConfigSpec {
    pub source_ref: SourceRef,
    pub provider: ProviderConfig,
    pub secrets: SecretsConfig,
    pub configs: Option<ConfigsConfig>,
    pub otel: Option<OtelConfig>,
    pub git_repository_pull_interval: String,
    pub reconcile_interval: String,
    pub diff_discovery: bool,
}
\`\`\`

### SourceRef

GitOps source reference.

\`\`\`rust
pub struct SourceRef {
    pub kind: String,  // "GitRepository" or "Application"
    pub name: String,
    pub namespace: String,
}
\`\`\`

### ProviderConfig

Cloud provider configuration.

\`\`\`rust
pub enum ProviderConfig {
    Gcp { project_id: String },
    Aws { region: String },
    Azure { vault_url: String },
}
\`\`\`

### SecretsConfig

Secret sync configuration.

\`\`\`rust
pub struct SecretsConfig {
    pub environment: String,
    pub kustomize_path: String,
    pub sops: Option<SopsConfig>,
}
\`\`\`

## Error Handling

All components use structured error handling:

- **Result Types**: All operations return \`Result<T, E>\`
- **Error Propagation**: Errors bubble up with context
- **Logging**: Errors are logged with full context
- **Status Updates**: Errors are reflected in CRD status

## Testing

Components are tested at multiple levels:

- **Unit Tests**: Individual function tests
- **Integration Tests**: Component interaction tests
- **Pact Tests**: Provider API contract tests
- **E2E Tests**: Full workflow tests in Kind cluster

## Next Steps

- [Architecture Overview](./overview.md) - High-level architecture
- [API Reference](../api-reference/crd-reference.md) - CRD reference
- [Development Guide](../../contributor/development/setup.md) - Contributing
`,E=Object.freeze(Object.defineProperty({__proto__:null,default:o},Symbol.toStringTag,{value:"Module"})),a=`# Architecture Overview

The Secret Manager Controller is a Kubernetes operator that syncs secrets from GitOps repositories to cloud secret management systems.

## High-Level Architecture

\`\`\`mermaid
flowchart TB
    subgraph Git["Git Repository"]
        SOPS[SOPS-Encrypted<br/>Secret Files]
        KUST[Kustomize<br/>Overlays]
    end
    
    subgraph GitOps["GitOps Tool"]
        FR[FluxCD<br/>GitRepository]
        AA[ArgoCD<br/>Application]
    end
    
    subgraph Controller["Secret Manager Controller"]
        CRD[SecretManagerConfig<br/>CRD]
        RECON[Reconciliation<br/>Loop]
        SOPS_DEC[SOPS<br/>Decryption]
        KUST_BUILD[Kustomize<br/>Build]
        SEC_EXTRACT[Secret<br/>Extraction]
    end
    
    subgraph Cloud["Cloud Providers"]
        GCP[GCP Secret<br/>Manager]
        AWS[AWS Secrets<br/>Manager]
        Azure[Azure Key<br/>Vault]
    end
    
    subgraph Services["Kubernetes Services"]
        APP1[Application 1]
        APP2[Application 2]
        APP3[Application 3]
    end
    
    SOPS --> FR
    KUST --> FR
    SOPS --> AA
    KUST --> AA
    
    FR --> CRD
    AA --> CRD
    
    CRD --> RECON
    RECON --> SOPS_DEC
    RECON --> KUST_BUILD
    RECON --> SEC_EXTRACT
    
    SOPS_DEC --> SEC_EXTRACT
    KUST_BUILD --> SEC_EXTRACT
    
    SEC_EXTRACT --> GCP
    SEC_EXTRACT --> AWS
    SEC_EXTRACT --> Azure
    
    GCP --> APP1
    AWS --> APP2
    Azure --> APP3
    
    style CRD fill:#e1f5ff
    style RECON fill:#fff4e1
    style GCP fill:#fff4e1
    style AWS fill:#fff4e1
    style Azure fill:#fff4e1
\`\`\`

## Core Concepts

### 1. GitOps-Driven

The controller follows GitOps principles:
- **Git is the source of truth** for all secrets
- Secrets are version-controlled and auditable
- Changes flow from Git → Controller → Cloud Provider
- No manual secret management in cloud providers

### 2. GitOps-Agnostic

Works with any GitOps tool:
- **FluxCD**: Uses \`GitRepository\` CRD and source-controller artifacts
- **ArgoCD**: Uses \`Application\` CRD and direct Git access
- **Others**: Can be extended to support additional tools

### 3. Multi-Cloud Support

Supports all major cloud providers:
- **GCP Secret Manager**: Native integration for GKE
- **AWS Secrets Manager**: Native integration for EKS
- **Azure Key Vault**: Native integration for AKS

### 4. SOPS Integration

Automatically decrypts SOPS-encrypted secrets:
- Uses GPG keys stored in Kubernetes Secrets
- Supports both file-level and value-level encryption
- Maintains security while enabling Git storage

## Workflow

### 1. Git Repository Setup

Secrets are stored in Git repositories:
- Organized by service and environment
- Encrypted with SOPS (optional but recommended)
- Managed via Kustomize overlays

### 2. GitOps Tool Sync

GitOps tool (FluxCD/ArgoCD) syncs repository:
- FluxCD: Creates artifacts in \`/tmp/flux-source-*\`
- ArgoCD: Clones repository directly
- Controller watches for updates

### 3. Controller Reconciliation

Controller reconciles secrets:
1. **Reads** GitRepository/Application artifacts
2. **Decrypts** SOPS-encrypted files (if enabled)
3. **Builds** Kustomize overlays
4. **Extracts** Kubernetes Secret resources
5. **Syncs** to cloud provider secret manager

### 4. Cloud Provider Sync

Secrets are synced to cloud provider:
- GCP: Stored in Secret Manager
- AWS: Stored in Secrets Manager
- Azure: Stored in Key Vault

### 5. Service Consumption

**Kubernetes Workloads:**
- External Secrets Operator syncs to ConfigMaps/Secrets
- Pods consume via volume mounts or environment variables
- No knowledge of GitOps or SOPS

**Serverless Workloads:**
- Crossplane provisions serverless resources
- Secrets injected via \`secretKeyRef\` (CloudRun) or Lambda Extensions
- Configs accessed via SDKs or environment variables
- No knowledge of GitOps or SOPS

## Key Components

### SecretManagerConfig CRD

The main configuration resource:
- Defines source (GitRepository/Application)
- Configures provider (GCP/AWS/Azure)
- Specifies secret paths and options

### Reconciliation Loop

Core controller logic:
- Watches SecretManagerConfig resources
- Polls GitOps artifacts for changes
- Syncs secrets to cloud providers
- Updates status and conditions

### SOPS Decryption

Handles encrypted secrets:
- Reads GPG keys from Kubernetes Secrets
- Decrypts SOPS-encrypted files
- Supports multiple encryption methods

### Kustomize Builder

Processes Kustomize overlays:
- Runs \`kustomize build\` on specified paths
- Extracts Kubernetes Secret resources
- Handles overlays, patches, and generators

### Provider Clients

Cloud provider integrations:
- GCP Secret Manager client
- AWS Secrets Manager client
- Azure Key Vault client
- All support Workload Identity/IRSA

## Data Flow

### Kubernetes Workloads

\`\`\`mermaid
flowchart TD
    A[Git Repository] -->|GitOps Tool Sync| B[GitOps Artifacts]
    B -->|Controller Reads| C{SOPS Enabled?}
    C -->|Yes| D[SOPS Decryption]
    C -->|No| E[Kustomize Build]
    D --> E
    E --> F[Secret Extraction]
    F -->|Controller Syncs| G[Cloud Provider Secret Manager]
    G -->|External Secrets Operator| H[Kubernetes Secrets/ConfigMaps]
    H -->|Pods Consume| I[Kubernetes Applications]
    
    style A fill:#e1f5ff
    style G fill:#fff4e1
    style H fill:#f0f9ff
    style I fill:#f0f9ff
\`\`\`

### Serverless Workloads

\`\`\`mermaid
flowchart TD
    A[Git Repository] -->|GitOps Tool Sync| B[GitOps Artifacts]
    B -->|Controller Reads| C{SOPS Enabled?}
    C -->|Yes| D[SOPS Decryption]
    C -->|No| E[Kustomize Build]
    D --> E
    E --> F[Secret Extraction]
    F -->|Controller Syncs| G[Cloud Provider Secret Manager]
    F -->|Controller Syncs| H[Cloud Provider Config Store]
    G -->|Crossplane Applies| I[Serverless Resources]
    H -->|Crossplane Applies| I
    I -->|Runtime Injection| J[CloudRun/Lambda/Functions]
    
    style A fill:#e1f5ff
    style G fill:#fff4e1
    style H fill:#ccffcc
    style I fill:#ae3ec9,color:#fff
    style J fill:#51cf66,color:#fff
\`\`\`

See [Serverless Integration](./serverless-integration.md) for complete serverless architecture details.

## Security Model

### Authentication

- **Workload Identity** (GCP/Azure): No stored credentials
- **IRSA** (AWS): IAM roles for service accounts
- **Service Account Keys**: Fallback option (less secure)

### Encryption

- **In Transit**: TLS for all API calls
- **At Rest**: Cloud provider encryption
- **In Git**: SOPS encryption (user-managed)

### Access Control

- **RBAC**: Kubernetes Role-Based Access Control
- **Cloud IAM**: Provider-specific permissions
- **Git Access**: Managed by GitOps tool

## Scalability

- **Horizontal Scaling**: Multiple controller replicas (future)
- **Rate Limiting**: Configurable Git pull intervals
- **Caching**: GitOps artifact caching
- **Batching**: Efficient secret sync operations

## Monitoring & Observability

- **Metrics**: Prometheus-compatible metrics endpoint
- **Logging**: Structured logging with tracing
- **Status**: Kubernetes conditions and status fields
- **OpenTelemetry**: Distributed tracing support

## Next Steps

- [Components](./components.md) - Detailed component documentation
- [Getting Started](../getting-started/installation.md) - Installation guide
- [Configuration](../getting-started/configuration.md) - Configuration options
`,G=Object.freeze(Object.defineProperty({__proto__:null,default:a},Symbol.toStringTag,{value:"Module"})),i=`# Serverless Integration Architecture

Complete guide to how the Secret Manager Controller integrates with serverless systems and Crossplane.

## Overview

The Secret Manager Controller is designed to work with **both Kubernetes and serverless systems**. While the controller runs in Kubernetes, it syncs secrets to cloud provider secret managers that are consumed by serverless workloads (CloudRun, Lambda, Azure Functions) as well as Kubernetes workloads.

## Architecture Flow

### Complete End-to-End Flow

\`\`\`mermaid
flowchart TB
    subgraph Git["Git Repository"]
        SECRETS[application.secrets.env<br/>application.secrets.yaml]
        PROPS[application.properties]
        CROSSPLANE[Crossplane<br/>Manifests]
    end
    
    subgraph GitOps["GitOps Tool"]
        FLUX[FluxCD<br/>GitRepository]
        ARGO[ArgoCD<br/>Application]
    end
    
    subgraph K8s["Kubernetes Cluster"]
        CRD[SecretManagerConfig<br/>CRD]
        CONTROLLER[Secret Manager<br/>Controller]
        CROSSPLANE_OP[Crossplane<br/>Operator]
    end
    
    subgraph Cloud["Cloud Providers"]
        GCP_SEC[GCP Secret<br/>Manager]
        AWS_SEC[AWS Secrets<br/>Manager]
        AZURE_KV[Azure Key<br/>Vault]
        GCP_PARAM[GCP Parameter<br/>Manager]
        AWS_PARAM[AWS Parameter<br/>Store]
        AZURE_APP[Azure App<br/>Configuration]
    end
    
    subgraph Serverless["Serverless Workloads"]
        CLOUDRUN[CloudRun<br/>Services]
        LAMBDA[Lambda<br/>Functions]
        FUNCTIONS[Azure<br/>Functions]
    end
    
    subgraph K8s_Workloads["Kubernetes Workloads"]
        PODS[Pods<br/>Deployments]
        ESO[External Secrets<br/>Operator]
    end
    
    SECRETS --> FLUX
    PROPS --> FLUX
    CROSSPLANE --> FLUX
    
    FLUX --> CRD
    ARGO --> CRD
    
    CRD --> CONTROLLER
    CONTROLLER --> GCP_SEC
    CONTROLLER --> AWS_SEC
    CONTROLLER --> AZURE_KV
    CONTROLLER --> GCP_PARAM
    CONTROLLER --> AWS_PARAM
    CONTROLLER --> AZURE_APP
    
    FLUX --> CROSSPLANE_OP
    CROSSPLANE_OP --> CLOUDRUN
    CROSSPLANE_OP --> LAMBDA
    CROSSPLANE_OP --> FUNCTIONS
    
    GCP_SEC --> CLOUDRUN
    GCP_PARAM --> CLOUDRUN
    AWS_SEC --> LAMBDA
    AWS_PARAM --> LAMBDA
    AZURE_KV --> FUNCTIONS
    AZURE_APP --> FUNCTIONS
    
    GCP_SEC --> ESO
    AWS_SEC --> ESO
    AZURE_KV --> ESO
    ESO --> PODS
    
    style CONTROLLER fill:#4a90e2,stroke:#2c5aa0,stroke-width:2px,color:#fff
    style CROSSPLANE_OP fill:#ae3ec9,stroke:#862e9c,stroke-width:2px,color:#fff
    style GCP_SEC fill:#ffd43b,stroke:#f59f00,stroke-width:2px,color:#000
    style AWS_SEC fill:#ffd43b,stroke:#f59f00,stroke-width:2px,color:#000
    style AZURE_KV fill:#ffd43b,stroke:#f59f00,stroke-width:2px,color:#000
    style CLOUDRUN fill:#51cf66,stroke:#2f9e44,stroke-width:2px,color:#fff
    style LAMBDA fill:#51cf66,stroke:#2f9e44,stroke-width:2px,color:#fff
    style FUNCTIONS fill:#51cf66,stroke:#2f9e44,stroke-width:2px,color:#fff
\`\`\`

## Step-by-Step Flow

### 1. Git Repository Setup

Secrets and configuration are stored in Git:

\`\`\`bash
my-service/
└── deployment-configuration/
    └── profiles/
        └── prod/
            ├── application.secrets.env    # Secrets
            ├── application.secrets.yaml   # Secrets (alternative)
            └── application.properties     # Config values
\`\`\`

**Crossplane Manifests** define serverless resources:

\`\`\`yaml
# cloudrun-service.yaml
apiVersion: cloudrun.gcp.upbound.io/v1beta1
kind: Service
metadata:
  name: my-service
spec:
  forProvider:
    template:
      spec:
        containers:
        - image: gcr.io/my-project/my-service:latest
          env:
          - name: DATABASE_URL
            valueFrom:
              secretKeyRef:
                name: my-service-database-url
                key: latest
\`\`\`

### 2. FluxCD Applies Resources

FluxCD syncs the Git repository and applies resources:

\`\`\`mermaid
sequenceDiagram
    participant Git as Git Repository
    participant Flux as FluxCD
    participant K8s as Kubernetes API
    participant Crossplane as Crossplane Operator
    participant Controller as Secret Manager Controller
    
    Git->>Flux: Repository updated
    Flux->>K8s: Apply SecretManagerConfig
    Flux->>K8s: Apply Crossplane Service
    K8s->>Controller: Watch event: CRD created
    K8s->>Crossplane: Watch event: Service created
\`\`\`

**FluxCD GitRepository:**

\`\`\`yaml
apiVersion: source.toolkit.fluxcd.io/v1beta2
kind: GitRepository
metadata:
  name: my-service-repo
  namespace: flux-system
spec:
  url: https://github.com/my-org/my-service
  ref:
    branch: main
  interval: 1m
\`\`\`

**SecretManagerConfig:**

\`\`\`yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: my-service-secrets
  namespace: microscaler-system
spec:
  sourceRef:
    kind: GitRepository
    name: my-service-repo
    namespace: flux-system
  provider:
    type: gcp
    gcp:
      region: us-central1
  secrets:
    environment: prod
    kustomizePath: deployment-configuration/profiles/prod
    prefix: my-service
\`\`\`

### 3. Controller Syncs Secrets

The controller reconciles secrets:

\`\`\`mermaid
sequenceDiagram
    participant Controller as Secret Manager Controller
    participant Flux as FluxCD Source
    participant SOPS as SOPS Decryption
    participant Parser as File Parser
    participant GCP as GCP Secret Manager
    
    Controller->>Flux: Get artifact path
    Flux-->>Controller: /tmp/flux-source-*/deployment-configuration/profiles/prod
    
    Controller->>Parser: Find application files
    Parser-->>Controller: application.secrets.env found
    
    Controller->>SOPS: Decrypt if encrypted
    SOPS-->>Controller: Decrypted content
    
    Controller->>Parser: Parse secrets
    Parser-->>Controller: {DATABASE_URL: "postgresql://..."}
    
    loop For each secret
        Controller->>GCP: Create/update secret
        Note over Controller,GCP: Secret name: my-service-database-url
        GCP-->>Controller: Success
    end
\`\`\`

**Secret Storage:**

- **Secrets** (\`application.secrets.*\`) → Secret Manager
- **Configs** (\`application.properties\`) → Parameter Manager (if enabled)

### 4. Crossplane Provisions Serverless Resources

Crossplane applies serverless resource manifests:

\`\`\`mermaid
sequenceDiagram
    participant Flux as FluxCD
    participant Crossplane as Crossplane Operator
    participant GCP as GCP CloudRun API
    participant AWS as AWS Lambda API
    participant Azure as Azure Functions API
    
    Flux->>Crossplane: Apply CloudRun Service manifest
    Crossplane->>GCP: Create CloudRun Service
    GCP-->>Crossplane: Service created
    Crossplane->>Crossplane: Update status
    
    Flux->>Crossplane: Apply Lambda Function manifest
    Crossplane->>AWS: Create Lambda Function
    AWS-->>Crossplane: Function created
    
    Flux->>Crossplane: Apply Azure Function manifest
    Crossplane->>Azure: Create Azure Function
    Azure-->>Crossplane: Function created
\`\`\`

**Crossplane Service References Secrets:**

\`\`\`yaml
apiVersion: cloudrun.gcp.upbound.io/v1beta1
kind: Service
metadata:
  name: my-service
spec:
  forProvider:
    template:
      spec:
        containers:
        - image: gcr.io/my-project/my-service:latest
          env:
          # Reference secret from Secret Manager
          - name: DATABASE_URL
            valueFrom:
              secretKeyRef:
                name: my-service-database-url  # Matches controller prefix
                key: latest
          # Reference config from Parameter Manager
          - name: API_TIMEOUT
            value: "30s"  # From application.properties
\`\`\`

### 5. Serverless Consumption

Serverless workloads consume secrets at runtime:

#### GCP CloudRun

\`\`\`mermaid
flowchart LR
    A[CloudRun Service] -->|secretKeyRef| B[GCP Secret Manager]
    A -->|Environment Variables| C[GCP Parameter Manager]
    B -->|Runtime| D[Application Code]
    C -->|Runtime| D
\`\`\`

**Consumption Methods:**

1. **Secret Manager** (via \`secretKeyRef\`):
   \`\`\`yaml
   env:
   - name: DATABASE_URL
     valueFrom:
       secretKeyRef:
         name: my-service-database-url
         key: latest
   \`\`\`

2. **Parameter Manager** (via environment variables):
   \`\`\`yaml
   env:
   - name: API_TIMEOUT
     value: "30s"  # From Parameter Manager
   \`\`\`

3. **SDK Access** (programmatic):
   \`\`\`python
   from google.cloud import secretmanager
   
   client = secretmanager.SecretManagerServiceClient()
   name = f"projects/{project_id}/secrets/my-service-database-url/versions/latest"
   response = client.access_secret_version(request={"name": name})
   secret_value = response.payload.data.decode("UTF-8")
   \`\`\`

#### AWS Lambda

\`\`\`mermaid
flowchart LR
    A[Lambda Function] -->|Lambda Extension| B[AWS Secrets Manager]
    A -->|Lambda Extension| C[AWS Parameter Store]
    B -->|Runtime| D[Application Code]
    C -->|Runtime| D
\`\`\`

**Consumption Methods:**

1. **Lambda Extension** (automatic caching):
   \`\`\`yaml
   # Lambda function configuration
   environment:
     SECRETS_EXTENSION_HTTP_PORT: 2773
   \`\`\`
   
   \`\`\`python
   import requests
   
   # Access via Lambda extension
   response = requests.get(
       f"http://localhost:2773/secretsmanager/get?secretId=my-service-database-url"
   )
   secret = response.json()["SecretString"]
   \`\`\`

2. **SDK Access**:
   \`\`\`python
   import boto3
   
   client = boto3.client('secretsmanager')
   response = client.get_secret_value(SecretId='my-service-database-url')
   secret = response['SecretString']
   \`\`\`

3. **Parameter Store** (for configs):
   \`\`\`python
   import boto3
   
   ssm = boto3.client('ssm')
   response = ssm.get_parameter(
       Name='/my-service/prod/api-timeout',
       WithDecryption=False
   )
   value = response['Parameter']['Value']
   \`\`\`

#### Azure Functions

\`\`\`mermaid
flowchart LR
    A[Azure Function] -->|Key Vault Reference| B[Azure Key Vault]
    A -->|App Config SDK| C[Azure App Configuration]
    B -->|Runtime| D[Application Code]
    C -->|Runtime| D
\`\`\`

**Consumption Methods:**

1. **Key Vault References** (in app settings):
   \`\`\`json
   {
     "DATABASE_URL": "@Microsoft.KeyVault(SecretUri=https://my-vault.vault.azure.net/secrets/my-service-database-url/)"
   }
   \`\`\`

2. **App Configuration SDK**:
   \`\`\`python
   from azure.appconfiguration import AzureAppConfigurationClient
   
   client = AzureAppConfigurationClient.from_connection_string(connection_string)
   setting = client.get_configuration_setting(key="api-timeout")
   value = setting.value
   \`\`\`

## Provider-Specific Details

### GCP (CloudRun)

| Component | Service | Purpose |
|-----------|---------|---------|
| **Secret Store** | Secret Manager | Secrets (passwords, API keys) |
| **Config Store** | Parameter Manager | Non-secret configs (timeouts, URLs) |
| **Consumption** | \`secretKeyRef\` | Native CloudRun integration |

**Example Flow:**

1. Controller syncs \`application.secrets.env\` → Secret Manager
2. Controller syncs \`application.properties\` → Parameter Manager (if \`configs.enabled=true\`)
3. Crossplane creates CloudRun Service with \`secretKeyRef\`
4. CloudRun injects secrets at runtime

### AWS (Lambda)

| Component | Service | Purpose |
|-----------|---------|---------|
| **Secret Store** | Secrets Manager | Secrets (passwords, API keys) |
| **Config Store** | Parameter Store | Non-secret configs (timeouts, URLs) |
| **Consumption** | Lambda Extension | Automatic caching and injection |

**Example Flow:**

1. Controller syncs \`application.secrets.env\` → Secrets Manager
2. Controller syncs \`application.properties\` → Parameter Store (if \`configs.enabled=true\`)
3. Crossplane creates Lambda Function
4. Lambda Extension caches secrets/parameters
5. Application accesses via extension or SDK

### Azure (Functions)

| Component | Service | Purpose |
|-----------|---------|---------|
| **Secret Store** | Key Vault | Secrets (passwords, API keys) |
| **Config Store** | App Configuration | Non-secret configs (timeouts, URLs) |
| **Consumption** | Key Vault References | Native Azure integration |

**Example Flow:**

1. Controller syncs \`application.secrets.env\` → Key Vault
2. Controller syncs \`application.properties\` → App Configuration (if \`configs.enabled=true\`)
3. Crossplane creates Azure Function
4. Function references Key Vault secrets in app settings
5. Function accesses configs via App Configuration SDK

## Complete Example: GCP CloudRun

### 1. Git Repository Structure

\`\`\`
my-service/
├── deployment-configuration/
│   └── profiles/
│       └── prod/
│           ├── application.secrets.env
│           └── application.properties
└── infrastructure/
    └── cloudrun-service.yaml
\`\`\`

**application.secrets.env:**
\`\`\`bash
DATABASE_URL=postgresql://user:password@host:5432/db
API_KEY=sk_live_1234567890
\`\`\`

**application.properties:**
\`\`\`properties
api.timeout=30s
api.retries=3
database.pool.size=10
\`\`\`

**cloudrun-service.yaml:**
\`\`\`yaml
apiVersion: cloudrun.gcp.upbound.io/v1beta1
kind: Service
metadata:
  name: my-service
spec:
  forProvider:
    location: us-central1
    template:
      spec:
        containers:
        - image: gcr.io/my-project/my-service:latest
          env:
          # Secret from Secret Manager
          - name: DATABASE_URL
            valueFrom:
              secretKeyRef:
                name: my-service-database-url
                key: latest
          - name: API_KEY
            valueFrom:
              secretKeyRef:
                name: my-service-api-key
                key: latest
          # Config from Parameter Manager (or environment variables)
          - name: API_TIMEOUT
            value: "30s"
          - name: API_RETRIES
            value: "3"
\`\`\`

### 2. FluxCD Configuration

**GitRepository:**
\`\`\`yaml
apiVersion: source.toolkit.fluxcd.io/v1beta2
kind: GitRepository
metadata:
  name: my-service-repo
  namespace: flux-system
spec:
  url: https://github.com/my-org/my-service
  ref:
    branch: main
  interval: 1m
\`\`\`

**SecretManagerConfig:**
\`\`\`yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: my-service-secrets
  namespace: microscaler-system
spec:
  sourceRef:
    kind: GitRepository
    name: my-service-repo
    namespace: flux-system
  provider:
    type: gcp
    gcp:
      region: us-central1
  secrets:
    environment: prod
    kustomizePath: deployment-configuration/profiles/prod
    prefix: my-service
  configs:
    enabled: true  # Route properties to Parameter Manager
\`\`\`

**Kustomization (applies Crossplane manifests):**
\`\`\`yaml
apiVersion: kustomize.toolkit.fluxcd.io/v1
kind: Kustomization
metadata:
  name: my-service-infra
  namespace: flux-system
spec:
  interval: 5m
  path: ./infrastructure
  sourceRef:
    kind: GitRepository
    name: my-service-repo
  prune: true
\`\`\`

### 3. Controller Sync Process

1. **Controller watches** \`SecretManagerConfig\`
2. **Reads** Git repository via FluxCD artifact
3. **Decrypts** SOPS-encrypted files (if applicable)
4. **Parses** \`application.secrets.env\` and \`application.properties\`
5. **Syncs secrets** to GCP Secret Manager:
   - \`my-service-database-url\` = \`postgresql://user:password@host:5432/db\`
   - \`my-service-api-key\` = \`sk_live_1234567890\`
6. **Syncs configs** to GCP Parameter Manager:
   - \`my-service-prod/api.timeout\` = \`30s\`
   - \`my-service-prod/api.retries\` = \`3\`
   - \`my-service-prod/database.pool.size\` = \`10\`

### 4. Crossplane Provisioning

1. **FluxCD applies** \`cloudrun-service.yaml\`
2. **Crossplane operator** reconciles the Service resource
3. **Crossplane creates** CloudRun Service in GCP
4. **CloudRun Service** references secrets via \`secretKeyRef\`:
   - \`my-service-database-url\` → Injected as \`DATABASE_URL\`
   - \`my-service-api-key\` → Injected as \`API_KEY\`

### 5. Runtime Consumption

**Application code** (no changes needed):

\`\`\`python
import os

# Secrets from Secret Manager (injected via secretKeyRef)
database_url = os.environ["DATABASE_URL"]
api_key = os.environ["API_KEY"]

# Configs from Parameter Manager (injected as env vars)
api_timeout = os.environ["API_TIMEOUT"]  # "30s"
api_retries = int(os.environ["API_RETRIES"])  # 3
\`\`\`

## Key Benefits

1. **GitOps-Driven**: All secrets and configs in Git
2. **Automatic Sync**: Controller syncs to cloud providers
3. **Serverless-Native**: Works with CloudRun, Lambda, Functions
4. **Crossplane Integration**: Infrastructure as Code for serverless
5. **Separation of Concerns**: Secrets vs configs routed appropriately
6. **No Manual Steps**: Fully automated from Git to runtime

## Comparison: Kubernetes vs Serverless

| Aspect | Kubernetes | Serverless |
|--------|------------|------------|
| **Secret Consumption** | External Secrets Operator → ConfigMaps/Secrets | Native cloud provider integration |
| **Config Consumption** | ConfigMaps (native) or Config Store operators | Config Store SDKs or environment variables |
| **Infrastructure** | Kubernetes manifests | Crossplane manifests |
| **Deployment** | \`kubectl apply\` or GitOps | Crossplane operator |
| **Runtime Access** | Volume mounts or env vars | Environment variables or SDK calls |

## Next Steps

- [Application Files Guide](../guides/application-files.md) - Learn about file formats
- [GitOps Integration](../guides/gitops-integration.md) - Set up GitOps workflow
- [Configuration Reference](../getting-started/configuration.md) - Complete configuration guide
- [Provider Setup](../guides/aws-setup.md) - Provider-specific setup

`,O=Object.freeze(Object.defineProperty({__proto__:null,default:i},Symbol.toStringTag,{value:"Module"})),c=`# Configuration

Complete guide to configuring the Secret Manager Controller.

## SecretManagerConfig Spec

The \`SecretManagerConfig\` CRD is the main configuration resource. Here's the complete spec:

\`\`\`yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: my-config
  namespace: default
spec:
  # Source reference (required)
  sourceRef:
    kind: GitRepository  # or Application for ArgoCD
    name: my-repo
    namespace: microscaler-system
  
  # Provider configuration (required)
  provider:
    gcp:
      projectId: my-project
    # OR
    aws:
      region: us-east-1
    # OR
    azure:
      vaultUrl: https://my-vault.vault.azure.net/
  
  # Secrets configuration (required)
  secrets:
    environment: dev
    kustomizePath: path/to/kustomize/overlay
    sops:
      enabled: true
      gpgSecretRef:
        name: sops-gpg-key
        namespace: microscaler-system
        key: private.key
  
  # Config store configuration (optional)
  configs:
    enabled: true
    parameterPath: /my-service/dev  # AWS only
    appConfigEndpoint: https://my-app-config.azconfig.io  # Azure only
    store: SecretManager  # GCP: SecretManager or ParameterManager
  
  # OpenTelemetry configuration (optional)
  otel:
    exporter: otlp
    endpoint: http://otel-collector:4317
    serviceName: secret-manager-controller
  
  # Timing configuration (optional)
  gitRepositoryPullInterval: 5m  # Default: 5m
  reconcileInterval: 1m  # Default: 1m
  
  # Feature flags (optional)
  diffDiscovery: true  # Default: false
\`\`\`

## Source Reference

The \`sourceRef\` field references your GitOps source. It supports:

### FluxCD GitRepository

\`\`\`yaml
sourceRef:
  kind: GitRepository
  name: my-repo
  namespace: microscaler-system
\`\`\`

### ArgoCD Application

\`\`\`yaml
sourceRef:
  kind: Application
  name: my-app
  namespace: argocd
\`\`\`

## Provider Configuration

### GCP Secret Manager

\`\`\`yaml
provider:
  gcp:
    projectId: my-gcp-project
\`\`\`

**Authentication:**
- Uses Workload Identity by default (recommended)
- Or service account key via Kubernetes Secret

### AWS Secrets Manager

\`\`\`yaml
provider:
  aws:
    region: us-east-1
\`\`\`

**Authentication:**
- Uses IRSA (IAM Roles for Service Accounts) by default (recommended)
- Or access keys via Kubernetes Secret

### Azure Key Vault

\`\`\`yaml
provider:
  azure:
    vaultUrl: https://my-vault.vault.azure.net/
\`\`\`

**Authentication:**
- Uses Workload Identity by default (recommended)
- Or service principal via Kubernetes Secret

## Secrets Configuration

### Basic Configuration

\`\`\`yaml
secrets:
  environment: dev
  kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
\`\`\`

- \`environment\`: Environment name (e.g., \`dev\`, \`staging\`, \`prod\`)
- \`kustomizePath\`: Path to Kustomize overlay in Git repository

### SOPS Decryption

Enable SOPS decryption:

\`\`\`yaml
secrets:
  sops:
    enabled: true
    gpgSecretRef:
      name: sops-gpg-key
      namespace: microscaler-system
      key: private.key
\`\`\`

**GPG Key Secret Format:**

\`\`\`yaml
apiVersion: v1
kind: Secret
metadata:
  name: sops-gpg-key
  namespace: microscaler-system
type: Opaque
data:
  private.key: <base64-encoded-gpg-private-key>
\`\`\`

## Config Store Configuration

Route \`application.properties\` files to config stores instead of secret stores:

\`\`\`yaml
configs:
  enabled: true
  # AWS: Parameter Store path prefix
  parameterPath: /my-service/dev
  # Azure: App Configuration endpoint
  appConfigEndpoint: https://my-app-config.azconfig.io
  # GCP: Store type (SecretManager or ParameterManager)
  store: SecretManager
\`\`\`

## OpenTelemetry Configuration

Enable distributed tracing:

\`\`\`yaml
otel:
  exporter: otlp  # or "datadog"
  endpoint: http://otel-collector:4317
  serviceName: secret-manager-controller
\`\`\`

**Supported exporters:**
- \`otlp\`: OpenTelemetry Protocol (default)
- \`datadog\`: Direct Datadog export

## Timing Configuration

### Git Repository Pull Interval

How often to check for updates from Git:

\`\`\`yaml
gitRepositoryPullInterval: 5m  # Default: 5m, minimum: 1m
\`\`\`

**Recommendation:** 5m or greater to avoid Git API rate limits.

### Reconcile Interval

How often to reconcile secrets between Git and cloud provider:

\`\`\`yaml
reconcileInterval: 1m  # Default: 1m
\`\`\`

## Feature Flags

### Diff Discovery

Detect if secrets have been tampered with in cloud provider:

\`\`\`yaml
diffDiscovery: true  # Default: false
\`\`\`

When enabled, logs warnings when differences are found between Git (source of truth) and cloud provider.

## Environment Variables

The controller can also be configured via environment variables:

- \`RUST_LOG\`: Log level (e.g., \`info\`, \`debug\`, \`trace\`)
- \`METRICS_PORT\`: Metrics server port (default: \`8080\`)
- \`HEALTH_PORT\`: Health check port (default: \`8081\`)

## Validation

The controller validates your configuration:

- **Required fields**: \`sourceRef\`, \`provider\`, \`secrets\`
- **Provider-specific**: Required fields vary by provider
- **Intervals**: Must be valid Kubernetes duration strings

Check validation errors:

\`\`\`bash
kubectl describe secretmanagerconfig my-config
\`\`\`

## Examples

See the [Examples](../tutorials/basic-usage.md) section for complete working examples.

## Next Steps

- [API Reference](../api-reference/crd-reference.md) - Complete CRD reference
- [Provider Setup Guides](../guides/aws-setup.md) - Detailed provider configuration
- [Tutorials](../tutorials/basic-usage.md) - Step-by-step tutorials
`,D=Object.freeze(Object.defineProperty({__proto__:null,default:c},Symbol.toStringTag,{value:"Module"})),l=`# Installation

This guide will help you install the Secret Manager Controller in your Kubernetes cluster.

## Prerequisites

- Kubernetes cluster (v1.20+)
- \`kubectl\` configured to access your cluster
- Helm 3.x (optional, for Helm installation)
- GitOps tool installed (FluxCD or ArgoCD) - see [GitOps Integration Guide](../guides/gitops-integration.md)

## Installation Methods

### Method 1: Using kubectl (Recommended)

1. **Apply the CRD:**

\`\`\`bash
kubectl apply -f https://raw.githubusercontent.com/microscaler/secret-manager-controller/main/config/crd/secretmanagerconfig.yaml
\`\`\`

2. **Apply the controller manifests:**

\`\`\`bash
kubectl apply -k https://github.com/microscaler/secret-manager-controller/config/
\`\`\`

This will create:
- The \`microscaler-system\` namespace
- ServiceAccount, Role, and RoleBinding for the controller
- Deployment for the controller

### Method 2: Using Helm

\`\`\`bash
# Add the Helm repository
helm repo add secret-manager-controller https://microscaler.github.io/secret-manager-controller
helm repo update

# Install the controller
helm install secret-manager-controller secret-manager-controller/secret-manager-controller
\`\`\`

## Verify Installation

Check that the controller is running:

\`\`\`bash
kubectl get pods -n microscaler-system
\`\`\`

You should see the \`secret-manager-controller\` pod in \`Running\` state:

\`\`\`
NAME                                      READY   STATUS    RESTARTS   AGE
secret-manager-controller-xxxxxxxxxx-xxx  1/1     Running   0          1m
\`\`\`

Check the controller logs:

\`\`\`bash
kubectl logs -n microscaler-system -l app=secret-manager-controller --tail=50
\`\`\`

## Cloud Provider Setup

Before using the controller, you'll need to configure authentication for your cloud provider:

- **GCP**: Set up [Workload Identity](https://cloud.google.com/kubernetes-engine/docs/how-to/workload-identity) or service account key
- **AWS**: Configure [IRSA (IAM Roles for Service Accounts)](https://docs.aws.amazon.com/eks/latest/userguide/iam-roles-for-service-accounts.html) or access keys
- **Azure**: Set up [Workload Identity](https://learn.microsoft.com/en-us/azure/aks/workload-identity) or service principal

See the provider-specific setup guides:
- [AWS Setup Guide](../guides/aws-setup.md)
- [Azure Setup Guide](../guides/azure-setup.md)
- [GCP Setup Guide](../guides/gcp-setup.md)

## Next Steps

- [Quick Start Guide](./quick-start.md) - Get up and running in minutes
- [Configuration](./configuration.md) - Learn about configuration options
- [Architecture Overview](../architecture/overview.md) - Understand how it works
`,w=Object.freeze(Object.defineProperty({__proto__:null,default:l},Symbol.toStringTag,{value:"Module"})),p=`# Quick Start

Get the Secret Manager Controller up and running in minutes with this quick start guide.

## Prerequisites

- Kubernetes cluster with kubectl access
- GitOps tool (FluxCD or ArgoCD) installed
- Cloud provider credentials configured (see [Installation](./installation.md))

## Step 1: Create a GitRepository (FluxCD)

If you're using FluxCD, create a GitRepository resource:

\`\`\`yaml
apiVersion: source.toolkit.fluxcd.io/v1
kind: GitRepository
metadata:
  name: my-secrets-repo
  namespace: microscaler-system
spec:
  url: https://github.com/your-org/your-secrets-repo
  interval: 5m
  ref:
    branch: main
\`\`\`

Apply it:

\`\`\`bash
kubectl apply -f gitrepository.yaml
\`\`\`

## Step 2: Create a SecretManagerConfig

Create a \`SecretManagerConfig\` resource that references your GitRepository:

\`\`\`yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: my-service-secrets
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-secrets-repo
    namespace: microscaler-system
  provider:
    gcp:
      projectId: my-gcp-project
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
\`\`\`

**Key fields:**
- \`sourceRef\`: References your GitRepository or ArgoCD Application
- \`provider\`: Your cloud provider configuration (GCP, AWS, or Azure)
- \`secrets.environment\`: The environment name (e.g., \`dev\`, \`staging\`, \`prod\`)
- \`secrets.kustomizePath\`: Path to your Kustomize overlay in the Git repository

## Step 3: Apply the Configuration

\`\`\`bash
kubectl apply -f secretmanagerconfig.yaml
\`\`\`

## Step 4: Verify Sync

Check the status of your SecretManagerConfig:

\`\`\`bash
kubectl get secretmanagerconfig my-service-secrets -n default
\`\`\`

You should see output like:

\`\`\`
NAME                  PHASE      DESCRIPTION                    READY
my-service-secrets    Synced    Successfully synced 5 secrets  True
\`\`\`

Check the detailed status:

\`\`\`bash
kubectl describe secretmanagerconfig my-service-secrets -n default
\`\`\`

## Step 5: Verify Secrets in Cloud Provider

### GCP Secret Manager

\`\`\`bash
gcloud secrets list --project=my-gcp-project
\`\`\`

### AWS Secrets Manager

\`\`\`bash
aws secretsmanager list-secrets --region us-east-1
\`\`\`

### Azure Key Vault

\`\`\`bash
az keyvault secret list --vault-name my-vault
\`\`\`

## Example: SOPS-Encrypted Secrets

If your secrets are encrypted with SOPS, the controller will automatically decrypt them. Make sure you have:

1. **SOPS-encrypted files** in your Git repository
2. **GPG private key** stored in a Kubernetes Secret:

\`\`\`yaml
apiVersion: v1
kind: Secret
metadata:
  name: sops-gpg-key
  namespace: microscaler-system
type: Opaque
data:
  private.key: <base64-encoded-gpg-private-key>
\`\`\`

3. **Reference the key** in your SecretManagerConfig:

\`\`\`yaml
spec:
  secrets:
    sops:
      enabled: true
      gpgSecretRef:
        name: sops-gpg-key
        namespace: microscaler-system
        key: private.key
\`\`\`

## Troubleshooting

### Controller Not Syncing

Check the controller logs:

\`\`\`bash
kubectl logs -n microscaler-system -l app=secret-manager-controller --tail=100
\`\`\`

Common issues:
- **GitRepository not found**: Verify the \`sourceRef\` name and namespace
- **Authentication errors**: Check cloud provider credentials
- **SOPS decryption failures**: Verify GPG key is correct

### Secrets Not Appearing in Cloud Provider

1. Check the SecretManagerConfig status for errors
2. Verify the \`kustomizePath\` is correct
3. Ensure secrets are properly formatted in your Git repository

## Next Steps

- [Configuration Guide](./configuration.md) - Learn about all configuration options
- [Provider Setup Guides](../guides/aws-setup.md) - Detailed provider configuration
- [Architecture Overview](../architecture/overview.md) - Understand the system architecture
`,M=Object.freeze(Object.defineProperty({__proto__:null,default:p},Symbol.toStringTag,{value:"Module"})),u=`# Application Files Guide

Complete guide to the application file formats supported by the Secret Manager Controller.

## Supported File Types

The controller processes three types of application files:

1. **\`application.secrets.env\`** - Environment variable format for secrets
2. **\`application.secrets.yaml\`** - YAML format for secrets
3. **\`application.properties\`** - Java properties format for configuration

## File Discovery

The controller automatically discovers these files in your Git repository based on the \`kustomizePath\` or \`basePath\` configuration:

### With Kustomize Path

When \`kustomizePath\` is specified, the controller:
1. Runs \`kustomize build\` on the specified path
2. Extracts Kubernetes Secret resources from the generated YAML
3. Uses the \`data\` or \`stringData\` fields as secrets

### Without Kustomize Path (Raw File Mode)

When \`kustomizePath\` is not specified, the controller searches for files in this order:

\`\`\`
{basePath}/{service}/deployment-configuration/profiles/{environment}/
  ├── application.secrets.env
  ├── application.secrets.yaml
  └── application.properties
\`\`\`

Or for single-service repositories:

\`\`\`
deployment-configuration/profiles/{environment}/
  ├── application.secrets.env
  ├── application.secrets.yaml
  └── application.properties
\`\`\`

## application.secrets.env

Environment variable format for secrets. This is the simplest format for key-value pairs.

### Format

\`\`\`bash
# Enabled secrets
DATABASE_PASSWORD=super-secret-password
API_KEY=sk_live_1234567890
JWT_SECRET=my-jwt-secret-key

# Disabled secrets (commented out)
#OLD_API_KEY=deprecated-key
#LEGACY_PASSWORD=old-password
\`\`\`

### Features

- **Simple key-value pairs**: \`KEY=value\` format
- **Comment support**: Lines starting with \`#\` are treated as disabled secrets
- **SOPS encryption**: Can be encrypted with SOPS (GPG or AGE keys)
- **Merging**: If both \`.env\` and \`.yaml\` files exist, \`.yaml\` values override \`.env\` values

### Disabled Secrets

Commented lines (starting with \`#\`) are parsed but marked as disabled:
- **Enabled secrets**: Synced to cloud provider secret manager
- **Disabled secrets**: Disabled in cloud provider (but not deleted) - useful for secret rotation

**Example:**
\`\`\`bash
# Active secret
DATABASE_PASSWORD=new-password

# Disabled (old secret being rotated out)
#DATABASE_PASSWORD=old-password
\`\`\`

The controller will:
1. Create/update \`DATABASE_PASSWORD\` with value \`new-password\`
2. Disable (but not delete) any existing \`DATABASE_PASSWORD\` with value \`old-password\`

### SOPS Encryption

Encrypt with SOPS:

\`\`\`bash
# Encrypt the file
sops -e -i application.secrets.env

# Verify encryption
sops -d application.secrets.env
\`\`\`

The encrypted file will have SOPS metadata embedded:

\`\`\`yaml
DATABASE_PASSWORD: ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]
sops:
  kms: []
  gcp_kms: []
  azure_kv: []
  hc_vault: []
  age:
    - recipient: age1...
      enc: |
        ...
  lastmodified: "2024-01-15T10:30:00Z"
  mac: ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]
  pgp: []
  encrypted_regex: ^(data|stringData|DATABASE_PASSWORD|API_KEY)
  version: 3.8.1
\`\`\`

## application.secrets.yaml

YAML format for secrets. Supports nested structures that are automatically flattened.

### Format

\`\`\`yaml
database:
  password: super-secret-password
  username: admin

api:
  key: sk_live_1234567890
  secret: api-secret-key

jwt:
  secret: my-jwt-secret-key
\`\`\`

### Features

- **Nested structures**: Supports hierarchical YAML
- **Automatic flattening**: Nested keys are flattened with dot notation
- **SOPS encryption**: Can be encrypted with SOPS (GPG or AGE keys)
- **Merging**: If both \`.env\` and \`.yaml\` files exist, \`.yaml\` values override \`.env\` values

### Flattening

Nested YAML structures are automatically flattened:

**Input:**
\`\`\`yaml
database:
  connection:
    password: secret123
    host: db.example.com
\`\`\`

**Flattened to:**
\`\`\`
database.connection.password = secret123
database.connection.host = db.example.com
\`\`\`

### SOPS Encryption

Encrypt with SOPS:

\`\`\`bash
# Encrypt the file
sops -e -i application.secrets.yaml

# Verify encryption
sops -d application.secrets.yaml
\`\`\`

The encrypted file will have SOPS metadata and encrypted values:

\`\`\`yaml
database:
  password: ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]
  username: ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]
sops:
  # ... SOPS metadata ...
\`\`\`

## application.properties

Java properties format for configuration values. These are routed to config stores (when enabled) instead of secret stores.

### Format

\`\`\`properties
# Database configuration
database.host=db.example.com
database.port=5432
database.name=myapp

# API configuration
api.timeout=30s
api.retries=3

# Feature flags
feature.new-ui.enabled=true
feature.analytics.enabled=false
\`\`\`

### Features

- **Key-value pairs**: \`key=value\` format
- **Comment support**: Lines starting with \`#\` are ignored
- **Config store routing**: When \`configs.enabled=true\`, properties are stored in config stores (not secret stores)
- **Individual storage**: Each property is stored as a separate parameter in config stores

### Routing Behavior

#### When \`configs.enabled=false\` (Default)

Properties are stored as a JSON blob in the secret store:

\`\`\`json
{
  "database.host": "db.example.com",
  "database.port": "5432",
  "api.timeout": "30s"
}
\`\`\`

Stored as secret: \`{prefix}-properties-{suffix}\`

#### When \`configs.enabled=true\`

Properties are stored individually in config stores:

- **AWS**: Parameter Store at \`/my-service/dev/database.host\`, \`/my-service/dev/database.port\`, etc.
- **GCP**: Secret Manager (interim) or Parameter Manager (future) as individual secrets
- **Azure**: App Configuration as individual configuration keys

### SOPS Encryption

Properties files can also be encrypted with SOPS:

\`\`\`bash
# Encrypt the file
sops -e -i application.properties

# Verify encryption
sops -d application.properties
\`\`\`

## File Processing Order

When multiple files are present, they are processed in this order:

1. **\`application.secrets.env\`** - Parsed first
2. **\`application.secrets.yaml\`** - Parsed second (overrides \`.env\` values)
3. **\`application.properties\`** - Parsed separately (routed to config stores)

**Merging behavior:**
- If a key exists in both \`.env\` and \`.yaml\`, the \`.yaml\` value takes precedence
- Properties are always processed separately (never merged with secrets)

## SOPS Encryption

All three file types support SOPS encryption with both GPG and AGE keys.

### GPG Encryption

See [SOPS Setup Guide](./sops-setup.md) for GPG key setup.

### AGE Encryption

AGE (Actually Good Encryption) is a modern alternative to GPG.

#### Generate AGE Key

\`\`\`bash
# Generate a new AGE key pair
age-keygen -o age-key.txt
\`\`\`

This creates:
- **Public key**: \`age1...\` (share this)
- **Private key**: \`AGE-SECRET-KEY-1...\` (keep this secret)

#### Configure SOPS for AGE

Create or update \`.sops.yaml\`:

\`\`\`yaml
creation_rules:
  - path_regex: .*\\.secrets\\.(env|yaml)$
    encrypted_regex: ^(data|stringData|DATABASE_|API_|JWT_)
    age: >-
      age1abc123def456...
      age1xyz789uvw012...
\`\`\`

#### Encrypt Files

\`\`\`bash
# Encrypt with AGE
sops -e -i application.secrets.env

# Verify
sops -d application.secrets.env
\`\`\`

#### Store AGE Key in Kubernetes

\`\`\`bash
# Export private key
cat age-key.txt | grep "AGE-SECRET-KEY" > /tmp/age-private-key.txt

# Create Kubernetes Secret
kubectl create secret generic sops-age-key \\
  --from-file=private.key=/tmp/age-private-key.txt \\
  -n microscaler-system

# Clean up
rm /tmp/age-private-key.txt
\`\`\`

#### Configure SecretManagerConfig

\`\`\`yaml
spec:
  secrets:
    sops:
      enabled: true
      ageSecretRef:
        name: sops-age-key
        namespace: microscaler-system
        key: private.key
\`\`\`

**Note:** The controller supports both GPG and AGE keys. You can use either or both.

## Controller Processing Flow

\`\`\`mermaid
flowchart TD
    A[Git Repository] -->|GitOps Sync| B[Controller Reads Files]
    B --> C{SOPS Encrypted?}
    C -->|Yes| D[Decrypt with GPG/AGE]
    C -->|No| E[Parse Files]
    D --> E
    E --> F{File Type?}
    F -->|.secrets.env| G[Parse ENV Format]
    F -->|.secrets.yaml| H[Parse YAML Format]
    F -->|.properties| I[Parse Properties Format]
    G --> J[Merge Secrets]
    H --> J
    J --> K[Sync to Secret Store]
    I --> L{Config Store Enabled?}
    L -->|Yes| M[Sync to Config Store]
    L -->|No| N[Sync to Secret Store as JSON]
    
    style D fill:#fff4e1
    style K fill:#e1f5ff
    style M fill:#e1f5ff
    style N fill:#e1f5ff
\`\`\`

## Examples

### Complete Example

**Repository structure:**
\`\`\`
my-service/
└── deployment-configuration/
    └── profiles/
        └── dev/
            ├── application.secrets.env
            ├── application.secrets.yaml
            └── application.properties
\`\`\`

**application.secrets.env:**
\`\`\`bash
DATABASE_PASSWORD=secret123
API_KEY=key-abc-123
\`\`\`

**application.secrets.yaml:**
\`\`\`yaml
database:
  username: admin
  password: yaml-override-password  # Overrides DATABASE_PASSWORD from .env
jwt:
  secret: jwt-secret-key
\`\`\`

**application.properties:**
\`\`\`properties
database.host=db.example.com
database.port=5432
api.timeout=30s
\`\`\`

**Result:**
- **Secrets synced:**
  - \`DATABASE_PASSWORD\` = \`yaml-override-password\` (from \`.yaml\`, overrides \`.env\`)
  - \`API_KEY\` = \`key-abc-123\` (from \`.env\`)
  - \`database.username\` = \`admin\` (from \`.yaml\`, flattened)
  - \`jwt.secret\` = \`jwt-secret-key\` (from \`.yaml\`, flattened)

- **Properties synced** (if \`configs.enabled=true\`):
  - \`/my-service/dev/database.host\` = \`db.example.com\`
  - \`/my-service/dev/database.port\` = \`5432\`
  - \`/my-service/dev/api.timeout\` = \`30s\`

### With SOPS Encryption

**Encrypted application.secrets.env:**
\`\`\`bash
DATABASE_PASSWORD: ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]
API_KEY: ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]
sops:
  age:
    - recipient: age1abc123...
      enc: |
        ...
  lastmodified: "2024-01-15T10:30:00Z"
  mac: ENC[AES256_GCM,data:...,iv:...,tag:...,type:str]
  encrypted_regex: ^(DATABASE_|API_)
  version: 3.8.1
\`\`\`

The controller will:
1. Detect SOPS encryption
2. Load AGE or GPG key from Kubernetes Secret
3. Decrypt the file
4. Parse the decrypted content
5. Sync secrets to cloud provider

## Best Practices

1. **Use SOPS encryption**: Always encrypt secrets in Git
2. **Separate secrets from configs**: Use \`.secrets.*\` for secrets, \`.properties\` for configs
3. **Use YAML for complex structures**: \`.yaml\` format for nested configurations
4. **Use ENV for simplicity**: \`.env\` format for flat key-value pairs
5. **Enable config stores**: Set \`configs.enabled=true\` to route properties to config stores
6. **Comment out disabled secrets**: Use \`#\` prefix to disable secrets without deleting them
7. **Version control**: All files should be in Git with proper encryption

## Troubleshooting

### Files Not Found

**Error:** \`No application files found\`

**Solutions:**
1. Check the \`kustomizePath\` or \`basePath\` is correct
2. Verify files exist in the Git repository
3. Check the \`environment\` matches the directory name

### Parsing Errors

**Error:** \`Failed to parse YAML\` or \`Invalid ENV format\`

**Solutions:**
1. Validate YAML syntax: \`yamllint application.secrets.yaml\`
2. Check ENV format: Each line should be \`KEY=value\`
3. Verify properties format: Each line should be \`key=value\`

### SOPS Decryption Fails

See [SOPS Setup Guide](./sops-setup.md) for troubleshooting SOPS issues.

## Next Steps

- [SOPS Setup](./sops-setup.md) - Set up SOPS encryption
- [Configuration Reference](../getting-started/configuration.md) - Complete configuration guide
- [Config Store Setup](../getting-started/configuration.md#config-store-configuration) - Enable config stores

`,T=Object.freeze(Object.defineProperty({__proto__:null,default:u},Symbol.toStringTag,{value:"Module"})),m=`# AWS Setup Guide

Configure the Secret Manager Controller to work with AWS Secrets Manager.

## Prerequisites

- AWS account with Secrets Manager access
- IAM user or role with appropriate permissions
- Kubernetes cluster with controller installed

## IAM Permissions

Your AWS credentials need the following permissions:

\`\`\`json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "secretsmanager:GetSecretValue",
        "secretsmanager:DescribeSecret",
        "secretsmanager:ListSecrets"
      ],
      "Resource": "*"
    }
  ]
}
\`\`\`

## Authentication Methods

### Method 1: IAM Role (Recommended)

If running on EKS, use an IAM role for the service account:

\`\`\`yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: secret-manager-controller
  namespace: microscaler-system
  annotations:
    eks.amazonaws.com/role-arn: arn:aws:iam::ACCOUNT_ID:role/SecretManagerRole
\`\`\`

### Method 2: Access Keys

Create a Kubernetes Secret with AWS credentials:

\`\`\`yaml
apiVersion: v1
kind: Secret
metadata:
  name: aws-credentials
  namespace: microscaler-system
type: Opaque
stringData:
  AWS_ACCESS_KEY_ID: your-access-key-id
  AWS_SECRET_ACCESS_KEY: your-secret-access-key
  AWS_REGION: us-east-1
\`\`\`

Reference in your SecretManagerConfig:

\`\`\`yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: aws-secrets
spec:
  provider: aws
  region: us-east-1
  credentials:
    secretRef:
      name: aws-credentials
      namespace: microscaler-system
  secrets:
    - name: database-password
      key: /myapp/database/password
\`\`\`

## Configuration Example

\`\`\`yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: production-secrets
  namespace: production
spec:
  provider: aws
  region: us-east-1
  secrets:
    - name: db-password
      key: /production/database/password
    - name: api-key
      key: /production/api/key
\`\`\`

## Troubleshooting

### Common Issues

1. **Authentication Failed**
   - Verify IAM permissions
   - Check credential configuration
   - Ensure region is correct

2. **Secret Not Found**
   - Verify secret exists in AWS Secrets Manager
   - Check secret key path
   - Verify IAM permissions include the secret

3. **Network Issues**
   - Check cluster network connectivity to AWS
   - Verify VPC endpoints if using private networking

## Next Steps

- [Azure Setup](./azure-setup.md)
- [GCP Setup](./gcp-setup.md)
- [GitOps Integration](./gitops-integration.md)

`,x=Object.freeze(Object.defineProperty({__proto__:null,default:m},Symbol.toStringTag,{value:"Module"})),d=`# Azure Setup Guide

Configure the Secret Manager Controller to work with Azure Key Vault.

## Prerequisites

- Azure subscription
- Azure Key Vault created
- Service principal or managed identity
- Kubernetes cluster with controller installed

## Authentication Methods

### Method 1: Managed Identity (Recommended)

If running on AKS, use managed identity:

\`\`\`yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: secret-manager-controller
  namespace: microscaler-system
  annotations:
    azure.workload.identity/client-id: <client-id>
\`\`\`

### Method 2: Service Principal

Create a Kubernetes Secret with Azure credentials:

\`\`\`yaml
apiVersion: v1
kind: Secret
metadata:
  name: azure-credentials
  namespace: microscaler-system
type: Opaque
stringData:
  AZURE_CLIENT_ID: <client-id>
  AZURE_CLIENT_SECRET: <client-secret>
  AZURE_TENANT_ID: <tenant-id>
\`\`\`

## Configuration Example

\`\`\`yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: azure-secrets
  namespace: production
spec:
  provider: azure
  vaultUrl: https://myvault.vault.azure.net/
  secrets:
    - name: db-password
      key: database-password
    - name: api-key
      key: api-key
\`\`\`

## Required Permissions

Your service principal needs:
- \`Key Vault Secrets User\` role
- Or \`Get\` and \`List\` permissions on secrets

## Troubleshooting

See [AWS Setup Guide](./aws-setup.md) for common troubleshooting steps.

`,_=Object.freeze(Object.defineProperty({__proto__:null,default:d},Symbol.toStringTag,{value:"Module"})),g=`# GCP Setup Guide

Configure the Secret Manager Controller to work with GCP Secret Manager.

## Prerequisites

- GCP project with Secret Manager API enabled
- Service account with appropriate permissions
- Kubernetes cluster with controller installed

## Authentication Methods

### Method 1: Workload Identity (Recommended)

If running on GKE, use Workload Identity:

\`\`\`yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: secret-manager-controller
  namespace: microscaler-system
  annotations:
    iam.gke.io/gcp-service-account: secret-manager@PROJECT_ID.iam.gserviceaccount.com
\`\`\`

### Method 2: Service Account Key

Create a Kubernetes Secret with GCP credentials:

\`\`\`yaml
apiVersion: v1
kind: Secret
metadata:
  name: gcp-credentials
  namespace: microscaler-system
type: Opaque
stringData:
  GOOGLE_APPLICATION_CREDENTIALS_JSON: |
    {
      "type": "service_account",
      "project_id": "...",
      ...
    }
\`\`\`

## Configuration Example

\`\`\`yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: gcp-secrets
  namespace: production
spec:
  provider: gcp
  project: my-gcp-project
  secrets:
    - name: db-password
      key: database-password
    - name: api-key
      key: api-key
\`\`\`

## Required Permissions

Your service account needs:
- \`Secret Manager Secret Accessor\` role
- Or \`secretmanager.secrets.get\` permission

## Troubleshooting

See [AWS Setup Guide](./aws-setup.md) for common troubleshooting steps.

`,I=Object.freeze(Object.defineProperty({__proto__:null,default:g},Symbol.toStringTag,{value:"Module"})),y=`# GitOps Integration

The Secret Manager Controller is GitOps-agnostic and works with both FluxCD and ArgoCD.

## Supported GitOps Tools

### FluxCD

The controller integrates with FluxCD's \`GitRepository\` CRD and source-controller.

**Requirements:**
- FluxCD source-controller installed
- \`GitRepository\` resource created
- Artifacts available in \`/tmp/flux-source-*\` directories

**Example GitRepository:**

\`\`\`yaml
apiVersion: source.toolkit.fluxcd.io/v1
kind: GitRepository
metadata:
  name: my-secrets-repo
  namespace: microscaler-system
spec:
  url: https://github.com/your-org/your-secrets-repo
  interval: 5m
  ref:
    branch: main
  secretRef:
    name: git-credentials  # Optional: for private repos
\`\`\`

**Reference in SecretManagerConfig:**

\`\`\`yaml
spec:
  sourceRef:
    kind: GitRepository
    name: my-secrets-repo
    namespace: microscaler-system
\`\`\`

### ArgoCD

The controller integrates with ArgoCD's \`Application\` CRD.

**Requirements:**
- ArgoCD installed
- \`Application\` resource created
- Repository accessible from controller

**Example Application:**

\`\`\`yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: my-secrets-app
  namespace: argocd
spec:
  project: default
  source:
    repoURL: https://github.com/your-org/your-secrets-repo
    targetRevision: main
    path: .
  destination:
    server: https://kubernetes.default.svc
    namespace: default
\`\`\`

**Reference in SecretManagerConfig:**

\`\`\`yaml
spec:
  sourceRef:
    kind: Application
    name: my-secrets-app
    namespace: argocd
\`\`\`

## Repository Structure

Your Git repository should be organized like this:

\`\`\`
your-secrets-repo/
├── microservices/
│   └── my-service/
│       └── deployment-configuration/
│           └── profiles/
│               ├── dev/
│               │   ├── kustomization.yaml
│               │   └── secrets.yaml  # SOPS-encrypted
│               ├── staging/
│               │   ├── kustomization.yaml
│               │   └── secrets.yaml
│               └── prod/
│                   ├── kustomization.yaml
│                   └── secrets.yaml
└── application.properties  # Optional: for config stores
\`\`\`

## Kustomize Path Configuration

The \`kustomizePath\` in your SecretManagerConfig should point to the Kustomize overlay:

\`\`\`yaml
spec:
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
\`\`\`

This path is relative to the repository root.

## Private Repositories

### FluxCD

For private repositories, create a Kubernetes Secret with Git credentials:

\`\`\`yaml
apiVersion: v1
kind: Secret
metadata:
  name: git-credentials
  namespace: microscaler-system
type: Opaque
stringData:
  username: your-username
  password: your-token-or-password
\`\`\`

Reference it in your GitRepository:

\`\`\`yaml
spec:
  secretRef:
    name: git-credentials
\`\`\`

### ArgoCD

Configure repository credentials in ArgoCD:

\`\`\`bash
argocd repo add https://github.com/your-org/private-repo \\
  --username your-username \\
  --password your-token
\`\`\`

Or use SSH keys:

\`\`\`bash
argocd repo add git@github.com:your-org/private-repo.git \\
  --ssh-private-key-path ~/.ssh/id_rsa
\`\`\`

## Branch and Tag Support

### FluxCD

Specify branch or tag in GitRepository:

\`\`\`yaml
spec:
  ref:
    branch: main
    # OR
    tag: v1.0.0
    # OR
    commit: abc123def456
\`\`\`

### ArgoCD

Specify target revision in Application:

\`\`\`yaml
spec:
  source:
    targetRevision: main  # branch, tag, or commit
\`\`\`

## Update Intervals

### GitRepository Pull Interval

How often the controller checks for Git updates:

\`\`\`yaml
spec:
  gitRepositoryPullInterval: 5m  # Default: 5m, minimum: 1m
\`\`\`

**Recommendation:** 5 minutes or greater to avoid Git API rate limits.

### Reconcile Interval

How often the controller reconciles secrets:

\`\`\`yaml
spec:
  reconcileInterval: 1m  # Default: 1m
\`\`\`

## Troubleshooting

### GitRepository Not Found

**Error:** \`GitRepository "my-repo" not found\`

**Solution:**
1. Verify the GitRepository exists:
   \`\`\`bash
   kubectl get gitrepository -n microscaler-system
   \`\`\`
2. Check the \`sourceRef\` name and namespace match
3. Ensure FluxCD source-controller is running

### Artifacts Not Available

**Error:** \`No artifacts found for GitRepository\`

**Solution:**
1. Check source-controller logs:
   \`\`\`bash
   kubectl logs -n flux-system -l app=source-controller
   \`\`\`
2. Verify GitRepository status:
   \`\`\`bash
   kubectl describe gitrepository my-repo -n microscaler-system
   \`\`\`
3. Check artifact directory exists:
   \`\`\`bash
   kubectl exec -n flux-system -l app=source-controller -- ls -la /tmp/flux-source-*
   \`\`\`

### ArgoCD Application Not Found

**Error:** \`Application "my-app" not found\`

**Solution:**
1. Verify the Application exists:
   \`\`\`bash
   kubectl get application -n argocd
   \`\`\`
2. Check the \`sourceRef\` name and namespace match
3. Ensure ArgoCD is running and can access the repository

## Best Practices

1. **Use Kustomize Overlays**: Organize secrets by environment using Kustomize
2. **SOPS Encryption**: Encrypt all secrets in Git (see [SOPS Setup](./sops-setup.md))
3. **Separate Repositories**: Consider separate repos for secrets vs. application code
4. **Branch Protection**: Use branch protection rules for production secrets
5. **Audit Logging**: Enable Git audit logs for secret changes
6. **Access Control**: Limit who can push to secrets repositories

## Next Steps

- [SOPS Setup](./sops-setup.md) - Encrypt secrets in Git
- [Provider Setup Guides](./aws-setup.md) - Configure cloud providers
- [Configuration Reference](../getting-started/configuration.md) - Complete configuration guide
`,F=Object.freeze(Object.defineProperty({__proto__:null,default:y},Symbol.toStringTag,{value:"Module"})),f=`# MSMCTL CLI

\`msmctl\` (Microscaler Secret Manager Controller) is a command-line tool for interacting with the Secret Manager Controller running in Kubernetes. Similar to \`fluxctl\`, it provides commands to trigger reconciliations, view status, and manage SecretManagerConfig resources.

## Installation

### Build from Source

\`\`\`bash
# Build the CLI tool
cargo build --bin msmctl

# Build release version
cargo build --release --bin msmctl
\`\`\`

The binary will be available at:
- Debug build: \`target/debug/msmctl\`
- Release build: \`target/release/msmctl\`

### Install to Local Bin

\`\`\`bash
# Using just (recommended)
just install-cli

# Or manually
mkdir -p ~/.local/bin
cp target/release/msmctl ~/.local/bin/
\`\`\`

Make sure \`~/.local/bin\` is in your \`PATH\`.

### Prerequisites

- Kubernetes cluster with Secret Manager Controller deployed
- \`kubectl\` configured with access to the cluster
- RBAC permissions to read/update SecretManagerConfig resources

## Authentication

\`msmctl\` uses Kubernetes authentication primitives:

- **kubeconfig**: Uses the default kubeconfig (\`~/.kube/config\`) or \`KUBECONFIG\` environment variable
- **Service Account**: When running in-cluster, uses the pod's service account token
- **Client Certificates**: Supports client certificate authentication from kubeconfig

No additional authentication is required - \`msmctl\` leverages Kubernetes' built-in security mechanisms.

## Commands

### \`msmctl reconcile\`

Trigger a manual reconciliation for a SecretManagerConfig resource.

**Usage:**
\`\`\`bash
msmctl reconcile secretmanagerconfig <name> [--namespace <namespace>] [--force]
\`\`\`

**Arguments:**
- \`secretmanagerconfig\` (or \`smc\`): Resource type (required)
- \`<name>\`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- \`--namespace, -n\`: Namespace of the resource (defaults to current context namespace)
- \`--force\`: Force reconciliation by deleting and waiting for GitOps to recreate the resource (useful when resources get stuck)

**Examples:**
\`\`\`bash
# Trigger reconciliation in default namespace
msmctl reconcile secretmanagerconfig myapp-dev-secrets

# Trigger reconciliation in specific namespace
msmctl reconcile secretmanagerconfig myapp-dev-secrets --namespace mysystem

# Using short form 'smc'
msmctl reconcile smc myapp-dev-secrets

# Force reconciliation (delete and wait for GitOps recreation)
msmctl reconcile secretmanagerconfig myapp-dev-secrets --namespace mysystem --force
\`\`\`

**How it works:**
- **Normal mode**: Updates the \`secret-management.microscaler.io/reconcile\` annotation with a timestamp. The controller watches for annotation changes and triggers reconciliation. This is a Kubernetes-native approach that doesn't require HTTP endpoints.
- **Force mode (\`--force\`)**: 
  1. Deletes the SecretManagerConfig resource
  2. Waits for GitOps (Flux/ArgoCD) to recreate it (up to 5 minutes)
  3. Shows progress logs during the wait
  4. Once recreated, triggers reconciliation
  5. Provides command to view reconciliation logs

**Force mode output:**
\`\`\`
🔄 Force reconciliation mode enabled
   Resource: mysystem/myapp-dev-secrets

🗑️  Deleting SecretManagerConfig 'mysystem/myapp-dev-secrets'...

⏳ Waiting for GitOps to recreate resource...
   (This may take a few moments depending on GitOps sync interval)
   ⏳ Still waiting... (10s elapsed)
   ⏳ Still waiting... (20s elapsed)
   ✅ Resource recreated (generation: 1)

⏳ Waiting for resource to stabilize...

🔄 Triggering reconciliation for SecretManagerConfig 'mysystem/myapp-dev-secrets'...
✅ Reconciliation triggered successfully
   Resource: mysystem/myapp-dev-secrets
   Timestamp: 1702567890

📊 Watching reconciliation logs...
   (Use 'kubectl logs -n microscaler-system -l app=secret-manager-controller --tail=50 -f' to see detailed logs)
\`\`\`

### \`msmctl list\`

List all SecretManagerConfig resources.

**Usage:**
\`\`\`bash
msmctl list secretmanagerconfig [--namespace <namespace>]
\`\`\`

**Arguments:**
- \`secretmanagerconfig\` (or \`smc\`): Resource type (required)

**Options:**
- \`--namespace, -n\`: Namespace to list resources in (defaults to all namespaces)

**Examples:**
\`\`\`bash
# List all resources in all namespaces
msmctl list secretmanagerconfig

# List resources in specific namespace
msmctl list secretmanagerconfig --namespace mysystem

# Using short form 'smc'
msmctl list smc
\`\`\`

**Output:**
\`\`\`
NAME                           NAMESPACE            SUSPEND      READY           SECRETS SYNCED 
--------------------------------------------------------------------------------------------
test-sops-config               default              No           False           -              
test-sops-config-prod          default              No           False           -              
test-sops-config-stage         default              No           False           -              
\`\`\`

**Note:** The \`SUSPEND\` column shows whether reconciliation is paused for each resource.

### \`msmctl status\`

Show detailed status of a SecretManagerConfig resource.

**Usage:**
\`\`\`bash
msmctl status secretmanagerconfig <name> [--namespace <namespace>]
\`\`\`

**Arguments:**
- \`secretmanagerconfig\` (or \`smc\`): Resource type (required)
- \`<name>\`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- \`--namespace, -n\`: Namespace of the resource (defaults to current context namespace)

**Examples:**
\`\`\`bash
# Show status in default namespace
msmctl status secretmanagerconfig myapp-dev-secrets --namespace mysystem

# Using short form 'smc'
msmctl status smc myapp-dev-secrets
\`\`\`

**Output:**
\`\`\`
SecretManagerConfig: mysystem/myapp-dev-secrets

Phase: Synced
Description: Successfully synced 5 secrets
Ready: True

Conditions:
  - Type: Ready
    Status: True
    Reason: ReconciliationSucceeded
    Message: Successfully synced 5 secrets
    Last Transition: 2024-01-15T10:30:00Z

Status:
  Last Sync Time: 2024-01-15T10:30:00Z
  Secrets Count: 5
  Suspended: false
  Git Pulls Suspended: false
\`\`\`

### \`msmctl suspend\`

Suspend reconciliation for a SecretManagerConfig resource.

**Usage:**
\`\`\`bash
msmctl suspend secretmanagerconfig <name> [--namespace <namespace>]
\`\`\`

**Arguments:**
- \`secretmanagerconfig\` (or \`smc\`): Resource type (required)
- \`<name>\`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- \`--namespace, -n\`: Namespace of the resource (defaults to current context namespace)

**Examples:**
\`\`\`bash
# Suspend reconciliation
msmctl suspend secretmanagerconfig test-sops-config --namespace default

# Using short form 'smc'
msmctl suspend smc test-sops-config
\`\`\`

**What it does:**
- Sets the \`secret-management.microscaler.io/suspend\` annotation to \`"true"\`
- Controller will skip reconciliation for this resource
- Manual reconciliation via \`msmctl reconcile\` will also be blocked

**To resume:**
\`\`\`bash
msmctl resume secretmanagerconfig test-sops-config --namespace default
\`\`\`

### \`msmctl resume\`

Resume reconciliation for a SecretManagerConfig resource.

**Usage:**
\`\`\`bash
msmctl resume secretmanagerconfig <name> [--namespace <namespace>]
\`\`\`

**Arguments:**
- \`secretmanagerconfig\` (or \`smc\`): Resource type (required)
- \`<name>\`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- \`--namespace, -n\`: Namespace of the resource (defaults to current context namespace)

**Examples:**
\`\`\`bash
# Resume reconciliation
msmctl resume secretmanagerconfig test-sops-config --namespace default

# Using short form 'smc'
msmctl resume smc test-sops-config
\`\`\`

**What it does:**
- Removes the \`secret-management.microscaler.io/suspend\` annotation
- Controller will resume normal reconciliation

### \`msmctl suspend-git-pulls\`

Suspend Git repository pulls for a SecretManagerConfig resource.

**Usage:**
\`\`\`bash
msmctl suspend-git-pulls secretmanagerconfig <name> [--namespace <namespace>]
\`\`\`

**Arguments:**
- \`secretmanagerconfig\` (or \`smc\`): Resource type (required)
- \`<name>\`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- \`--namespace, -n\`: Namespace of the resource (defaults to current context namespace)

**Examples:**
\`\`\`bash
# Suspend Git pulls
msmctl suspend-git-pulls secretmanagerconfig test-sops-config --namespace default

# Using short form 'smc'
msmctl suspend-git-pulls smc test-sops-config
\`\`\`

**What it does:**
- Sets the \`secret-management.microscaler.io/suspend-git-pulls\` annotation to \`"true"\`
- Controller will stop checking for updates from the Git repository
- Existing secrets will continue to be reconciled, but new changes from Git will be ignored

**To resume:**
\`\`\`bash
msmctl resume-git-pulls secretmanagerconfig test-sops-config --namespace default
\`\`\`

### \`msmctl resume-git-pulls\`

Resume Git repository pulls for a SecretManagerConfig resource.

**Usage:**
\`\`\`bash
msmctl resume-git-pulls secretmanagerconfig <name> [--namespace <namespace>]
\`\`\`

**Arguments:**
- \`secretmanagerconfig\` (or \`smc\`): Resource type (required)
- \`<name>\`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- \`--namespace, -n\`: Namespace of the resource (defaults to current context namespace)

**Examples:**
\`\`\`bash
# Resume Git pulls
msmctl resume-git-pulls secretmanagerconfig test-sops-config --namespace default

# Using short form 'smc'
msmctl resume-git-pulls smc test-sops-config
\`\`\`

**What it does:**
- Removes the \`secret-management.microscaler.io/suspend-git-pulls\` annotation
- Controller will resume checking for updates from the Git repository

### \`msmctl install\`

Install the Secret Manager Controller in a Kubernetes cluster.

**Usage:**
\`\`\`bash
msmctl install [--namespace <namespace>] [--export]
\`\`\`

**Options:**
- \`--namespace, -n\`: Namespace to install the controller in (default: \`microscaler-system\`)
- \`--export\`: Export manifests instead of applying them

**Examples:**
\`\`\`bash
# Install to default namespace
msmctl install

# Install to custom namespace
msmctl install --namespace my-namespace

# Export manifests without installing
msmctl install --export
\`\`\`

**What it installs:**
- CRD: \`SecretManagerConfig\` Custom Resource Definition
- Namespace: \`microscaler-system\` (or specified namespace)
- ServiceAccount, Role, RoleBinding: RBAC resources
- Deployment: Controller deployment

### \`msmctl check\`

Check the installation and prerequisites of the Secret Manager Controller.

**Usage:**
\`\`\`bash
msmctl check [--pre]
\`\`\`

**Options:**
- \`--pre\`: Check prerequisites only (Kubernetes version, CRDs, etc.)

**Examples:**
\`\`\`bash
# Full check
msmctl check

# Prerequisites only
msmctl check --pre
\`\`\`

**What it checks:**
- Kubernetes version compatibility
- CRD availability
- Controller deployment status
- RBAC permissions
- Controller health

## Resource Types

The following resource types are supported:

- \`secretmanagerconfig\` (or \`smc\`): SecretManagerConfig resource

## Short Forms

You can use \`smc\` as a short form for \`secretmanagerconfig\` in all commands:

\`\`\`bash
# These are equivalent:
msmctl list secretmanagerconfig
msmctl list smc

msmctl reconcile secretmanagerconfig my-secrets
msmctl reconcile smc my-secrets
\`\`\`

## RBAC Requirements

The user/service account running \`msmctl\` needs the following permissions:

\`\`\`yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: msmctl-user
rules:
- apiGroups: ["secret-management.microscaler.io"]
  resources: ["secretmanagerconfigs"]
  verbs: ["get", "list", "watch", "update", "patch", "delete"]
\`\`\`

## Troubleshooting

### Command Not Found

If \`msmctl\` is not found, ensure it's in your PATH:

\`\`\`bash
# Check if it's installed
which msmctl

# Add to PATH if needed
export PATH="$HOME/.local/bin:$PATH"
\`\`\`

### Permission Denied

If you get permission errors, check your RBAC permissions:

\`\`\`bash
# Check if you can list SecretManagerConfig resources
kubectl get secretmanagerconfigs --all-namespaces

# Check your current context
kubectl config current-context
\`\`\`

### Resource Not Found

If a resource is not found, check the namespace:

\`\`\`bash
# List all resources
msmctl list secretmanagerconfig

# Check specific namespace
msmctl list secretmanagerconfig --namespace <namespace>
\`\`\`

## Examples

### Batch Operations

List all resources and check their status:

\`\`\`bash
for config in $(msmctl list secretmanagerconfig --namespace mysystem | awk 'NR>2 {print $1}'); do
  msmctl status secretmanagerconfig "$config" --namespace mysystem
done
\`\`\`

### Force Reconciliation for Stuck Resources

If a resource is stuck and not reconciling:

\`\`\`bash
msmctl reconcile secretmanagerconfig my-secrets --namespace default --force
\`\`\`

This will delete and wait for GitOps to recreate the resource, then trigger reconciliation.

## Next Steps

- [Quick Start Guide](../getting-started/quick-start.md) - Get started with the controller
- [Configuration Reference](../getting-started/configuration.md) - Learn about configuration options
- [Troubleshooting](../tutorials/troubleshooting.md) - Common issues and solutions

`,K=Object.freeze(Object.defineProperty({__proto__:null,default:f},Symbol.toStringTag,{value:"Module"})),S=`# SOPS Setup

Guide for setting up SOPS encryption for secrets in Git repositories.

## Overview

SOPS (Secrets OPerationS) allows you to encrypt secret files before committing them to Git. The Secret Manager Controller automatically decrypts SOPS-encrypted files using GPG or AGE keys stored in Kubernetes Secrets.

## Prerequisites

- GPG key pair OR AGE key pair generated
- SOPS installed locally
- Kubernetes cluster with controller installed

## Encryption Methods

The controller supports two encryption methods:

1. **GPG (GNU Privacy Guard)** - Traditional PGP encryption
2. **AGE (Actually Good Encryption)** - Modern, simpler encryption

You can use either or both methods. This guide covers both.

## GPG Key Setup

### Step 1: Generate GPG Key

If you don't have a GPG key, generate one:

\`\`\`bash
gpg --full-generate-key
\`\`\`

Follow the prompts:
- Key type: RSA and RSA (default)
- Key size: 4096 (recommended)
- Expiration: Set as needed (or 0 for no expiration)
- Name and email: Use your identity

### Step 2: Export Public Key

Export your public key for sharing (if needed):

\`\`\`bash
gpg --armor --export your-email@example.com > public-key.asc
\`\`\`

### Step 3: Get GPG Fingerprint

Get your GPG key fingerprint:

\`\`\`bash
gpg --list-keys --fingerprint
\`\`\`

You'll see output like:
\`\`\`
pub   rsa4096 2024-01-15 [SC]
      ABC1 2345 DEF6 7890 ABCD EF12 3456 7890 ABCD EF12
uid           [ultimate] Your Name <your-email@example.com>
\`\`\`

The fingerprint is: \`ABC12345DEF67890ABCDEF1234567890ABCDEF12\`

## AGE Key Setup

### Step 1: Generate AGE Key

Generate a new AGE key pair:

\`\`\`bash
# Generate key pair
age-keygen -o age-key.txt
\`\`\`

This creates a file with:
- **Public key**: \`age1...\` (share this)
- **Private key**: \`AGE-SECRET-KEY-1...\` (keep this secret)

**Example output:**
\`\`\`
# created: 2024-01-15T10:30:00Z
# public key: age1abc123def456ghi789jkl012mno345pqr678stu901vwx234yz
AGE-SECRET-KEY-1ABC123DEF456GHI789JKL012MNO345PQR678STU901VWX234YZ567890ABCDEF
\`\`\`

### Step 2: Extract Keys

\`\`\`bash
# Extract public key
grep "public key" age-key.txt | cut -d' ' -f4 > age-public-key.txt

# Extract private key
grep "AGE-SECRET-KEY" age-key.txt > age-private-key.txt
\`\`\`

## Step 4: Create SOPS Configuration

Create a \`.sops.yaml\` file in your repository root:

### GPG-Only Configuration

\`\`\`yaml
creation_rules:
  - path_regex: .*\\.secrets\\.(env|yaml)$
    encrypted_regex: ^(data|stringData|DATABASE_|API_|JWT_)
    pgp: >-
      ABC12345DEF67890ABCDEF1234567890ABCDEF12,
      XYZ98765UVW43210ZYXWVU9876543210ZYXWVU98
\`\`\`

### AGE-Only Configuration

\`\`\`yaml
creation_rules:
  - path_regex: .*\\.secrets\\.(env|yaml)$
    encrypted_regex: ^(data|stringData|DATABASE_|API_|JWT_)
    age: >-
      age1abc123def456ghi789jkl012mno345pqr678stu901vwx234yz,
      age1xyz987uvw654rst321qpo098nml765kji432hgf210edc876ba
\`\`\`

### Combined GPG and AGE Configuration

\`\`\`yaml
creation_rules:
  - path_regex: .*\\.secrets\\.(env|yaml)$
    encrypted_regex: ^(data|stringData|DATABASE_|API_|JWT_)
    pgp: >-
      ABC12345DEF67890ABCDEF1234567890ABCDEF12
    age: >-
      age1abc123def456ghi789jkl012mno345pqr678stu901vwx234yz
\`\`\`

This allows decryption with either GPG or AGE keys (redundancy).

## Step 5: Encrypt Secrets

Encrypt your secret files:

\`\`\`bash
# Encrypt a YAML file
sops -e -i application.secrets.yaml

# Encrypt an ENV file
sops -e -i application.secrets.env

# Encrypt a properties file
sops -e -i application.properties
\`\`\`

The files will be encrypted in place. SOPS will use the encryption method(s) specified in \`.sops.yaml\`.

## Step 6: Create Kubernetes Secrets

Export your private keys and create Kubernetes Secrets:

### GPG Key Setup

#### Automated Setup

Use the setup script:

\`\`\`bash
python3 scripts/setup_sops_key.py --key-email your-email@example.com
\`\`\`

This will:
1. Export the GPG private key
2. Create a Kubernetes Secret \`sops-gpg-key\` in \`microscaler-system\` namespace
3. Store the private key securely

#### Manual Setup

\`\`\`bash
# Export private key
gpg --armor --export-secret-keys your-email@example.com > /tmp/private-key.asc

# Create Kubernetes Secret
kubectl create secret generic sops-gpg-key \\
  --from-file=private.key=/tmp/private-key.asc \\
  -n microscaler-system

# Clean up
rm /tmp/private-key.asc
\`\`\`

### AGE Key Setup

\`\`\`bash
# Extract private key from age-key.txt
grep "AGE-SECRET-KEY" age-key.txt > /tmp/age-private-key.txt

# Create Kubernetes Secret
kubectl create secret generic sops-age-key \\
  --from-file=private.key=/tmp/age-private-key.txt \\
  -n microscaler-system

# Clean up
rm /tmp/age-private-key.txt
\`\`\`

**Note:** Keep \`age-key.txt\` secure - it contains both public and private keys.

## Step 7: Configure SecretManagerConfig

Reference the encryption keys in your SecretManagerConfig:

### GPG-Only Configuration

\`\`\`yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: my-config
spec:
  secrets:
    sops:
      enabled: true
      gpgSecretRef:
        name: sops-gpg-key
        namespace: microscaler-system
        key: private.key
\`\`\`

### AGE-Only Configuration

\`\`\`yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: my-config
spec:
  secrets:
    sops:
      enabled: true
      ageSecretRef:
        name: sops-age-key
        namespace: microscaler-system
        key: private.key
\`\`\`

### Combined GPG and AGE Configuration

You can specify both for redundancy:

\`\`\`yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: my-config
spec:
  secrets:
    sops:
      enabled: true
      gpgSecretRef:
        name: sops-gpg-key
        namespace: microscaler-system
        key: private.key
      ageSecretRef:
        name: sops-age-key
        namespace: microscaler-system
        key: private.key
\`\`\`

The controller will try GPG first, then AGE if GPG fails.

## Verification

### Test Encryption Locally

#### Test GPG Encryption

\`\`\`bash
# Encrypt a test file with GPG
echo "password: secret123" > test.yaml
sops -e -i test.yaml

# Decrypt to verify
sops -d test.yaml
\`\`\`

#### Test AGE Encryption

\`\`\`bash
# Encrypt a test file with AGE
echo "password: secret123" > test.yaml
sops -e -i test.yaml

# Decrypt to verify (requires AGE key in environment)
export SOPS_AGE_KEY_FILE=age-key.txt
sops -d test.yaml
\`\`\`

### Verify Controller Can Decrypt

Check controller logs:

\`\`\`bash
kubectl logs -n microscaler-system -l app=secret-manager-controller | grep -i sops
\`\`\`

You should see successful decryption messages:
\`\`\`
✅ Loaded SOPS private key from secret 'microscaler-system/sops-gpg-key'
🔑 SOPS file requires GPG key fingerprints: ABC12345DEF67890...
✅ SOPS decryption successful
\`\`\`

Or for AGE:
\`\`\`
✅ Loaded SOPS private key from secret 'microscaler-system/sops-age-key'
✅ SOPS decryption successful
\`\`\`

## Key Management

### Secret Names

The controller checks for secrets in this order:

1. \`sops-private-key\`
2. \`sops-gpg-key\`
3. \`sops-age-key\`
4. \`gpg-key\`

### Secret Keys

Within each secret, the controller checks for keys in this order:

1. \`private.key\`
2. \`key\`
3. \`gpg-key\`
4. \`age-key\`

### Namespace Placement

SOPS private keys should be placed in the **same namespace** as the \`SecretManagerConfig\` resource. The controller will:

1. First check the resource's namespace for the SOPS key secret
2. If not found, log a critical error (no fallback to controller namespace)
3. This ensures proper namespace isolation and prevents configuration errors

## Best Practices

1. **Multiple Keys**: Use multiple GPG or AGE keys for redundancy
2. **Combined Methods**: Use both GPG and AGE for maximum redundancy
3. **Key Rotation**: Rotate keys periodically
4. **Backup Keys**: Store private keys securely (not in Git!)
5. **Access Control**: Limit who has access to the Kubernetes Secret
6. **Key Management**: Use a key management system for production
7. **AGE for Simplicity**: Consider AGE for new projects (simpler than GPG)
8. **GPG for Compatibility**: Use GPG if you need compatibility with existing tools

## Troubleshooting

### Decryption Fails

**Error:** \`Failed to decrypt SOPS file\`

**Solutions:**

#### For GPG Keys

1. Verify GPG key matches the encryption key:
   \`\`\`bash
   kubectl get secret sops-gpg-key -n microscaler-system -o jsonpath='{.data.private\\.key}' | base64 -d | gpg --import
   \`\`\`
2. Check key fingerprint matches \`.sops.yaml\`:
   \`\`\`bash
   gpg --list-keys --fingerprint
   \`\`\`
3. Verify the secret name and namespace are correct

#### For AGE Keys

1. Verify AGE key format:
   \`\`\`bash
   kubectl get secret sops-age-key -n microscaler-system -o jsonpath='{.data.private\\.key}' | base64 -d
   \`\`\`
   Should start with: \`AGE-SECRET-KEY-1\`
2. Check public key matches \`.sops.yaml\`:
   \`\`\`bash
   # Extract public key from private key
   age-keygen -y < age-key.txt
   \`\`\`
3. Verify the secret name and namespace are correct

### Key Not Found

**Error:** \`GPG key secret not found\` or \`AGE key secret not found\`

**Solutions:**
1. Verify the secret exists:
   \`\`\`bash
   # For GPG
   kubectl get secret sops-gpg-key -n microscaler-system
   
   # For AGE
   kubectl get secret sops-age-key -n microscaler-system
   \`\`\`
2. Check the \`gpgSecretRef\` or \`ageSecretRef\` in SecretManagerConfig
3. Ensure the namespace matches the SecretManagerConfig namespace
4. Verify the secret is in the correct namespace (same as SecretManagerConfig)

### Invalid Key Format

**Error:** \`Invalid GPG key format\` or \`Invalid AGE key format\`

**Solutions:**

#### For GPG Keys

1. Verify the key is in ASCII-armored format:
   \`\`\`bash
   kubectl get secret sops-gpg-key -n microscaler-system -o jsonpath='{.data.private\\.key}' | base64 -d | head -1
   \`\`\`
   Should show: \`-----BEGIN PGP PRIVATE KEY BLOCK-----\`
2. Re-export the key if needed:
   \`\`\`bash
   gpg --armor --export-secret-keys your-email@example.com
   \`\`\`

#### For AGE Keys

1. Verify the key format:
   \`\`\`bash
   kubectl get secret sops-age-key -n microscaler-system -o jsonpath='{.data.private\\.key}' | base64 -d | head -1
   \`\`\`
   Should start with: \`AGE-SECRET-KEY-1\`
2. Regenerate the key if needed:
   \`\`\`bash
   age-keygen -o age-key.txt
   \`\`\`

## GPG vs AGE Comparison

| Feature | GPG | AGE |
|---------|-----|-----|
| **Complexity** | More complex | Simpler |
| **Key Size** | Larger (4096-bit RSA) | Smaller (128-bit) |
| **Performance** | Slower | Faster |
| **Compatibility** | Widely supported | Modern tooling |
| **Key Management** | More complex | Simpler |
| **Recommended For** | Existing projects, compatibility | New projects, simplicity |

**Recommendation:**
- **New projects**: Use AGE for simplicity
- **Existing projects**: Use GPG for compatibility
- **Production**: Use both for redundancy

## Next Steps

- [Application Files Guide](./application-files.md) - Learn about file formats
- [GitOps Integration](./gitops-integration.md) - Set up GitOps workflow
- [Quick Start](../getting-started/quick-start.md) - Get started quickly
- [Configuration](../getting-started/configuration.md) - Complete configuration guide
`,L=Object.freeze(Object.defineProperty({__proto__:null,default:S},Symbol.toStringTag,{value:"Module"})),h=`# Advanced Scenarios

Advanced usage patterns and scenarios.

## Multiple Providers

You can use multiple providers in the same cluster:

\`\`\`yaml
# AWS secrets
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: aws-secrets
spec:
  provider: aws
  region: us-east-1
  secrets: [...]
---
# Azure secrets
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: azure-secrets
spec:
  provider: azure
  vaultUrl: https://myvault.vault.azure.net/
  secrets: [...]
\`\`\`

## Namespace Isolation

Create separate configurations per namespace:

\`\`\`yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: prod-secrets
  namespace: production
spec:
  provider: aws
  region: us-east-1
  secrets: [...]
---
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: dev-secrets
  namespace: development
spec:
  provider: aws
  region: us-east-1
  secrets: [...]
\`\`\`

## GitOps with SOPS

Store encrypted secrets in Git:

1. Encrypt secrets with SOPS
2. Commit encrypted files to Git
3. Configure controller to decrypt:

\`\`\`yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: gitops-secrets
spec:
  provider: aws
  region: us-east-1
  gitRepository:
    name: my-repo
    namespace: flux-system
  sops:
    enabled: true
    keySecret:
      name: sops-key
  secrets:
    - name: config
      key: /myapp/config
      sopsFile: config.enc.yaml
\`\`\`

## Version Pinning

Pin to specific secret versions:

\`\`\`yaml
secrets:
  - name: stable-secret
    key: /myapp/secret
    version: "12345678-1234-1234-1234-123456789012"
\`\`\`

## Update Policies

Control when secrets are updated:

\`\`\`yaml
spec:
  updatePolicy: OnChange  # Only update when provider value changes
  secrets: [...]
\`\`\`

## Learn More

- [Troubleshooting](./troubleshooting.md)
- [API Reference](../api-reference/)

`,N=Object.freeze(Object.defineProperty({__proto__:null,default:h},Symbol.toStringTag,{value:"Module"})),v=`# Basic Usage Tutorial

Learn how to use Secret Manager Controller with a simple example.

## Step 1: Install the Controller

See [Installation Guide](../getting-started/installation.md) for installation instructions.

## Step 2: Configure Provider

Set up your cloud provider credentials. For AWS:

\`\`\`bash
kubectl create secret generic aws-credentials \\
  --from-literal=AWS_ACCESS_KEY_ID=your-key \\
  --from-literal=AWS_SECRET_ACCESS_KEY=your-secret \\
  --from-literal=AWS_REGION=us-east-1 \\
  -n microscaler-system
\`\`\`

## Step 3: Create SecretManagerConfig

Create a configuration file:

\`\`\`yaml
apiVersion: secret-management.microscaler.io/v1
kind: SecretManagerConfig
metadata:
  name: my-app-secrets
  namespace: default
spec:
  provider: aws
  region: us-east-1
  credentials:
    secretRef:
      name: aws-credentials
      namespace: microscaler-system
  secrets:
    - name: database-password
      key: /myapp/database/password
\`\`\`

Apply it:

\`\`\`bash
kubectl apply -f secret-config.yaml
\`\`\`

## Step 4: Verify

Check that the Kubernetes Secret was created:

\`\`\`bash
kubectl get secret my-app-secrets -n default
kubectl get secret my-app-secrets -n default -o yaml
\`\`\`

## Step 5: Use in Your Application

Reference the secret in your deployment:

\`\`\`yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: my-app
spec:
  template:
    spec:
      containers:
      - name: app
        env:
        - name: DB_PASSWORD
          valueFrom:
            secretKeyRef:
              name: my-app-secrets
              key: database-password
\`\`\`

## Next Steps

- [Advanced Scenarios](./advanced-scenarios.md)
- [Troubleshooting](./troubleshooting.md)

`,z=Object.freeze(Object.defineProperty({__proto__:null,default:v},Symbol.toStringTag,{value:"Module"})),C=`# Troubleshooting Guide

Common issues and solutions.

## Controller Not Running

### Check Pod Status

\`\`\`bash
kubectl get pods -n microscaler-system
\`\`\`

### Check Logs

\`\`\`bash
kubectl logs -n microscaler-system -l app=secret-manager-controller
\`\`\`

### Common Causes

- Missing RBAC permissions
- Image pull errors
- Resource constraints

## Secrets Not Created

### Verify SecretManagerConfig

\`\`\`bash
kubectl get secretmanagerconfig -A
kubectl describe secretmanagerconfig <name> -n <namespace>
\`\`\`

### Check Status

The status field shows:
- Last sync time
- Error messages
- Secret count

### Common Issues

1. **Authentication Failed**
   - Verify provider credentials
   - Check IAM/role permissions
   - Ensure credentials secret exists

2. **Secret Not Found**
   - Verify secret exists in provider
   - Check secret key/path
   - Verify permissions include the secret

3. **Network Issues**
   - Check cluster network connectivity
   - Verify VPC endpoints (if using)
   - Check firewall rules

## Secrets Not Updating

### Check Update Policy

\`\`\`yaml
spec:
  updatePolicy: Always  # Ensure this is set
\`\`\`

### Verify Reconciliation

Check controller logs for reconciliation events.

### Force Reconciliation

Delete and recreate the SecretManagerConfig:

\`\`\`bash
kubectl delete secretmanagerconfig <name> -n <namespace>
kubectl apply -f config.yaml
\`\`\`

## Provider-Specific Issues

### AWS

- Verify IAM role/credentials
- Check region configuration
- Ensure Secrets Manager is enabled in region

### Azure

- Verify managed identity or service principal
- Check Key Vault URL format
- Ensure Key Vault access policies

### GCP

- Verify service account permissions
- Check project ID
- Ensure Secret Manager API is enabled

## Getting Help

- Check controller logs
- Review SecretManagerConfig status
- Verify provider credentials
- Check network connectivity

## Next Steps

- [Basic Usage](./basic-usage.md)
- [Advanced Scenarios](./advanced-scenarios.md)

`,U=Object.freeze(Object.defineProperty({__proto__:null,default:C},Symbol.toStringTag,{value:"Module"}));export{P as a,E as b,k as c,O as d,D as e,T as f,x as g,_ as h,w as i,I as j,F as k,L as l,K as m,N as n,G as o,R as p,M as q,z as r,b as s,A as t,U as u};
