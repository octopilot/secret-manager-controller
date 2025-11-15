#!/usr/bin/env python3
"""
Undeploy controller from Kubernetes.

Deletes all resources deployed via kustomize.
Replaces shell-specific error handling in justfile.
"""

import subprocess
import sys


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def main():
    """Main undeploy function."""
    log_info("🗑️  Undeploying from Kubernetes...")
    
    # Delete resources using kustomize
    # Use check=False to allow graceful failure if resources don't exist
    result = subprocess.run(
        ["kubectl", "delete", "-k", "config/"],
        capture_output=False,
        check=False
    )
    
    # Don't fail if resources don't exist (already deleted or never deployed)
    if result.returncode != 0:
        log_info("⚠️  Some resources may not exist (this is OK)")
    
    log_info("✅ Undeployed")


if __name__ == "__main__":
    main()

