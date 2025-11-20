#!/usr/bin/env python3
"""
Set GitHub Container Registry package visibility to public.

This script waits for a package to be available by checking the GitHub API,
then sets its visibility to public using the GitHub API.
"""

import argparse
import os
import sys
import time
import subprocess
import json
from typing import Optional


def run_command(cmd: list, check: bool = True, env: Optional[dict] = None) -> subprocess.CompletedProcess:
    """Run a command and return the result."""
    result = subprocess.run(cmd, capture_output=True, text=True, check=check, env=env)
    return result


def check_package_available(org: str, package_name: str, gh_token: str, max_attempts: int = 30, delay: int = 2) -> bool:
    """
    Check if package is available by querying GitHub API.
    Returns True if package exists and is accessible, False otherwise.
    """
    print(f"🔍 Checking if package is available: {package_name}")
    
    api_endpoint = f"/orgs/{org}/packages/container/{package_name}"
    env = os.environ.copy()
    env["GH_TOKEN"] = gh_token
    
    for attempt in range(1, max_attempts + 1):
        try:
            result = run_command(
                ["gh", "api", api_endpoint],
                check=False,
                env=env
            )
            
            if result.returncode == 0:
                package_info = json.loads(result.stdout)
                package_id = package_info.get("id")
                if package_id:
                    print(f"   ✅ Package found (attempt {attempt}/{max_attempts})")
                    print(f"   Package ID: {package_id}")
                    return True
        except (json.JSONDecodeError, KeyError):
            pass
        
        if attempt < max_attempts:
            print(f"   ⏳ Waiting for package... (attempt {attempt}/{max_attempts})")
            time.sleep(delay)
    
    print(f"   ⚠️  Package not found after {max_attempts} attempts")
    return False


def set_package_visibility(org: str, package_name: str, gh_token: str) -> bool:
    """
    Set package visibility to public using GitHub API.
    Returns True if successful, False otherwise.
    """
    api_endpoint = f"/orgs/{org}/packages/container/{package_name}"
    
    print(f"📦 Setting package visibility to public: {package_name}")
    print(f"   Organization: {org}")
    print(f"   API Endpoint: {api_endpoint}")
    
    # Set GH_TOKEN environment variable for gh CLI
    env = os.environ.copy()
    env["GH_TOKEN"] = gh_token
    
    try:
        result = run_command(
            [
                "gh", "api",
                "--method", "PATCH",
                "-H", "Accept: application/vnd.github+json",
                api_endpoint,
                "-f", "visibility=public"
            ],
            check=False,
            env=env
        )
        
        if result.returncode == 0:
            print("   ✅ Package visibility set to public")
            return True
        else:
            error_msg = result.stderr.strip() if result.stderr else result.stdout.strip()
            print(f"   ❌ Failed to set visibility: {error_msg}")
            
            # Check if it's a 404 (package not found)
            if "404" in error_msg or "Not Found" in error_msg:
                print("   ℹ️  Package may not be accessible yet or requires admin permissions")
                print(f"   Manual setup: https://github.com/orgs/{org}/packages/container/{package_name}/settings")
            
            return False
    except Exception as e:
        print(f"   ❌ Error setting visibility: {e}")
        return False


def main():
    """Main function."""
    parser = argparse.ArgumentParser(
        description="Set GitHub Container Registry package visibility to public"
    )
    parser.add_argument(
        "image_ref",
        help="Full image reference (e.g., ghcr.io/org/package-name:tag)"
    )
    parser.add_argument(
        "--org",
        required=True,
        help="GitHub organization name"
    )
    parser.add_argument(
        "--gh-token",
        default=os.getenv("GH_TOKEN") or os.getenv("GITHUB_TOKEN"),
        help="GitHub token (default: from GH_TOKEN or GITHUB_TOKEN env var)"
    )
    parser.add_argument(
        "--skip-digest-check",
        action="store_true",
        help="Skip waiting for package availability check (set visibility immediately)"
    )
    parser.add_argument(
        "--max-attempts",
        type=int,
        default=30,
        help="Maximum attempts to check for image digest (default: 30)"
    )
    parser.add_argument(
        "--delay",
        type=int,
        default=2,
        help="Delay between attempts in seconds (default: 2)"
    )
    
    args = parser.parse_args()
    
    if not args.gh_token:
        print("❌ Error: GitHub token required (set GH_TOKEN or GITHUB_TOKEN env var)", file=sys.stderr)
        sys.exit(1)
    
    # Extract package name from image reference
    # Format: ghcr.io/org/package-name:tag -> package-name
    if not args.image_ref.startswith("ghcr.io/"):
        print(f"❌ Error: Invalid image reference format: {args.image_ref}", file=sys.stderr)
        print("   Expected format: ghcr.io/org/package-name:tag", file=sys.stderr)
        sys.exit(1)
    
    # Remove ghcr.io/ prefix and extract package name (before : or /)
    image_path = args.image_ref.replace("ghcr.io/", "")
    parts = image_path.split("/")
    
    if len(parts) < 2:
        print(f"❌ Error: Invalid image reference: {args.image_ref}", file=sys.stderr)
        sys.exit(1)
    
    # Package name is the part after org, before :tag
    package_with_tag = parts[1]
    package_name = package_with_tag.split(":")[0]
    
    # Check if package is available if not skipping
    if not args.skip_digest_check:
        package_available = check_package_available(
            args.org, package_name, args.gh_token, args.max_attempts, args.delay
        )
        if not package_available:
            print("⚠️  Warning: Package not found, but proceeding to set visibility anyway")
            print("   Package may still be creating, visibility will be set when package is ready")
    
    # Set package visibility
    success = set_package_visibility(args.org, package_name, args.gh_token)
    
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()

