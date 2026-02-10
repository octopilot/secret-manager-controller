#!/usr/bin/env python3
"""
Copy mock server binaries from target/ to build_artifacts/.

This script copies the binaries built by build_all_binaries.py
to the build_artifacts directory for Docker packaging.
"""

import shutil
import sys
from pathlib import Path


def main():
    """Copy mock server binaries to build_artifacts."""
    target_dir = Path("target/x86_64-unknown-linux-musl/debug")
    artifact_dir = Path("build_artifacts/mock-server")
    artifact_dir.mkdir(parents=True, exist_ok=True)
    
    binaries = {
        "gcp-mock-server": "gcp-mock-server",
        "aws-mock-server": "aws-mock-server",
        "azure-mock-server": "azure-mock-server",
        "webhook": "webhook",
        "manager": "manager",
        "postgres-manager": "postgres-manager",
    }
    
    print("📋 Copying mock server binaries to build_artifacts...")
    
    all_copied = True
    for source_name, dest_name in binaries.items():
        source = target_dir / source_name
        dest = artifact_dir / dest_name
        
        if not source.exists():
            print(f"  ❌ {source_name}: NOT FOUND in {target_dir}", file=sys.stderr)
            all_copied = False
            continue
        
        # Copy binary
        shutil.copy2(source, dest)
        size = dest.stat().st_size
        print(f"  ✅ {dest_name}: {size:,} bytes")
    
    if not all_copied:
        print("❌ Failed: Some binaries not found", file=sys.stderr)
        sys.exit(1)
    
    print("✅ All mock server binaries copied successfully!")


if __name__ == "__main__":
    main()

