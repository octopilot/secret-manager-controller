# Tilt Script Replacements

This directory contains Python replacements for inline shell scripts in the Tiltfile.

## Scripts

### `build_binaries.py`
Replaces the inline script for `secret-manager-controller-build` resource.
- Cleans old binaries
- Builds Linux binaries (cross-compilation)
- Builds native crdgen
- Verifies binaries were created

**Environment Variables:**
- `CONTROLLER_DIR` - Controller directory (default: `.`)
- `BINARY_NAME` - Binary name (default: `secret-manager-controller`)

### `copy_binaries.py`
Replaces the inline script for `secret-manager-controller-copy` resource.
- Creates build_artifacts directory
- Copies binaries with verification
- Outputs MD5 hashes and sizes

**Environment Variables:**
- `CONTROLLER_DIR` - Controller directory (default: `.`)
- `BINARY_NAME` - Binary name (default: `secret-manager-controller`)

### `generate_crd.py`
Replaces the inline script for `secret-manager-controller-crd-gen` resource.
- Runs crdgen binary
- Validates generated YAML
- Applies CRD to Kubernetes cluster

**Environment Variables:**
- `CONTROLLER_DIR` - Controller directory (default: `.`)

### `cleanup.py`
Replaces the inline script for `secret-manager-controller-cleanup` resource.
- Deletes controller pods
- Removes Docker images
- Cleans up kind registry cache

**Environment Variables:**
- `IMAGE_NAME` - Docker image name (default: `localhost:5002/secret-manager-controller`)
- `CONTROLLER_NAME` - Controller name (default: `secret-manager-controller`)

### `docker_build.py`
Replaces the inline script for `custom_build` command.
- Cleans up old images
- Builds Docker image with timestamp
- Tags and pushes to registry

**Environment Variables:**
- `IMAGE_NAME` - Docker image name (default: `localhost:5002/secret-manager-controller`)
- `CONTROLLER_NAME` - Controller name (default: `secret-manager-controller`)
- `CONTROLLER_DIR` - Controller directory (default: `.`)
- `EXPECTED_REF` - Expected image reference (default: `{IMAGE_NAME}:tilt`)

### `reset_test_resource.py`
Replaces the inline script for `test-resource-update` resource.
- Installs/updates CRD if it has changed (without deleting first)
- Optionally deletes existing test resources (with `--delete` flag)
- Applies multiple test resources from YAML (dev, stage, prod)

**Resources managed:**
- `test-sops-config` (dev): reconcileInterval=1m
- `test-sops-config-stage`: reconcileInterval=3m
- `test-sops-config-prod`: reconcileInterval=5m

**Arguments:**
- `--delete` - Delete existing test resources before applying (default: False)

**Environment variables:**
- `CONTROLLER_DIR` - Controller directory (default: `.`)

**Usage:**
```bash
# Update all resources without deleting (default)
python3 scripts/tilt/reset_test_resource.py

# Delete all resources before applying (clean reset)
python3 scripts/tilt/reset_test_resource.py --delete
```

## Usage

These scripts are designed to be called from the Tiltfile. To use them, replace the inline `cmd='''...'''` blocks with calls to these Python scripts:

```python
local_resource(
    'resource-name',
    cmd='python3 scripts/tilt/script_name.py',
    deps=[...],
    ...
)
```

## Benefits

1. **Better error handling** - Python provides more robust error handling than shell scripts
2. **Cross-platform** - Python scripts work on macOS, Linux, and Windows (with appropriate tools)
3. **Maintainability** - Easier to read and maintain than inline shell scripts
4. **Testability** - Python scripts can be unit tested
5. **Consistency** - Follows the project's "zero shell script policy"

