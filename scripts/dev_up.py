#!/usr/bin/env python3
"""
Development environment startup script.

Starts K3s cluster and Tilt for local development.
Replaces embedded shell script in justfile.
"""

import os
import subprocess
import sys
from pathlib import Path

# Add scripts directory to path for imports
sys.path.insert(0, str(Path(__file__).parent))

def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_error(msg):
    """Print error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def check_command(cmd):
    """Check if a command exists."""
    import shutil
    if not shutil.which(cmd):
        log_error(f"{cmd} is not installed. Please install it first.")
        sys.exit(1)


def check_docker():
    """Check if Docker is running."""
    log_info("Checking Docker daemon...")
    result = subprocess.run(
        ["docker", "info"],
        capture_output=True,
        text=True
    )
    if result.returncode != 0:
        log_error("Docker daemon is not running")
        print("   Please start Docker Desktop and try again")
        sys.exit(1)
    log_info("✅ Docker daemon is running")


def start_k3s():
    """Start or create K3s cluster."""
    log_info("Setting up K3s cluster...")
    
    # Always call setup_k3s.py - it handles both creation and updates
    # Force non-interactive mode to avoid prompts when called from dev_up
    setup_script = Path(__file__).parent / "setup_k3s.py"
    env = os.environ.copy()
    env["NON_INTERACTIVE"] = "1"
    result = subprocess.run(
        [sys.executable, str(setup_script)],
        capture_output=False,
        env=env
    )
    if result.returncode != 0:
        log_error("Failed to setup K3s cluster")
        sys.exit(1)


def set_kubeconfig_context():
    """Set kubeconfig context to k3s cluster."""
    log_info("Setting kubeconfig context...")
    result = subprocess.run(
        ["kubectl", "config", "use-context", "k3s-secret-manager-controller"],
        capture_output=True,
        text=True
    )
    if result.returncode != 0:
        log_info("⚠️  Warning: Could not set k3s context, using current context")
    else:
        log_info("✅ Context set to k3s-secret-manager-controller")


def start_tilt():
    """Start Tilt development environment."""
    log_info("🎯 Starting Tilt...")
    # Run tilt up in foreground (will block until user stops it)
    subprocess.run(["tilt", "up"], check=False)


def main():
    """Main development environment startup."""
    log_info("🚀 Starting Secret Manager Controller development environment (K3s)...")
    
    # Check prerequisites
    check_command("docker")
    check_command("kubectl")
    check_command("tilt")
    
    # Check Docker is running
    check_docker()
    
    # Start K3s cluster
    start_k3s()
    
    # Set kubeconfig context
    set_kubeconfig_context()
    
    # Start Tilt
    start_tilt()


if __name__ == "__main__":
    main()

