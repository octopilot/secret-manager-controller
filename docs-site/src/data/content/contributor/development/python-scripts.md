# Python Scripts

The Secret Manager Controller uses Python scripts for development automation, replacing shell scripts to comply with the zero shell script policy. This document describes the key Python scripts and their usage.

## Overview

All Python scripts are located in the `scripts/` directory and follow these principles:

- **Zero Shell Scripts**: All automation is Python-based
- **Cross-Platform**: Works on macOS, Linux, and Windows (where applicable)
- **Error Handling**: Robust error handling with clear messages
- **Logging**: Consistent logging format (`[INFO]`, `[WARN]`, `[ERROR]`)
- **Idempotent**: Safe to run multiple times

## Core Development Scripts

### `setup_kind.py`

**Purpose**: Sets up a local Kind cluster with Docker registry for development.

**Usage**:
```bash
python3 scripts/setup_kind.py
```

**What It Does**:
1. Checks prerequisites (Docker, kubectl, Kind)
2. Creates local Docker registry (`secret-manager-controller-registry` on port 5000)
3. Creates Kind cluster with custom network configuration
4. Connects registry to Kind network
5. Configures containerd on nodes to use local registry as mirror
6. Installs GitOps components (FluxCD, ArgoCD CRDs)
7. Creates `microscaler-system` namespace

**Key Features**:
- **Polling for Readiness**: Uses polling loops instead of fixed `sleep()` calls
- **Network Detection**: Waits for Docker network to exist before connecting registry
- **Containerd Configuration**: Configures containerd to pull from local registry
- **CI-Friendly**: Detects CI environment and runs non-interactively

**Configuration**:
- `CLUSTER_NAME`: `secret-manager-controller` (default)
- `REGISTRY_NAME`: `secret-manager-controller-registry` (default)
- `REGISTRY_PORT`: `5000` (default)

**See Also**: [Kind Cluster Setup](./kind-cluster-setup.md)

### `dev_up.py`

**Purpose**: Starts the complete development environment (Kind cluster + Tilt).

**Usage**:
```bash
python3 scripts/dev_up.py
```

**What It Does**:
1. Checks Docker daemon is running
2. Creates Kind cluster (if not exists) via `setup_kind.py`
3. Starts Tilt for local development

**Key Features**:
- **Idempotent**: Safe to run if cluster already exists
- **Prerequisite Checks**: Verifies Docker, kubectl, Kind are available
- **Error Handling**: Clear error messages if prerequisites missing

### `dev_down.py`

**Purpose**: Stops the development environment (Tilt + Kind cluster + registry).

**Usage**:
```bash
python3 scripts/dev_down.py
```

**What It Does**:
1. Stops Tilt processes
2. Deletes Kind cluster
3. Stops and removes local Docker registry container
4. Removes registry volume (`secret-manager-controller-registry-data`)

**Key Features**:
- **Complete Cleanup**: Removes all resources for fresh start
- **Graceful Shutdown**: Handles missing resources gracefully
- **Volume Cleanup**: Removes registry data volume

### `fix_registry_config.py`

**Purpose**: Fixes containerd registry configuration on Kind cluster nodes.

**Usage**:
```bash
python3 scripts/fix_registry_config.py
```

**When to Use**:
- After Kind cluster restart
- When registry networking issues occur
- When pods can't pull images from local registry

**What It Does**:
1. Finds registry container IP address
2. Connects registry to Kind network (if not connected)
3. Updates containerd config on all nodes
4. Restarts containerd on nodes

**Key Features**:
- **Idempotent**: Safe to run multiple times
- **Automatic Detection**: Finds registry IP automatically
- **Node Iteration**: Configures all nodes in cluster

## Testing Scripts

### `pact_tests.py`

**Purpose**: Runs Pact contract tests and publishes contracts to broker.

**Usage**:
```bash
# Run all Pact tests
python3 scripts/pact_tests.py

# Run specific test
python3 scripts/pact_tests.py --test pact_gcp_secret_manager

# Skip publishing
python3 scripts/pact_tests.py --no-publish
```

