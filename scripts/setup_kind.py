#!/usr/bin/env python3
"""
Secret Manager Controller Kind Cluster Setup Script.

This script replaces setup-kind.sh and provides better error handling
and cross-platform support.

Creates a local Kind cluster with Docker registry for development.
"""

import os
import shutil
import subprocess
import sys
import time
from pathlib import Path


# Configuration
CLUSTER_NAME = "secret-manager-controller"
REGISTRY_NAME = "secret-manager-controller-registry"
REGISTRY_PORT = "5000"


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_warn(msg):
    """Print warning message."""
    print(f"[WARN] {msg}")


def log_error(msg):
    """Print error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def check_command(cmd):
    """Check if a command exists."""
    if not shutil.which(cmd):
        log_error(f"{cmd} is not installed. Please install it first.")
        sys.exit(1)


def run_command(cmd, check=True, capture_output=True, **kwargs):
    """Run a command and return the result."""
    result = subprocess.run(
        cmd,
        shell=isinstance(cmd, str),
        capture_output=capture_output,
        text=True,
        check=check,
        **kwargs
    )
    return result


def find_registry_on_port(port):
    """Find Docker registry container running on specified port."""
    # Check for containers with port mapping to the specified port
    result = run_command(
        f"docker ps --format '{{{{.Names}}}}\\t{{{{.Ports}}}}'",
        check=False,
        capture_output=True
    )
    
    if result.returncode != 0:
        return None
    
    for line in result.stdout.strip().split('\n'):
        if not line.strip():
            continue
        parts = line.split('\t')
        if len(parts) >= 2:
            name = parts[0]
            ports = parts[1]
            # Check if this port is mapped (format: "127.0.0.1:5000->5000/tcp" or "0.0.0.0:5000->5000/tcp")
            if f":{port}->" in ports or f"->{port}/" in ports:
                # Verify it's actually a registry by checking the image
                inspect_result = run_command(
                    f"docker inspect {name} --format='{{{{.Config.Image}}}}'",
                    check=False,
                    capture_output=True
                )
                if inspect_result.returncode == 0 and "registry" in inspect_result.stdout.lower():
                    return name
    return None


def setup_registry():
    """Setup local Docker registry.
    
    Checks if a registry is already running on port 5000 and uses it if found.
    Otherwise creates a new registry container.
    
    Returns the name of the registry container to use.
    """
    global REGISTRY_NAME
    
    # First check if our named registry exists
    result = run_command(f"docker ps -a --format '{{{{.Names}}}}'", check=False)
    registry_exists = REGISTRY_NAME in result.stdout
    
    if registry_exists:
        # Check if it's running
        running_result = run_command(f"docker ps --format '{{{{.Names}}}}'", check=False)
        if REGISTRY_NAME in running_result.stdout:
            log_info(f"Local registry '{REGISTRY_NAME}' already running")
            return REGISTRY_NAME
        else:
            log_info(f"Registry '{REGISTRY_NAME}' exists but not running, starting it...")
            run_command(f"docker start {REGISTRY_NAME}", check=False)
            return REGISTRY_NAME
    
    # Check if any registry is already running on port 5000
    existing_registry = find_registry_on_port(REGISTRY_PORT)
    if existing_registry:
        log_info(f"Found existing registry '{existing_registry}' running on port {REGISTRY_PORT}")
        log_info(f"Using existing registry instead of creating new one")
        # Update REGISTRY_NAME to use the existing one (for network connection)
        REGISTRY_NAME = existing_registry
        return REGISTRY_NAME
    
    # No registry found, create a new one
    log_info("Creating local Docker registry...")
    # Bind to 127.0.0.1 only to avoid conflicts with macOS Control Center on port 5000
    # Use a named volume for persistent storage (survives container restarts)
    # This prevents losing images when the registry container is recreated
    volume_name = f"{REGISTRY_NAME}-data"
    run_command(
        f"docker volume create {volume_name}",
        check=False  # Volume may already exist
    )
    run_command(
        f"docker run -d --restart=always -p 127.0.0.1:{REGISTRY_PORT}:5000 "
        f"-v {volume_name}:/var/lib/registry --name {REGISTRY_NAME} registry:2"
    )
    log_info(f"✅ Created registry '{REGISTRY_NAME}' on port {REGISTRY_PORT} with persistent volume '{volume_name}'")
    return REGISTRY_NAME


def get_registry_ip():
    """Get the registry container's IP address on the kind network."""
    # Get the registry container's IP on the kind network
    result = run_command(
        f"docker inspect {REGISTRY_NAME} --format='{{{{range .NetworkSettings.Networks}}}}{{{{.IPAddress}}}}{{{{end}}}}'",
        check=False,
        capture_output=True
    )
    if result.returncode == 0 and result.stdout.strip():
        # Try to find IP on kind network specifically
        result = run_command(
            f"docker inspect {REGISTRY_NAME} --format='{{{{range $key, $value := .NetworkSettings.Networks}}}}{{{{if eq $key \"kind\"}}}}{{{{.IPAddress}}}}{{{{end}}}}{{{{end}}}}'",
            check=False,
            capture_output=True
        )
        if result.returncode == 0 and result.stdout.strip():
            return result.stdout.strip()
    
    # Fallback: try to get any IP
    result = run_command(
        f"docker inspect {REGISTRY_NAME} --format='{{{{.NetworkSettings.IPAddress}}}}'",
        check=False,
        capture_output=True
    )
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip()
    
    return None


