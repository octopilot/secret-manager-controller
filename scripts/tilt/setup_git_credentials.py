#!/usr/bin/env python3
"""
Setup Git credentials for FluxCD GitRepository.

This script:
1. Detects if running in CI (GitHub Actions, GitLab CI, etc.) or locally
2. For CI: Reads git credentials from environment variables (GITHUB_TOKEN, etc.)
3. For local: Reads git credentials from SOPS-encrypted .env file
4. Creates Kubernetes secrets for GitRepository authentication
5. Supports both HTTPS (token-based) and SSH (private key) authentication

Environment Variables (CI):
  - GITHUB_TOKEN: GitHub personal access token (recommended)
  - GIT_TOKEN or GIT_PASSWORD: Generic git token
  - GIT_SSH_KEY or GIT_SSH_PRIVATE_KEY: SSH private key

.env File (Local):
  - GITHUB_TOKEN: GitHub personal access token (recommended)
  - GIT_TOKEN or GIT_PASSWORD: Generic git token
  - GIT_SSH_KEY: SSH private key
"""

import os
import subprocess
import sys
from pathlib import Path


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_warn(msg):
    """Print warning message."""
    print(f"[WARN] {msg}")


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


def check_sops_installed():
    """Check if SOPS is installed."""
    result = run_command("sops --version", check=False, capture_output=True)
    if result.returncode != 0:
        return False
    return True


def is_ci_environment() -> bool:
    """Check if running in CI environment (GitHub Actions, GitLab CI, etc.)."""
    # Check for common CI environment variables
    ci_indicators = [
        "GITHUB_ACTIONS",  # GitHub Actions
        "CI",  # Generic CI indicator
        "GITLAB_CI",  # GitLab CI
        "JENKINS_URL",  # Jenkins
        "CIRCLECI",  # CircleCI
    ]
    
    for indicator in ci_indicators:
        if os.environ.get(indicator):
            return True
    
    return False


def get_env_vars_from_file(env_file: Path) -> dict:
    """Decrypt SOPS-encrypted .env file and return as dictionary."""
    if not env_file.exists():
        log_warn(f".env file not found: {env_file}")
        return {}
    
    log_info(f"Decrypting .env file: {env_file}")
    
    # Check if SOPS is installed (only needed for local .env files)
    if not check_sops_installed():
        log_warn("SOPS not available, cannot decrypt .env file")
        return {}
    
    # Decrypt using SOPS
    result = run_command(
        ["sops", "-d", str(env_file)],
        check=False,
        capture_output=True
    )
    
    if result.returncode != 0:
        log_error(f"Failed to decrypt .env file: {result.stderr}")
        log_info("Make sure SOPS is configured and GPG keys are available")
        return {}
    
    # Parse decrypted content
    env_vars = {}
    for line in result.stdout.splitlines():
        line = line.strip()
        # Skip comments and empty lines
        if not line or line.startswith('#'):
            continue
        # Parse KEY=VALUE format
        if '=' in line:
            key, value = line.split('=', 1)
            env_vars[key.strip()] = value.strip()
    
    log_info(f"Decrypted {len(env_vars)} environment variables from .env file")
    return env_vars


def get_env_vars_from_environment() -> dict:
    """Get git credentials from environment variables (for CI environments)."""
    env_vars = {}
    
    # Check for git credential environment variables
    # Focus on tokens (HTTPS) and SSH keys
    git_credential_vars = [
        "GITHUB_TOKEN",
        "GIT_TOKEN",
        "GIT_PASSWORD",  # Treated as token
        "GIT_SSH_KEY",
        "GIT_SSH_PRIVATE_KEY",
    ]
    
    for var in git_credential_vars:
        value = os.environ.get(var)
        if value:
            env_vars[var] = value
    
    if env_vars:
        log_info(f"Found {len(env_vars)} git credential environment variables")
    
    return env_vars


