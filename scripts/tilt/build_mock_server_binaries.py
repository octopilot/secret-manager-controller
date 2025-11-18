#!/usr/bin/env python3
"""
Build and copy mock server binaries for Tilt.

Builds the mock server binaries on host (cross-compilation) and copies them to build_artifacts.
Similar to build_and_copy_binaries.py but for the pact-mock-server package.
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
    mock_server_dir = Path("pact-broker/mock-server")
    
    # Paths for Linux binaries (for container)
    linux_gcp = mock_server_dir / "target/x86_64-unknown-linux-musl/debug/gcp-mock-server"
    linux_aws = mock_server_dir / "target/x86_64-unknown-linux-musl/debug/aws-mock-server"
    linux_azure = mock_server_dir / "target/x86_64-unknown-linux-musl/debug/azure-mock-server"
    
    # Artifact paths
    artifact_dir = Path("build_artifacts/mock-server")
    artifact_dir.mkdir(parents=True, exist_ok=True)
    
    gcp_artifact = artifact_dir / "gcp-mock-server"
    aws_artifact = artifact_dir / "aws-mock-server"
    azure_artifact = artifact_dir / "azure-mock-server"
    
    # ====================
    # Build Phase
    # ====================
    
    # Delete old binaries to force fresh build
    print("🧹 Cleaning old binaries from target directory...")
    for path in [linux_gcp, linux_aws, linux_azure]:
        if path.exists():
            path.unlink()
    
    # Clean Cargo build artifacts
    print("🧹 Cleaning Cargo build artifacts...")
    run_command(
        ["cargo", "clean", "-p", "pact-mock-server", "--target", "x86_64-unknown-linux-musl"],
        check=False
    )
    
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
    print("🔨 Building mock server binaries (debug mode)...")
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
             "--bin", "gcp-mock-server", 
             "--bin", "aws-mock-server", 
             "--bin", "azure-mock-server"],
            check=False,
            env=build_env
        )
        if build_result.returncode != 0:
            print("❌ Error: Failed to build mock server binaries", file=sys.stderr)
            sys.exit(1)
    finally:
        os.chdir(original_cwd)
    
    # Verify binaries were created
    print("🔍 Verifying binaries were built...")
    build_error = False
    
    for binary_path, name in [
        (linux_gcp, "gcp-mock-server"),
        (linux_aws, "aws-mock-server"),
        (linux_azure, "azure-mock-server"),
    ]:
        if not binary_path.exists():
            print(f"❌ Error: {name} binary not found at {binary_path}", file=sys.stderr)
            build_error = True
        else:
            size = get_file_size(binary_path)
            print(f"  ✅ {name}: {size:,} bytes")
    
    if build_error:
        print("❌ Error: One or more binaries failed to build", file=sys.stderr)
        sys.exit(1)
    
    # ====================
    # Copy Phase
    # ====================
    
    # Delete old binaries from build_artifacts
    print("🧹 Cleaning old binaries from build_artifacts...")
    for path in [gcp_artifact, aws_artifact, azure_artifact]:
        if path.exists():
            path.unlink()
    
    # Copy new binaries
    print("📋 Copying new binaries...")
    copy_error = False
    
    copy_script = Path("scripts/copy_binary.py")
    if not copy_script.exists():
        print(f"❌ Error: Copy script not found at {copy_script}", file=sys.stderr)
        sys.exit(1)
    
    for source, dest, name in [
        (linux_gcp, gcp_artifact, "gcp-mock-server"),
        (linux_aws, aws_artifact, "aws-mock-server"),
        (linux_azure, azure_artifact, "azure-mock-server"),
    ]:
        copy_result = subprocess.run(
            ["python3", str(copy_script), str(source), str(dest), name],
            capture_output=True,
            text=True
        )
        if copy_result.returncode != 0:
            print(f"❌ Error: Failed to copy {name}", file=sys.stderr)
            if copy_result.stderr:
                print(copy_result.stderr, file=sys.stderr)
            copy_error = True
        else:
            size = get_file_size(dest)
            print(f"  ✅ Copied {name}: {size:,} bytes")
    
    if copy_error:
        print("❌ Error: One or more binaries failed to copy", file=sys.stderr)
        sys.exit(1)
    
    print("✅ Mock server binaries built and copied successfully!")


if __name__ == "__main__":
    main()

