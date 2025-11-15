#!/usr/bin/env python3
"""
Build Rust binaries for secret-manager-controller.

This script replaces the inline shell script in Tiltfile for building binaries.
It handles:
- Cleaning old binaries
- Building Linux binaries (cross-compilation)
- Building native crdgen
- Verifying binaries were created
"""

import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path


def run_command(cmd, check=True, shell=False, env=None):
    """Run a command and return the result."""
    result = subprocess.run(cmd, shell=shell, check=check, capture_output=True, text=True, env=env)
    if result.stdout:
        print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)
    return result


def main():
    """Main build function."""
    controller_dir = os.getenv("CONTROLLER_DIR", ".")
    binary_name = os.getenv("BINARY_NAME", "secret-manager-controller")
    
    # Paths
    linux_binary = Path(controller_dir) / "target/x86_64-unknown-linux-musl/debug" / binary_name
    linux_crdgen = Path(controller_dir) / "target/x86_64-unknown-linux-musl/debug/crdgen"
    linux_msmctl = Path(controller_dir) / "target/x86_64-unknown-linux-musl/debug/msmctl"
    native_crdgen = Path(controller_dir) / "target/debug/crdgen"
    native_msmctl = Path(controller_dir) / "target/debug/msmctl"
    
    # Delete old binaries to force fresh build
    print("🧹 Cleaning old binaries from target directory...")
    for path in [linux_binary, linux_crdgen, linux_msmctl, native_crdgen, native_msmctl]:
        if path.exists():
            path.unlink()
    
    # Clean Cargo build artifacts
    # Note: cargo clean doesn't support --bin flag, so we clean the entire package/target
    print("🧹 Cleaning Cargo build artifacts...")
    clean_commands = [
        ["cargo", "clean", "-p", "secret-manager-controller", "--target", "x86_64-unknown-linux-musl"],
        ["cargo", "clean", "-p", "secret-manager-controller"],  # Clean native target as well
    ]
    for cmd in clean_commands:
        run_command(cmd, check=False)
    
    # Generate fresh timestamp for this build
    build_timestamp = str(int(time.time()))
    build_datetime = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    
    # Get git hash
    try:
        git_hash_result = run_command(["git", "rev-parse", "--short", "HEAD"], check=False)
        build_git_hash = git_hash_result.stdout.strip() if git_hash_result.returncode == 0 else "unknown"
    except Exception:
        build_git_hash = "unknown"
    
    # Check if git is dirty
    try:
        git_diff_result = run_command(["git", "diff", "--quiet"], check=False)
        build_git_dirty = "-dirty" if git_diff_result.returncode != 0 else ""
    except Exception:
        build_git_dirty = ""
    
    print("📋 Build info:")
    print(f"  Timestamp: {build_timestamp}")
    print(f"  DateTime: {build_datetime}")
    print(f"  Git Hash: {build_git_hash}{build_git_dirty}")
    
    # Build Linux binaries for container (cross-compilation)
    print("🔨 Building Linux binaries (debug mode)...")
    build_env = os.environ.copy()
    build_env["BUILD_TIMESTAMP"] = build_timestamp
    build_env["BUILD_DATETIME"] = build_datetime
    build_env["BUILD_GIT_HASH"] = f"{build_git_hash}{build_git_dirty}"
    
    # Use Python host-aware-build script
    build_script = Path(controller_dir) / "scripts/host_aware_build.py"
    if not build_script.exists():
        print(f"❌ Error: Build script not found at {build_script}", file=sys.stderr)
        sys.exit(1)
    
    build_result = run_command(
        ["python3", str(build_script), "--bin", binary_name, "--bin", "crdgen", "--bin", "msmctl"],
        check=False,
        env=build_env
    )
    if build_result.returncode != 0:
        print("❌ Error: Failed to build Linux binaries", file=sys.stderr)
        sys.exit(1)
    
    # Build native binaries for host execution (crdgen and msmctl)
    print("🔨 Building native binaries (crdgen, msmctl) (debug mode)...")
    cargo_build_env = os.environ.copy()
    cargo_build_env["BUILD_TIMESTAMP"] = build_timestamp
    cargo_build_env["BUILD_DATETIME"] = build_datetime
    cargo_build_env["BUILD_GIT_HASH"] = f"{build_git_hash}{build_git_dirty}"
    
    cargo_build_result = run_command(
        ["cargo", "build", "--bin", "crdgen", "--bin", "msmctl"],
        check=False,
        env=cargo_build_env
    )
    if cargo_build_result.returncode != 0:
        print("❌ Error: Failed to build native binaries", file=sys.stderr)
        sys.exit(1)
    
    # Verify binaries were created
    print("🔍 Verifying binaries were built...")
    build_error = False
    
    if not linux_binary.exists():
        print(f"❌ Error: Binary not found at {linux_binary}", file=sys.stderr)
        build_error = True
    else:
        print(f"  ✅ {binary_name} built successfully")
    
    if not linux_crdgen.exists():
        print(f"❌ Error: crdgen not found at {linux_crdgen}", file=sys.stderr)
        build_error = True
    else:
        print("  ✅ crdgen (Linux) built successfully")
    
    if not linux_msmctl.exists():
        print(f"❌ Error: msmctl (Linux) not found at {linux_msmctl}", file=sys.stderr)
        build_error = True
    else:
        print("  ✅ msmctl (Linux) built successfully")
    
    if not native_crdgen.exists():
        print(f"❌ Error: Native crdgen not found at {native_crdgen}", file=sys.stderr)
        build_error = True
    else:
        print("  ✅ crdgen (native) built successfully")
    
    if not native_msmctl.exists():
        print(f"❌ Error: Native msmctl not found at {native_msmctl}", file=sys.stderr)
        build_error = True
    else:
        print("  ✅ msmctl (native) built successfully")
    
    if build_error:
        print("❌ Build failed - some binaries are missing", file=sys.stderr)
        sys.exit(1)
    
    print("✅ Build complete - all binaries verified")


if __name__ == "__main__":
    main()
