#!/usr/bin/env python3
"""
Check and install development dependencies.

Checks for Docker, and installs Tilt and Just if missing.
Replaces embedded shell script in justfile.
"""

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


def check_command(cmd):
    """Check if a command exists."""
    return shutil.which(cmd) is not None


def run_command(cmd, check=True):
    """Run a command and return the result."""
    result = subprocess.run(
        cmd,
        shell=True,
        capture_output=False,
        check=check
    )
    return result.returncode == 0


def install_tilt():
    """Install Tilt using official installer."""
    log_info("Installing Tilt...")
    install_script = (
        "curl -fsSL https://raw.githubusercontent.com/tilt-dev/tilt/master/scripts/install.sh | bash"
    )
    if not run_command(install_script, check=False):
        log_error("Failed to install Tilt")
        return False
    log_info("✅ Tilt installed")
    return True


def install_just():
    """Install Just using official installer."""
    log_info("Installing Just...")
    install_script = (
        "curl --proto '=https' --tlsv1.2 -sSf "
        "https://just.systems/install.sh | bash -s -- --to ~/.local/bin"
    )
    if not run_command(install_script, check=False):
        log_error("Failed to install Just")
        return False
    log_info("✅ Just installed")
    return True


def main():
    """Main dependency check function."""
    log_info("Checking dependencies...")
    
    # Check Docker
    if not check_command("docker"):
        log_error("docker is required but not installed.")
        sys.exit(1)
    log_info("✅ Docker is installed")
    
    # Install Tilt if not present
    if not check_command("tilt"):
        if not install_tilt():
            log_error("Failed to install Tilt")
            sys.exit(1)
    else:
        log_info("✅ Tilt is already installed")
    
    # Install Just if not present
    if not check_command("just"):
        if not install_just():
            log_error("Failed to install Just")
            sys.exit(1)
    else:
        log_info("✅ Just is already installed")
    
    log_info("✅ All tools are available!")


if __name__ == "__main__":
    main()