**What It Does**:
1. Waits for Pact broker to be ready
2. Runs Pact contract tests (`cargo test --test pact_*`)
3. Publishes contracts to broker
4. Verifies contracts were published

**Key Features**:
- **Broker Readiness**: Waits for broker before running tests
- **Contract Publishing**: Automatically publishes contracts after tests
- **Error Handling**: Handles test failures gracefully
- **CI Integration**: Works in CI/CD pipelines

**Configuration**:
- `PACT_BROKER_URL`: Broker URL (default: `http://localhost:9292`)
- `PACT_BROKER_USERNAME`: Broker username (default: `pact`)
- `PACT_BROKER_PASSWORD`: Broker password (default: `pact`)

**See Also**: [Pact Testing Overview](../testing/pact-testing/overview.md)

### `test_sops_decrypt_and_pact.py`

**Purpose**: Tests SOPS decryption and Pact contract generation.

**Usage**:
```bash
python3 scripts/test_sops_decrypt_and_pact.py
```

**What It Does**:
1. Decrypts SOPS-encrypted files
2. Validates decrypted content
3. Runs Pact tests with decrypted secrets

**Key Features**:
- **SOPS Integration**: Tests SOPS decryption workflow
- **Pact Integration**: Validates contracts with real secrets
- **Error Handling**: Clear errors if SOPS key missing

## Utility Scripts

### `status.py`

**Purpose**: Shows development environment status.

**Usage**:
```bash
python3 scripts/status.py
```

**What It Shows**:
- Kind cluster status
- Docker registry status
- Tilt process status
- Kubernetes context
- Pod status

**Key Features**:
- **Quick Overview**: Fast status check
- **Resource Listing**: Shows relevant resources
- **Color Output**: Uses colors for readability (if terminal supports)

### `validate_cr_locations.py`

**Purpose**: Validates all SecretManagerConfig CRs meet CRD pattern requirements.

**Usage**:
```bash
python3 scripts/validate_cr_locations.py
```

**What It Does**:
1. Finds all CR files in `gitops/cluster/`
2. Validates location/region fields against CRD patterns
3. Reports validation errors

**Key Features**:
- **Pattern Validation**: Validates GCP/AWS/Azure location formats
- **Comprehensive**: Checks all CRs in all environments
- **Clear Errors**: Shows specific validation failures

**Patterns Validated**:
- **GCP**: `^[a-z]+-[a-z]+[0-9]+$` (e.g., `us-central1`)
- **AWS**: `^[a-z]{2}-[a-z]+-[0-9]+$` (e.g., `us-east-1`)
- **Azure**: `^[a-z]+[0-9]*$` (e.g., `eastus`)

### `check_deps.py`

**Purpose**: Checks if required dependencies are installed.

**Usage**:
```bash
python3 scripts/check_deps.py
```

**What It Checks**:
- Docker
- kubectl
- Kind
- Python packages (if any)

**Key Features**:
- **Prerequisite Validation**: Ensures all tools available
- **Version Checking**: Checks minimum versions
- **Clear Messages**: Shows installation instructions if missing

### `copy_binary.py`

**Purpose**: Copies built binaries to specific locations.

**Usage**:
```bash
python3 scripts/copy_binary.py <binary-name> <destination>
```

**What It Does**:
1. Finds binary in `target/release/`
2. Copies to destination
3. Sets executable permissions

**Key Features**:
- **Binary Discovery**: Finds binaries automatically
- **Permission Setting**: Ensures binaries are executable
- **Error Handling**: Validates source and destination

## Tilt Integration Scripts

Scripts in `scripts/tilt/` are used by Tilt for development:

### `build_all_binaries.py`

**Purpose**: Builds all Rust binaries for Tilt.

**Usage**: Called by Tilt automatically.

