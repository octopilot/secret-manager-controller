#!/usr/bin/env python3
"""
Semver version bump script for base images.

Usage:
    python3 scripts/semver_bump.py <image-name> <major|minor|patch>
    
Example:
    python3 scripts/semver_bump.py controller-base-image patch
"""

import sys
import subprocess
import re
from typing import Tuple, Optional


def run_command(cmd: list[str], check: bool = True) -> subprocess.CompletedProcess:
    """Run a command and return the result."""
    result = subprocess.run(cmd, capture_output=True, text=True, check=check)
    return result


def get_current_version(image_name: str) -> Optional[Tuple[int, int, int]]:
    """Get the current version from git tags.
    
    Returns (major, minor, patch) or None if no tags exist.
    """
    # Get all tags matching the image name pattern
    result = run_command(
        ["git", "tag", "-l", f"{image_name}-v*"],
        check=False
    )
    
    if not result.stdout.strip():
        return None
    
    # Extract versions from tags and sort
    versions = []
    for tag in result.stdout.strip().split('\n'):
        # Extract version from tag (e.g., controller-base-image-v1.2.3 -> 1.2.3)
        match = re.search(rf"{re.escape(image_name)}-v(\d+)\.(\d+)\.(\d+)", tag)
        if match:
            major, minor, patch = map(int, match.groups())
            versions.append((major, minor, patch, tag))
    
    if not versions:
        return None
    
    # Sort by version (major, minor, patch)
    versions.sort(key=lambda x: (x[0], x[1], x[2]))
    major, minor, patch, _ = versions[-1]
    return (major, minor, patch)


def bump_version(major: int, minor: int, patch: int, bump_type: str) -> Tuple[int, int, int]:
    """Bump version based on type."""
    if bump_type == "major":
        return (major + 1, 0, 0)
    elif bump_type == "minor":
        return (minor + 1, 0)
    elif bump_type == "patch":
        return (major, minor, patch + 1)
    else:
        raise ValueError(f"Invalid bump type: {bump_type}. Must be 'major', 'minor', or 'patch'")


def main():
    """Main function."""
    if len(sys.argv) < 3:
        print("Usage: python3 scripts/semver_bump.py <image-name> <major|minor|patch>")
        print("Example: python3 scripts/semver_bump.py controller-base-image patch")
        sys.exit(1)
    
    image_name = sys.argv[1]
    bump_type = sys.argv[2].lower()
    
    if bump_type not in ("major", "minor", "patch"):
        print(f"Error: Bump type must be 'major', 'minor', or 'patch', got '{bump_type}'")
        sys.exit(1)
    
    # Get current version
    current_version = get_current_version(image_name)
    
    if current_version is None:
        # No existing tag, start at v0.1.0
        major, minor, patch = 0, 1, 0
        current_tag = None
    else:
        major, minor, patch = current_version
        current_tag = f"{image_name}-v{major}.{minor}.{patch}"
    
    # Bump version
    new_major, new_minor, new_patch = bump_version(major, minor, patch, bump_type)
    new_version = f"{new_major}.{new_minor}.{new_patch}"
    new_tag = f"{image_name}-v{new_version}"
    
    print(f"Current version: {current_tag or 'none'}")
    print(f"New version: {new_tag}")
    print(f"Bump type: {bump_type}")
    
    # Create and push the tag
    response = input(f"Create and push tag {new_tag}? (y/N): ").strip().lower()
    if response != 'y':
        print("Cancelled")
        sys.exit(1)
    
    # Create tag
    run_command(["git", "tag", new_tag])
    
    # Push tag
    run_command(["git", "push", "origin", new_tag])
    
    print(f"✅ Tag {new_tag} created and pushed")
    print(f"Version: {new_version}")


if __name__ == "__main__":
    main()