def get_git_credentials(env_file: Path) -> dict:
    """Get git credentials from either .env file (local) or environment variables (CI).
    
    Priority:
    1. Environment variables - always takes precedence (works in both CI and local)
    2. .env file - used if env vars not found or as base values
    
    This allows:
    - CI: Use GITHUB_TOKEN from GitHub Actions secrets
    - Local: Use .env file (SOPS-encrypted)
    - Override: Environment variables can override .env file values (useful for testing)
    
    Returns:
        Dictionary of environment variables containing git credentials
    """
    is_ci = is_ci_environment()
    env_vars = {}
    
    # Always check environment variables first (they take precedence)
    env_var_creds = get_env_vars_from_environment()
    
    if is_ci:
        log_info("Running in CI environment - checking environment variables...")
        if env_var_creds:
            log_info("✅ Using git credentials from environment variables")
            return env_var_creds
        else:
            log_warn("No git credentials found in environment variables")
            log_info("Falling back to .env file (if available)...")
    else:
        log_info("Running in local environment - checking .env file...")
        if env_var_creds:
            log_info("Found environment variables (will override .env file values)")
    
    # Try .env file (local development or fallback in CI)
    file_vars = get_env_vars_from_file(env_file)
    
    if file_vars:
        if is_ci:
            log_info("✅ Using git credentials from .env file (fallback)")
        else:
            log_info("✅ Using git credentials from .env file")
        env_vars.update(file_vars)
    
    # Merge: environment variables take precedence over .env file
    # This allows overriding .env file values with env vars (useful for testing)
    env_vars.update(env_var_creds)
    
    return env_vars


def create_https_secret(env_vars: dict, secret_name: str, namespace: str) -> bool:
    """Create Kubernetes secret for HTTPS git authentication using bearerToken.
    
    FluxCD GitRepository supports bearerToken for token-based authentication.
    This works with GitHub, GitLab, Bitbucket, and other modern Git providers.
    """
    # Check for token credentials
    # Support GITHUB_TOKEN, GIT_TOKEN, or GIT_PASSWORD (all treated as tokens)
    token = env_vars.get('GITHUB_TOKEN') or env_vars.get('GIT_TOKEN') or env_vars.get('GIT_PASSWORD')
    
    if not token:
        log_info("No HTTPS git token found in .env")
        log_info("Supported variables: GITHUB_TOKEN, GIT_TOKEN, or GIT_PASSWORD")
        return False
    
    log_info(f"Creating HTTPS git credentials secret with bearerToken: {secret_name}")
    
    result = run_command(
        [
            "kubectl", "create", "secret", "generic", secret_name,
            "--from-literal=bearerToken=" + token,
            "-n", namespace,
            "--dry-run=client", "-o", "yaml"
        ],
        check=False,
        capture_output=True
    )
    
    if result.returncode != 0:
        log_error(f"Failed to generate secret YAML: {result.stderr}")
        return False
    
    # Apply the secret
    apply_result = run_command(
        ["kubectl", "apply", "-f", "-"],
        input=result.stdout,
        check=False,
        capture_output=True
    )
    
    if apply_result.returncode != 0:
        # Check if secret already exists
        if "already exists" in apply_result.stderr:
            log_warn(f"Secret {secret_name} already exists. Updating...")
            # Delete and recreate
            run_command(
                ["kubectl", "delete", "secret", secret_name, "-n", namespace],
                check=False
            )
            # Try again
            apply_result = run_command(
                ["kubectl", "apply", "-f", "-"],
                input=result.stdout,
                check=False,
                capture_output=True
            )
        
        if apply_result.returncode != 0:
            log_error(f"Failed to create secret: {apply_result.stderr}")
            return False
    
    log_info(f"✅ Created HTTPS git credentials secret: {secret_name}")
    return True


