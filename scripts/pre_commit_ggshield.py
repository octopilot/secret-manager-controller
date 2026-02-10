#!/usr/bin/env python3
"""
Pre-commit hook for GitGuardian CLI (ggshield) secret scanning.

Scans staged files for secrets and sensitive information before committing.
This helps prevent accidental commits of secrets, API keys, tokens, etc.

Usage:
    This script is called by git pre-commit hook automatically
    Can also be run manually: python3 scripts/pre_commit_ggshield.py

Requirements:
    - ggshield must be installed: pip install ggshield
    - Or via homebrew: brew install gitguardian/tap/ggshield
    - Or via pipx: pipx install ggshield
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_error(msg):
    """Print error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def log_warn(msg):
    """Print warning message."""
    print(f"[WARN] {msg}")


def check_ggshield_installed() -> bool:
    """Check if ggshield is installed and available."""
    return shutil.which("ggshield") is not None


def install_ggshield_hint():
    """Print installation instructions for ggshield."""
    log_info("To install ggshield, use one of the following methods:")
    log_info("  - pip: pip install ggshield")
    log_info("  - pipx: pipx install ggshield")
    log_info("  - homebrew: brew install gitguardian/tap/ggshield")
    log_info("  - See: https://docs.gitguardian.com/platform/gitguardian-suite/gitguardian-cli-ggshield")


def scan_staged_files() -> bool:
    """Scan staged files for secrets using ggshield."""
    script_dir = Path(__file__).parent
    repo_root = script_dir.parent
    
    os.chdir(repo_root)
    
    log_info("Scanning staged files for secrets...")
    
    # Run ggshield scan on staged files
    # Use 'ggshield secret scan pre-commit' which is designed for git hooks
    # It automatically scans staged files from git index
    try:
        result = subprocess.run(
            ["ggshield", "secret", "scan", "pre-commit"],
            capture_output=True,
            text=True,
            check=False
        )
        
        # Check for authentication errors first
        if result.stderr and ("API key is needed" in result.stderr or "authenticate" in result.stderr.lower()):
            log_warn("ggshield is not authenticated. Secret scanning will be skipped.")
            log_warn("To authenticate, run: ggshield auth login")
            log_warn("Or set GITGUARDIAN_API_KEY environment variable")
            log_info("Continuing with commit (ggshield not authenticated)...")
            return True  # Don't block commits if not authenticated
        
        # ggshield pre-commit returns non-zero exit code if secrets are found
        if result.returncode == 0:
            log_info("✅ No secrets detected in staged files")
            if result.stdout:
                print(result.stdout)
            return True
        else:
            # Non-zero exit code means secrets were detected
            log_error("🚨 Secrets detected in staged files!")
            if result.stdout:
                print(result.stdout)
            if result.stderr:
                print(result.stderr, file=sys.stderr)
            log_error("Please remove secrets before committing.")
            log_info("You can review the scan results above or run: ggshield secret scan pre-commit")
            return False
                
    except FileNotFoundError:
        log_error("ggshield command not found")
        install_ggshield_hint()
        log_warn("Skipping secret scan (ggshield not installed)")
        return True  # Don't fail the commit if ggshield isn't installed
    except subprocess.CalledProcessError as e:
        # Check if it's an authentication error
        if e.stderr and ("API key is needed" in e.stderr or "authenticate" in e.stderr.lower()):
            log_warn("ggshield is not authenticated. Secret scanning will be skipped.")
            log_warn("To authenticate, run: ggshield auth login")
            return True  # Don't block commits if not authenticated
        log_error(f"ggshield scan failed: {e}")
        if e.stdout:
            print(e.stdout)
        if e.stderr:
            print(e.stderr, file=sys.stderr)
        return False


def main():
    """Main pre-commit function."""
    if not check_ggshield_installed():
        log_warn("ggshield is not installed. Secret scanning will be skipped.")
        log_warn("Consider installing ggshield for better security:")
        install_ggshield_hint()
        log_info("Continuing with commit (ggshield not required)...")
        sys.exit(0)  # Don't block commits if ggshield isn't installed
    
    if not scan_staged_files():
        sys.exit(1)
    
    log_info("Secret scanning passed!")
    sys.exit(0)


if __name__ == "__main__":
    main()

