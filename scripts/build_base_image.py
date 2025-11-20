#!/usr/bin/env python3
"""
Build and publish base images for Secret Manager Controller.

This script builds base images with all runtime dependencies and publishes them to ghcr.io.
It checks for git changes to avoid unnecessary rebuilds.

Usage:
    python3 scripts/build_base_image.py <image-name> [options]
    
Examples:
    python3 scripts/build_base_image.py controller-base-image
    python3 scripts/build_base_image.py controller-base-image --version v1.0.0
    python3 scripts/build_base_image.py controller-base-image --flatten
    python3 scripts/build_base_image.py controller-base-image --check-changes
"""

import os
import sys
import subprocess
import argparse
from pathlib import Path
from typing import Optional


def run_command(cmd: list[str], check: bool = True, capture_output: bool = True) -> subprocess.CompletedProcess:
    """Run a command and return the result."""
    result = subprocess.run(
        cmd,
        capture_output=capture_output,
        text=True,
        check=check
    )
    if capture_output and result.stdout:
        print(result.stdout, end="")
    if capture_output and result.stderr and result.returncode != 0:
        print(result.stderr, end="", file=sys.stderr)
    return result


def check_git_changes(dockerfile_path: str, base_ref: str = "HEAD~1") -> bool:
    """Check if Dockerfile has changed since the last commit.
    
    Returns True if changes detected, False otherwise.
    """
    dockerfile = Path(dockerfile_path)
    if not dockerfile.exists():
        print(f"⚠️  Warning: Dockerfile not found: {dockerfile_path}")
        return True  # Build if file doesn't exist (first time)
    
    # Check if file has changed compared to base ref
    try:
        result = run_command(
            ["git", "diff", "--quiet", base_ref, "HEAD", "--", str(dockerfile)],
            check=False
        )
        # git diff --quiet returns 0 if no changes, 1 if changes
        has_changes = result.returncode != 0
        
        if has_changes:
            print(f"📝 Changes detected in {dockerfile_path} since {base_ref}")
        else:
            print(f"✅ No changes in {dockerfile_path} since {base_ref}")
        
        return has_changes
    except subprocess.CalledProcessError as e:
        print(f"⚠️  Warning: Could not check git changes: {e}")
        return True  # Build if we can't check (safer)


def get_image_config(image_name: str) -> dict:
    """Get configuration for a base image."""
    configs = {
        "controller-base-image": {
            "dockerfile": "dockerfiles/Dockerfile.base.controller",
            "registry": "ghcr.io",
            "org": "microscaler",
            "full_name": "ghcr.io/microscaler/secret-manager-controller-base-image",
        },
        "rust-builder-base-image": {
            "dockerfile": "dockerfiles/Dockerfile.base.rust-builder",
            "registry": "ghcr.io",
            "org": "microscaler",
            "full_name": "ghcr.io/microscaler/rust-builder-base-image",
        },
        "pact-mock-server-base-image": {
            "dockerfile": "dockerfiles/Dockerfile.base.pact-mock-server",
            "registry": "ghcr.io",
            "org": "microscaler",
            "full_name": "ghcr.io/microscaler/pact-mock-server-base-image",
        },
    }
    
    if image_name not in configs:
        raise ValueError(
            f"Unknown image name: {image_name}. "
            f"Available: {', '.join(configs.keys())}"
        )
    
    return configs[image_name]