def create_ssh_secret(env_vars: dict, secret_name: str, namespace: str) -> bool:
    """Create Kubernetes secret for SSH git authentication."""
    # Check for SSH private key
    ssh_key = env_vars.get('GIT_SSH_KEY') or env_vars.get('GIT_SSH_PRIVATE_KEY')
    
    if not ssh_key:
        log_info("No SSH git credentials found in .env (GIT_SSH_KEY)")
        return False
    
    log_info(f"Creating SSH git credentials secret: {secret_name}")
    
    # Create secret using kubectl
    result = run_command(
        [
            "kubectl", "create", "secret", "generic", secret_name,
            "--from-literal=identity=" + ssh_key,
            "-n", namespace,
            "--dry-run=client", "-o", "yaml"
        ],
        check=False,
        capture_output=True
    )
    
    if result.returncode != 0:
        log_error(f"Failed to generate secret YAML: {result.stderr}")
        return False
    
    # Apply the secret
    apply_result = run_command(
        ["kubectl", "apply", "-f", "-"],
        input=result.stdout,
        check=False,
        capture_output=True
    )
    
    if apply_result.returncode != 0:
        # Check if secret already exists
        if "already exists" in apply_result.stderr:
            log_warn(f"Secret {secret_name} already exists. Updating...")
            # Delete and recreate
            run_command(
                ["kubectl", "delete", "secret", secret_name, "-n", namespace],
                check=False
            )
            # Try again
            apply_result = run_command(
                ["kubectl", "apply", "-f", "-"],
                input=result.stdout,
                check=False,
                capture_output=True
            )
        
        if apply_result.returncode != 0:
            log_error(f"Failed to create secret: {apply_result.stderr}")
            return False
    
    log_info(f"✅ Created SSH git credentials secret: {secret_name}")
    return True