**What It Does**:
1. Builds controller binary
2. Builds mock server binaries (GCP, AWS, Azure)
3. Builds manager binaries (postgres-manager, manager, webhook)
4. Copies binaries to Tilt working directory

### `populate_migrations_configmap.py`

**Purpose**: Populates ConfigMap with PostgreSQL migrations.

**Usage**: Called by Tilt automatically.

**What It Does**:
1. Reads migration files from `crates/pact-mock-server/migrations/`
2. Organizes by schema (gcp/, aws/, azure/)
3. Creates/updates ConfigMap with migration files

### `populate_pact_configmap.py`

**Purpose**: Populates ConfigMap with Pact contracts.

**Usage**: Called by Tilt automatically.

**What It Does**:
1. Reads Pact contract files
2. Creates/updates ConfigMap with contracts
3. Triggers manager to publish contracts

### `install_fluxcd.py`

**Purpose**: Installs FluxCD source-controller CRDs.

**Usage**:
```bash
python3 scripts/tilt/install_fluxcd.py
```

**What It Installs**:
- `GitRepository` CRD
- `Bucket` CRD
- Source controller deployment
- Required RBAC resources

### `install_argocd.py`

**Purpose**: Installs ArgoCD CRDs (Application, ApplicationSet).

**Usage**:
```bash
python3 scripts/tilt/install_argocd.py
```

**What It Installs**:
- `Application` CRD
- `ApplicationSet` CRD
- Other required ArgoCD CRDs

**Note**: Only CRDs are installed, not the full ArgoCD server.

## Pre-Commit Scripts

### `pre_commit_rust.py`

**Purpose**: Runs Rust linting and formatting checks.

**Usage**: Called by git pre-commit hook.

**What It Checks**:
- `cargo fmt --check`: Code formatting
- `cargo clippy`: Linting
- `cargo test`: Unit tests

### `pre_commit_sops.py`

**Purpose**: Validates SOPS-encrypted files.

**Usage**: Called by git pre-commit hook.

**What It Checks**:
- SOPS file format
- Encryption status
- Key availability

## Cleanup Scripts

### `cleanup_kind_storage.py`

**Purpose**: Cleans up Kind cluster storage (volumes, images).

**Usage**:
```bash
python3 scripts/cleanup_kind_storage.py
```

**What It Cleans**:
- Unused Docker volumes
- Old Kind cluster images
- Orphaned containers

### `delete_workflow_runs.py`

**Purpose**: Deletes old GitHub Actions workflow runs.

**Usage**:
```bash
python3 scripts/delete_workflow_runs.py
```

**What It Does**:
- Lists workflow runs
- Deletes runs older than threshold
- Requires GitHub token

## Best Practices

### Writing New Scripts

1. **Use Consistent Logging**:
   ```python
   def log_info(msg):
       print(f"[INFO] {msg}")
   
   def log_error(msg):
       print(f"[ERROR] {msg}", file=sys.stderr)
   ```

2. **Check Prerequisites**:
   ```python
   def check_command(cmd):
       if not shutil.which(cmd):
           log_error(f"{cmd} is not installed")
           sys.exit(1)
   ```

3. **Handle Errors Gracefully**:
   ```python
   try:
       result = subprocess.run(cmd, check=True)
   except subprocess.CalledProcessError as e:
       log_error(f"Command failed: {e}")
       sys.exit(1)
   ```

4. **Make Scripts Idempotent**:
   ```python
   # Check if resource exists before creating
   if resource_exists():
       log_info("Resource already exists, skipping")
       return
   ```

5. **Use Polling Instead of Sleep**:
   ```python
   # Bad: Fixed sleep
   time.sleep(10)
   
   # Good: Polling with timeout
   for i in range(max_retries):
       if check_condition():
           break
       time.sleep(1)
   ```

## Related Documentation

- [Kind Cluster Setup](./kind-cluster-setup.md) - Cluster configuration details
- [Tilt Integration](./tilt-integration.md) - Tilt development workflow
- [Development Setup](./setup.md) - Complete development environment

