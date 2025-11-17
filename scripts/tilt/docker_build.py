#!/usr/bin/env python3
"""
Docker build script for Tilt custom_build.

This script replaces the inline shell script in custom_build for Docker builds.
It handles:
- Building Docker image (with layer caching for speed)
- Tagging and pushing to registry

Note: Uses Docker layer caching for faster incremental builds. Only changed layers
will be rebuilt, significantly speeding up the build process.
"""

import os
import subprocess
import sys


def run_command(cmd_list, check=False, capture_output=True):
    """Run a command as a list (not shell string) and return the result."""
    result = subprocess.run(cmd_list, capture_output=capture_output, text=True)
    if not capture_output:
        return result
    if result.stdout:
        print(result.stdout, end="")
    if result.stderr and result.returncode != 0:
        print(result.stderr, end="", file=sys.stderr)
    return result


def cleanup_docker_resources():
    """Clean up Docker resources to free space before building."""
    print("🧹 Cleaning up Docker resources to free space...")
    
    # Remove dangling images (unused intermediate layers)
    print("  Removing dangling images...")
    run_command(["docker", "image", "prune", "-f"], check=False)
    
    # Remove old build cache (keeps recent cache for faster builds)
    print("  Pruning build cache...")
    run_command(["docker", "builder", "prune", "-f", "--filter", "until=24h"], check=False)
    
    # Remove unused images (not just dangling)
    # This removes images not used by any container
    print("  Removing unused images...")
    run_command(["docker", "image", "prune", "-a", "-f", "--filter", "until=24h"], check=False)


def main():
    """Main Docker build function."""
    image_name = os.getenv("IMAGE_NAME", "localhost:5000/secret-manager-controller")
    controller_name = os.getenv("CONTROLLER_NAME", "secret-manager-controller")
    controller_dir = os.getenv("CONTROLLER_DIR", ".")
    expected_ref = os.getenv("EXPECTED_REF", f"{image_name}:tilt")
    
    dockerfile_path = os.path.join(controller_dir, "Dockerfile.dev")
    
    # Clean up Docker resources before building to prevent "No space left on device" errors
    # This is especially important when Docker Desktop's VM disk is getting full
    cleanup_docker_resources()
    
    print(f"🔨 Building Docker image (using cache)...")
    
    # Build Docker image with the expected reference tag
    # Using Docker layer caching for faster builds - only changed layers rebuild
    # Tilt will generate content-hash tags (e.g., tilt-23c8db1e702a59c9) automatically
    build_result = run_command(
        ["docker", "build", "-f", dockerfile_path, "-t", expected_ref, controller_dir],
        check=False,
        capture_output=False
    )
    if build_result.returncode != 0:
        print("❌ Error: Docker build failed", file=sys.stderr)
        sys.exit(build_result.returncode)
    
    # Push image - Tilt will retag with content hash and use that
    push_result = run_command(
        ["docker", "push", expected_ref],
        check=False,
        capture_output=False
    )
    if push_result.returncode != 0:
        print("❌ Error: Docker push failed", file=sys.stderr)
        sys.exit(push_result.returncode)
    
    print(f"✅ Docker image built and pushed: {expected_ref}")
    
    # CRITICAL: Output the image reference to stdout for Tilt's custom_build
    # Tilt expects the script to output the final image reference
    # Tilt will automatically create content-hash tags (e.g., tilt-{hash}) 
    # and retag/push as needed
    print(expected_ref, file=sys.stdout)


if __name__ == "__main__":
    main()
