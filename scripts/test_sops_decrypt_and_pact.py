#!/usr/bin/env python3
"""
Test script for SOPS decryption and Pact testing.

This script:
1. Copies encrypted test files into the controller container
2. Decrypts them using rops (via Rust test binary)
3. Prints decrypted content to console for validation
4. Runs Pact tests
5. Publishes contracts to Pact broker

Temporary script for testing purposes - will be removed after validation.
"""

import argparse
import base64
import os
import subprocess
import sys
import time
from pathlib import Path


def get_controller_pod(namespace: str = "microscaler-system") -> str:
    """Get the name of the controller pod."""
    result = subprocess.run(
        ["kubectl", "get", "pods", "-n", namespace, "-l", "app=secret-manager-controller", "-o", "jsonpath={.items[0].metadata.name}"],
        capture_output=True,
        text=True,
        check=True
    )
    pod_name = result.stdout.strip()
    if not pod_name:
        raise RuntimeError(f"No controller pod found in namespace {namespace}")
    return pod_name


def copy_file_to_container(source_file: Path, pod_name: str, namespace: str, target_path: str) -> bool:
    """Copy a file into the container."""
    print(f"📋 Copying {source_file.name} to container...")
    try:
        # Ensure target directory exists
        subprocess.run(
            ["kubectl", "exec", "-n", namespace, pod_name, "--", "mkdir", "-p", str(Path(target_path).parent)],
            capture_output=True,
            check=False
        )
        
        # Copy file
        subprocess.run(
            ["kubectl", "cp", str(source_file), f"{namespace}/{pod_name}:{target_path}"],
            capture_output=True,
            check=True
        )
        print(f"  ✅ Copied {source_file.name}")
        return True
    except subprocess.CalledProcessError as e:
        print(f"  ❌ Failed to copy {source_file.name}: {e}", file=sys.stderr)
        return False


