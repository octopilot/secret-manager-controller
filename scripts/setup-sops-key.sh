#!/usr/bin/env python3
"""
Export GPG private key from local keyring and create Kubernetes secret
Uses the existing flux@pricewhisperer.io key
"""

import subprocess
import sys
import argparse


def main():
    parser = argparse.ArgumentParser(
        description="Export GPG private key and create Kubernetes secret for SOPS"
    )
    parser.add_argument(
        "--key-email",
        default="flux@pricewhisperer.ai",
        help="GPG key email to export (default: flux@pricewhisperer.ai)",
    )
    parser.add_argument(
        "--secret-name",
        default="sops-private-key",
        help="Kubernetes secret name (default: sops-private-key)",
    )
    parser.add_argument(
        "--namespace",
        default="microscaler-system",
        help="Kubernetes namespace (default: microscaler-system)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would be done without creating secret",
    )

    args = parser.parse_args()

    print(f"🔑 Exporting GPG private key for: {args.key_email}")
    print()

    # Check if key exists
    try:
        result = subprocess.run(
            ["gpg", "--list-secret-keys", "--keyid-format", "LONG"],
            capture_output=True,
            text=True,
            check=True,
        )
        if args.key_email not in result.stdout:
            print(f"⚠️  Key not found for: {args.key_email}")
            print()
            print("Available secret keys:")
            print(result.stdout)
            print()
            print("💡 Tip: Use --key-email with one of the emails above, or use key ID")
            print("   Example: python3 scripts/setup-sops-key.sh --key-email <email>")
            sys.exit(1)
        print(f"✅ Found GPG key for: {args.key_email}")
    except subprocess.CalledProcessError as e:
        print(f"❌ Failed to list GPG keys: {e.stderr}")
        print("   Make sure GPG is installed and keyring is accessible")
        sys.exit(1)

    print()

    # Export private key
    print("📤 Exporting private key...")
    try:
        result = subprocess.run(
            ["gpg", "--armor", "--export-secret-keys", args.key_email],
            capture_output=True,
            text=True,
            check=True,
        )
        private_key = result.stdout

        if not private_key or "BEGIN PGP PRIVATE KEY BLOCK" not in private_key:
            print("❌ Failed to export private key")
            sys.exit(1)

        print(f"✅ Exported private key ({len(private_key)} bytes)")
        print()

        if args.dry_run:
            print("🔍 DRY RUN - Would create secret:")
            print(f"   Name: {args.secret_name}")
            print(f"   Namespace: {args.namespace}")
            print(f"   Key length: {len(private_key)} bytes")
            print()
            print("First 100 characters of key:")
            print(private_key[:100] + "...")
            return

        # Create Kubernetes secret
        print(f"📦 Creating Kubernetes secret: {args.secret_name}")
        print(f"   Namespace: {args.namespace}")
        print()

        # Write private key to temporary file
        import tempfile
        import os
        
        with tempfile.NamedTemporaryFile(mode='w', suffix='.asc', delete=False) as tmp_file:
            tmp_file.write(private_key)
            tmp_file_path = tmp_file.name

        try:
            # Create secret from file
            result = subprocess.run(
                [
                    "kubectl",
                    "create",
                    "secret",
                    "generic",
                    args.secret_name,
                    f"--from-file=private-key={tmp_file_path}",
                    "-n",
                    args.namespace,
                ],
                capture_output=True,
                text=True,
                check=False,
            )

            if result.returncode != 0:
                # Check if secret already exists
                if "already exists" in result.stderr:
                    print("⚠️  Secret already exists. Updating...")
                    # Delete and recreate
                    subprocess.run(
                        ["kubectl", "delete", "secret", args.secret_name, "-n", args.namespace],
                        capture_output=True,
                        check=False,
                    )
                    # Create again
                    result = subprocess.run(
                        [
                            "kubectl",
                            "create",
                            "secret",
                            "generic",
                            args.secret_name,
                            f"--from-file=private-key={tmp_file_path}",
                            "-n",
                            args.namespace,
                        ],
                        capture_output=True,
                        text=True,
                        check=True,
                    )
                    print("✅ Secret updated successfully!")
                else:
                    print(f"❌ Failed to create secret: {result.stderr}")
                    sys.exit(1)
            else:
                print("✅ Secret created successfully!")
        finally:
            # Clean up temp file
            try:
                os.unlink(tmp_file_path)
            except:
                pass

        print()
        print(f"📋 Verify secret:")
        print(f"   kubectl get secret {args.secret_name} -n {args.namespace}")
        print()
        print(f"📋 View secret details:")
        print(
            f"   kubectl describe secret {args.secret_name} -n {args.namespace}"
        )

    except subprocess.CalledProcessError as e:
        print(f"❌ Failed to export GPG key: {e}")
        print(f"   Error: {e.stderr}")
        sys.exit(1)
    except FileNotFoundError:
        print("❌ gpg or kubectl not found in PATH")
        sys.exit(1)


if __name__ == "__main__":
    main()

