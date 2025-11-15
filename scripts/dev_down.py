#!/usr/bin/env python3
"""
Development environment shutdown script.

Stops Tilt and K3s cluster for local development.
Replaces embedded shell script in justfile.
"""

import subprocess
import sys
from pathlib import Path

# Add scripts directory to path for imports
sys.path.insert(0, str(Path(__file__).parent))

from setup_k3s import log_info


def stop_tilt():
    """Stop Tilt processes."""
    log_info("Stopping Tilt...")
    # Kill tilt processes
    subprocess.run(
        ["pkill", "-f", "tilt up"],
        capture_output=True,
        check=False
    )


def stop_k3s():
    """Stop K3s container."""
    log_info("Stopping K3s container...")
    subprocess.run(
        ["docker", "stop", "k3s-secret-manager-controller"],
        capture_output=True,
        check=False
    )


def main():
    """Main development environment shutdown."""
    log_info("🛑 Stopping Secret Manager Controller development environment...")
    
    # Stop Tilt
    stop_tilt()
    
    # Stop K3s container
    stop_k3s()
    
    log_info("✅ Development environment stopped")


if __name__ == "__main__":
    main()

