#!/usr/bin/env python3
"""
Update test SecretManagerConfig resources.

This script replaces the inline shell script in Tiltfile for test resource management.
It handles:
- Installing/updating CRD if it has changed (without deleting first)
- Optionally deleting existing test resources (with --delete flag)
- Applying multiple test resources from YAML (dev, stage, prod)

Resources managed:
- test-sops-config (dev): reconcileInterval=1m
- test-sops-config-stage: reconcileInterval=3m
- test-sops-config-prod: reconcileInterval=5m

By default, the script does NOT delete resources before applying, allowing
for incremental updates. Use --delete flag for a clean reset.
"""

import argparse
import os
import subprocess
import sys
import time
from pathlib import Path


def main():
    """Main test resource update function."""
    parser = argparse.ArgumentParser(
        description="Update test SecretManagerConfig resources (dev, stage, prod)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Update all resources without deleting first (default)
  python3 reset_test_resource.py
  
  # Delete all resources before applying (for clean reset)
  python3 reset_test_resource.py --delete
        """
    )
    parser.add_argument(
        "--delete",
        action="store_true",
        help="Delete existing test resources before applying (default: False)"
    )
    
    args = parser.parse_args()
    
    controller_dir = os.getenv("CONTROLLER_DIR", ".")
    crd_yaml_path = Path(controller_dir) / "config/crd/secretmanagerconfig.yaml"
    
    # Define all test resources with their reconcile intervals
    test_resources = [
        {
            "name": "test-sops-config",
            "file": Path("examples/test-sops-config.yaml"),
            "environment": "dev",
            "reconcile_interval": "1m",
        },
        {
            "name": "test-sops-config-stage",
            "file": Path("examples/test-sops-config-stage.yaml"),
            "environment": "stage",
            "reconcile_interval": "3m",
        },
        {
            "name": "test-sops-config-prod",
            "file": Path("examples/test-sops-config-prod.yaml"),
            "environment": "prod",
            "reconcile_interval": "5m",
        },
    ]
    
    # Validate all test resource files exist
    missing_files = [r for r in test_resources if not r["file"].exists()]
    if missing_files:
        print("❌ Error: Test resource YAML files not found:", file=sys.stderr)
        for resource in missing_files:
            print(f"   - {resource['file']}", file=sys.stderr)
        sys.exit(1)
    
    print("🔄 Updating test SecretManagerConfig resources...")
    print(f"📋 Found {len(test_resources)} test resource(s) to update")
    
    # Apply CRD if it exists (will install if missing, update if changed)
    # kubectl apply handles both cases without needing to delete first
    if crd_yaml_path.exists():
        print("📋 Installing/updating CRD (if changed)...")
        crd_apply_result = subprocess.run(
            ["kubectl", "apply", "-f", str(crd_yaml_path)],
            capture_output=True,
            text=True
        )
        
        if crd_apply_result.returncode != 0:
            print(f"⚠️  Warning: CRD apply returned exit code {crd_apply_result.returncode}", file=sys.stderr)
            if crd_apply_result.stderr:
                print(crd_apply_result.stderr, file=sys.stderr)
            # Continue anyway - CRD might already be installed
        else:
            print("✅ CRD installed/updated successfully")
    else:
        print(f"⚠️  Warning: CRD file not found at {crd_yaml_path}", file=sys.stderr)
        print("   Make sure 'secret-manager-controller-crd-gen' has completed", file=sys.stderr)
        # Continue anyway - CRD might already be installed in cluster
    
    # Delete existing test resources only if --delete flag is provided
    if args.delete:
        print("📋 Deleting existing test resources (if exist)...")
        for resource in test_resources:
            delete_result = subprocess.run(
                ["kubectl", "delete", "secretmanagerconfig", resource["name"], "--ignore-not-found=true"],
                capture_output=True,
                text=True
            )
            # Ignore errors - resource may not exist
        
        # Wait a moment for deletion to complete
        time.sleep(1)
    else:
        print("📋 Skipping deletion (use --delete flag to delete before applying)")
    
    # Apply all test resources
    print("")
    print("📋 Applying test SecretManagerConfig resources...")
    print("╔════════════════════════════════════════════════════════════════════════════╗")
    print("║                    Test Resources Summary                                  ║")
    print("╠════════════════════════════════════════════════════════════════════════════╣")
    
    failed_resources = []
    
    for resource in test_resources:
        print(f"║ Resource: {resource['name']:<66} ║")
        print(f"║   Environment: {resource['environment']:<62} ║")
        print(f"║   Reconcile Interval: {resource['reconcile_interval']:<58} ║")
        
        apply_result = subprocess.run(
            ["kubectl", "apply", "-f", str(resource["file"])],
            capture_output=True,
            text=True
        )
        
        if apply_result.returncode == 0:
            print(f"║   Status: ✅ Applied successfully{' ' * 50} ║")
        else:
            print(f"║   Status: ❌ Failed (exit code: {apply_result.returncode}){' ' * 40} ║")
            failed_resources.append(resource)
            if apply_result.stderr:
                # Print error details (truncated if too long)
                error_msg = apply_result.stderr[:60].replace('\n', ' ')
                print(f"║   Error: {error_msg:<60} ║")
        
        if resource != test_resources[-1]:
            print("╠════════════════════════════════════════════════════════════════════════════╣")
    
    print("╚════════════════════════════════════════════════════════════════════════════╝")
    print("")
    
    if failed_resources:
        print(f"❌ Error: Failed to apply {len(failed_resources)} resource(s):", file=sys.stderr)
        for resource in failed_resources:
            print(f"   - {resource['name']}", file=sys.stderr)
        sys.exit(1)
    else:
        print("✅ All test resources applied successfully")
        print("📋 Resources:")
        for resource in test_resources:
            print(f"   - {resource['name']} ({resource['environment']}, reconcileInterval: {resource['reconcile_interval']})")


if __name__ == "__main__":
    main()

