#!/usr/bin/env python3
"""
Export GPG private key from local keyring and create Kubernetes secrets in all namespaces.

This script:
1. Reads the GPG key ID from .sops.yaml (the key used to encrypt SOPS files)
2. Exports that private key from the local GPG keyring
3. Creates Kubernetes secrets in all required namespaces (tilt, dev, stage, prod, microscaler-system)

The key MUST be the same one used to encrypt the SOPS files (identified in .sops.yaml).
"""

import os
import subprocess
import sys
import argparse
import tempfile
import yaml
from pathlib import Path


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_error(msg):
    """Print error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def run_command(cmd, check=True, capture_output=True, **kwargs):
    """Run a command and return the result."""
    result = subprocess.run(
        cmd,
        shell=isinstance(cmd, str),
        capture_output=capture_output,
        text=True,
        check=check,
        **kwargs
    )
    return result


def read_sops_yaml(sops_yaml_path: Path) -> list:
    """Read GPG key IDs from .sops.yaml file."""
    if not sops_yaml_path.exists():
        log_error(f".sops.yaml file not found: {sops_yaml_path}")
        sys.exit(1)

    log_info(f"Reading .sops.yaml: {sops_yaml_path}")
    
    try:
        with open(sops_yaml_path, 'r') as f:
            sops_config = yaml.safe_load(f)
    except Exception as e:
        log_error(f"Failed to parse .sops.yaml: {e}")
        sys.exit(1)

    # Extract GPG key IDs from creation_rules
    key_ids = []
    if 'creation_rules' in sops_config:
        for rule in sops_config['creation_rules']:
            if 'key_groups' in rule:
                for group in rule['key_groups']:
                    if 'pgp' in group:
                        key_ids.extend(group['pgp'])
    
    if not key_ids:
        log_error("No GPG key IDs found in .sops.yaml")
        log_info("Make sure .sops.yaml has creation_rules with pgp key IDs")
        sys.exit(1)

    # Remove duplicates while preserving order
    seen = set()
    unique_key_ids = []
    for key_id in key_ids:
        if key_id not in seen:
            seen.add(key_id)
            unique_key_ids.append(key_id)

    log_info(f"Found {len(unique_key_ids)} GPG key ID(s) in .sops.yaml")
    return unique_key_ids


def export_gpg_key(key_id: str) -> str:
    """Export GPG private key from local keyring by key ID."""
    log_info(f"Exporting GPG private key: {key_id}")
    
    # First, check if key exists
    result = run_command(
        ["gpg", "--list-secret-keys", "--keyid-format", "LONG", key_id],
        check=False,
        capture_output=True
    )
    
    if result.returncode != 0 or key_id not in result.stdout:
        log_error(f"GPG key not found in local keyring: {key_id}")
        log_info("")
        log_info("Available secret keys:")
        list_result = run_command(
            ["gpg", "--list-secret-keys", "--keyid-format", "LONG"],
            check=False,
            capture_output=True
        )
        if list_result.returncode == 0:
            print(list_result.stdout)
        log_info("")
        log_info("💡 Make sure the GPG key from .sops.yaml is imported into your local keyring:")
        log_info(f"   gpg --import /path/to/private-key.asc")
        sys.exit(1)

    # Export the private key
    result = run_command(
        ["gpg", "--armor", "--export-secret-keys", key_id],
        check=True,
        capture_output=True
    )
    
    private_key = result.stdout
    if not private_key or "BEGIN PGP PRIVATE KEY BLOCK" not in private_key:
        log_error("Failed to export private key")
        sys.exit(1)

    log_info(f"✅ Exported private key ({len(private_key)} bytes)")
    return private_key


def main():
    parser = argparse.ArgumentParser(
        description="Export GPG private key from local keyring (using .sops.yaml key ID) and create Kubernetes secrets"
    )
    parser.add_argument(
        "--sops-yaml",
        type=Path,
        default=Path(".sops.yaml"),
        help="Path to .sops.yaml file (default: .sops.yaml)",
    )
    parser.add_argument(
        "--key-id",
        help="GPG key ID to export (overrides .sops.yaml, use full fingerprint or short ID)",
    )
    parser.add_argument(
        "--secret-name",
        default="sops-private-key",
        help="Kubernetes secret name (default: sops-private-key)",
    )
    parser.add_argument(
        "--namespace",
        help="Single namespace to create secret in (default: all namespaces)",
    )
    parser.add_argument(
        "--all-environments",
        action="store_true",
        help="Create secrets in all environment namespaces (tilt, dev, stage, prod, microscaler-system)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would be done without creating secrets",
    )

    args = parser.parse_args()

    print("🔑 Exporting GPG private key from local keyring")
    print()

    # Determine which key ID to use
    if args.key_id:
        key_id = args.key_id
        log_info(f"Using specified key ID: {key_id}")
    else:
        # Read from .sops.yaml
        key_ids = read_sops_yaml(args.sops_yaml)
        if len(key_ids) > 1:
            log_info(f"Found multiple key IDs in .sops.yaml, using first one: {key_ids[0]}")
            log_info(f"Other key IDs: {', '.join(key_ids[1:])}")
        key_id = key_ids[0]

    # Export the private key
    private_key = export_gpg_key(key_id)
    print()

    # Determine namespaces
    if args.namespace:
        namespaces = [args.namespace]
    elif args.all_environments:
        namespaces = ["tilt", "dev", "stage", "prod", "microscaler-system"]
    else:
        # Default: all environments
        namespaces = ["tilt", "dev", "stage", "prod", "microscaler-system"]

    if args.dry_run:
        print("🔍 DRY RUN - Would create secrets:")
        for ns in namespaces:
            print(f"   Name: {args.secret_name}")
            print(f"   Namespace: {ns}")
        print(f"   Key length: {len(private_key)} bytes")
        print()
        print("First 100 characters of key:")
        print(private_key[:100] + "...")
        return

    # Write private key to temporary file
    with tempfile.NamedTemporaryFile(mode='w', suffix='.asc', delete=False) as tmp_file:
        tmp_file.write(private_key)
        tmp_file_path = tmp_file.name

    try:
        # Create secrets in all namespaces
        print(f"📦 Creating Kubernetes secrets: {args.secret_name}")
        print(f"   Namespaces: {', '.join(namespaces)}")
        print()

        success_count = 0
        failed_namespaces = []

        for namespace in namespaces:
            log_info(f"Creating secret in namespace: {namespace}")
            
            # Ensure namespace exists
            ns_check = run_command(
                ["kubectl", "get", "namespace", namespace],
                check=False,
                capture_output=True
            )
            if ns_check.returncode != 0:
                log_info(f"  Creating namespace: {namespace}")
                run_command(
                    ["kubectl", "create", "namespace", namespace],
                    check=False,
                )

            # Create secret from file
            result = run_command(
                [
                    "kubectl",
                    "create",
                    "secret",
                    "generic",
                    args.secret_name,
                    f"--from-file=private-key={tmp_file_path}",
                    "-n",
                    namespace,
                ],
                check=False,
                capture_output=True,
            )

            if result.returncode != 0:
                # Check if secret already exists
                if "already exists" in result.stderr:
                    log_info(f"  ⚠️  Secret already exists in {namespace}. Updating...")
                    # Delete and recreate
                    run_command(
                        ["kubectl", "delete", "secret", args.secret_name, "-n", namespace],
                        check=False,
                    )
                    # Create again
                    result = run_command(
                        [
                            "kubectl",
                            "create",
                            "secret",
                            "generic",
                            args.secret_name,
                            f"--from-file=private-key={tmp_file_path}",
                            "-n",
                            namespace,
                        ],
                        check=True,
                    )
                    log_info(f"  ✅ Secret updated successfully in {namespace}")
                    success_count += 1
                else:
                    log_error(f"  ❌ Failed to create secret in {namespace}: {result.stderr}")
                    failed_namespaces.append(namespace)
            else:
                log_info(f"  ✅ Secret created successfully in {namespace}")
                success_count += 1

        print()
        print(f"✅ Successfully created/updated secrets in {success_count}/{len(namespaces)} namespace(s)")
        if failed_namespaces:
            log_error(f"Failed namespaces: {', '.join(failed_namespaces)}")
            sys.exit(1)
    finally:
        # Clean up temp file
        try:
            os.unlink(tmp_file_path)
        except:
            pass

    print()
    print(f"📋 Verify secrets:")
    for namespace in namespaces:
        print(f"   kubectl get secret {args.secret_name} -n {namespace}")


if __name__ == "__main__":
    main()

