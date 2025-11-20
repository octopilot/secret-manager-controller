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
    """Clean up Docker resources to free space before building.
    
    Only cleans up images we build ourselves (localhost:5000/* with tilt-* tags),
    not base images or dependencies. This prevents re-downloading images and
    hitting Docker rate limits.
    
    CRITICAL: Never removes infrastructure images like kindest/node or registry:2.
    """
    print("🧹 Cleaning up Docker resources to free space...")
    
    # CRITICAL: List of infrastructure images that must NEVER be removed
    # These are used by Kind clusters and local registries
    protected_images = {
        "kindest/node",
        "registry:",
        "registry/registry:",
    }
    
    # Remove dangling images (unused intermediate layers from our builds)
    # This is safe - only removes intermediate layers, not base images
    # However, we need to be careful not to remove layers shared with kindest/node
    print("  Removing dangling images (intermediate build layers only)...")
    result = run_command(["docker", "image", "prune", "-f"], check=False)
    if result.stdout:
        print(f"Total reclaimed space: {result.stdout.strip()}")
    
    # Remove old build cache (keeps only last 1 hour for faster builds)
    # This doesn't affect images, only build cache layers
    print("  Pruning build cache (older than 1 hour)...")
    result = run_command(["docker", "builder", "prune", "-a", "-f", "--filter", "until=1h"], check=False)
    if result.stdout:
        print(f"Total: {result.stdout.strip()}")
    
    # Remove old Tilt images - keep only the last 2 per service
    # For Tilt deployments, we only need the last 2 images per service
    print("  Removing old Tilt images (keeping last 2 per service)...")
    
    # Get all images with tilt-* tags (all Tilt services)
    # Group by repository and keep only the 2 most recent per repository
    result = run_command(
        ["docker", "images", "--format", "{{.Repository}}\t{{.Tag}}\t{{.ID}}\t{{.CreatedAt}}"],
        check=False
    )
    
    if result.returncode == 0 and result.stdout:
        # Group images by repository, filtering for tilt-* tags
        repos = {}
        for line in result.stdout.strip().split('\n'):
            if not line.strip():
                continue
            parts = line.strip().split('\t')
            if len(parts) >= 4:
                repo = parts[0]
                tag = parts[1]
                img_id = parts[2]
                created = parts[3]
                
                # Only process tilt-* tags or "tilt" tag (Tilt builds)
                if tag.startswith("tilt-") or tag == "tilt":
                    if repo not in repos:
                        repos[repo] = []
                    repos[repo].append((created, img_id, tag))
        
        # For each repository, keep only the 2 most recent images
        total_removed = 0
        for repo, images in repos.items():
            repo_tag_prefix = f"{repo}:"
            
            # CRITICAL: Never remove infrastructure images
            is_protected = False
            for protected_pattern in protected_images:
                if protected_pattern in repo_tag_prefix:
                    is_protected = True
                    print(f"    🔒 Protected (infrastructure): {repo}")
                    break
            
            if is_protected:
                continue
            
            # Sort by creation date (newest first)
            images.sort(key=lambda x: x[0], reverse=True)
            
            # Keep the 2 most recent, remove the rest
            if len(images) > 2:
                for created, img_id, tag in images[2:]:  # Skip first 2 (most recent)
                    repo_tag = f"{repo}:{tag}"
                    # CRITICAL: Remove by repository:tag, NOT by ID (to avoid removing shared layers)
                    remove_result = run_command(["docker", "rmi", repo_tag], check=False)
                    if remove_result.returncode == 0:
                        total_removed += 1
                        print(f"    Removed (old): {repo_tag}")
        
        if total_removed > 0:
            print(f"  Removed {total_removed} old Tilt image(s) (kept last 2 per service)")
    
    # NOTE: We do NOT run 'docker image prune -a' as it would remove:
    # - Base images (rust:alpine, debian, etc.)
    # - Pact broker images
    # - kindest/node images (used by Kind clusters)
    # - Other dependencies we download
    # This causes re-downloads and hits Docker rate limits


def main():
    """Main Docker build function."""
    image_name = os.getenv("IMAGE_NAME", "localhost:5000/secret-manager-controller")
    controller_name = os.getenv("CONTROLLER_NAME", "secret-manager-controller")
    controller_dir = os.getenv("CONTROLLER_DIR", ".")
    expected_ref = os.getenv("EXPECTED_REF", f"{image_name}:tilt")
    
    # Determine Dockerfile path - use dockerfiles directory
    dockerfile_path = "dockerfiles/Dockerfile.controller.dev"
    if not os.path.exists(dockerfile_path):
        dockerfile_path = "dockerfiles/Dockerfile.controller"
    if not os.path.exists(dockerfile_path):
        print(f"❌ Error: Dockerfile not found: {dockerfile_path}", file=sys.stderr)
        sys.exit(1)
    
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