def configure_containerd_registry():
    """Configure containerd on all nodes to use local registry.
    
    This function is idempotent and can be called multiple times.
    It will update the registry configuration if the IP has changed.
    """
    # Get all node names
    result = run_command("kubectl get nodes -o jsonpath='{.items[*].metadata.name}'", check=True)
    nodes = result.stdout.strip().split()
    
    if not nodes:
        log_warn("No nodes found in cluster")
        return
    
    # Get registry IP address on kind network
    # Retry a few times in case the network connection is still being established
    registry_ip = None
    max_retries = 5
    for attempt in range(max_retries):
        registry_ip = get_registry_ip()
        if registry_ip:
            break
        if attempt < max_retries - 1:
            log_info(f"Waiting for registry IP (attempt {attempt + 1}/{max_retries})...")
            time.sleep(2)
    
    if not registry_ip:
        log_error("Could not determine registry IP address after retries")
        log_error("Registry may not be connected to kind network")
        log_error("Please run 'python3 scripts/fix_registry_config.py' to fix this")
        # Don't exit - try to continue with container name (may work if DNS is configured)
        registry_endpoint = f"http://{REGISTRY_NAME}:5000"
        log_warn(f"Falling back to container name: {registry_endpoint}")
    else:
        log_info(f"Using registry IP: {registry_ip}")
        registry_endpoint = f"http://{registry_ip}:5000"
    
    # Containerd config patch to add registry mirror
    # Use IP address for reliable connectivity (avoids DNS resolution issues)
    containerd_patch = f"""
[plugins."io.containerd.grpc.v1.cri".registry.mirrors."localhost:5000"]
  endpoint = ["{registry_endpoint}"]
"""
    
    for node in nodes:
        log_info(f"Configuring containerd on node: {node}")
        
        # Read current containerd config
        read_cmd = f"docker exec {node} cat /etc/containerd/config.toml"
        result = run_command(read_cmd, check=False, capture_output=True)
        
        if result.returncode != 0:
            log_error(f"Could not read containerd config on {node}, skipping registry configuration")
            continue
        
        config_content = result.stdout
        
        # Check if registry mirror is already configured with the correct endpoint
        if f'endpoint = ["{registry_endpoint}"]' in config_content:
            log_info(f"Registry mirror already configured correctly on {node}")
            continue
        
        # Remove existing localhost:5000 mirror config if present (to avoid duplicates)
        lines = config_content.split('\n')
        new_lines = []
        skip_until_end = False
        for i, line in enumerate(lines):
            if '[plugins."io.containerd.grpc.v1.cri".registry.mirrors."localhost:5000"]' in line:
                # Skip this section
                skip_until_end = True
                continue
            if skip_until_end:
                if line.strip().startswith('[') and 'registry.mirrors' in line:
                    # Next section started, stop skipping
                    skip_until_end = False
                    new_lines.append(line)
                elif line.strip() and not line.strip().startswith('endpoint') and not line.strip().startswith('  '):
                    # Non-indented line, stop skipping
                    skip_until_end = False
                    new_lines.append(line)
                # Otherwise continue skipping
                continue
            new_lines.append(line)
        
        # Append new registry mirror configuration
        config_content = '\n'.join(new_lines).rstrip() + containerd_patch
        
        # Write updated config back
        write_cmd = f"docker exec -i {node} sh -c 'cat > /etc/containerd/config.toml'"
        result = run_command(write_cmd, input=config_content, check=False)
        
        if result.returncode != 0:
            log_error(f"Could not write containerd config on {node}")
            continue
        
        # Restart containerd
        log_info(f"Restarting containerd on {node}...")
        result = run_command(f"docker exec {node} systemctl restart containerd", check=False)
        if result.returncode != 0:
            log_warn(f"Could not restart containerd on {node} (may already be restarted)")
        
        # Poll for containerd to be ready (check if it's responding)
        log_info(f"Waiting for containerd to be ready on {node}...")
        max_containerd_wait = 10  # Wait up to 10 seconds
        containerd_ready = False
        for i in range(max_containerd_wait):
            # Check if containerd is responding
            result = run_command(f"docker exec {node} ctr version", check=False, capture_output=True)
            if result.returncode == 0:
                containerd_ready = True
                break
            if i < max_containerd_wait - 1:
                time.sleep(1)
        
        if not containerd_ready:
            log_warn(f"Containerd may not be fully ready on {node}, but continuing...")
        
        log_info(f"✅ Configured registry mirror on {node} (endpoint: {registry_endpoint})")


