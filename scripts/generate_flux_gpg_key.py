#!/usr/bin/env python3
"""
Generate a new GPG key for Flux SOPS encryption.

This script:
1. Generates a new GPG key pair for flux@octopilot.io
2. Exports the private key (for GitHub secret)
3. Shows the key fingerprint (for .sops.yaml)
4. Provides instructions for updating configuration

Usage:
    python3 scripts/generate_flux_gpg_key.py
"""

import os
import subprocess
import sys
import tempfile
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


def check_gpg_installed():
    """Check if GPG is installed."""
    if not run_command("which gpg", check=False).returncode == 0:
        log_error("GPG is not installed. Please install it first:")
        log_error("  macOS: brew install gnupg")
        log_error("  Linux: sudo apt-get install gnupg")
        sys.exit(1)


def generate_gpg_key():
    """Generate a new GPG key for flux@octopilot.io."""
    log_info("Generating GPG key for Flux SOPS...")
    
    # Create batch file for key generation
    batch_content = """Key-Type: RSA
Key-Length: 4096
Name-Real: Flux SOPS Key
Name-Email: flux@octopilot.io
Expire-Date: 0
%no-protection
%commit
"""
    
    with tempfile.NamedTemporaryFile(mode='w', suffix='.batch', delete=False) as f:
        batch_file = f.name
        f.write(batch_content)
    
    try:
        # Generate the key
        log_info("Generating key (this may take a moment)...")
        result = run_command(
            f"gpg --batch --gen-key {batch_file}",
            check=False
        )
        
        if result.returncode != 0:
            log_error("Failed to generate GPG key")
            if result.stderr:
                print(result.stderr, file=sys.stderr)
            sys.exit(1)
        
        log_info("✅ GPG key generated successfully!")
        
        # Get the key fingerprint
        result = run_command(
            "gpg --list-keys --keyid-format LONG flux@octopilot.io",
            check=True
        )
        
        # Extract fingerprint from output
        lines = result.stdout.split('\n')
        fingerprint = None
        for line in lines:
            if 'pub' in line or 'sec' in line:
                # Extract the long key ID (last part after the /)
                parts = line.split('/')
                if len(parts) > 1:
                    key_id = parts[1].split()[0]
                    # Get full fingerprint
                    fp_result = run_command(
                        f"gpg --fingerprint --keyid-format LONG flux@octopilot.io",
                        check=True
                    )
                    for fp_line in fp_result.stdout.split('\n'):
                        if 'Key fingerprint' in fp_line or 'Fingerprint' in fp_line:
                            # Extract fingerprint (remove spaces and colons)
                            fingerprint = fp_line.split('=')[-1].strip().replace(' ', '').replace(':', '')
                            break
                    break
        
        if not fingerprint:
            # Fallback: try to get it from key list
            result = run_command(
                "gpg --list-keys --fingerprint --keyid-format LONG flux@octopilot.io",
                check=True
            )
            for line in result.stdout.split('\n'):
                if len(line) > 0 and not line.startswith('pub') and not line.startswith('uid') and not line.startswith('sub'):
                    # This might be the fingerprint line
                    fingerprint = line.strip().replace(' ', '').replace(':', '')
                    if len(fingerprint) == 40:  # GPG fingerprint is 40 chars
                        break
        
        if not fingerprint:
            log_error("Could not extract fingerprint. Please run manually:")
            log_error("  gpg --list-keys --fingerprint --keyid-format LONG flux@octopilot.io")
            sys.exit(1)
        
        return fingerprint
        
    finally:
        # Clean up batch file
        if os.path.exists(batch_file):
            os.unlink(batch_file)


def export_private_key(fingerprint: str):
    """Export the private key and show base64 encoded version."""
    log_info("Exporting private key...")
    
    result = run_command(
        f"gpg --armor --export-secret-keys {fingerprint}",
        check=True
    )
    
    private_key = result.stdout
    
    # Show base64 encoded version (for GitHub secret)
    import base64
    private_key_bytes = private_key.encode('utf-8')
    base64_key = base64.b64encode(private_key_bytes).decode('utf-8')
    
    print("\n" + "="*80)
    print("PRIVATE KEY (for GitHub Secret 'GPG_KEY'):")
    print("="*80)
    print(base64_key)
    print("="*80)
    
    # Also save to file
    key_file = Path("flux-private-key.asc")
    key_file.write_text(private_key)
    log_info(f"✅ Private key saved to: {key_file}")
    
    base64_file = Path("flux-private-key-base64.txt")
    base64_file.write_text(base64_key)
    log_info(f"✅ Base64 encoded key saved to: {base64_file}")
    
    return fingerprint


def main():
    """Main function."""
    check_gpg_installed()
    
    # Check if key already exists
    result = run_command(
        "gpg --list-keys flux@octopilot.io",
        check=False
    )
    
    if result.returncode == 0:
        log_error("GPG key for flux@octopilot.io already exists!")
        log_info("Existing key:")
        print(result.stdout)
        response = input("\nDo you want to generate a new key anyway? (yes/no): ")
        if response.lower() != 'yes':
            log_info("Aborted.")
            sys.exit(0)
        log_info("Generating new key (old key will remain in keyring)...")
    
    fingerprint = generate_gpg_key()
    
    print("\n" + "="*80)
    print("KEY FINGERPRINT (for .sops.yaml):")
    print("="*80)
    print(fingerprint)
    print("="*80)
    
    export_private_key(fingerprint)
    
    print("\n" + "="*80)
    print("NEXT STEPS:")
    print("="*80)
    print("1. Add the new key to .sops.yaml:")
    print(f"   Add the fingerprint: {fingerprint}")
    print("   Keep the old key temporarily (for rotation)")
    print()
    print("2. Update encryption keys on all SOPS files:")
    print("   # Use the helper script")
    print("   ./scripts/rotate_sops_keys.sh")
    print()
    print("   OR manually update each file:")
    print("   sops updatekeys -y <file>")
    print()
    print("3. Remove the old key from .sops.yaml:")
    print("   After rotation is complete, remove the old key fingerprint")
    print()
    print("4. Update GitHub Secret 'GPG_KEY':")
    print("   - Copy the base64 encoded key from above")
    print("   - Go to GitHub repository Settings > Secrets and variables > Actions")
    print("   - Update the 'GPG_KEY' secret with the base64 encoded value")
    print("="*80)


if __name__ == "__main__":
    main()