def main():
    """Main function."""
    import argparse
    
    parser = argparse.ArgumentParser(
        description="Setup Git credentials for FluxCD GitRepository from SOPS-encrypted .env file"
    )
    parser.add_argument(
        "--env-file",
        type=Path,
        default=Path(".env"),
        help="Path to SOPS-encrypted .env file (default: .env)"
    )
    parser.add_argument(
        "--secret-name",
        default="git-credentials",
        help="Kubernetes secret name (default: git-credentials)"
    )
    parser.add_argument(
        "--namespace",
        default="flux-system",
        help="Kubernetes namespace (default: flux-system). Use comma-separated list for multiple namespaces."
    )
    parser.add_argument(
        "--also-namespace",
        action="append",
        help="Additional namespace to create secret in (can be used multiple times, e.g., --also-namespace tilt --also-namespace dev)"
    )
    parser.add_argument(
        "--all-environments",
        action="store_true",
        help="Create secrets in all environment namespaces (tilt, dev, stage, prod) plus flux-system and microscaler-system"
    )
    parser.add_argument(
        "--auth-type",
        choices=["auto", "https", "ssh"],
        default="auto",
        help="Authentication type: auto (detect), https, or ssh (default: auto)"
    )
    
    args = parser.parse_args()
    
    log_info("Git Credentials Setup Script")
    log_info("=" * 50)
    
    # Detect environment
    is_ci = is_ci_environment()
    if is_ci:
        log_info("Environment: CI (GitHub Actions/GitLab CI/etc.)")
        log_info("  - Will use environment variables (GITHUB_TOKEN, etc.)")
        log_info("  - Will fall back to .env file if env vars not found")
    else:
        log_info("Environment: Local development")
        log_info("  - Will use SOPS-encrypted .env file")
        log_info("  - Environment variables will override .env file if present")
    
    # Get git credentials from appropriate source
    env_vars = get_git_credentials(args.env_file)
    
    if not env_vars:
        log_warn("No git credentials found.")
        if is_ci:
            log_info("For CI environments, set environment variables:")
            log_info("  - GITHUB_TOKEN (recommended for GitHub)")
            log_info("  - GIT_TOKEN or GIT_PASSWORD (for generic Git providers)")
            log_info("  - GIT_SSH_KEY (for SSH authentication)")
            log_info("")
            log_info("Or provide a .env file as fallback.")
        else:
            log_info("For local development, add credentials to .env file:")
            log_info("  - GITHUB_TOKEN=ghp_... (recommended for GitHub)")
            log_info("  - GIT_TOKEN=... or GIT_PASSWORD=... (for generic Git providers)")
            log_info("  - GIT_SSH_KEY=... (for SSH authentication)")
            log_info("")
            log_info("Encrypt the .env file with SOPS:")
            log_info("  sops -e -i .env")
        sys.exit(0)
    
    # Determine namespaces to create secrets in
    namespaces = [args.namespace]
    
    if args.all_environments:
        # Create secrets in all environment namespaces
        environment_namespaces = ["tilt", "dev", "stage", "prod", "microscaler-system"]
        namespaces.extend(environment_namespaces)
        # Remove duplicates while preserving order
        namespaces = list(dict.fromkeys(namespaces))
    elif args.also_namespace:
        # Add additional namespaces specified via --also-namespace
        namespaces.extend(args.also_namespace)
        # Remove duplicates while preserving order
        namespaces = list(dict.fromkeys(namespaces))
    
    # Create secrets based on auth type in all specified namespaces
    created = False
    
    for namespace in namespaces:
        log_info(f"Processing namespace: {namespace}")
        
        # Check if namespace exists, create it if it doesn't (for environment namespaces)
        check_ns_result = run_command(
            f"kubectl get namespace {namespace}",
            check=False,
            capture_output=True
        )
        
        if check_ns_result.returncode != 0:
            # Try to create the namespace if it's an environment namespace
            if namespace in ["tilt", "dev", "stage", "prod"]:
                log_info(f"Creating namespace: {namespace}")
                create_ns_result = run_command(
                    f"kubectl create namespace {namespace}",
                    check=False,
                    capture_output=True
                )
                if create_ns_result.returncode != 0:
                    log_warn(f"⚠️  Could not create namespace {namespace}: {create_ns_result.stderr}")
                    log_warn(f"   Skipping secret creation in {namespace}")
                    continue
            else:
                log_warn(f"⚠️  Namespace {namespace} does not exist, skipping secret creation")
                log_warn(f"   (flux-system and microscaler-system should be created by their respective installers)")
                continue
        
        if args.auth_type == "auto":
            # Try HTTPS first, then SSH
            if create_https_secret(env_vars, args.secret_name, namespace):
                created = True
            elif create_ssh_secret(env_vars, args.secret_name, namespace):
                created = True
        elif args.auth_type == "https":
            if create_https_secret(env_vars, args.secret_name, namespace):
                created = True
        elif args.auth_type == "ssh":
            if create_ssh_secret(env_vars, args.secret_name, namespace):
                created = True
    
    if not created:
        log_warn("No git credentials found or credentials were invalid")
        if is_ci:
            log_info("For CI environments, ensure environment variables are set:")
            log_info("  - GITHUB_TOKEN (recommended for GitHub)")
            log_info("  - GIT_TOKEN or GIT_PASSWORD (for generic Git providers)")
            log_info("  - GIT_SSH_KEY (for SSH authentication)")
        else:
            log_info("For local development, add credentials to .env file:")
            log_info("  - GITHUB_TOKEN=ghp_... (recommended for GitHub)")
            log_info("  - GIT_TOKEN=... or GIT_PASSWORD=... (for generic Git providers)")
            log_info("  - GIT_SSH_KEY=... (for SSH authentication)")
        sys.exit(0)
    
    log_info("")
    log_info("✅ Git credentials setup complete!")
    log_info(f"📋 Secret name: {args.secret_name}")
    log_info(f"📋 Namespaces: {', '.join(namespaces)}")
    log_info("")
    log_info("Next steps:")
    log_info("  1. Update GitRepository to reference this secret:")
    log_info(f"     secretRef:")
    log_info(f"       name: {args.secret_name}")
    log_info("  2. Verify secrets exist in all namespaces:")
    for namespace in namespaces:
        log_info(f"     kubectl get secret {args.secret_name} -n {namespace}")


if __name__ == "__main__":
    main()