def create_octopilot_system_namespace():
    """Create octopilot-system namespace with proper labels.
    
    Creates the namespace at cluster startup so it's not managed by Tilt or GitOps.
    This ensures the namespace is always available and has the correct labels for
    FluxCD NetworkPolicy namespaceSelector matching.
    """
    log_info("Creating octopilot-system namespace...")
    
    # Check if namespace already exists
    result = run_command(
        ["kubectl", "get", "namespace", "octopilot-system"],
        check=False,
        capture_output=True
    )
    if result.returncode == 0:
        log_info("octopilot-system namespace already exists")
        # Update labels if needed (idempotent)
        namespace_yaml = """apiVersion: v1
kind: Namespace
metadata:
  name: octopilot-system
  labels:
    name: octopilot-system
    app: secret-manager-controller
    environment: system
    managed-by: kind-setup
"""
        run_command(
            "kubectl apply -f -",
            input=namespace_yaml,
            check=False
        )
        return
    
    # Create namespace with labels
    namespace_yaml = """apiVersion: v1
kind: Namespace
metadata:
  name: octopilot-system
  labels:
    name: octopilot-system
    app: secret-manager-controller
    environment: system
    managed-by: kind-setup
"""
    result = run_command(
        "kubectl apply -f -",
        input=namespace_yaml,
        check=False,
        capture_output=True
    )
    
    if result.returncode == 0:
        log_info("✅ octopilot-system namespace created successfully")
    else:
        log_warn(f"Failed to create namespace: {result.stderr}")
        # Try simple create as fallback
        run_command(
            ["kubectl", "create", "namespace", "octopilot-system"],
            check=False,
        )


def create_pvc():
    """Create PVC for controller cache.
    
    Creates the PVC at cluster startup so it's not managed by Tilt.
    This prevents Tilt from deleting/recreating PVCs which can lock up the system.
    """
    log_info("Creating PVC for controller cache...")
    
    pvc_yaml_path = Path("config/storage/pvc.yaml")
    if not pvc_yaml_path.exists():
        log_warn(f"PVC YAML not found at {pvc_yaml_path}, skipping PVC creation")
        return
    
    # Apply PVC (idempotent - won't fail if it already exists)
    result = run_command(
        ["kubectl", "apply", "-f", str(pvc_yaml_path)],
        check=False,
        capture_output=True
    )
    
    if result.returncode == 0:
        log_info("✅ PVC created successfully")
    else:
        # Check if PVC already exists (that's okay)
        if "already exists" in result.stderr or "unchanged" in result.stdout:
            log_info("✅ PVC already exists")
        else:
            log_warn(f"Failed to create PVC: {result.stderr}")
            log_warn("PVC will be created by kustomize during controller deployment")


