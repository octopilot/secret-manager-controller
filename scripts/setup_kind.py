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
    run_command(
        f"docker run -d --restart=always -p 127.0.0.1:{REGISTRY_PORT}:5000 --name {REGISTRY_NAME} registry:2"
    )
    log_info(f"✅ Created registry '{REGISTRY_NAME}' on port {REGISTRY_PORT}")
    return REGISTRY_NAME


def configure_containerd_registry():
    """Configure containerd on all nodes to use local registry."""
    # Get all node names
    result = run_command("kubectl get nodes -o jsonpath='{.items[*].metadata.name}'", check=True)
    nodes = result.stdout.strip().split()
    
    # Containerd config patch to add registry mirror
    # This will be appended to the config file
    containerd_patch = """
[plugins."io.containerd.grpc.v1.cri".registry.mirrors."localhost:5000"]
  endpoint = ["http://secret-manager-controller-registry:5000"]
"""
    
    for node in nodes:
        log_info(f"Configuring containerd on node: {node}")
        
        # Check if registry mirror is already configured
        check_cmd = f"docker exec {node} grep -q 'localhost:5000' /etc/containerd/config.toml"
        result = run_command(check_cmd, check=False)
        if result.returncode == 0:
            log_info(f"Registry mirror already configured on {node}")
            continue
        
        # Read current containerd config
        read_cmd = f"docker exec {node} cat /etc/containerd/config.toml"
        result = run_command(read_cmd, check=False, capture_output=True)
        
        if result.returncode != 0:
            log_warn(f"Could not read containerd config on {node}, skipping registry configuration")
            continue
        
        # Append registry mirror configuration to the end
        config_content = result.stdout.rstrip() + containerd_patch
        
        # Write updated config back
        write_cmd = f"docker exec -i {node} sh -c 'cat > /etc/containerd/config.toml'"
        result = run_command(write_cmd, input=config_content, check=False)
        
        if result.returncode != 0:
            log_warn(f"Could not write containerd config on {node}")
            continue
        
        # Restart containerd
        log_info(f"Restarting containerd on {node}...")
        run_command(f"docker exec {node} systemctl restart containerd", check=False)
        
        # Wait a moment for containerd to restart
        time.sleep(2)
        
        log_info(f"✅ Configured registry mirror on {node}")


def ensure_registry_connected():
    """Ensure registry is connected to kind network."""
    # Check if kind network exists
    result = run_command("docker network ls --format '{{{{.Name}}}}'", check=False)
    if "kind" not in result.stdout:
        log_warn("Kind network not found - cluster may not exist")
        return False
    
    # Check if registry is already connected
    result = run_command(
        "docker network inspect kind --format='{{range .Containers}}{{.Name}}{{\"\\n\"}}{{end}}'",
        check=False,
        capture_output=True
    )
    
    if REGISTRY_NAME in result.stdout:
        log_info("Registry already connected to kind network")
        return True
    
    # Connect registry to kind network
    log_info("Connecting registry to kind network...")
    result = run_command(f"docker network connect kind {REGISTRY_NAME}", check=False)
    if result.returncode == 0:
        log_info("✅ Registry connected to kind network")
        return True
    else:
        log_warn(f"Failed to connect registry to kind network: {result.stderr}")
        return False


def setup_kind_cluster():
    """Setup Kind cluster."""
    result = run_command("kind get clusters", check=False)
    
    cluster_exists = CLUSTER_NAME in result.stdout
    
    if cluster_exists:
        # Check if NON_INTERACTIVE mode is set (called from dev_up.py)
        if os.getenv("NON_INTERACTIVE", "").lower() in ("1", "true", "yes"):
            log_info(f"Cluster {CLUSTER_NAME} already exists, using existing cluster")
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
    run_command(f"kind create cluster --config {config_path}")
    
    # Connect registry to cluster network
    ensure_registry_connected()
    
    # Configure containerd on all nodes to use local registry
    log_info("Configuring containerd on nodes to use local registry...")
    configure_containerd_registry()
    
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

