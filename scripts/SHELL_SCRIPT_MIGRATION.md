# Shell Script Migration to Python

This document tracks the migration of all shell scripts to Python replacements.

## Migration Status

### ✅ Completed Migrations

| Shell Script | Python Replacement | Status | Notes |
|-------------|-------------------|--------|-------|
| `copy-binary.sh` | `copy_binary.py` | ✅ Complete | Used by Tiltfile |
| `host-aware-build.sh` | `host_aware_build.py` | ✅ Complete | Used by Tiltfile |
| `setup-sops-key.sh` | `setup_sops_key.py` | ✅ Complete | Renamed from .sh to .py |
| `setup-kind.sh` | `setup_kind.py` | ✅ Complete | Standalone setup script |
| `build-and-push.sh` | `build_and_push.py` | ✅ Complete | Production build script |
| `extract-crd.sh` | `extract_crd.py` | ✅ Complete | CRD extraction utility |
| `pre-commit-rust.sh` | `pre_commit_rust.py` | ✅ Complete | Pre-commit hook |
| `test-sops-complete.sh` | `test-sops-complete.py` | ✅ Complete | Already exists |

### Tiltfile Scripts (migrated to `scripts/tilt/`)

| Original Location | Python Replacement | Status |
|-------------------|-------------------|--------|
| Tiltfile inline script | `scripts/tilt/build_binaries.py` | ✅ Complete |
| Tiltfile inline script | `scripts/tilt/copy_binaries.py` | ✅ Complete |
| Tiltfile inline script | `scripts/tilt/generate_crd.py` | ✅ Complete |
| Tiltfile inline script | `scripts/tilt/cleanup.py` | ✅ Complete |
| Tiltfile inline script | `scripts/tilt/docker_build.py` | ✅ Complete |
| Tiltfile inline script | `scripts/tilt/reset_test_resource.py` | ✅ Complete |

## Script Details

### `copy_binary.py`
**Replaces:** `copy-binary.sh`  
**Usage:** `python3 scripts/copy_binary.py <target_path> <artifact_path> <binary_name>`  
**Functionality:**
- Copies binary from build target to artifacts directory
- Creates MD5 hash file for Docker rebuild triggers
- Better error handling and cross-platform support

### `host_aware_build.py`
**Replaces:** `host-aware-build.sh`  
**Usage:** `python3 scripts/host_aware_build.py [extra cargo args...]`  
**Functionality:**
- Selects build strategy based on host OS/arch
- macOS: Uses `cargo zigbuild`
- Linux x86_64: Uses `cargo build` with musl-gcc linker

### `setup_kind.py`
**Replaces:** `setup-kind.sh`  
**Usage:** `python3 scripts/setup_kind.py`  
**Functionality:**
- Creates local Kind cluster
- Sets up Docker registry
- Configures cluster registry access

### `build_and_push.py`
**Replaces:** `build-and-push.sh`  
**Usage:** `python3 scripts/build_and_push.py [tag] [registry]`  
**Functionality:**
- Builds Docker image using buildx
- Pushes to registry
- Supports multi-platform builds

### `extract_crd.py`
**Replaces:** `extract-crd.sh`  
**Usage:** `python3 scripts/extract_crd.py <image-name> <output-path>`  
**Functionality:**
- Extracts CRD from Docker image
- Cleans ANSI escape sequences
- Validates YAML content

### `pre_commit_rust.py`
**Replaces:** `pre-commit-rust.sh`  
**Usage:** `python3 scripts/pre_commit_rust.py`  
**Functionality:**
- Runs `cargo fmt --check`
- Runs `cargo check`
- Auto-formats if needed

## Migration Checklist

- [x] Create Python replacements for all shell scripts
- [x] Update Tiltfile to use Python scripts
- [x] Make all Python scripts executable
- [x] Test Python scripts work correctly
- [ ] Update documentation references
- [ ] Remove shell scripts
- [ ] Update CI/CD pipelines if needed

## Benefits

1. **Cross-platform:** Python works on macOS, Linux, and Windows
2. **Better error handling:** More robust exception handling
3. **Maintainability:** Easier to read and maintain
4. **Testability:** Can be unit tested
5. **Consistency:** Follows "zero shell script policy"
6. **Type safety:** Can use type hints for better code quality

## Next Steps

1. Test all Python scripts in development environment
2. Update any documentation that references shell scripts
3. Remove shell scripts after verification
4. Update CI/CD pipelines to use Python scripts