def create_postgres_pvc():
    """Create PVC for PostgreSQL database.
    
    Creates the postgres-data PVC at cluster startup so it's not managed by Tilt.
    This prevents Tilt from deleting/recreating PVCs which can lock up the system
    and cause database corruption.
    """
    log_info("Creating PVC for PostgreSQL database...")
    
    # First ensure the namespace exists
    namespace = "secret-manager-controller-pact-broker"
    namespace_result = run_command(
        ["kubectl", "get", "namespace", namespace],
        check=False,
        capture_output=True
    )
    
    if namespace_result.returncode != 0:
        log_info(f"Creating namespace {namespace}...")
        namespace_yaml = f"""apiVersion: v1
kind: Namespace
metadata:
  name: {namespace}
"""
        run_command(
            "kubectl apply -f -",
            input=namespace_yaml,
            check=False
        )
    
    # Create PVC YAML inline (matches postgres-deployment.yaml)
    pvc_yaml = """apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: postgres-data
  namespace: secret-manager-controller-pact-broker
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 1Gi
"""
    
    # Apply PVC (idempotent - won't fail if it already exists)
    result = run_command(
        "kubectl apply -f -",
        input=pvc_yaml,
        check=False,
        capture_output=True
    )
    
    if result.returncode == 0:
        log_info("✅ PostgreSQL PVC created successfully")
    else:
        # Check if PVC already exists (that's okay)
        if "already exists" in result.stderr or "unchanged" in result.stdout:
            log_info("✅ PostgreSQL PVC already exists")
        else:
            log_warn(f"Failed to create PostgreSQL PVC: {result.stderr}")
            log_warn("PVC will be created by kustomize during postgres deployment")


def ensure_registry_connected():
    """Ensure registry is connected to kind network.
    
    Returns True if registry is connected, False otherwise.
    This function is idempotent and can be called multiple times.
    """
    # Poll for kind network to exist (cluster may have just been created)
    log_info("Checking for kind network...")
    max_network_wait = 10  # Wait up to 10 seconds
    network_exists = False
    for i in range(max_network_wait):
        result = run_command(["docker", "network", "ls", "--format", "{{.Name}}"], check=False, capture_output=True)
        # Check stdout and handle empty results
        network_list = result.stdout or ""
        if "kind" in network_list:
            network_exists = True
            break
        # Poll every 1 second
        if i < max_network_wait - 1:
            time.sleep(1)
    
    if not network_exists:
        log_warn("Kind network not found after polling - cluster may not exist or may not be ready")
        return False
    
    # Check if registry container exists and is running
    result = run_command(f"docker ps --format '{{{{.Names}}}}'", check=False)
    if REGISTRY_NAME not in result.stdout:
        log_warn(f"Registry container '{REGISTRY_NAME}' is not running")
        log_info("Starting registry container...")
        result = run_command(f"docker start {REGISTRY_NAME}", check=False)
        if result.returncode != 0:
            log_error(f"Failed to start registry container: {result.stderr}")
            return False
        # Poll for container to be running
        log_info("Waiting for registry container to start...")
        max_start_wait = 5  # Wait up to 5 seconds
        for i in range(max_start_wait):
            result = run_command(f"docker ps --format '{{{{.Names}}}}'", check=False)
            if REGISTRY_NAME in result.stdout:
                break
            if i < max_start_wait - 1:
                time.sleep(1)
    
    # Check if registry is already connected to kind network
    result = run_command(
        "docker network inspect kind --format='{{range .Containers}}{{.Name}}{{\"\\n\"}}{{end}}'",
        check=False,
        capture_output=True
    )
    
    if REGISTRY_NAME in result.stdout:
        log_info("✅ Registry already connected to kind network")
        return True
    
    # Connect registry to kind network
    log_info(f"Connecting registry '{REGISTRY_NAME}' to kind network...")
    result = run_command(f"docker network connect kind {REGISTRY_NAME}", check=False)
    if result.returncode == 0:
        # Poll to verify the connection is established
        log_info("Verifying registry connection to kind network...")
        max_verify_wait = 5  # Wait up to 5 seconds
        for i in range(max_verify_wait):
            result = run_command(
                "docker network inspect kind --format='{{range .Containers}}{{.Name}}{{\"\\n\"}}{{end}}'",
                check=False,
                capture_output=True
            )
            if REGISTRY_NAME in result.stdout:
                log_info("✅ Registry connected to kind network")
                return True
            if i < max_verify_wait - 1:
                time.sleep(1)
        # If we get here, connection might have failed
        log_warn("Registry connection verification timed out, but connection may have succeeded")
        return True
    else:
        # Check if it's already connected (race condition)
        result = run_command(
            "docker network inspect kind --format='{{range .Containers}}{{.Name}}{{\"\\n\"}}{{end}}'",
            check=False,
            capture_output=True
        )
        if REGISTRY_NAME in result.stdout:
            log_info("✅ Registry is connected to kind network (verified)")
            return True
        log_error(f"Failed to connect registry to kind network: {result.stderr}")
        return False


