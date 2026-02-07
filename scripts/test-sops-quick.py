#!/usr/bin/env python3
"""
Quick test script to copy SOPS-encrypted files into container
This bypasses Git/Flux setup for quick SOPS decryption testing

Usage:
    python3 scripts/test-sops-quick.py --artifact-path /tmp/test-repo --env dev
"""

import argparse
import shutil
import sys
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(
        description="Copy SOPS-encrypted files to controller artifact path for quick testing"
    )
    parser.add_argument(
        "--artifact-path",
        default="/tmp/flux-source-test-namespace-test-repo",
        help="Artifact path where controller expects files (default: /tmp/flux-source-test-namespace-test-repo)",
    )
    parser.add_argument(
        "--env",
        default="dev",
        help="Environment name (default: dev)",
    )
    parser.add_argument(
        "--base-path",
        default="",
        help="Base path within artifact (default: empty)",
    )
    parser.add_argument(
        "--service",
        default="test-service",
        help="Service name (default: test-service)",
    )
    parser.add_argument(
        "--examples-dir",
        default="examples/sample-deployment-configuration/profiles",
        help="Source directory with example files (default: examples/sample-deployment-configuration/profiles)",
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
        "--copy-to-container",
        action="store_true",
        help="Copy files into container (default: False)",
    )

    args = parser.parse_args()

    # Determine target directory based on structure
    artifact_path = Path(args.artifact_path)
    if args.base_path:
        target_dir = artifact_path / args.base_path / args.service / "deployment-configuration" / "profiles" / args.env
    else:
        target_dir = artifact_path / "deployment-configuration" / "profiles" / args.env

    print(f"📁 Target directory: {target_dir}")
    print(f"📋 Environment: {args.env}")
    print()

    # Create directory structure
    target_dir.mkdir(parents=True, exist_ok=True)

    # Check if SOPS files exist in examples
    examples_dir = Path(args.examples_dir) / args.env
    if not examples_dir.exists():
        print(f"❌ Examples directory not found: {examples_dir}")
        print("   Please create SOPS-encrypted files first")
        print("   See: examples/sample-deployment-configuration/SOPS_SETUP.md")
        sys.exit(1)

    # Files to copy
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
        print("❌ No files copied! Please ensure SOPS-encrypted files exist in examples directory")
        sys.exit(1)

    print()
    
    # Encrypt files with SOPS
    print("🔐 Encrypting files with SOPS...")
    import subprocess
    encrypted_count = 0
    for filename in files_to_copy:
        file_path = target_dir / filename
        if file_path.exists():
            try:
                # Encrypt file in place using sops -e -i
                subprocess.run(
                    ["sops", "-e", "-i", str(file_path)],
                    capture_output=True,
                    text=True,
                    check=True
                )
                print(f"  ✅ Encrypted {filename}")
                encrypted_count += 1
            except subprocess.CalledProcessError as e:
                print(f"  ⚠️  Failed to encrypt {filename}: {e.stderr}")
            except FileNotFoundError:
                print(f"  ⚠️  sops binary not found. Skipping encryption.")
                break
    
    if encrypted_count > 0:
        print(f"  ✅ Successfully encrypted {encrypted_count} file(s)")
    print()
    print("✅ Files copied and encrypted successfully!")
    print()
    
    # Copy to container if requested
    if args.copy_to_container:
        print("📦 Copying files into container...")
        import subprocess
        
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
                if pod_name:
                    print(f"  ✅ Found controller pod: {pod_name}")
            except subprocess.CalledProcessError:
                print("  ⚠️  Controller pod not found. Skipping container copy.")
                pod_name = None
        
        if pod_name:
            # Create directory in container
            container_parent_dir = str(target_dir.parent)
            try:
                subprocess.run(
                    f"kubectl exec -n {args.namespace} {pod_name} -- mkdir -p {container_parent_dir}",
                    shell=True,
                    check=True,
                    capture_output=True
                )
            except subprocess.CalledProcessError:
                pass
            
            # Copy files
            for filename in files_to_copy:
                source_file = target_dir / filename
                if source_file.exists():
                    try:
                        subprocess.run(
                            f"kubectl cp {source_file} {args.namespace}/{pod_name}:{target_dir}/{filename}",
                            shell=True,
                            check=True,
                            capture_output=True
                        )
                        print(f"  ✅ Copied {filename} to container")
                    except subprocess.CalledProcessError:
                        print(f"  ⚠️  Failed to copy {filename} to container")
            print()
    
    print("📝 Next steps:")
    print("  1. Ensure SOPS private key secret exists in Kubernetes:")
    print(f"     kubectl create secret generic sops-private-key \\")
    print(f"       --from-file=private-key=/path/to/gpg-private-key.asc \\")
    print(f"       -n {args.namespace}")
    print()
    print("  2. Create SecretManagerConfig pointing to this artifact path")
    print("  3. Controller will automatically detect and decrypt SOPS files")
    print()
    if not args.copy_to_container:
        print("💡 To copy files to container:")
        print(f"     kubectl cp {target_dir}/ <controller-pod>:{target_dir.parent}/ -n {args.namespace}")
        print()
    print("💡 To verify files in container:")
    print(f"     kubectl exec -it <controller-pod> -n {args.namespace} -- ls -la {target_dir}")
    print()
    print("💡 To verify SOPS detection:")
    print(f"     kubectl exec -it <controller-pod> -n {args.namespace} -- cat {target_dir}/application.secrets.env | head -5")


if __name__ == "__main__":
    main()

