#!/usr/bin/env python3
"""
Cleanup controller images before rebuild.

This script replaces the inline shell script in Tiltfile for cleanup.
It handles:
- Removing Docker images
- Cleaning up kind registry cache

Note: Pod deletion is no longer needed as the root cause of binary/container
update issues has been identified and resolved.
"""

import os
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


def main():
    """Main cleanup function."""
    image_name = os.getenv("IMAGE_NAME", "localhost:5000/secret-manager-controller")
    controller_name = os.getenv("CONTROLLER_NAME", "secret-manager-controller")
    
    print("🧹 Cleaning up controller images before rebuild...")
    
    # Delete all versions of the image to force fresh build
    print("📋 Deleting all image tags...")
    run_command(["docker", "rmi", f"{image_name}:tilt"], check=False)
    
    # Remove all tilt-* tags (Tilt generates these based on content hash)
    list_tags_result = run_command(
        ["docker", "images", image_name, "--format", "{{.Tag}}"],
        check=False
    )
    if list_tags_result.returncode == 0 and list_tags_result.stdout:
        for tag in list_tags_result.stdout.strip().split("\n"):
            tag = tag.strip()
            if tag.startswith("tilt-"):
                run_command(["docker", "rmi", f"{image_name}:{tag}"], check=False)
                run_command(["docker", "rmi", f"localhost:5000/{controller_name}:{tag}"], check=False)
    
    run_command(["docker", "rmi", f"localhost:5000/{controller_name}:tilt"], check=False)
    
    # Also try to remove from kind's containerd if it's a kind cluster
    print("📋 Cleaning up kind registry cache...")
    run_command(
        ["docker", "exec", "kind-registry", "sh", "-c", f"rm -rf /var/lib/registry/docker/registry/v2/repositories/{controller_name}/"],
        check=False
    )
    
    # Force remove dangling images
    run_command(["docker", "image", "prune", "-f"], check=False)
    
    print("✅ Cleanup complete")


if __name__ == "__main__":
    main()

