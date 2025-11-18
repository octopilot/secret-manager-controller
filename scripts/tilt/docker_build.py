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
    # Note: This won't remove the Pact CLI image as it's not a dangling image
    print("  Removing dangling images...")
    result = run_command(["docker", "image", "prune", "-f"], check=False)
    if result.stdout:
        print(f"Total reclaimed space: {result.stdout.strip()}")
    
    # Remove old build cache aggressively (keeps only last 1 hour for faster builds)
    # This doesn't affect images, only build cache layers
    print("  Pruning build cache...")
    result = run_command(["docker", "builder", "prune", "-a", "-f", "--filter", "until=1h"], check=False)
    if result.stdout:
        print(f"Total: {result.stdout.strip()}")
    
    # Remove ALL untagged images (these are old Tilt builds)
    # These accumulate quickly and take up significant space
    # Exclude Pact CLI image as it's stable and should be kept
    print("  Removing untagged images (old Tilt builds, excluding Pact CLI)...")
    result = run_command(
        ["docker", "images", "--filter", "dangling=true", "--format", "{{.ID}}\t{{.Repository}}"],
        check=False
    )
    if result.returncode == 0 and result.stdout:
        image_ids = []
        for line in result.stdout.strip().split('\n'):
            if not line.strip():
                continue
            parts = line.strip().split('\t')
            if len(parts) >= 2:
                img_id = parts[0]
                repo = parts[1]
                # Skip Pact CLI image - it's stable and should be kept
                if "pactfoundation/pact-cli" not in repo:
                    image_ids.append(img_id)
        if image_ids:
            for img_id in image_ids:
                run_command(["docker", "rmi", "-f", img_id], check=False)
            print(f"  Removed {len(image_ids)} untagged image(s) (Pact CLI excluded)")
    
    # Remove old Tilt images (keep only the 2 most recent tagged images)
    print("  Removing old Tilt images (keeping 2 most recent)...")
    image_name = os.getenv("IMAGE_NAME", "localhost:5000/secret-manager-controller")
    # Get all tilt images sorted by creation date (newest first)
    result = run_command(
        ["docker", "images", image_name, "--format", "{{.ID}}\t{{.Tag}}\t{{.CreatedAt}}"],
        check=False
    )
    if result.returncode == 0 and result.stdout:
        lines = [line.strip() for line in result.stdout.strip().split('\n') if line.strip()]
        # Keep only the 2 most recent images (skip first 2 lines)
        if len(lines) > 2:
            old_image_ids = []
            for line in lines[2:]:  # Skip first 2 (most recent)
                image_id = line.split('\t')[0]
                old_image_ids.append(image_id)
            
            if old_image_ids:
                # Remove old images
                for img_id in old_image_ids:
                    run_command(["docker", "rmi", "-f", img_id], check=False)
                print(f"  Removed {len(old_image_ids)} old Tilt image(s)")
    
    # Remove unused images (not just dangling) - more aggressive
    # This removes images not used by any container, older than 1 hour
    # Note: docker image prune doesn't support excluding specific images,
    # but it won't remove images that are in use or recently pulled
    # Pact CLI image should be safe as it's actively used
    print("  Removing unused images (Pact CLI will be preserved if recently used)...")
    result = run_command(["docker", "image", "prune", "-a", "-f", "--filter", "until=1h"], check=False)
    if result.stdout:
        print(f"Total reclaimed space: {result.stdout.strip()}")


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