def preload_required_images():
    """Pre-load required Docker images into Kind cluster.
    
    This function loads images that are needed by init containers and other
    infrastructure components, avoiding network issues when pulling from external registries.
    """
    log_info("Pre-loading required images into Kind cluster...")
    
    # List of images to pre-load
    required_images = [
        "busybox:1.36",
    ]
    
    for image in required_images:
        log_info(f"  Pre-loading {image}...")
        # Check if image exists locally
        result = run_command(f"docker images --format '{{{{.Repository}}}}:{{{{.Tag}}}}' {image}", check=False)
        if image not in result.stdout:
            # Pull image first
            log_info(f"    Pulling {image}...")
            pull_result = run_command(f"docker pull {image}", check=False)
            if pull_result.returncode != 0:
                log_warn(f"    Failed to pull {image}: {pull_result.stderr}")
                log_warn(f"    Cluster will try to pull it at runtime (may fail if network is unavailable)")
                continue
        
        # Load image into Kind cluster
        load_result = run_command(f"kind load docker-image {image} --name {CLUSTER_NAME}", check=False)
        if load_result.returncode == 0:
            log_info(f"    ✅ Successfully loaded {image}")
        else:
            log_warn(f"    ⚠️  Failed to load {image}: {load_result.stderr}")
            log_warn(f"    Cluster will try to pull it at runtime (may fail if network is unavailable)")


def install_secret_manager_crd():
    """Install SecretManagerConfig CRD from committed version.
    
    The CRD is committed to the repo, so we can install it during cluster setup.
    This ensures the CRD is always available and established before any resources
    are applied, preventing "no matches for kind" errors.
    
    Later, build_all_binaries.py can update the CRD if it has changed (kubectl apply is idempotent).
    """
    log_info("Installing SecretManagerConfig CRD...")
    
    # Get script directory and project root
    script_dir = Path(__file__).parent
    project_root = script_dir.parent
    crd_path = project_root / "config" / "crd" / "secretmanagerconfig.yaml"
    
    if not crd_path.exists():
        log_warn(f"CRD file not found at {crd_path}")
        log_warn("CRD will be installed later by build_all_binaries.py")
        return
    
    # Apply CRD (idempotent - won't fail if it already exists)
    result = run_command(
        ["kubectl", "apply", "-f", str(crd_path)],
        check=False,
        capture_output=True
    )
    
    if result.returncode != 0:
        log_warn(f"Failed to apply CRD: {result.stderr}")
        log_warn("CRD will be installed later by build_all_binaries.py")
        return
    
    log_info("✅ CRD applied successfully")
    
    # Wait for CRD to be established
    log_info("Waiting for CRD to be established...")
    crd_name = "secretmanagerconfigs.secret-management.octopilot.io"
    max_attempts = 30  # Wait up to 1 minute
    
    for i in range(max_attempts):
        wait_result = run_command(
            f"kubectl wait --for=condition=established crd {crd_name} --timeout=2s",
            check=False,
            capture_output=True
        )
        
        if wait_result.returncode == 0:
            log_info("✅ CRD is established and ready")
            return
        
        # Poll every 2 seconds (no fixed sleep, just continue loop)
        if i < max_attempts - 1:
            time.sleep(2)
    
    log_warn("CRD not established after 60 seconds, but continuing")
    log_warn("CRD may not be ready when resources are applied")


