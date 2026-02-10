#!/usr/bin/env python3
"""
Comprehensive Docker cleanup to prevent overwhelming Docker.

This script performs a full Docker purge routine:
1. Removes stopped containers (particularly Tilt build containers)
2. Prunes dangling images
3. Prunes unused images (older than 1 hour)
4. Prunes build cache
5. Prunes unused volumes
6. Prunes unused networks

It's safe to run repeatedly as it only removes unused resources.

Runs as a one-shot cleanup after controller builds complete.
"""

import subprocess
import sys
import os


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


def cleanup_stopped_containers():
    """Remove stopped containers."""
    print("📦 Removing stopped containers...")
    
    stopped_containers = get_stopped_containers()
    
    if not stopped_containers:
        print("  ✅ No stopped containers found")
        return 0, 0
    
    print(f"  📋 Found {len(stopped_containers)} stopped container(s)")
    
    removed_count = 0
    failed_count = 0
    
    for container_id in stopped_containers:
        container_info = get_container_info(container_id)
        if container_info:
            container_name, image = container_info.split(" ", 1)
            # Log controller-related containers
            if "secret-manager-controller" in container_name or "secret-manager-controller" in image:
                print(f"    Removing: {container_name} ({image[:50]}...)")
        
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
                print(f"    ⚠️  Failed to remove: {container_info}", file=sys.stderr)
    
    print(f"  ✅ Removed {removed_count} container(s)")
    if failed_count > 0:
        print(f"  ⚠️  Failed to remove {failed_count} container(s)", file=sys.stderr)
    
    return removed_count, failed_count


def cleanup_dangling_images():
    """Remove dangling images (unused intermediate layers).
    
    CRITICAL: Only removes truly dangling images (untagged intermediate layers).
    Does NOT remove images that are referenced by other images (like kindest/node).
    """
    print("🖼️  Pruning dangling images (unused intermediate layers only)...")
    # Use --filter to exclude images that might be referenced by infrastructure
    # This is safer than plain prune, but we still need to be careful
    result = run_command(["docker", "image", "prune", "-f"], check=False)
    if result.stdout:
        # Extract reclaimed space from output
        output_lines = result.stdout.strip().split('\n')
        for line in output_lines:
            if 'reclaimed' in line.lower() or 'total' in line.lower():
                print(f"  {line.strip()}")
    return result.returncode == 0


def cleanup_unused_images():
    """Remove unused images we build ourselves (localhost:5000/* with tilt-* tags).
    
    NOTE: We do NOT use 'docker image prune -a' as it would remove:
    - Base images (rust:alpine, debian, etc.)
    - Pact broker images
    - Other dependencies we download
    - Our published base images (ghcr.io/<org>/*)
    This causes re-downloads and hits Docker rate limits.
    
    Tilt-specific images are handled separately by cleanup_old_tilt_images().
    """
    print("🖼️  Pruning unused images we build (localhost:5000/* with tilt-* tags)...")
    # Only clean up images we build ourselves, not base images or dependencies
    # This prevents re-downloading images and hitting Docker rate limits
    result = run_command(
        ["docker", "images", "localhost:5000/*", "--format", "{{.Repository}}\t{{.Tag}}\t{{.ID}}"],
        check=False
    )
    if result.returncode == 0 and result.stdout:
        # Count images we build (tilt-* tags) that are not in use
        # Note: We don't actually remove them here as cleanup_old_tilt_images() handles that
        # This function is kept for compatibility but doesn't do aggressive cleanup
        tilt_images = [line for line in result.stdout.strip().split('\n') 
                      if line.strip() and 'tilt-' in line]
        if tilt_images:
            print(f"  Found {len(tilt_images)} Tilt build image(s) (handled by cleanup_old_tilt_images)")
    return True  # Always succeed - actual cleanup is done by cleanup_old_tilt_images()


def cleanup_build_cache():
    """Prune build cache (keeps only last 1 hour for faster builds)."""
    print("🔨 Pruning build cache (keeping last 1 hour)...")
    result = run_command(
        ["docker", "builder", "prune", "-a", "-f", "--filter", "until=1h"],
        check=False
    )
    if result.stdout:
        # Extract reclaimed space from output
        output_lines = result.stdout.strip().split('\n')
        for line in output_lines:
            if 'reclaimed' in line.lower() or 'total' in line.lower():
                print(f"  {line.strip()}")
    return result.returncode == 0


def cleanup_unused_volumes():
    """Remove unused volumes."""
    print("💾 Pruning unused volumes...")
    result = run_command(["docker", "volume", "prune", "-f"], check=False)
    if result.stdout:
        # Extract reclaimed space from output
        output_lines = result.stdout.strip().split('\n')
        for line in output_lines:
            if 'reclaimed' in line.lower() or 'total' in line.lower():
                print(f"  {line.strip()}")
    return result.returncode == 0


