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
    """Remove dangling images (unused intermediate layers)."""
    print("🖼️  Pruning dangling images...")
    result = run_command(["docker", "image", "prune", "-f"], check=False)
    if result.stdout:
        # Extract reclaimed space from output
        output_lines = result.stdout.strip().split('\n')
        for line in output_lines:
            if 'reclaimed' in line.lower() or 'total' in line.lower():
                print(f"  {line.strip()}")
    return result.returncode == 0


def cleanup_unused_images():
    """Remove unused images (not used by any container, older than 1 hour).
    
    Note: This is a general Docker prune. Tilt-specific images are handled separately
    by cleanup_old_tilt_images() which checks running containers.
    """
    print("🖼️  Pruning unused images (older than 1 hour)...")
    result = run_command(
        ["docker", "image", "prune", "-a", "-f", "--filter", "until=1h"],
        check=False
    )
    if result.stdout:
        # Extract reclaimed space from output
        output_lines = result.stdout.strip().split('\n')
        for line in output_lines:
            if 'reclaimed' in line.lower() or 'total' in line.lower():
                print(f"  {line.strip()}")
    return result.returncode == 0


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
    """Remove Tilt images that are not currently used by running containers.
    
    CRITICAL: Only removes images matching IMAGE_NAME (default: localhost:5000/secret-manager-controller).
    Never removes infrastructure images like kindest/node or registry:2.
    """
    print("🏷️  Removing unused Tilt images (not in use by running containers)...")
    image_name = os.getenv("IMAGE_NAME", "localhost:5000/secret-manager-controller")
    
    # CRITICAL: List of infrastructure images that must NEVER be removed
    # These are used by Kind clusters and local registries
    protected_images = {
        "kindest/node",
        "registry:",
        "registry/registry:",
    }
    
    # Get all images for this image name (only Tilt images)
    result = run_command(
        ["docker", "images", image_name, "--format", "{{.ID}}\t{{.Repository}}\t{{.Tag}}"],
        check=False
    )
    
    if result.returncode != 0 or not result.stdout:
        print("  ✅ No Tilt images found")
        return True
    
    # Get set of image references and IDs currently in use by running containers
    running_image_refs, running_image_ids = get_running_container_images()
    
    lines = [line.strip() for line in result.stdout.strip().split('\n') if line.strip()]
    if not lines:
        print("  ✅ No Tilt images found")
        return True
    
    removed_count = 0
    kept_count = 0
    
    for line in lines:
        parts = line.split('\t')
        if len(parts) < 3:
            continue
        
        image_id = parts[0]
        repository = parts[1]
        tag = parts[2]
        repo_tag = f"{repository}:{tag}"
        
        # CRITICAL: Never remove infrastructure images, even if they match the image name pattern
        is_protected = False
        for protected_pattern in protected_images:
            if protected_pattern in repo_tag:
                is_protected = True
                print(f"    🔒 Protected (infrastructure): {repo_tag}")
                break
        
        if is_protected:
            kept_count += 1
            continue
        
        # Check if this image reference is currently in use by any running container
        is_in_use = repo_tag in running_image_refs
        
        # Also check by image ID (in case tag changed but same image)
        if not is_in_use:
            # Check if image ID matches any running container image ID
            # Normalize image IDs (remove sha256: prefix if present for comparison)
            normalized_id = image_id.replace("sha256:", "")
            for running_id in running_image_ids:
                normalized_running_id = running_id.replace("sha256:", "")
                # Compare IDs - they might be full SHA256 or short IDs
                if normalized_id == normalized_running_id or normalized_id.startswith(normalized_running_id) or normalized_running_id.startswith(normalized_id):
                    is_in_use = True
                    break
        
        if is_in_use:
            kept_count += 1
            print(f"    Keeping (in use): {repo_tag}")
        else:
            # CRITICAL: Remove by repository:tag, NOT by image ID
            # Removing by ID can delete shared layers used by other images (like kindest/node)
            remove_result = run_command(["docker", "rmi", repo_tag], check=False)
            if remove_result.returncode == 0:
                removed_count += 1
                print(f"    Removed (unused): {repo_tag}")
            else:
                # If removal by tag fails, it might be because the tag is <none>
                # In that case, we can try by ID, but only if we're absolutely sure it's not shared
                if tag == "<none>":
                    # For untagged images, we can try by ID, but log a warning
                    print(f"    ⚠️  Tag is <none>, attempting removal by ID (may affect shared layers): {image_id[:12]}...")
                    remove_result = run_command(["docker", "rmi", image_id], check=False)
                    if remove_result.returncode == 0:
                        removed_count += 1
                        print(f"    Removed (untagged): {image_id[:12]}...")
                    else:
                        print(f"    ⚠️  Failed to remove untagged image: {image_id[:12]}...", file=sys.stderr)
                else:
                    print(f"    ⚠️  Failed to remove: {repo_tag}", file=sys.stderr)
    
    print(f"  ✅ Removed {removed_count} unused Tilt image(s), kept {kept_count} in-use image(s)")
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

