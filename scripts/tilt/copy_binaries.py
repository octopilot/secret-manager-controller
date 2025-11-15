#!/usr/bin/env python3
"""
Copy binaries to build_artifacts directory.

This script replaces the inline shell script in Tiltfile for copying binaries.
It handles:
- Creating build_artifacts directory
- Copying binaries with verification
- Outputting MD5 hashes and sizes
"""

import hashlib
import os
import shutil
import sys
from pathlib import Path


def get_md5_hash(filepath):
    """Calculate MD5 hash of a file."""
    hash_md5 = hashlib.md5()
    with open(filepath, "rb") as f:
        for chunk in iter(lambda: f.read(4096), b""):
            hash_md5.update(chunk)
    return hash_md5.hexdigest()


def get_file_size(filepath):
    """Get file size in bytes."""
    return Path(filepath).stat().st_size


def main():
    """Main copy function."""
    controller_dir = os.getenv("CONTROLLER_DIR", ".")
    binary_name = os.getenv("BINARY_NAME", "secret-manager-controller")
    
    # Paths
    binary_path = Path(controller_dir) / "target/x86_64-unknown-linux-musl/debug" / binary_name
    crdgen_path = Path(controller_dir) / "target/x86_64-unknown-linux-musl/debug/crdgen"
    msmctl_path = Path(controller_dir) / "target/x86_64-unknown-linux-musl/debug/msmctl"
    artifact_path = Path("build_artifacts") / binary_name
    crdgen_artifact_path = Path("build_artifacts/crdgen")
    msmctl_artifact_path = Path("build_artifacts/msmctl")
    
    # Ensure build_artifacts directory exists
    Path("build_artifacts").mkdir(parents=True, exist_ok=True)
    
    # Delete old binaries to ensure fresh copy
    print("🧹 Cleaning old binaries from build_artifacts...")
    for path in [artifact_path, crdgen_artifact_path, msmctl_artifact_path]:
        if path.exists():
            path.unlink()
    
    # Copy new binaries with error checking
    print("📋 Copying new binaries...")
    copy_error = False
    
    # Use Python copy_binary script
    copy_script = Path("scripts/copy_binary.py")
    if not copy_script.exists():
        print(f"❌ Error: Copy script not found at {copy_script}", file=sys.stderr)
        sys.exit(1)
    
    # Copy main binary
    import subprocess
    copy_result = subprocess.run(
        ["python3", str(copy_script), str(binary_path), str(artifact_path), binary_name],
        capture_output=True,
        text=True
    )
    if copy_result.returncode != 0:
        print(f"❌ Error: Failed to copy {binary_name}", file=sys.stderr)
        if copy_result.stderr:
            print(copy_result.stderr, file=sys.stderr)
        copy_error = True
    
    # Copy crdgen
    crdgen_copy_result = subprocess.run(
        ["python3", str(copy_script), str(crdgen_path), str(crdgen_artifact_path), "crdgen"],
        capture_output=True,
        text=True
    )
    if crdgen_copy_result.returncode != 0:
        print("❌ Error: Failed to copy crdgen", file=sys.stderr)
        if crdgen_copy_result.stderr:
            print(crdgen_copy_result.stderr, file=sys.stderr)
        copy_error = True
    
    # Copy msmctl
    msmctl_copy_result = subprocess.run(
        ["python3", str(copy_script), str(msmctl_path), str(msmctl_artifact_path), "msmctl"],
        capture_output=True,
        text=True
    )
    if msmctl_copy_result.returncode != 0:
        print("❌ Error: Failed to copy msmctl", file=sys.stderr)
        if msmctl_copy_result.stderr:
            print(msmctl_copy_result.stderr, file=sys.stderr)
        copy_error = True
    
    # Output hashes to verify what was copied
    print("")
    print("📊 Binary Hashes (verify what was built):")
    binary_ok = False
    crdgen_ok = False
    msmctl_ok = False
    
    if artifact_path.exists():
        md5_hash = get_md5_hash(artifact_path)
        file_size = get_file_size(artifact_path)
        print(f"  {binary_name}: {md5_hash}")
        print(f"    Size: {file_size} bytes")
        binary_ok = True
    else:
        print(f"  ❌ {binary_name} not found!", file=sys.stderr)
        copy_error = True
    
    if crdgen_artifact_path.exists():
        md5_hash = get_md5_hash(crdgen_artifact_path)
        file_size = get_file_size(crdgen_artifact_path)
        print(f"  crdgen: {md5_hash}")
        print(f"    Size: {file_size} bytes")
        crdgen_ok = True
    else:
        print("  ❌ crdgen not found!", file=sys.stderr)
        copy_error = True
    
    if msmctl_artifact_path.exists():
        md5_hash = get_md5_hash(msmctl_artifact_path)
        file_size = get_file_size(msmctl_artifact_path)
        print(f"  msmctl: {md5_hash}")
        print(f"    Size: {file_size} bytes")
        msmctl_ok = True
    else:
        print("  ❌ msmctl not found!", file=sys.stderr)
        copy_error = True
    
    # Only report success if all binaries exist
    if copy_error or not binary_ok or not crdgen_ok or not msmctl_ok:
        print("❌ Binary copy failed - check errors above", file=sys.stderr)
        sys.exit(1)
    
    print("✅ Binaries copied successfully")


if __name__ == "__main__":
    main()
