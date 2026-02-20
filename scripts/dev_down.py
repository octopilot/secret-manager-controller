#!/usr/bin/env python3
"""
Development environment shutdown script.

Stops Tilt and the Kind cluster for local development.
Does NOT stop octopilot-registry — it is shared infrastructure used by
multiple projects (op run, other Tilt setups) and is managed independently.
"""

import subprocess
import sys
from pathlib import Path


# octopilot-registry is shared — dev_down deliberately does not touch it.
# To stop it manually: docker stop octopilot-registry
REGISTRY_NAME = "octopilot-registry"


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
    """No-op: octopilot-registry is shared infrastructure and is not stopped here.

    The registry is used by multiple local development setups (op run, other Tilt
    sessions). Stopping it here would break those workflows unexpectedly.

    To stop the registry manually:
        docker stop octopilot-registry
    To remove it entirely:
        docker stop octopilot-registry && docker rm octopilot-registry
    """
    log_info(
        f"Skipping registry teardown — '{REGISTRY_NAME}' is shared infrastructure. "
        "Stop it manually if needed: docker stop octopilot-registry"
    )


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