def cleanup_unused_networks():
    """Remove unused networks."""
    print("🌐 Pruning unused networks...")
    result = run_command(["docker", "network", "prune", "-f"], check=False)
    if result.stdout:
        # Extract reclaimed space from output
        output_lines = result.stdout.strip().split('\n')
        for line in output_lines:
            if 'reclaimed' in line.lower() or 'total' in line.lower():
                print(f"  {line.strip()}")
    return result.returncode == 0


def get_running_container_images():
    """Get set of image references (repo:tag) and image IDs currently used by running containers."""
    result = run_command(
        ["docker", "ps", "--format", "{{.Image}}"],
        check=False
    )
    if result.returncode != 0:
        return set(), set()
    
    image_refs = [line.strip() for line in result.stdout.strip().split('\n') if line.strip()]
    image_ids = set()
    
    # Get image IDs for all running container images
    for image_ref in image_refs:
        inspect_result = run_command(
            ["docker", "inspect", "--format", "{{.Id}}", image_ref],
            check=False
        )
        if inspect_result.returncode == 0 and inspect_result.stdout:
            image_id = inspect_result.stdout.strip()
            image_ids.add(image_id)
    
    return set(image_refs), image_ids


def cleanup_registry_images():
    """Clean up old images from the local Docker registry.
    
    Uses the registry's garbage collection API to remove unused manifests and blobs.
    This is safer than manually deleting manifests as it properly handles layer references.
    """
    print("🗑️  Cleaning up old images from local registry...")
    
    # Find the registry container
    registry_name = os.getenv("REGISTRY_NAME", "secret-manager-controller-registry")
    
    # Check if registry container exists and is running
    result = run_command(
        ["docker", "ps", "--filter", f"name={registry_name}", "--format", "{{.Names}}"],
        check=False
    )
    
    if result.returncode != 0 or registry_name not in result.stdout:
        # Try to find any registry container
        result = run_command(
            ["docker", "ps", "--filter", "ancestor=registry:2", "--format", "{{.Names}}"],
            check=False
        )
        if result.returncode == 0 and result.stdout.strip():
            registry_name = result.stdout.strip().split('\n')[0]
        else:
            print("  ⚠️  No registry container found, skipping registry cleanup")
            return True
    
    print(f"  📦 Using registry container: {registry_name}")
    
    # Method 1: Use registry garbage collection API (if available)
    # This requires the registry to have delete enabled
    registry_url = os.getenv("REGISTRY_URL", "http://localhost:5000")
    
    # Try to get catalog first to see if API is accessible
    catalog_result = run_command(
        ["curl", "-s", f"{registry_url}/v2/_catalog"],
        check=False
    )
    
    if catalog_result.returncode == 0 and catalog_result.stdout:
        try:
            import json
            catalog = json.loads(catalog_result.stdout)
            repositories = catalog.get("repositories", [])
            
            if not repositories:
                print("  ✅ No repositories found in registry")
                return True
            
            print(f"  📋 Found {len(repositories)} repository/repositories in registry")
            
            # For each repository, list tags and identify old ones
            image_name = os.getenv("IMAGE_NAME", "localhost:5000/secret-manager-controller")
            repo_name = image_name.split("/")[-1] if "/" in image_name else image_name.split(":")[0]
            
            if repo_name not in repositories:
                print(f"  ✅ Repository '{repo_name}' not found in registry")
                return True
            
            # Get tags for this repository
            tags_result = run_command(
                ["curl", "-s", f"{registry_url}/v2/{repo_name}/tags/list"],
                check=False
            )
            
            if tags_result.returncode == 0 and tags_result.stdout:
                tags_data = json.loads(tags_result.stdout)
                tags = tags_data.get("tags", [])
                
                if not tags:
                    print(f"  ✅ No tags found for repository '{repo_name}'")
                    return True
                
                print(f"  📋 Found {len(tags)} tag(s) for repository '{repo_name}'")
                
                # Filter out special tags (like 'tilt' which is the current tag)
                # Keep the 'tilt' tag and the most recent content-hash tags
                special_tags = {"tilt", "latest"}
                content_hash_tags = [t for t in tags if t.startswith("tilt-") and len(t) > 10]
                other_tags = [t for t in tags if t not in special_tags and not t.startswith("tilt-")]
                
                # Keep the 3 most recent content-hash tags (by sorting and taking last 3)
                # Content hash tags are typically sorted chronologically
                content_hash_tags_sorted = sorted(content_hash_tags)
                tags_to_keep = set(special_tags)
                if len(content_hash_tags_sorted) > 3:
                    # Keep the last 3 content-hash tags
                    tags_to_keep.update(content_hash_tags_sorted[-3:])
                else:
                    tags_to_keep.update(content_hash_tags_sorted)
                
                tags_to_remove = [t for t in tags if t not in tags_to_keep]
                
                if not tags_to_remove:
                    print(f"  ✅ No old tags to remove (keeping {len(tags_to_keep)} tag(s))")
                    return True
                
                print(f"  🗑️  Removing {len(tags_to_remove)} old tag(s), keeping {len(tags_to_keep)} tag(s)")
                
                # Check if delete is enabled by trying to get a manifest
                # If we can get manifests, we can potentially delete them
                deleted_count = 0
                failed_count = 0
                
                for tag in tags_to_remove:
                    # Get manifest digest for this tag
                    manifest_result = run_command(
                        ["curl", "-s", "-I", f"{registry_url}/v2/{repo_name}/manifests/{tag}"],
                        check=False
                    )
                    
                    if manifest_result.returncode == 0:
                        # Extract digest from Docker-Content-Digest header
                        digest = None
                        for line in manifest_result.stdout.split('\n'):
                            if line.startswith("Docker-Content-Digest:"):
                                digest = line.split(":", 1)[1].strip()
                                break
                        
                        if digest:
                            # Delete manifest by digest
                            delete_result = run_command(
                                ["curl", "-s", "-X", "DELETE", f"{registry_url}/v2/{repo_name}/manifests/{digest}"],
                                check=False
                            )
                            
                            if delete_result.returncode == 0:
                                deleted_count += 1
                                print(f"    ✅ Deleted tag: {tag}")
                            else:
                                failed_count += 1
                                print(f"    ⚠️  Failed to delete tag: {tag} (delete may not be enabled)")
                        else:
                            failed_count += 1
                            print(f"    ⚠️  Could not get digest for tag: {tag}")
                    else:
                        failed_count += 1
                        print(f"    ⚠️  Could not get manifest for tag: {tag}")
                
                if deleted_count > 0:
                    print(f"  ✅ Deleted {deleted_count} tag(s) from registry")
                    print(f"  💡 Note: Run registry garbage collection to free disk space")
                    print(f"  💡 To enable delete, set REGISTRY_STORAGE_DELETE_ENABLED=true in registry container")
                
                if failed_count > 0:
                    print(f"  ⚠️  Failed to delete {failed_count} tag(s) (delete may not be enabled)")
                    print(f"  💡 To enable delete, restart registry with: -e REGISTRY_STORAGE_DELETE_ENABLED=true")
                
                return True
        except (json.JSONDecodeError, KeyError) as e:
            print(f"  ⚠️  Failed to parse registry API response: {e}")
            print(f"  💡 Registry may not support API or may require authentication")
    
    # Method 2: Use registry garbage collection command (requires registry:2.5+)
    # This removes unused blobs but doesn't delete manifests unless delete is enabled
    print(f"  🔄 Running registry garbage collection...")
    gc_result = run_command(
        ["docker", "exec", registry_name, "registry", "garbage-collect", "/etc/docker/registry/config.yml", "--delete-untagged"],
        check=False
    )
    
    if gc_result.returncode == 0:
        # Parse output to see if anything was deleted
        output = gc_result.stdout if gc_result.stdout else ""
        deleted_blobs = [line for line in output.split('\n') if 'deleting blob' in line.lower() or 'deleted' in line.lower()]
        
        if deleted_blobs:
            print(f"  ✅ Registry garbage collection completed - deleted unused blobs")
            # Show summary (don't print all deleted blobs, just count)
            print(f"  📊 Freed space by removing unused blob layers")
        else:
            print(f"  ✅ Registry garbage collection completed (no unused blobs to remove)")
            print(f"  💡 To delete old tags, enable delete: docker stop {registry_name} && docker rm {registry_name} && docker run -d --restart=always -p 127.0.0.1:5000:5000 -e REGISTRY_STORAGE_DELETE_ENABLED=true --name {registry_name} registry:2")
    else:
        # Try alternative garbage collection command (older registry versions)
        gc_result = run_command(
            ["docker", "exec", registry_name, "/bin/registry", "garbage-collect", "/etc/docker/registry/config.yml"],
            check=False
        )
        if gc_result.returncode == 0:
            print(f"  ✅ Registry garbage collection completed (legacy mode)")
        else:
            print(f"  ⚠️  Registry garbage collection not available (registry may be too old or command not found)")
            print(f"  💡 Manual cleanup: docker exec {registry_name} registry garbage-collect /etc/docker/registry/config.yml")
    
    return True


