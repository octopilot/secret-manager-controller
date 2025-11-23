#!/usr/bin/env python3
"""
Build all Rust binaries for Tilt development.

Builds all binaries (controller, mock servers, webhook) in a single cargo build.
This is more efficient than building each binary separately.
"""

import os
import platform
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path


def run_command(cmd, check=True, capture_output=True, env=None):
    """Run a command and return the result."""
    result = subprocess.run(
        cmd,
        shell=True,
        capture_output=capture_output,
        text=True,
        env=env
    )
    if capture_output:
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr and result.returncode != 0:
            print(result.stderr, end="", file=sys.stderr)
    if check and result.returncode != 0:
        sys.exit(result.returncode)
    return result


def get_git_hash():
    """Get git hash for build info."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            capture_output=True,
            text=True,
            check=True
        )
        git_hash = result.stdout.strip()
        
        # Check if git is dirty
        diff_result = subprocess.run(
            ["git", "diff", "--quiet"],
            capture_output=True
        )
        dirty_suffix = "-dirty" if diff_result.returncode != 0 else ""
        return f"{git_hash}{dirty_suffix}"
    except Exception:
        return "unknown"


def main():
    """Build all binaries."""
    print("🔨 Building all Rust binaries...")
    
    # Generate build info (required by build.rs)
    build_timestamp = str(int(time.time()))
    build_datetime = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    build_git_hash = get_git_hash()
    
    print(f"📋 Build info:")
    print(f"  Timestamp: {build_timestamp}")
    print(f"  DateTime: {build_datetime}")
    print(f"  Git Hash: {build_git_hash}")
    
    os_name = platform.system()
    arch = platform.machine()
    
    target = "x86_64-unknown-linux-musl"
    
    # Set up build environment with build info
    build_env = os.environ.copy()
    build_env["BUILD_TIMESTAMP"] = build_timestamp
    build_env["BUILD_DATETIME"] = build_datetime
    build_env["BUILD_GIT_HASH"] = build_git_hash
    
    # Build all binaries in one go using workspace
    print("🔨 Building workspace binaries (debug mode)...")
    
    if os_name == "Darwin":
        # macOS: Use cargo zigbuild (like microservices)
        print("  Using cargo-zigbuild for cross-compilation (macOS)")
        result = run_command(
            f"cargo zigbuild --target {target} --workspace --bins",
            check=False,
            env=build_env
        )
        if result.returncode != 0:
            print("❌ Build failed", file=sys.stderr)
            sys.exit(1)
    elif os_name == "Linux" and arch == "x86_64":
        # Linux x86_64: Use musl-gcc linker
        print("  Using musl-gcc linker (Linux x86_64)")
        build_env["CC_x86_64_unknown_linux_musl"] = "musl-gcc"
        build_env["CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER"] = "musl-gcc"
        result = subprocess.run(
            ["cargo", "build", "--target", target, "--workspace", "--bins"],
            env=build_env,
            capture_output=True,
            text=True
        )
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr:
            print(result.stderr, end="", file=sys.stderr)
        if result.returncode != 0:
            print("❌ Build failed", file=sys.stderr)
            sys.exit(1)
    else:
        # Fallback: Try regular cargo build
        print(f"  Using standard cargo build (OS: {os_name}, Arch: {arch})")
        result = run_command(
            f"cargo build --target {target} --workspace --bins",
            check=False,
            env=build_env
        )
        if result.returncode != 0:
            print("❌ Build failed", file=sys.stderr)
            sys.exit(1)
    
    # Verify binaries exist
    target_dir = Path(f"target/{target}/debug")
    binaries = [
        "secret-manager-controller",
        "crdgen",
        "msmctl",
        "gcp-mock-server",
        "aws-mock-server",
        "azure-mock-server",
        "webhook",
        "manager",
    ]
    
    print("🔍 Verifying binaries were built...")
    all_exist = True
    for binary in binaries:
        binary_path = target_dir / binary
        if binary_path.exists():
            size = binary_path.stat().st_size
            print(f"  ✅ {binary}: {size:,} bytes")
        else:
            print(f"  ❌ {binary}: NOT FOUND", file=sys.stderr)
            all_exist = False
    
    if not all_exist:
        print("❌ Build failed: Some binaries not found", file=sys.stderr)
        sys.exit(1)
    
    print("✅ All binaries built successfully!")


if __name__ == "__main__":
    main()