def install_gitops_components():
    """Install FluxCD and ArgoCD components in the cluster.
    
    These are infrastructure dependencies that should be available as soon as the cluster is up.
    Installing them here (outside of Tilt) ensures they're always available and don't need to be
    reinstalled every time Tilt starts.
    """
    log_info("Installing GitOps components (FluxCD and ArgoCD)...")
    
    # Get script directory
    script_dir = Path(__file__).parent
    project_root = script_dir.parent
    
    # Install FluxCD
    fluxcd_script = script_dir / "tilt" / "install_fluxcd.py"
    if fluxcd_script.exists():
        log_info("Installing FluxCD source-controller and notification-controller...")
        result = run_command(
            [sys.executable, str(fluxcd_script)],
            check=False,
            capture_output=True
        )
        if result.returncode == 0:
            log_info("✅ FluxCD installed successfully")
        else:
            log_warn(f"FluxCD installation had issues: {result.stderr}")
            # Don't fail - cluster setup should continue even if GitOps components have issues
    else:
        log_warn(f"FluxCD install script not found at {fluxcd_script}")
    
    # Install ArgoCD CRDs
    argocd_script = script_dir / "tilt" / "install_argocd.py"
    if argocd_script.exists():
        log_info("Installing ArgoCD CRDs...")
        result = run_command(
            [sys.executable, str(argocd_script)],
            check=False,
            capture_output=True
        )
        if result.returncode == 0:
            log_info("✅ ArgoCD CRDs installed successfully")
        else:
            log_warn(f"ArgoCD installation had issues: {result.stderr}")
            # Don't fail - cluster setup should continue even if GitOps components have issues
    else:
        log_warn(f"ArgoCD install script not found at {argocd_script}")
    
    log_info("✅ GitOps components installation complete")


