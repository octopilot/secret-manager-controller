#!/usr/bin/env python3
"""
Build and copy a single mock server binary for Tilt.

Builds a single mock server binary on host (cross-compilation) and copies it to build_artifacts.
Usage: build_mock_server_binary.py <binary-name>
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path
from datetime import datetime, timezone
import time


def run_command(cmd, check=True, shell=False, env=None, capture_output=True):
    """Run a command and return the result."""
    if isinstance(cmd, str) and not shell:
        cmd = cmd.split()
    
    result = subprocess.run(
        cmd,
        shell=shell,
        env=env,
        capture_output=capture_output,
        text=True
    )
    
    if check and result.returncode != 0:
        print(f"❌ Error: Command failed: {' '.join(cmd) if isinstance(cmd, list) else cmd}", file=sys.stderr)
        if result.stderr:
            print(result.stderr, file=sys.stderr)
        sys.exit(result.returncode)
    
    return result


def get_file_size(filepath):
    """Get file size in bytes."""
    return Path(filepath).stat().st_size


def main():
    """Main build and copy function."""
    if len(sys.argv) < 2:
        print("❌ Error: Binary name required", file=sys.stderr)
        print("Usage: build_mock_server_binary.py <binary-name>", file=sys.stderr)
        sys.exit(1)
    
    binary_name = sys.argv[1]
    if binary_name not in ['gcp-mock-server', 'aws-mock-server', 'azure-mock-server', 'webhook']:
        print(f"❌ Error: Invalid binary name: {binary_name}", file=sys.stderr)
        print("Valid names: gcp-mock-server, aws-mock-server, azure-mock-server, webhook", file=sys.stderr)
        sys.exit(1)
    
    mock_server_dir = Path("pact-broker/mock-server")
    
    # Paths for Linux binary (for container)
    linux_binary = mock_server_dir / f"target/x86_64-unknown-linux-musl/debug/{binary_name}"
    
    # Artifact paths
    artifact_dir = Path("build_artifacts/mock-server")
    artifact_dir.mkdir(parents=True, exist_ok=True)
    
    binary_artifact = artifact_dir / binary_name
    
    # ====================
    # Build Phase
    # ====================
    
    # Delete old binary to force fresh build
    print(f"🧹 Cleaning old {binary_name} binary from target directory...")
    if linux_binary.exists():
        linux_binary.unlink()
    
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
    
    print(f"📋 Build info for {binary_name}:")
    print(f"  Timestamp: {build_timestamp}")
    print(f"  DateTime: {build_datetime}")
    print(f"  Git Hash: {build_git_hash}{build_git_dirty}")
    
    # Build Linux binary for container (cross-compilation)
    print(f"🔨 Building {binary_name} (debug mode)...")
    build_env = os.environ.copy()
    build_env["BUILD_TIMESTAMP"] = build_timestamp
    build_env["BUILD_DATETIME"] = build_datetime
    build_env["BUILD_GIT_HASH"] = f"{build_git_hash}{build_git_dirty}"
    
    # Use Python host-aware-build script
    build_script = Path("scripts/host_aware_build.py")
    if not build_script.exists():
        print(f"❌ Error: Build script not found at {build_script}", file=sys.stderr)
        sys.exit(1)
    
    # Change to mock-server directory for build
    original_cwd = os.getcwd()
    os.chdir(mock_server_dir)
    
    try:
        build_result = run_command(
            ["python3", str(Path(original_cwd) / build_script), 
             "--bin", binary_name],
            check=False,
            env=build_env
        )
        if build_result.returncode != 0:
            print(f"❌ Error: Failed to build {binary_name}", file=sys.stderr)
            sys.exit(1)
    finally:
        os.chdir(original_cwd)
    
    # Verify binary was created
    print(f"🔍 Verifying {binary_name} was built...")
    if not linux_binary.exists():
        print(f"❌ Error: {binary_name} binary not found at {linux_binary}", file=sys.stderr)
        sys.exit(1)
    
    size = get_file_size(linux_binary)
    print(f"  ✅ {binary_name}: {size:,} bytes")
    
    # ====================
    # Copy Phase
    # ====================
    
    # Delete old binary from build_artifacts
    print(f"🧹 Cleaning old {binary_name} from build_artifacts...")
    if binary_artifact.exists():
        binary_artifact.unlink()
    
    # Copy new binary
    print(f"📋 Copying {binary_name}...")
    
    copy_script = Path("scripts/copy_binary.py")
    if not copy_script.exists():
        print(f"❌ Error: Copy script not found at {copy_script}", file=sys.stderr)
        sys.exit(1)
    
    copy_result = subprocess.run(
        ["python3", str(copy_script), str(linux_binary), str(binary_artifact), binary_name],
        capture_output=True,
        text=True
    )
    if copy_result.returncode != 0:
        print(f"❌ Error: Failed to copy {binary_name}", file=sys.stderr)
        if copy_result.stderr:
            print(copy_result.stderr, file=sys.stderr)
        sys.exit(1)
    
    size = get_file_size(binary_artifact)
    print(f"  ✅ Copied {binary_name}: {size:,} bytes")
    print(f"✅ {binary_name} built and copied successfully!")


if __name__ == "__main__":
    main()

