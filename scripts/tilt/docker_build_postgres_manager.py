#!/usr/bin/env python3
"""
Build Docker image for postgres-manager.

This script builds the postgres-manager Docker image using the Dockerfile.
It follows the same pattern as docker_build_mock_server.py and docker_build_webhook.py.
"""

import os
import subprocess
import sys
from pathlib import Path


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_error(msg):
    """Print error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def main():
    """Build postgres-manager Docker image."""
    # Tilt provides EXPECTED_REF which is the full image reference it expects
    # This includes the registry, image name, and tag
    expected_ref = os.getenv("EXPECTED_REF")
    if not expected_ref:
        # Fallback for manual execution
        image_name = os.getenv("IMAGE_NAME", "localhost:5000/postgres-manager")
        tag = os.getenv("TAG", "tilt")
        expected_ref = f"{image_name}:{tag}"
    
    tagged_image = expected_ref
    
    log_info(f"Building postgres-manager Docker image: {tagged_image}")
    
    # Verify binary exists
    binary_path = Path("build_artifacts/mock-server/postgres-manager")
    if not binary_path.exists():
        log_error(f"Binary not found: {binary_path}")
        log_error("  Make sure 'copy-mock-server-binaries' has run first")
        sys.exit(1)
    
    log_info(f"  Binary found: {binary_path} ({binary_path.stat().st_size:,} bytes)")
    
    # Build Docker image (use optimized version)
    dockerfile = Path("dockerfiles/Dockerfile.postgres-manager.optimized")
    if not dockerfile.exists():
        # Fallback to non-optimized if optimized doesn't exist
        dockerfile = Path("dockerfiles/Dockerfile.postgres-manager")
        if not dockerfile.exists():
            log_error(f"Dockerfile not found: {dockerfile}")
            sys.exit(1)
        log_info("  Using non-optimized Dockerfile (optimized not found)")
    else:
        log_info("  Using optimized Dockerfile (alpine base)")
    
    log_info(f"  Dockerfile: {dockerfile}")
    
    # Build command
    build_cmd = [
        "docker", "build",
        "-f", str(dockerfile),
        "-t", tagged_image,
        ".",  # Build context is root
    ]
    
    # Run build
    log_info("Running docker build...")
    result = subprocess.run(build_cmd, capture_output=True, text=True)
    if result.returncode != 0:
        log_error("Docker build failed")
        if result.stdout:
            log_error(result.stdout)
        if result.stderr:
            log_error(result.stderr)
        sys.exit(1)
    
    # Push to registry (for Kind cluster access)
    if tagged_image.startswith("localhost:5000"):
        log_info(f"Pushing image to registry: {tagged_image}")
        push_result = subprocess.run(
            ["docker", "push", tagged_image],
            capture_output=True,
            text=True
        )
        if push_result.returncode != 0:
            log_error("Failed to push image to registry")
            if push_result.stderr:
                log_error(push_result.stderr)
            log_error("  The image may not be accessible to the Kind cluster")
    
    log_info(f"✅ Successfully built and pushed: {tagged_image}")
    
    # CRITICAL: Output the image reference to stdout for Tilt's custom_build
    # Tilt expects this output to know what image was built
    print(tagged_image, file=sys.stdout)


if __name__ == "__main__":
    main()