def setup_kind_cluster():
    """Setup Kind cluster."""
    result = run_command("kind get clusters", check=False)
    
    cluster_exists = CLUSTER_NAME in result.stdout
    
    if cluster_exists:
        # Check if NON_INTERACTIVE mode is set (called from dev_up.py or CI)
        # Also check for CI environment variables (GitHub Actions, GitLab CI, etc.)
        is_non_interactive = (
            os.getenv("NON_INTERACTIVE", "").lower() in ("1", "true", "yes") or
            os.getenv("CI", "").lower() in ("1", "true", "yes") or
            os.getenv("GITHUB_ACTIONS", "").lower() in ("1", "true", "yes") or
            not sys.stdin.isatty()  # No TTY available (CI environments)
        )
        
        if is_non_interactive:
            log_info(f"Cluster {CLUSTER_NAME} already exists, using existing cluster (non-interactive mode)")
            # Ensure registry is connected even if cluster already exists
            ensure_registry_connected()
            # Ensure containerd is configured
            configure_containerd_registry()
            return
        log_warn(f"Cluster {CLUSTER_NAME} already exists")
        response = input("Do you want to delete and recreate it? (y/N) ")
        if response.lower() == 'y':
            log_info("Deleting existing cluster...")
            run_command(f"kind delete cluster --name {CLUSTER_NAME}")
        else:
            log_info("Using existing cluster")
            # Ensure registry is connected
            ensure_registry_connected()
            # Ensure containerd is configured
            configure_containerd_registry()
            return
    
    # Check if kind-config.yaml exists
    config_path = Path("kind-config.yaml")
    if not config_path.exists():
        log_error(f"kind-config.yaml not found at {config_path}")
        log_info("Please create kind-config.yaml in the project root")
        sys.exit(1)
    
    log_info("Creating Kind cluster...")
    result = run_command(f"kind create cluster --config {config_path}", check=False, capture_output=True)
    if result.returncode != 0:
        # Check if cluster already exists (this is okay, we'll use it)
        if "already exists" in result.stderr.lower() or "already exists" in result.stdout.lower():
            log_info(f"Cluster {CLUSTER_NAME} already exists, using existing cluster")
            # Ensure registry is connected
            ensure_registry_connected()
            # Ensure containerd is configured
            configure_containerd_registry()
            return
        log_error(f"Failed to create Kind cluster: {result.stderr}")
        if result.stdout:
            log_error(f"stdout: {result.stdout}")
        sys.exit(1)
    
    # Poll for the cluster network to be created
    # The network is created asynchronously, so we need to wait for it
    log_info("Waiting for cluster network to be ready...")
    max_network_wait = 10  # Wait up to 10 seconds
    network_ready = False
    for i in range(max_network_wait):
        result = run_command(["docker", "network", "ls", "--format", "{{.Name}}"], check=False, capture_output=True)
        # Check stdout and handle empty results
        network_list = result.stdout or ""
        if "kind" in network_list:
            network_ready = True
            log_info("✅ Cluster network is ready")
            break
        # Poll every 1 second
        if i < max_network_wait - 1:
            time.sleep(1)
    
    if not network_ready:
        # Verify cluster was actually created
        cluster_check = run_command("kind get clusters", check=False)
        if CLUSTER_NAME in cluster_check.stdout:
            log_warn("Cluster exists but network not found - network may have a different name")
            log_warn("Attempting to continue with registry connection...")
            # Try to connect anyway - the network might exist with a different name
        else:
            log_error("Kind network not found after cluster creation - cluster may not have started correctly")
            log_error("Please check 'kind get clusters' and 'docker network ls' to verify cluster status")
            sys.exit(1)
    
    # Connect registry to cluster network IMMEDIATELY after cluster creation
    # This must happen before any pods try to pull images
    log_info("Connecting registry to cluster network...")
    if not ensure_registry_connected():
        log_error("Failed to connect registry to kind network - cluster may not work correctly")
        log_error("Please run 'python3 scripts/fix_registry_config.py' to fix this")
        sys.exit(1)
    
    # Poll to verify registry is accessible from nodes
    log_info("Verifying registry is accessible from cluster nodes...")
    max_verify_wait = 10  # Wait up to 10 seconds
    registry_accessible = False
    for i in range(max_verify_wait):
        # Get a node name to test from
        result = run_command("kubectl get nodes -o jsonpath='{.items[0].metadata.name}'", check=False)
        if result.returncode == 0 and result.stdout.strip():
            node_name = result.stdout.strip()
            # Try to ping the registry from the node
            registry_ip = get_registry_ip()
            if registry_ip:
                test_result = run_command(
                    f"docker exec {node_name} ping -c 1 -W 1 {registry_ip}",
                    check=False,
                    capture_output=True
                )
                if test_result.returncode == 0:
                    registry_accessible = True
                    log_info("✅ Registry is accessible from cluster nodes")
                    break
        # Poll every 1 second
        if i < max_verify_wait - 1:
            time.sleep(1)
    
    if not registry_accessible:
        log_warn("Registry accessibility verification timed out, but continuing...")
    
    # Configure containerd on all nodes to use local registry
    # This must happen IMMEDIATELY after cluster creation and registry connection
    log_info("Configuring containerd on nodes to use local registry...")
    configure_containerd_registry()
    
    # Create octopilot-system namespace (created at cluster startup, not managed by Tilt/GitOps)
    # This ensures the namespace is always available with proper labels
    create_octopilot_system_namespace()
    
    # Pre-load required images into Kind cluster
    # This avoids network issues when pulling images at runtime
    preload_required_images()
    
    # Create PVCs (created at cluster startup, not managed by Tilt)
    # This prevents Tilt from deleting/recreating PVCs which can lock up the system
    create_pvc()
    create_postgres_pvc()
    
    # Configure cluster to use local registry
    configmap_yaml = f"""apiVersion: v1
kind: ConfigMap
metadata:
  name: local-registry-hosting
  namespace: kube-public
data:
  localRegistryHosting.v1: |
    host: "localhost:{REGISTRY_PORT}"
    help: "https://kind.sigs.k8s.io/docs/user/local-registry/"
"""
    
    run_command(
        "kubectl apply -f -",
        input=configmap_yaml,
        check=True
    )
    
    # Install SecretManagerConfig CRD from committed version
    # This ensures the CRD is always available and established before any resources are applied
    install_secret_manager_crd()
    
    # Install FluxCD and ArgoCD components
    # These are infrastructure dependencies that should be available as soon as the cluster is up
    install_gitops_components()
    
    log_info(f"✅ Kind cluster '{CLUSTER_NAME}' created successfully!")
    log_info(f"📦 Local registry: {REGISTRY_NAME} (localhost:{REGISTRY_PORT})")
    log_info("🚀 You can now run 'tilt up' to start the controller")


def main():
    """Main setup function."""
    log_info("Checking prerequisites...")
    check_command("docker")
    check_command("kind")
    check_command("kubectl")
    
    setup_registry()
    setup_kind_cluster()


if __name__ == "__main__":
    main()

