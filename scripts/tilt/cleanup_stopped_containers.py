#!/usr/bin/env python3
"""
Cleanup stopped Docker containers to prevent overwhelming Docker.

This script removes stopped containers, particularly those created by Tilt
for image builds (e.g., secret-manager-controller containers).

It's safe to run repeatedly as it only removes stopped containers.

Runs as a one-shot cleanup after controller builds complete.
"""

import subprocess
import sys


def run_command(cmd, check=False, capture_output=True):
    """Run a command and return the result."""
    result = subprocess.run(cmd, capture_output=capture_output, text=True)
    if not capture_output:
        return result
    if result.stdout:
        print(result.stdout, end="")
    if result.stderr and result.returncode != 0:
        print(result.stderr, end="", file=sys.stderr)
    return result


def get_stopped_containers():
    """Get list of stopped container IDs."""
    result = run_command(
        ["docker", "ps", "-a", "--filter", "status=exited", "--format", "{{.ID}}"],
        check=False
    )
    if result.returncode != 0:
        return []
    
    container_ids = [line.strip() for line in result.stdout.strip().split("\n") if line.strip()]
    return container_ids


def get_container_info(container_id):
    """Get container name and image for a container ID."""
    result = run_command(
        ["docker", "inspect", "--format", "{{.Name}} {{.Config.Image}}", container_id],
        check=False
    )
    if result.returncode == 0 and result.stdout:
        return result.stdout.strip()
    return None


def main():
    """Main cleanup function - one-shot mode."""
    print("🧹 Cleaning up stopped Docker containers...")
    
    # Get all stopped containers
    stopped_containers = get_stopped_containers()
    
    if not stopped_containers:
        print("✅ No stopped containers found")
        return 0
    
    print(f"📋 Found {len(stopped_containers)} stopped container(s)")
    
    removed_count = 0
    failed_count = 0
    
    for container_id in stopped_containers:
        container_info = get_container_info(container_id)
        if container_info:
            container_name, image = container_info.split(" ", 1)
            # Log controller-related containers
            if "secret-manager-controller" in container_name or "secret-manager-controller" in image:
                print(f"  Removing: {container_name} ({image[:50]}...)")
        
        # Remove the container
        result = run_command(
            ["docker", "rm", container_id],
            check=False
        )
        
        if result.returncode == 0:
            removed_count += 1
        else:
            failed_count += 1
            if container_info:
                print(f"  ⚠️  Failed to remove: {container_info}", file=sys.stderr)
    
    print(f"✅ Cleanup complete: removed {removed_count} container(s)")
    if failed_count > 0:
        print(f"⚠️  Failed to remove {failed_count} container(s)", file=sys.stderr)
        return 1
    
    return 0


if __name__ == "__main__":
    sys.exit(main())