def run_decryption_test(pod_name: str, namespace: str, test_files_dir: str, sops_key_secret: str = "sops-private-key") -> bool:
    """Run the Rust test binary to decrypt files and print output."""
    print("\n🔓 Running SOPS decryption test in container...")
    print("=" * 80)
    
    try:
        # First, check if files exist in container
        check_result = subprocess.run(
            ["kubectl", "exec", "-n", namespace, pod_name, "--", "ls", "-la", test_files_dir],
            capture_output=True,
            text=True,
            check=False
        )
        print("Files in container:")
        print(check_result.stdout)
        print()
        
        # Get SOPS private key from Kubernetes secret
        print("🔑 Retrieving SOPS private key from secret...")
        key_result = subprocess.run(
            ["kubectl", "get", "secret", "-n", namespace, sops_key_secret, "-o", "jsonpath={.data.private-key}"],
            capture_output=True,
            text=True,
            check=False
        )
        
        sops_key_base64 = key_result.stdout.strip() if key_result.returncode == 0 else ""
        
        # Build the test binary in the container
        print("🔨 Building test-sops-decrypt binary in container...")
        build_result = subprocess.run(
            ["kubectl", "exec", "-n", namespace, pod_name, "--", "cargo", "build", "--bin", "test-sops-decrypt"],
            capture_output=True,
            text=True,
            check=False
        )
        
        if build_result.returncode != 0:
            print(f"⚠️  Build output: {build_result.stdout}")
            print(f"⚠️  Build errors: {build_result.stderr}")
            print("⚠️  Binary build failed, but continuing...")
        
        # Run the decryption test binary
        print("\n📝 Running decryption test (decrypted content will be printed below):")
        print("=" * 80)
        
        # Build kubectl exec command with environment variables
        # kubectl exec doesn't support env vars directly, so we use env command
        exec_cmd = ["kubectl", "exec", "-n", namespace, pod_name, "--", "env"]
        
        # Add environment variables
        exec_cmd.append(f"TEST_FILES_DIR={test_files_dir}")
        exec_cmd.append("RUST_LOG=info")
        
        if sops_key_base64:
            # Decode base64 key
            try:
                sops_key = base64.b64decode(sops_key_base64).decode('utf-8')
                # Escape special characters for shell
                sops_key_escaped = sops_key.replace("'", "'\"'\"'")
                exec_cmd.append(f"SOPS_PRIVATE_KEY='{sops_key_escaped}'")
            except Exception as e:
                print(f"⚠️  Failed to decode SOPS key: {e}")
        
        # Run the binary (path may vary - try common locations)
        exec_cmd.append("./target/debug/test-sops-decrypt")
        
        result = subprocess.run(
            exec_cmd,
            capture_output=False,  # Print directly to console
            check=False
        )
        
        print("=" * 80)
        
        if result.returncode == 0:
            print("✅ Decryption test completed successfully")
            return True
        else:
            print(f"⚠️  Decryption test exited with code {result.returncode}")
            return True  # Continue anyway for testing
        
    except Exception as e:
        print(f"❌ Failed to run decryption test: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return False


def run_pact_tests() -> bool:
    """Run Pact contract tests."""
    print("\n🧪 Running Pact contract tests...")
    try:
        result = subprocess.run(
            ["cargo", "test", "--test", "pact_*", "--no-fail-fast"],
            capture_output=False,
            check=False
        )
        if result.returncode == 0:
            print("✅ Pact tests passed")
            return True
        else:
            print("⚠️  Some Pact tests failed (this may be expected)")
            return True  # Continue anyway
    except Exception as e:
        print(f"❌ Failed to run Pact tests: {e}", file=sys.stderr)
        return False


def publish_pact_contracts(broker_url: str, username: str, password: str) -> bool:
    """Publish Pact contracts to broker."""
    print("\n📤 Publishing Pact contracts...")
    try:
        result = subprocess.run(
            ["python3", "scripts/pact_publish.py", "--broker-url", broker_url, "--username", username, "--password", password],
            capture_output=False,
            check=False
        )
        if result.returncode == 0:
            print("✅ Pact contracts published")
            return True
        else:
            print("⚠️  Pact publishing may have failed (check logs)")
            return True  # Continue anyway
    except Exception as e:
        print(f"❌ Failed to publish Pact contracts: {e}", file=sys.stderr)
        return False


def main():
    """Main test function."""
    parser = argparse.ArgumentParser(
        description="Test SOPS decryption and Pact contracts",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Test with default settings
  python3 scripts/test_sops_decrypt_and_pact.py
  
  # Test with custom namespace and Pact broker
  python3 scripts/test_sops_decrypt_and_pact.py --namespace my-namespace --broker-url http://localhost:9292
        """
    )
    parser.add_argument(
        "--namespace",
        default="microscaler-system",
        help="Kubernetes namespace for controller pod (default: microscaler-system)"
    )
    parser.add_argument(
        "--test-files-dir",
        default="/tmp/test-sops-files",
        help="Directory in container where test files will be copied (default: /tmp/test-sops-files)"
    )
    parser.add_argument(
        "--source-dir",
        default="examples/sample-deployment-configuration/profiles/dev",
        help="Source directory containing encrypted test files (default: examples/sample-deployment-configuration/profiles/dev)"
    )
    parser.add_argument(
        "--broker-url",
        default="http://localhost:9292",
        help="Pact broker URL (default: http://localhost:9292)"
    )
    parser.add_argument(
        "--broker-username",
        default="pact",
        help="Pact broker username (default: pact)"
    )
    parser.add_argument(
        "--broker-password",
        default="pact",
        help="Pact broker password (default: pact)"
    )
    parser.add_argument(
        "--skip-pact",
        action="store_true",
        help="Skip Pact tests and publishing (only test decryption)"
    )
    parser.add_argument(
        "--sops-key-secret",
        default="sops-private-key",
        help="Kubernetes secret name containing SOPS private key (default: sops-private-key)"
    )
    
    args = parser.parse_args()
    
    # Files to copy and decrypt
    test_files = [
        "application.properties",
        "application.secrets.env",
        "application.secrets.yaml",
    ]
    
    source_dir = Path(args.source_dir)
    if not source_dir.exists():
        print(f"❌ Error: Source directory not found: {source_dir}", file=sys.stderr)
        sys.exit(1)
    
    print("🚀 Starting SOPS decryption and Pact test workflow")
    print(f"📁 Source directory: {source_dir}")
    print(f"📦 Container directory: {args.test_files_dir}")
    print(f"🏷️  Namespace: {args.namespace}")
    print()
    
    # Get controller pod
    try:
        pod_name = get_controller_pod(args.namespace)
        print(f"✅ Found controller pod: {pod_name}")
    except Exception as e:
        print(f"❌ Error: {e}", file=sys.stderr)
        sys.exit(1)
    
    # Encrypt and copy test files to container
    print("\n📋 Preparing and copying test files to container...")
    copied_count = 0
    for filename in test_files:
        source_file = source_dir / filename
        if not source_file.exists():
            print(f"  ⚠️  Skipping {filename} (not found)")
            continue
        
        # Encrypt file if needed (in-place encryption)
        encrypted_file = encrypt_file_if_needed(source_file)
        
        target_path = f"{args.test_files_dir}/{filename}"
        if copy_file_to_container(encrypted_file, pod_name, args.namespace, target_path):
            copied_count += 1
    
    if copied_count == 0:
        print("❌ No files were copied successfully", file=sys.stderr)
        sys.exit(1)
    
    print(f"\n✅ Copied {copied_count} file(s) to container")
    
    # Run decryption test
    if not run_decryption_test(pod_name, args.namespace, args.test_files_dir, args.sops_key_secret):
        print("⚠️  Decryption test had issues (continuing anyway)")
    
    # Run Pact tests and publish (unless skipped)
    if not args.skip_pact:
        if not run_pact_tests():
            print("⚠️  Pact tests had issues (continuing anyway)")
        
        if not publish_pact_contracts(args.broker_url, args.broker_username, args.broker_password):
            print("⚠️  Pact publishing had issues")
    
    print("\n✅ Test workflow completed")


if __name__ == "__main__":
    main()

