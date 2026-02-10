#!/usr/bin/env python3
"""
Development environment shutdown script.

Stops Tilt, Kind cluster, and local registry for local development.
Replaces embedded shell script in justfile.
"""

import subprocess
import sys
from pathlib import Path


# Configuration (matches setup_kind.py)
REGISTRY_NAME = "secret-manager-controller-registry"


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_warn(msg):
    """Print warning message."""
    print(f"[WARN] {msg}")


def run_command(cmd, check=False, capture_output=True):
    """Run a command and return the result."""
    result = subprocess.run(
        cmd,
        shell=isinstance(cmd, str),
        capture_output=capture_output,
        text=True,
        check=check
    )
    return result


def stop_tilt():
    """Stop Tilt processes."""
    log_info("Stopping Tilt...")
    # Kill tilt processes
    result = run_command(["pkill", "-f", "tilt up"], check=False)
    if result.returncode == 0:
        log_info("✅ Tilt stopped")
    else:
        log_warn("No Tilt processes found (or already stopped)")


def stop_kind():
    """Stop Kind cluster."""
    log_info("Stopping Kind cluster...")
    result = run_command(
        ["kind", "delete", "cluster", "--name", "secret-manager-controller"],
        check=False,
        capture_output=True
    )
    if result.returncode == 0:
        log_info("✅ Kind cluster deleted")
    else:
        # Check if cluster exists
        cluster_check = run_command("kind get clusters", check=False, capture_output=True)
        if "secret-manager-controller" in cluster_check.stdout:
            log_warn("Cluster deletion had issues, but continuing with cleanup")
        else:
            log_info("Cluster already deleted or does not exist")


def stop_registry():
    """Stop and remove local Docker registry."""
    log_info("Stopping local Docker registry...")
    
    # Check if registry container exists
    result = run_command(f"docker ps -a --format '{{{{.Names}}}}'", check=False, capture_output=True)
    if REGISTRY_NAME not in result.stdout:
        log_info("Registry container does not exist")
        return
    
    # Stop the container if it's running
    result = run_command(f"docker ps --format '{{{{.Names}}}}'", check=False, capture_output=True)
    if REGISTRY_NAME in result.stdout:
        log_info(f"Stopping registry container '{REGISTRY_NAME}'...")
        stop_result = run_command(f"docker stop {REGISTRY_NAME}", check=False, capture_output=True)
        if stop_result.returncode == 0:
            log_info("✅ Registry container stopped")
        else:
            log_warn(f"Failed to stop registry: {stop_result.stderr}")
    
    # Remove the container
    log_info(f"Removing registry container '{REGISTRY_NAME}'...")
    remove_result = run_command(f"docker rm {REGISTRY_NAME}", check=False, capture_output=True)
    if remove_result.returncode == 0:
        log_info("✅ Registry container removed")
    else:
        if "No such container" in remove_result.stderr:
            log_info("Registry container already removed")
        else:
            log_warn(f"Failed to remove registry: {remove_result.stderr}")
    
    # Remove the registry volume if it exists
    volume_name = f"{REGISTRY_NAME}-data"
    log_info(f"Removing registry volume '{volume_name}'...")
    volume_result = run_command(f"docker volume rm {volume_name}", check=False, capture_output=True)
    if volume_result.returncode == 0:
        log_info("✅ Registry volume removed")
    else:
        if "No such volume" in volume_result.stderr:
            log_info("Registry volume already removed or does not exist")
        else:
            log_warn(f"Failed to remove registry volume: {volume_result.stderr}")


def main():
    """Main development environment shutdown."""
    log_info("🛑 Stopping Secret Manager Controller development environment...")
    
    # Stop Tilt
    stop_tilt()
    
    # Stop Kind cluster
    stop_kind()
    
    # Stop and remove registry
    stop_registry()
    
    log_info("✅ Development environment stopped and cleaned up")


if __name__ == "__main__":
    main()

