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
        default="octopilot-system",
        help="Kubernetes namespace for controller",
    )
    parser.add_argument(
        "--pod-name",
        default="",
        help="Controller pod name (auto-detected if not provided)",
    )
    parser.add_argument(
        "--skip-gpg-check",
        action="store_true",
        help="Skip GPG key availability check",
    )
    parser.add_argument(
        "--copy-to-container",
        action="store_true",
        help="Copy files into container (default: True)",
    )
    parser.add_argument(
        "--no-copy-to-container",
        dest="copy_to_container",
        action="store_false",
        help="Don't copy files into container",
    )
    parser.set_defaults(copy_to_container=True)

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
    print("📋 Step 3: Copying and encrypting files...")
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
    
    # Step 3b: Encrypt files with SOPS
    print("🔐 Step 3b: Encrypting files with SOPS...")
    encrypted_count = 0
    for filename in files_to_copy:
        file_path = target_dir / filename
        if file_path.exists():
            try:
                # Encrypt file in place using sops -e -i
                result = subprocess.run(
                    ["sops", "-e", "-i", str(file_path)],
                    capture_output=True,
                    text=True,
                    check=True
                )
                print(f"  ✅ Encrypted {filename}")
                encrypted_count += 1
            except subprocess.CalledProcessError as e:
                print(f"  ⚠️  Failed to encrypt {filename}: {e.stderr}")
                print(f"     Make sure SOPS is configured and GPG keys are available")
            except FileNotFoundError:
                print(f"  ⚠️  sops binary not found. Skipping encryption for {filename}")
                print(f"     Install sops: brew install sops")
    
    if encrypted_count > 0:
        print(f"  ✅ Successfully encrypted {encrypted_count} file(s)")
    else:
        print("  ⚠️  No files were encrypted. Files will be copied as plaintext.")
    print()

    # Step 4: Verify files
    print("🔍 Step 4: Verifying local files...")
    for filename in files_to_copy:
        file_path = target_dir / filename
        if file_path.exists():
            size = file_path.stat().st_size
            print(f"  ✅ {filename} ({size} bytes)")
    print()

    # Step 5: Copy files into container
    if args.copy_to_container:
        print("📦 Step 5: Copying files into container...")
        
        # Find controller pod
        pod_name = args.pod_name
        if not pod_name:
            try:
                result = subprocess.run(
                    f"kubectl get pods -n {args.namespace} -l app=secret-manager-controller -o jsonpath='{{.items[0].metadata.name}}'",
                    shell=True,
                    capture_output=True,
                    text=True,
                    check=True
                )
                pod_name = result.stdout.strip()
                if not pod_name:
                    print("  ⚠️  Controller pod not found. Skipping container copy.")
                    print(f"     Please copy files manually or specify --pod-name")
                    pod_name = None
                else:
                    print(f"  ✅ Found controller pod: {pod_name}")
            except subprocess.CalledProcessError:
                print("  ⚠️  Failed to find controller pod. Skipping container copy.")
                print(f"     Please copy files manually:")
                print(f"     kubectl cp {target_dir}/ <pod-name>:{target_dir.parent}/ -n {args.namespace}")
                pod_name = None
        
        if pod_name:
            # Create full directory path in container first
            container_target_dir = str(target_dir)
            try:
                subprocess.run(
                    f"kubectl exec -n {args.namespace} {pod_name} -- mkdir -p {container_target_dir}",
                    shell=True,
                    check=True,
                    capture_output=True
                )
                print(f"  ✅ Created directory in container: {container_target_dir}")
            except subprocess.CalledProcessError as e:
                print(f"  ⚠️  Failed to create directory in container: {e}")
            
            # Copy each file
            copied_to_container = 0
            for filename in files_to_copy:
                source_file = target_dir / filename
                if source_file.exists():
                    try:
                        # Use kubectl cp to copy file into container
                        # kubectl cp requires the target directory to exist
                        result = subprocess.run(
                            f"kubectl cp {source_file} {args.namespace}/{pod_name}:{container_target_dir}/{filename}",
                            shell=True,
                            check=True,
                            capture_output=True,
                            text=True
                        )
                        print(f"  ✅ Copied {filename} to container")
                        copied_to_container += 1
                    except subprocess.CalledProcessError as e:
                        # Try alternative: copy to parent directory and move
                        try:
                            # Copy to a temp location first
                            temp_path = f"/tmp/{filename}"
                            subprocess.run(
                                f"kubectl cp {source_file} {args.namespace}/{pod_name}:{temp_path}",
                                shell=True,
                                check=True,
                                capture_output=True
                            )
                            # Move to final location
                            subprocess.run(
                                f"kubectl exec -n {args.namespace} {pod_name} -- mv {temp_path} {container_target_dir}/{filename}",
                                shell=True,
                                check=True,
                                capture_output=True
                            )
                            print(f"  ✅ Copied {filename} to container (via temp)")
                            copied_to_container += 1
                        except subprocess.CalledProcessError:
                            print(f"  ⚠️  Failed to copy {filename} to container")
                            print(f"     Manual copy: kubectl cp {source_file} {args.namespace}/{pod_name}:{container_target_dir}/{filename}")
            
            if copied_to_container > 0:
                print(f"  ✅ Successfully copied {copied_to_container} file(s) to container")
            
            # Verify files in container
            print()
            print("🔍 Step 6: Verifying files in container...")
            try:
                result = subprocess.run(
                    f"kubectl exec -n {args.namespace} {pod_name} -- ls -la {target_dir}",
                    shell=True,
                    capture_output=True,
                    text=True,
                    check=True
                )
                print(result.stdout)
            except subprocess.CalledProcessError as e:
                print(f"  ⚠️  Failed to verify files in container: {e}")
        print()
        # Store pod_name for use in next steps
        container_pod_name = pod_name if 'pod_name' in locals() else None
    else:
        container_pod_name = None

    # Step 6/7: Provide next steps
    step_num = "7" if args.copy_to_container and container_pod_name else "5"
    print(f"📝 Step {step_num}: Next Steps")
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
    print(f"   kubectl apply -f gitops/cluster/fluxcd/env/tilt/gitrepository.yaml")
    print()
    print("3️⃣  Create SecretManagerConfig:")
    print()
    print(f"   kubectl apply -f gitops/cluster/fluxcd/env/tilt/secretmanagerconfig.yaml")
    print()
    print("   Or update the config with your artifact path:")
    print(f"   artifact path: {args.artifact_path}")
    print()
    if args.copy_to_container and container_pod_name:
        print("4️⃣  Files already copied to container!")
        print(f"   Pod: {container_pod_name}")
        print(f"   Path: {target_dir}")
        print()
    else:
        print("4️⃣  Copy files to container:")
        print()
        print(f"   kubectl cp {target_dir}/ <controller-pod>:{target_dir.parent}/ -n {args.namespace}")
        print()
        print("   Or copy individual files:")
        for filename in files_to_copy:
            if (target_dir / filename).exists():
                print(f"   kubectl cp {target_dir}/{filename} {args.namespace}/<controller-pod>:{target_dir}/{filename}")
        print()
    
    print("5️⃣  Verify files in container:")
    print()
    print(f"   kubectl exec -it <controller-pod> -n {args.namespace} -- ls -la {target_dir}")
    print()
    print("6️⃣  Check controller logs:")
    print()
    print(f"   kubectl logs -f <controller-pod> -n {args.namespace} | grep -i sops")
    print()
    print("=" * 60)
    print()
    print("✅ Setup complete! Files are ready for controller testing.")
    print()


if __name__ == "__main__":
    main()

