#!/usr/bin/env python3
"""
Update test SecretManagerConfig resources.

This script replaces the inline shell script in Tiltfile for test resource management.
It handles:
- Installing/updating CRD if it has changed (without deleting first)
- Optionally deleting existing test resources (with --delete flag)
- Applying multiple test resources from YAML (dev, stage, prod)

Resources managed:
- test-sops-config (tilt): reconcileInterval=1m (gitops/cluster/fluxcd/env/tilt/)
- test-sops-config-stage: reconcileInterval=3m (gitops/cluster/fluxcd/env/stage/)
- test-sops-config-prod: reconcileInterval=5m (gitops/cluster/fluxcd/env/prod/)

Note: This script manages FluxCD resources. For ArgoCD resources, use:
  kubectl apply -k gitops/cluster/argocd/env/{env}

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
    # Resources are now organized in gitops/cluster/fluxcd/env/{env}/ directories
    # Each environment includes namespace.yaml, gitrepository.yaml, and secretmanagerconfig.yaml
    # Note: ArgoCD resources are in gitops/cluster/argocd/env/{env}/ and managed separately
    test_resources = [
        {
            "name": "test-sops-config",
            "kustomize_path": Path("gitops/cluster/fluxcd/env/tilt"),
            "namespace": "tilt",
            "environment": "tilt",
            "reconcile_interval": "1m",
        },
        {
            "name": "test-sops-config-stage",
            "kustomize_path": Path("gitops/cluster/fluxcd/env/stage"),
            "namespace": "stage",
            "environment": "stage",
            "reconcile_interval": "3m",
        },
        {
            "name": "test-sops-config-prod",
            "kustomize_path": Path("gitops/cluster/fluxcd/env/prod"),
            "namespace": "prod",
            "environment": "prod",
            "reconcile_interval": "5m",
        },
    ]
    
    # Validate all test resource kustomize paths exist
    missing_paths = [r for r in test_resources if not r["kustomize_path"].exists()]
    if missing_paths:
        print("❌ Error: Test resource kustomize paths not found:", file=sys.stderr)
        for resource in missing_paths:
            print(f"   - {resource['kustomize_path']}", file=sys.stderr)
        sys.exit(1)
    
    # Validate kustomization.yaml exists in each path
    missing_kustomizations = [
        r for r in test_resources 
        if not (r["kustomize_path"] / "kustomization.yaml").exists()
    ]
    if missing_kustomizations:
        print("❌ Error: kustomization.yaml not found in:", file=sys.stderr)
        for resource in missing_kustomizations:
            print(f"   - {resource['kustomize_path']}", file=sys.stderr)
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
            # Delete SecretManagerConfig
            delete_result = subprocess.run(
                ["kubectl", "delete", "secretmanagerconfig", resource["name"], 
                 "-n", resource["namespace"], "--ignore-not-found=true"],
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
    print("Test Resources Summary")
    
    failed_resources = []
    
    for resource in test_resources:
        print(f"Resource: {resource['name']}")
        print(f"  Environment: {resource['environment']}")
        print(f"  Namespace: {resource['namespace']}")
        print(f"  Reconcile Interval: {resource['reconcile_interval']}")
        
        # Apply using kustomize to ensure namespace and all resources are created
        # First ensure namespace exists, then apply SecretManagerConfig
        # GitRepository might already exist or require different permissions
        
        # Step 1: Apply namespace
        namespace_file = resource["kustomize_path"] / "namespace.yaml"
        namespace_result = subprocess.run(
            ["kubectl", "apply", "-f", str(namespace_file)],
            capture_output=True,
            text=True
        )
        
        # Step 2: Apply SecretManagerConfig (main resource we care about)
        secretmanagerconfig_file = resource["kustomize_path"] / "secretmanagerconfig.yaml"
        apply_result = subprocess.run(
            ["kubectl", "apply", "-f", str(secretmanagerconfig_file)],
            capture_output=True,
            text=True
        )
        
        # Step 3: Try to apply GitRepository (optional - might fail if already exists or no permissions)
        gitrepository_file = resource["kustomize_path"] / "gitrepository.yaml"
        gitrepo_result = subprocess.run(
            ["kubectl", "apply", "-f", str(gitrepository_file)],
            capture_output=True,
            text=True
        )
        
        # Success if SecretManagerConfig was applied successfully
        # GitRepository failure is acceptable (might already exist or require different permissions)
        if apply_result.returncode == 0:
            print(f"  Status: ✅ Applied successfully")
            # Only show GitRepository note if it failed AND it's not a common expected error
            if gitrepo_result.returncode != 0:
                gitrepo_error = gitrepo_result.stderr.lower() if gitrepo_result.stderr else ""
                # Common expected errors that we can safely ignore
                expected_errors = ["forbidden", "already exists", "unchanged"]
                if any(err in gitrepo_error for err in expected_errors):
                    # Don't show note for expected errors - GitRepository might be managed elsewhere
                    pass
                else:
                    # Show unexpected errors
                    error_msg = gitrepo_result.stderr[:50].replace('\n', ' ')
                    print(f"  Note: GitRepository: {error_msg}")
        else:
            print(f"  Status: ❌ Failed (exit code: {apply_result.returncode})")
            failed_resources.append(resource)
            if apply_result.stderr:
                # Print error details (truncated if too long)
                error_msg = apply_result.stderr[:60].replace('\n', ' ')
                print(f"  Error: {error_msg}")
        
        if resource != test_resources[-1]:
            print("")
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
            print(f"   - {resource['name']} (namespace: {resource['namespace']}, env: {resource['environment']}, reconcileInterval: {resource['reconcile_interval']})")


if __name__ == "__main__":
    main()