def cleanup_old_tilt_images():
    """Remove old Tilt images, keeping only the current running image per service.
    
    For Tilt deployments in dev environment, we only need the current running image.
    No rollback capability needed in dev, so we keep only 1 image per service.
    
    CRITICAL: Never removes infrastructure images like kindest/node or registry:2.
    """
    print("🏷️  Removing old Tilt images (keeping current running image per service)...")
    
    # CRITICAL: List of infrastructure images that must NEVER be removed
    # These are used by Kind clusters, local registries, and base images
    protected_images = {
        "kindest/node",
        "registry:",
        "registry/registry:",
        "ghcr.io/octopilot/rust-builder-base-image",
        "ghcr.io/octopilot/secret-manager-controller-base-image",
        "ghcr.io/octopilot/pact-mock-server-base-image",
        "octopilot/rust-builder-base-image",
        "octopilot/secret-manager-controller-base-image",
        "octopilot/pact-mock-server-base-image",
    }
    
    # Get all images with tilt-* tags (all Tilt services)
    # Group by repository and keep only the most recent (current running) per repository
    result = run_command(
        ["docker", "images", "--format", "{{.Repository}}\t{{.Tag}}\t{{.ID}}\t{{.CreatedAt}}"],
        check=False
    )
    
    if result.returncode != 0 or not result.stdout:
        print("  ✅ No images found")
        return True
    
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
            
            # Only process tilt-* tags (Tilt builds)
            if tag.startswith("tilt-") or tag == "tilt":
                if repo not in repos:
                    repos[repo] = []
                repos[repo].append((created, img_id, tag))
    
    if not repos:
        print("  ✅ No Tilt images found")
        return True
    
    removed_count = 0
    kept_count = 0
    
    # For each repository, keep only the most recent (current running) image
    for repo, images in repos.items():
        repo_tag_prefix = f"{repo}:"
        
        # CRITICAL: Never remove infrastructure images or base images
        is_protected = False
        for protected_pattern in protected_images:
            if protected_pattern in repo_tag_prefix:
                is_protected = True
                print(f"    🔒 Protected (infrastructure/base image): {repo}")
                break
        
        # Also protect base images by repository name (regardless of tag)
        base_image_repos = [
            "ghcr.io/octopilot/rust-builder-base-image",
            "ghcr.io/octopilot/secret-manager-controller-base-image",
            "ghcr.io/octopilot/pact-mock-server-base-image",
            "octopilot/rust-builder-base-image",
            "octopilot/secret-manager-controller-base-image",
            "octopilot/pact-mock-server-base-image",
        ]
        if repo in base_image_repos:
            is_protected = True
            print(f"    🔒 Protected (base image): {repo}")
        
        if is_protected:
            kept_count += len(images)
            continue
        
        # Sort by creation date (newest first)
        images.sort(key=lambda x: x[0], reverse=True)
        
        # Keep only the most recent (current running), remove the rest
        if len(images) > 1:
            for created, img_id, tag in images[1:]:  # Skip first 1 (most recent/current)
                repo_tag = f"{repo}:{tag}"
                # CRITICAL: Remove by repository:tag, NOT by image ID
                # Removing by ID can delete shared layers used by other images (like kindest/node)
                remove_result = run_command(["docker", "rmi", repo_tag], check=False)
                if remove_result.returncode == 0:
                    removed_count += 1
                    print(f"    Removed (old): {repo_tag}")
                else:
                    print(f"    ⚠️  Failed to remove: {repo_tag}", file=sys.stderr)
            kept_count += 1
        else:
            kept_count += len(images)
    
    print(f"  ✅ Removed {removed_count} old Tilt image(s), kept {kept_count} image(s) (current running per service)")
    return True


