#!/usr/bin/env python3
"""
Pre-commit hook for SOPS encryption validation.

Ensures that files matching SOPS patterns (application.secrets.*, .env) are encrypted
before being committed to the repository.

SECURITY: This hook scans the entire repository for unencrypted secret files.
This ensures that even if a developer places an unencrypted secret file in the
wrong directory (and doesn't stage it), the hook will still catch it.

Usage:
    This script is called by git pre-commit hook automatically
    Can also be run manually: python3 scripts/pre_commit_sops.py
"""

import os
import re
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


def is_sops_encrypted(file_path: Path) -> bool:
    """
    Check if a file is SOPS-encrypted.
    
    SOPS-encrypted files have:
    - YAML files: "sops:" key at root level
    - JSON files: "sops" key at root level
    - ENV files: "sops:" in comments or ENC[...] markers
    """
    try:
        content = file_path.read_text()
        
        # Check for SOPS metadata indicators
        # YAML/JSON files have "sops:" or "sops" key
        if '"sops"' in content or "'sops'" in content or "sops:" in content:
            # Verify it's actually SOPS metadata (not just the word "sops" in content)
            # SOPS files have a specific structure
            if re.search(r'sops:\s*\{', content) or re.search(r'"sops"\s*:', content):
                return True
        
        # Check for ENC[...] markers (used in some SOPS formats)
        if re.search(r'ENC\[', content):
            return True
        
        # Check for SOPS version/metadata in comments (for ENV files)
        if re.search(r'sops_version|sops_encrypted', content, re.IGNORECASE):
            return True
        
        return False
    except Exception as e:
        log_warn(f"Could not read file {file_path}: {e}")
        return False


def find_secret_files_in_repo(repo_root: Path) -> list[Path]:
    """
    Scan the repository for files that should be encrypted.
    
    Finds all files matching application.secrets.* or .env patterns
    anywhere in the repository (excluding .git directory).
    """
    secret_files = []
    
    # Patterns to match
    patterns = [
        r'.*application\.secrets\..*',  # application.secrets.yaml, application.secrets.env, etc.
        r'.*\.env$',  # .env files (but we'll filter out .env.example, etc. later)
    ]
    
    # Walk the repository (excluding .git)
    for root, dirs, files in os.walk(repo_root):
        # Skip .git directory
        if '.git' in root:
            continue
        
        # Skip common build/dependency directories
        skip_dirs = {'.git', 'target', 'node_modules', '.venv', 'venv', '__pycache__', '.pytest_cache'}
        dirs[:] = [d for d in dirs if d not in skip_dirs]
        
        for file in files:
            file_path = Path(root) / file
            
            # Check if file matches our patterns
            file_str = str(file_path.relative_to(repo_root))
            
            # Check for application.secrets.* pattern
            if re.search(r'application\.secrets\.', file_str, re.IGNORECASE):
                secret_files.append(file_path)
            # Check for .env files (excluding .env.example, .env.template, etc.)
            elif re.search(r'\.env$', file_str, re.IGNORECASE) and not re.search(r'\.env\.(example|template|sample)', file_str, re.IGNORECASE):
                secret_files.append(file_path)
    
    return secret_files


def should_check_file(file_path: Path) -> bool:
    """
    Determine if a file should be checked for SOPS encryption.
    
    Files that should be encrypted:
    - application.secrets.* (any extension)
    - .env files (in root or specific directories)
    """
    file_str = str(file_path)
    
    # Check for application.secrets.* pattern
    if re.search(r'application\.secrets\.', file_str, re.IGNORECASE):
        return True
    
    # Check for .env files (but allow .env.example, .env.template, etc.)
    if re.search(r'\.env$', file_str, re.IGNORECASE) and not re.search(r'\.env\.(example|template|sample)', file_str, re.IGNORECASE):
        return True
    
    return False


def main():
    """Main pre-commit function."""
    # Check if sops is available
    if not shutil.which("sops"):
        log_warn("sops is not installed. Skipping SOPS encryption check.")
        log_warn("Install sops: brew install sops (macOS) or see https://github.com/mozilla/sops")
        # Don't fail - sops might not be needed for all commits
        sys.exit(0)
    
    # Get repository root
    repo_root_result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
        check=True
    )
    repo_root = Path(repo_root_result.stdout.strip())
    
    # SECURITY: Scan entire repository for secret files
    # This catches unencrypted secrets even if they're not staged
    secret_files = find_secret_files_in_repo(repo_root)
    
    if not secret_files:
        log_info("No secret files found in repository.")
        sys.exit(0)
    
    log_info(f"Scanning repository: Found {len(secret_files)} secret file(s) to check")
    
    # Check each secret file for encryption
    unencrypted_files = []
    for file_path in secret_files:
        # Check if file exists (might have been deleted)
        if not file_path.exists():
            continue
        
        # Validate encryption
        if not is_sops_encrypted(file_path):
            # Get relative path for cleaner output
            rel_path = file_path.relative_to(repo_root)
            unencrypted_files.append(rel_path)
    
    # Report results
    if unencrypted_files:
        log_error("=" * 80)
        log_error("SOPS ENCRYPTION CHECK FAILED")
        log_error("=" * 80)
        log_error("")
        log_error("The following files contain secrets but are NOT encrypted:")
        log_error("")
        for file_path in unencrypted_files:
            log_error(f"  - {file_path}")
        log_error("")
        log_error("SECURITY RISK: Unencrypted secrets found in repository!")
        log_error("")
        log_error("To encrypt these files, run:")
        log_error("")
        for file_path in unencrypted_files:
            log_error(f"  sops -e -i {file_path}")
        log_error("")
        log_error("Or use 'sops <file>' to edit and encrypt interactively.")
        log_error("")
        log_error("See .sops.yaml for encryption key configuration.")
        log_error("")
        log_error("NOTE: This check scans the entire repository, not just staged files.")
        log_error("      This ensures no unencrypted secrets exist anywhere.")
        log_error("=" * 80)
        sys.exit(1)
    
    log_info(f"SOPS encryption check passed - all {len(secret_files)} secret file(s) are encrypted!")
    sys.exit(0)


if __name__ == "__main__":
    main()

