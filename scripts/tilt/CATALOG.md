# Tiltfile Inline Scripts Catalog

This document catalogs all inline shell scripts in the Tiltfile and their Python replacements.

## Active Scripts

### 1. `secret-manager-controller-build` (Lines 85-146)
**Purpose:** Build Rust binaries for secret-manager-controller  
**Replacement:** `scripts/tilt/build_binaries.py`

**Original Script Location:** Tiltfile lines 85-146  
**Functionality:**
- Deletes old binaries to force fresh build
- Cleans Cargo build artifacts
- Generates build timestamp and git hash
- Builds Linux binaries (cross-compilation)
- Builds native crdgen
- Verifies binaries were created

**Environment Variables Used:**
- `CONTROLLER_DIR` (default: `.`)
- `BINARY_NAME` (default: `secret-manager-controller`)
- `BUILD_TIMESTAMP` (generated)
- `BUILD_DATETIME` (generated)
- `BUILD_GIT_HASH` (generated)

---

### 2. `secret-manager-controller-copy` (Lines 164-210)
**Purpose:** Copy binaries to build_artifacts directory  
**Replacement:** `scripts/tilt/copy_binaries.py`

**Original Script Location:** Tiltfile lines 164-210  
**Functionality:**
- Creates build_artifacts directory
- Deletes old binaries
- Copies binaries using copy-binary.sh script
- Outputs MD5 hashes and file sizes
- Verifies binaries were copied successfully

**Environment Variables Used:**
- `CONTROLLER_DIR` (default: `.`)
- `BINARY_NAME` (default: `secret-manager-controller`)

**Dependencies:**
- `scripts/copy-binary.sh`

---

### 3. `secret-manager-controller-crd-gen` (Lines 224-269)
**Purpose:** Generate CRD using crdgen binary  
**Replacement:** `scripts/tilt/generate_crd.py`

**Original Script Location:** Tiltfile lines 224-269  
**Functionality:**
- Creates config/crd directory
- Runs native crdgen binary
- Validates generated YAML
- Deletes existing CRD
- Applies CRD to Kubernetes cluster

**Environment Variables Used:**
- `CONTROLLER_DIR` (default: `.`)
- `RUST_LOG` (set to `off`)

**Dependencies:**
- Native crdgen binary at `{CONTROLLER_DIR}/target/debug/crdgen`

---

### 4. `secret-manager-controller-cleanup` (Lines 288-308)
**Purpose:** Cleanup controller pod and images before rebuild  
**Replacement:** `scripts/tilt/cleanup.py`

**Original Script Location:** Tiltfile lines 288-308  
**Functionality:**
- Deletes controller pods
- Removes Docker images (tilt and tilt-* tags)
- Cleans up kind registry cache
- Removes dangling images

**Environment Variables Used:**
- `IMAGE_NAME` (default: `localhost:5002/secret-manager-controller`)
- `CONTROLLER_NAME` (default: `secret-manager-controller`)

---

### 5. `custom_build` Command (Lines 317-328)
**Purpose:** Docker build with cleanup and timestamp  
**Replacement:** `scripts/tilt/docker_build.py`

**Original Script Location:** Tiltfile lines 317-328  
**Functionality:**
- Cleans up old Docker images
- Builds Docker image with timestamp
- Tags image with tilt-{timestamp}
- Pushes to registry

**Environment Variables Used:**
- `IMAGE_NAME` (default: `localhost:5002/secret-manager-controller`)
- `CONTROLLER_NAME` (default: `secret-manager-controller`)
- `CONTROLLER_DIR` (default: `.`)
- `EXPECTED_REF` (default: `{IMAGE_NAME}:tilt`)

**Dependencies:**
- `{CONTROLLER_DIR}/Dockerfile.dev`

---

### 6. `test-resource-update` (Lines 404-423)
**Purpose:** Update test SecretManagerConfig resources (dev, stage, prod)  
**Replacement:** `scripts/tilt/reset_test_resource.py`

**Original Script Location:** Tiltfile lines 404-423  
**Functionality:**
- Installs/updates CRD if it has changed (without deleting first)
- Optionally deletes existing test resources (with `--delete` flag, default: False)
- Waits for deletion to complete (if --delete flag used)
- Applies multiple test resources from YAML with different reconcile intervals

**Resources managed:**
- `test-sops-config` (dev): reconcileInterval=1m
- `test-sops-config-stage`: reconcileInterval=3m
- `test-sops-config-prod`: reconcileInterval=5m

**Dependencies:**
- `examples/test-sops-config.yaml`
- `examples/test-sops-config-stage.yaml`
- `examples/test-sops-config-prod.yaml`

---

## Commented Out Scripts

### 7. `secret-manager-controller-fmt-check` (Lines 40-47) - COMMENTED
**Purpose:** Check code formatting  
**Status:** Commented out, not active  
**Replacement:** Not created (can be added if needed)

**Functionality:**
- Runs `cargo fmt --all -- --check`
- Reports formatting errors

---

### 8. `secret-manager-controller-clippy` (Lines 59-66) - COMMENTED
**Purpose:** Run clippy linting  
**Status:** Commented out, not active  
**Replacement:** Not created (can be added if needed)

**Functionality:**
- Runs `cargo clippy --all-targets --all-features -- -D warnings`
- Reports clippy warnings

---

## Migration Guide

To migrate from inline scripts to Python replacements:

1. **Replace inline `cmd='''...'''` with Python script call:**

   **Before:**
   ```python
   local_resource(
       'resource-name',
       cmd='''
           # shell script here
       ''',
       ...
   )
   ```

   **After:**
   ```python
   local_resource(
       'resource-name',
       cmd='python3 scripts/tilt/script_name.py',
       ...
   )
   ```

2. **Set environment variables if needed:**
   ```python
   local_resource(
       'resource-name',
       cmd='python3 scripts/tilt/script_name.py',
       env={
           'CONTROLLER_DIR': CONTROLLER_DIR,
           'BINARY_NAME': BINARY_NAME,
       },
       ...
   )
   ```

3. **Update dependencies if paths changed:**
   - Python scripts may have different dependency requirements
   - Check script documentation for required dependencies

## Benefits of Python Replacements

1. **Better Error Handling:** Python provides more robust error handling and exception management
2. **Cross-Platform:** Works on macOS, Linux, and Windows (with appropriate tools)
3. **Maintainability:** Easier to read, test, and maintain than inline shell scripts
4. **Testability:** Python scripts can be unit tested
5. **Consistency:** Follows the project's "zero shell script policy"
6. **Type Safety:** Can use type hints for better code quality
7. **Linting:** Can use Python linters (pylint, mypy, etc.)

## Notes

- All Python scripts are executable (`chmod +x`)
- Scripts use environment variables for configuration
- Scripts maintain the same functionality as original shell scripts
- Error handling is improved with proper exit codes and error messages
- Scripts can be run independently for testing