def main():
    """Main cleanup function - full purge routine."""
    print("🧹 Starting comprehensive Docker cleanup...")
    print("")
    
    total_errors = 0
    
    # 1. Remove stopped containers
    removed, failed = cleanup_stopped_containers()
    if failed > 0:
        total_errors += failed
    print("")
    
    # 2. Prune dangling images
    if not cleanup_dangling_images():
        total_errors += 1
    print("")
    
    # 3. Prune unused images
    if not cleanup_unused_images():
        total_errors += 1
    print("")
    
    # 4. Prune build cache
    if not cleanup_build_cache():
        total_errors += 1
    print("")
    
    # 5. Clean up registry images
    if not cleanup_registry_images():
        total_errors += 1
    print("")
    
    # 6. Remove old Tilt images
    if not cleanup_old_tilt_images():
        total_errors += 1
    print("")
    
    # 7. Prune unused volumes
    if not cleanup_unused_volumes():
        total_errors += 1
    print("")
    
    # 8. Prune unused networks
    if not cleanup_unused_networks():
        total_errors += 1
    print("")
    
    print("✅ Comprehensive cleanup complete!")
    if total_errors > 0:
        print(f"⚠️  Encountered {total_errors} error(s) during cleanup", file=sys.stderr)
        return 1
    
    return 0


if __name__ == "__main__":
    sys.exit(main())

