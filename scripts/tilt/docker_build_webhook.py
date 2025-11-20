#!/usr/bin/env python3
"""
Build Docker image for webhook server.

Builds a Docker image containing the webhook binary.
"""

import os
import subprocess
import sys
from pathlib import Path


def run_command(cmd, check=True, shell=False):
    """Run a command and return the result."""
    if isinstance(cmd, str) and not shell:
        cmd = cmd.split()
    
    result = subprocess.run(cmd, shell=shell, capture_output=True, text=True)
    
    if check and result.returncode != 0:
        print(f"❌ Error: Command failed: {' '.join(cmd) if isinstance(cmd, list) else cmd}", file=sys.stderr)
        if result.stderr:
            print(result.stderr, file=sys.stderr)
        sys.exit(result.returncode)
    
    return result


def main():
    """Main build function."""
    image_name = os.getenv("IMAGE_NAME", "localhost:5000/mock-webhook")
    tag = os.getenv("TAG", "tilt")
    
    # Full image name with tag
    tagged_image = f"{image_name}:{tag}"
    
    # Verify binary exists
    binary_path = Path("build_artifacts/mock-server/webhook")
    if not binary_path.exists():
        print(f"❌ Error: Binary not found: {binary_path}", file=sys.stderr)
        print("   Please run the webhook-build resource first", file=sys.stderr)
        sys.exit(1)
    
    dockerfile = Path("dockerfiles/Dockerfile.pact-webhook")
    if not dockerfile.exists():
        print(f"❌ Error: Dockerfile not found: {dockerfile}", file=sys.stderr)
        sys.exit(1)
    
    # Build Docker image
    print(f"🐳 Building Docker image: {tagged_image}")
    
    # Use docker build with build context as root (to access build_artifacts)
    cmd = [
        "docker", "build",
        "-f", str(dockerfile),
        "-t", tagged_image,
        ".",  # Build context is root
    ]
    
    result = run_command(cmd, check=False)
    if result.returncode != 0:
        print("❌ Error: Docker build failed", file=sys.stderr)
        if result.stdout:
            print(result.stdout, file=sys.stderr)
        if result.stderr:
            print(result.stderr, file=sys.stderr)
        sys.exit(1)
    
    # Push to registry (for Kind cluster access)
    if image_name.startswith("localhost:5000"):
        print(f"📤 Pushing image to registry: {tagged_image}")
        push_result = run_command(["docker", "push", tagged_image], check=False)
        if push_result.returncode != 0:
            print("⚠️  Warning: Failed to push image to registry", file=sys.stderr)
            print("   The image may not be accessible to the Kind cluster", file=sys.stderr)
    
    print(f"✅ Docker image built and pushed successfully: {tagged_image}")
    
    # CRITICAL: Output the image reference to stdout for Tilt's custom_build
    print(tagged_image, file=sys.stdout)


if __name__ == "__main__":
    main()

