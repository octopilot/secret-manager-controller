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
    
    # Apply/update CRD if it exists (idempotent - installs if missing, updates if changed)
    # Note: CRD may already be installed from cluster setup (setup_kind.py)
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
        
        # Wait for CRD to be established before applying resources
        # This prevents "no matches for kind" errors when resources are applied too quickly
        print("⏳ Waiting for CRD to be established...")
        crd_name = "secretmanagerconfigs.secret-management.octopilot.io"
        max_attempts = 30  # Wait up to 1 minute
        crd_established = False
        
        for i in range(max_attempts):
            wait_result = subprocess.run(
                ["kubectl", "wait", "--for=condition=established", "crd", crd_name, "--timeout=2s"],
                capture_output=True,
                text=True
            )
            
            if wait_result.returncode == 0:
                print("✅ CRD is established and ready")
                crd_established = True
                break
            
            if i < max_attempts - 1:
                time.sleep(2)
        
        if not crd_established:
            print("⚠️  Warning: CRD not established after 60 seconds, but continuing anyway", file=sys.stderr)
            print("   Resources may fail to apply if CRD is not ready", file=sys.stderr)
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
        
        # Step 0: Ensure namespace exists
        # Namespaces are now managed at the top level (gitops/cluster/namespaces/namespace.yaml)
        # We need to apply the namespace from the top-level file
        namespace_file = Path("gitops/cluster/namespaces/namespace.yaml")
        if namespace_file.exists():
            # Apply namespace (kubectl apply is idempotent, so this is safe)
            namespace_result = subprocess.run(
                ["kubectl", "apply", "-f", str(namespace_file)],
                capture_output=True,
                text=True
            )
            if namespace_result.returncode != 0:
                print(f"  Status: ❌ Failed - Could not apply namespace")
                print(f"  Error: {namespace_result.stderr[:60] if namespace_result.stderr else 'Unknown error'}")
                failed_resources.append(resource)
                continue
        
        # Apply using kustomize to ensure all resources are created
        # We apply all SecretManagerConfig files (aws, azure, gcp) for the environment
        
        # Step 1: Apply all SecretManagerConfig files for this environment
        # Files are named: secretmanagerconfig-aws.yaml, secretmanagerconfig-azure.yaml, secretmanagerconfig-gcp.yaml
        secretmanagerconfig_files = [
            resource["kustomize_path"] / "secretmanagerconfig-aws.yaml",
            resource["kustomize_path"] / "secretmanagerconfig-azure.yaml",
            resource["kustomize_path"] / "secretmanagerconfig-gcp.yaml",
        ]
        
        # Check which files exist (all should exist, but be defensive)
        existing_files = [f for f in secretmanagerconfig_files if f.exists()]
        
        if not existing_files:
            print(f"  Status: ❌ Failed - No SecretManagerConfig files found")
            failed_resources.append(resource)
            continue
        
        # Apply all SecretManagerConfig files
        apply_results = []
        for config_file in existing_files:
            apply_result = subprocess.run(
                ["kubectl", "apply", "-f", str(config_file)],
                capture_output=True,
                text=True
            )
            apply_results.append((config_file, apply_result))
        
        # Check if all applies succeeded
        failed_applies = [r for r in apply_results if r[1].returncode != 0]
        all_succeeded = len(failed_applies) == 0
        
        if failed_applies:
            # At least one apply failed
            print(f"  Status: ❌ Failed")
            for config_file, result in failed_applies:
                error_msg = result.stderr[:60].replace('\n', ' ') if result.stderr else "Unknown error"
                print(f"    - {config_file.name}: {error_msg}")
            failed_resources.append(resource)
            apply_result = failed_applies[0][1]  # Use first failure for compatibility
        else:
            # All applies succeeded
            apply_result = subprocess.CompletedProcess([], 0, "", "")  # Success result
        
        # Step 2: Try to apply GitRepository (optional - might fail if already exists or no permissions)
        gitrepository_file = resource["kustomize_path"] / "gitrepository.yaml"
        gitrepo_result = subprocess.run(
            ["kubectl", "apply", "-f", str(gitrepository_file)],
            capture_output=True,
            text=True
        )
        
        # Success if all SecretManagerConfig files were applied successfully
        # GitRepository failure is acceptable (might already exist or require different permissions)
        if all_succeeded:
            print(f"  Status: ✅ Applied successfully ({len(existing_files)} SecretManagerConfig file(s))")
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

