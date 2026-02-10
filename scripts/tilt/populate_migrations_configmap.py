#!/usr/bin/env python3
"""
Populate and apply the postgres-migrations ConfigMap from local migration SQL files.

This script reads SQL migration files from crates/pact-mock-server/migrations/
and creates/updates the postgres-migrations ConfigMap in the cluster.
"""

import subprocess
import sys
from pathlib import Path


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_warn(msg):
    """Print warning message."""
    print(f"[WARN] {msg}", file=sys.stderr)


def log_error(msg):
    """Print error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def run_command(cmd, check=True, capture_output=True):
    """Run a shell command and return the result."""
    result = subprocess.run(
        cmd,
        shell=isinstance(cmd, str),
        capture_output=capture_output,
        text=True
    )
    if check and result.returncode != 0:
        log_error(f"Command failed: {cmd}")
        if result.stderr:
            log_error(result.stderr)
        sys.exit(1)
    return result


def find_migration_files(migrations_dir: Path):
    """Find all SQL migration files in the directory structure."""
    if not migrations_dir.exists():
        return []
    
    migration_files = []
    # Look for SQL files in gcp/, aws/, azure/ subdirectories
    for schema_dir in ["gcp", "aws", "azure"]:
        schema_path = migrations_dir / schema_dir
        if schema_path.exists():
            for file_path in sorted(schema_path.glob("*.sql")):
                if file_path.is_file():
                    # ConfigMap keys cannot contain '/', so replace with '_'
                    # e.g., "gcp/001_create_schema.sql" -> "gcp_001_create_schema.sql"
                    key = f"{schema_dir}_{file_path.name}"
                    migration_files.append((key, file_path, schema_dir, file_path.name))
    
    return sorted(migration_files)


def create_configmap_from_files(namespace: str, configmap_name: str, migration_files: list):
    """Create or update ConfigMap from migration files."""
    if not migration_files:
        log_warn("No migration files found - ConfigMap will remain empty")
        return False
    
    log_info(f"Found {len(migration_files)} migration file(s) to add to ConfigMap")
    
    # Build kubectl create command with --from-file for each file
    # Format: --from-file=key=path/to/file
    # Note: Keys use '_' instead of '/' (e.g., "gcp_001_create_schema.sql")
    cmd = [
        "kubectl", "create", "configmap", configmap_name,
        "--namespace", namespace,
        "--dry-run=client",
        "-o", "yaml"
    ]
    
    for key, file_path, schema_dir, filename in migration_files:
        cmd.extend(["--from-file", f"{key}={file_path}"])
    
    log_info(f"Generating ConfigMap YAML for {len(migration_files)} migration file(s)...")
    result = run_command(cmd, check=False, capture_output=True)
    
    if result.returncode != 0:
        log_error(f"Failed to generate ConfigMap YAML: {result.stderr}")
        return False
    
    # Apply the generated YAML (handles both create and update)
    log_info("Applying ConfigMap...")
    apply_cmd = [
        "kubectl", "apply", "-f", "-"
    ]
    apply_result = subprocess.run(
        apply_cmd,
        input=result.stdout,
        text=True,
        capture_output=True
    )
    
    if apply_result.returncode != 0:
        log_error(f"Failed to apply ConfigMap: {apply_result.stderr}")
        return False
    
    log_info(f"✅ ConfigMap {namespace}/{configmap_name} created/updated successfully")
    log_info(f"   Added {len(migration_files)} migration file(s):")
    for key, file_path, schema_dir, filename in migration_files:
        log_info(f"   - {schema_dir}/{filename} (key: {key})")
    
    return True


def main():
    """Main function."""
    namespace = "secret-manager-controller-pact-broker"
    configmap_name = "postgres-migrations"
    migrations_dir = Path("crates/pact-mock-server/migrations")
    
    log_info("Populating postgres-migrations ConfigMap from local migration files...")
    log_info(f"Migrations directory: {migrations_dir.absolute()}")
    
    # Find migration files
    migration_files = find_migration_files(migrations_dir)
    
    if not migration_files:
        log_warn("No migration files found in crates/pact-mock-server/migrations/")
        log_warn("This is expected if migration files haven't been created yet")
        
        # Check if ConfigMap already exists
        check_cmd = [
            "kubectl", "get", "configmap", configmap_name,
            "--namespace", namespace,
            "--ignore-not-found=true"
        ]
        result = run_command(check_cmd, check=False, capture_output=True)
        
        if result.returncode == 0 and configmap_name in result.stdout:
            log_info("ConfigMap already exists (empty) - no update needed")
        else:
            # Create empty ConfigMap if it doesn't exist
            log_info("Creating empty ConfigMap (will be populated when migration files are available)...")
            create_cmd = [
                "kubectl", "create", "configmap", configmap_name,
                "--namespace", namespace,
                "--from-literal", "placeholder=empty"
            ]
            result = run_command(create_cmd, check=False, capture_output=True)
            if result.returncode == 0:
                # Remove the placeholder
                patch_cmd = [
                    "kubectl", "patch", "configmap", configmap_name,
                    "--namespace", namespace,
                    "--type", "json",
                    "-p", '[{"op": "remove", "path": "/data/placeholder"}]'
                ]
                run_command(patch_cmd, check=False)
                log_info("✅ Empty ConfigMap created")
            elif "already exists" in result.stderr.lower():
                log_info("ConfigMap already exists")
            else:
                log_warn(f"Could not create ConfigMap: {result.stderr}")
        
        return 0
    
    # Create/update ConfigMap with migration files
    if create_configmap_from_files(namespace, configmap_name, migration_files):
        log_info("✅ ConfigMap populated and applied successfully")
        return 0
    else:
        log_error("Failed to populate ConfigMap")
        return 1


if __name__ == "__main__":
    sys.exit(main())

