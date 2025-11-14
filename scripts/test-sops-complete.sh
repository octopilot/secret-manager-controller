#!/usr/bin/env python3
"""
Complete SOPS test setup script
Creates directory structure, copies files, and provides next steps
"""

import argparse
import shutil
import subprocess
import sys
from pathlib import Path


def run_command(cmd, check=True):
    """Run a shell command and return the result"""
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    if check and result.returncode != 0:
        print(f"❌ Command failed: {cmd}")
        print(f"   Error: {result.stderr}")
        sys.exit(1)
    return result


def main():
    parser = argparse.ArgumentParser(
        description="Complete SOPS test setup - creates directory structure, copies files, and provides setup instructions"
    )
    parser.add_argument(
        "--artifact-path",
        default="/tmp/flux-source-test-namespace-test-repo",
        help="Artifact path where controller expects files",
    )
    parser.add_argument(
        "--env",
        default="dev",
        help="Environment name",
    )
    parser.add_argument(
        "--base-path",
        default="",
        help="Base path within artifact",
    )
    parser.add_argument(
        "--service",
        default="test-service",
        help="Service name",
    )
    parser.add_argument(
        "--namespace",
        default="microscaler-system",
        help="Kubernetes namespace for controller",
    )
    parser.add_argument(
        "--skip-gpg-check",
        action="store_true",
        help="Skip GPG key availability check",
    )

    args = parser.parse_args()

    print("🚀 SOPS Quick Test Setup")
    print("=" * 60)
    print()

    # Step 1: Check prerequisites
    print("📋 Step 1: Checking prerequisites...")
    
    # Check if sops is available
    sops_check = run_command("which sops", check=False)
    if sops_check.returncode == 0:
        print(f"  ✅ sops found: {sops_check.stdout.strip()}")
    else:
        print("  ⚠️  sops not found in PATH (controller will need it)")
    
    # Check if gpg is available
    if not args.skip_gpg_check:
        gpg_check = run_command("which gpg", check=False)
        if gpg_check.returncode == 0:
            print(f"  ✅ gpg found: {gpg_check.stdout.strip()}")
        else:
            print("  ⚠️  gpg not found in PATH (controller will need it)")
    
    print()

    # Step 2: Create directory structure
    print("📁 Step 2: Creating directory structure...")
    artifact_path = Path(args.artifact_path)
    if args.base_path:
        target_dir = artifact_path / args.base_path / args.service / "deployment-configuration" / "profiles" / args.env
    else:
        target_dir = artifact_path / "deployment-configuration" / "profiles" / args.env

    target_dir.mkdir(parents=True, exist_ok=True)
    print(f"  ✅ Created: {target_dir}")
    print()

    # Step 3: Copy SOPS files
    print("📋 Step 3: Copying SOPS-encrypted files...")
    examples_dir = Path("examples/sample-deployment-configuration/profiles") / args.env
    
    if not examples_dir.exists():
        print(f"  ❌ Examples directory not found: {examples_dir}")
        print("     Please create SOPS-encrypted files first")
        print("     See: examples/sample-deployment-configuration/SOPS_SETUP.md")
        sys.exit(1)

    files_to_copy = [
        "application.secrets.env",
        "application.secrets.yaml",
        "application.properties",
    ]

    copied_count = 0
    for filename in files_to_copy:
        source_file = examples_dir / filename
        if source_file.exists():
            dest_file = target_dir / filename
            shutil.copy2(source_file, dest_file)
            print(f"  ✅ Copied {filename}")
            copied_count += 1
        else:
            print(f"  ⏭️  Skipped {filename} (not found)")

    if copied_count == 0:
        print("  ❌ No files copied!")
        sys.exit(1)

    print()

    # Step 4: Verify files
    print("🔍 Step 4: Verifying files...")
    for filename in files_to_copy:
        file_path = target_dir / filename
        if file_path.exists():
            size = file_path.stat().st_size
            print(f"  ✅ {filename} ({size} bytes)")
    print()

    # Step 5: Provide next steps
    print("📝 Step 5: Next Steps")
    print("=" * 60)
    print()
    print("1️⃣  Create SOPS private key secret:")
    print()
    print(f"   kubectl create secret generic sops-private-key \\")
    print(f"     --from-file=private-key=/path/to/gpg-private-key.asc \\")
    print(f"     -n {args.namespace}")
    print()
    print("   Or use one of these secret names:")
    print("   - sops-private-key")
    print("   - sops-gpg-key")
    print("   - gpg-key")
    print()
    print("2️⃣  Create minimal GitRepository (for testing):")
    print()
    print(f"   kubectl apply -f examples/test-gitrepository-minimal.yaml")
    print()
    print("3️⃣  Create SecretManagerConfig:")
    print()
    print(f"   kubectl apply -f examples/test-sops-config.yaml")
    print()
    print("   Or update the config with your artifact path:")
    print(f"   artifact path: {args.artifact_path}")
    print()
    print("4️⃣  Verify files in container:")
    print()
    print(f"   kubectl exec -it <controller-pod> -- ls -la {target_dir}")
    print()
    print("5️⃣  Check controller logs:")
    print()
    print(f"   kubectl logs -f <controller-pod> -n {args.namespace} | grep -i sops")
    print()
    print("=" * 60)
    print()
    print("✅ Setup complete! Files are ready for controller testing.")
    print()


if __name__ == "__main__":
    main()

