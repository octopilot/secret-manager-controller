#!/usr/bin/env python3
"""
Development environment shutdown script.

Stops Tilt and Kind cluster for local development.
Replaces embedded shell script in justfile.
"""

import subprocess
import sys
from pathlib import Path


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def stop_tilt():
    """Stop Tilt processes."""
    log_info("Stopping Tilt...")
    # Kill tilt processes
    subprocess.run(
        ["pkill", "-f", "tilt up"],
        capture_output=True,
        check=False
    )


def stop_kind():
    """Stop Kind cluster."""
    log_info("Stopping Kind cluster...")
    subprocess.run(
        ["kind", "delete", "cluster", "--name", "secret-manager-controller"],
        capture_output=True,
        check=False
    )


def main():
    """Main development environment shutdown."""
    log_info("🛑 Stopping Secret Manager Controller development environment...")
    
    # Stop Tilt
    stop_tilt()
    
    # Stop Kind cluster
    stop_kind()
    
    log_info("✅ Development environment stopped")


if __name__ == "__main__":
    main()