def build_image(
    image_name: str,
    version: str,
    dockerfile: str,
    flatten: bool = False,
    dry_run: bool = False
) -> None:
    """Build and optionally publish a base image."""
    config = get_image_config(image_name)
    full_image_name = config["full_name"]
    tagged_image = f"{full_image_name}:{version}"
    
    print(f"🐳 Building base image: {tagged_image}")
    print(f"   Using Dockerfile: {dockerfile}")
    
    # Verify Dockerfile exists
    if not Path(dockerfile).exists():
        print(f"❌ Error: Dockerfile not found: {dockerfile}", file=sys.stderr)
        sys.exit(1)
    
    # Build the image
    build_cmd = [
        "docker", "build",
        "-f", dockerfile,
        "-t", tagged_image,
        "-t", f"{full_image_name}:latest",
        ".",
    ]
    
    result = run_command(build_cmd, check=False, capture_output=False)
    if result.returncode != 0:
        print("❌ Error: Docker build failed", file=sys.stderr)
        sys.exit(1)
    
    print("✅ Base image built successfully")
    
    # Optionally flatten the image to a single layer
    if flatten:
        print("📦 Flattening image to single layer...")
        
        # Create a temporary container from the image
        create_result = run_command(
            ["docker", "create", tagged_image],
            check=False
        )
        if create_result.returncode != 0:
            print("❌ Error: Failed to create container for flattening", file=sys.stderr)
            sys.exit(1)
        
        container_id = create_result.stdout.strip()
        
        try:
            # Export the container filesystem and import as a new single-layer image
            export_process = subprocess.Popen(
                ["docker", "export", container_id],
                stdout=subprocess.PIPE
            )
            import_result = run_command(
                ["docker", "import", "-", f"{tagged_image}-flat"],
                check=False
            )
            export_process.wait()
            
            if import_result.returncode != 0:
                print("❌ Error: Failed to flatten image", file=sys.stderr)
                sys.exit(1)
            
            # Tag the flattened image
            run_command(["docker", "tag", f"{tagged_image}-flat", tagged_image])
            run_command(["docker", "tag", f"{tagged_image}-flat", f"{full_image_name}:latest"])
            
            print("✅ Image flattened successfully")
        finally:
            # Clean up
            run_command(["docker", "rm", container_id], check=False)
    
    # Push to registry (if not in dry-run mode)
    if not dry_run:
        print(f"📤 Pushing image to registry...")
        push_result = run_command(
            ["docker", "push", tagged_image],
            check=False
        )
        if push_result.returncode != 0:
            print("❌ Error: Failed to push image", file=sys.stderr)
            sys.exit(1)
        
        latest_result = run_command(
            ["docker", "push", f"{full_image_name}:latest"],
            check=False
        )
        if latest_result.returncode != 0:
            print("⚠️  Warning: Failed to push 'latest' tag", file=sys.stderr)
        
        print("✅ Image pushed successfully")
    else:
        print("🔍 Dry-run mode: Skipping push")
    
    print(f"✅ Base image build complete: {tagged_image}")


def main():
    """Main function."""
    parser = argparse.ArgumentParser(
        description="Build and publish base images for Secret Manager Controller"
    )
    parser.add_argument(
        "image_name",
        help="Name of the base image (e.g., controller-base-image)"
    )
    parser.add_argument(
        "--version",
        default="latest",
        help="Version tag for the image (default: latest)"
    )
    parser.add_argument(
        "--dockerfile",
        help="Path to Dockerfile (default: from image config)"
    )
    parser.add_argument(
        "--flatten",
        action="store_true",
        help="Flatten the image to a single layer"
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Build but don't push to registry"
    )
    parser.add_argument(
        "--check-changes",
        action="store_true",
        help="Only build if Dockerfile has changed (checks git diff)"
    )
    parser.add_argument(
        "--base-ref",
        default="HEAD~1",
        help="Git reference to compare against for change detection (default: HEAD~1)"
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Force build even if no changes detected"
    )
    
    args = parser.parse_args()
    
    # Get image configuration
    try:
        config = get_image_config(args.image_name)
    except ValueError as e:
        print(f"❌ Error: {e}", file=sys.stderr)
        sys.exit(1)
    
    dockerfile = args.dockerfile or config["dockerfile"]
    
    # Check for changes if requested
    if args.check_changes and not args.force:
        has_changes = check_git_changes(dockerfile, args.base_ref)
        if not has_changes:
            print(f"⏭️  Skipping build: No changes detected in {dockerfile}")
            print("   Use --force to build anyway")
            sys.exit(0)
    
    # Build the image
    build_image(
        args.image_name,
        args.version,
        dockerfile,
        flatten=args.flatten,
        dry_run=args.dry_run
    )


if __name__ == "__main__":
    main()

