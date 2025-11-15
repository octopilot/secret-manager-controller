#!/usr/bin/env python3
"""
Secret Manager Controller K3s Setup Script.

This script replaces setup-k3s.sh and provides better error handling
and cross-platform support.

Creates a local k3s cluster with Docker registry for development.
"""

import os
import shutil
import subprocess
import sys
import time
from pathlib import Path


def is_interactive():
    """Check if running in interactive mode (TTY).
    
    Can be overridden by NON_INTERACTIVE environment variable.
    """
    # Check environment variable first (allows forcing non-interactive mode)
    if os.getenv("NON_INTERACTIVE", "").lower() in ("1", "true", "yes"):
        return False
    # Otherwise check if stdin is a TTY
    return sys.stdin.isatty()


# Configuration
CLUSTER_NAME = "secret-manager-controller"
CONTAINER_NAME = f"k3s-{CLUSTER_NAME}"
REGISTRY_NAME = "secret-manager-controller-registry"
REGISTRY_PORT = "5002"
K3S_PORT = "6443"


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


def check_docker_running():
    """Check if Docker daemon is running."""
    result = run_command("docker info", check=False, capture_output=True)
    if result.returncode != 0:
        log_error("Docker daemon is not running")
        print("   Please start Docker Desktop and try again")
        sys.exit(1)


def check_port_available(port):
    """Check if a port is available."""
    # Try using lsof (macOS/Linux)
    result = run_command(f"lsof -i :{port}", check=False, capture_output=True)
    if result.returncode == 0:
        return False
    
    # Try using netstat (Linux)
    result = run_command(f"netstat -an 2>/dev/null | grep -q ':{port}.*LISTEN'", check=False, shell=True)
    if result.returncode == 0:
        return False
    
    # Check Docker containers using the port
    result = run_command(
        f"docker ps --format '{{{{.Ports}}}}' | grep -q ':{port}'",
        check=False,
        shell=True
    )
    if result.returncode == 0:
        return False
    
    return True


def setup_registry():
    """Setup local Docker registry."""
    # Check if registry container exists
    result = run_command(
        f"docker ps -a --filter 'name={REGISTRY_NAME}' --format '{{{{.Names}}}}'",
        check=False
    )
    
    if REGISTRY_NAME in result.stdout:
        log_info(f"Registry container '{REGISTRY_NAME}' already exists")
        # Check if it's running
        result = run_command(
            f"docker ps --filter 'name={REGISTRY_NAME}' --format '{{{{.Names}}}}'",
            check=False
        )
        if REGISTRY_NAME in result.stdout:
            log_info("Registry container is already running")
        else:
            # Try to start it
            log_info("Starting existing registry container...")
            result = run_command(f"docker start {REGISTRY_NAME}", check=False)
            if result.returncode != 0:
                log_warn("Failed to start existing registry (port may be in use)")
                log_info("Removing failed container...")
                run_command(f"docker rm {REGISTRY_NAME}", check=False)
                if not check_port_available(REGISTRY_PORT):
                    log_error(f"Port {REGISTRY_PORT} is already in use by another process")
                    log_info("Please stop the process using port {REGISTRY_PORT} or use a different port")
                    sys.exit(1)
                log_info("Creating new registry container...")
                run_command(
                    f"docker run -d --restart=always -p {REGISTRY_PORT}:5000 --name {REGISTRY_NAME} registry:2"
                )
    else:
        # Check if port is available before creating
        if not check_port_available(REGISTRY_PORT):
            log_error(f"Port {REGISTRY_PORT} is already in use")
            log_info(f"Port {REGISTRY_PORT} is being used by:")
            run_command(f"lsof -i :{REGISTRY_PORT} 2>/dev/null || docker ps --format '{{{{.Names}}}}: {{{{.Ports}}}}' | grep {REGISTRY_PORT} || true", check=False)
            log_info(f"Please stop the process using port {REGISTRY_PORT} or modify REGISTRY_PORT in the script")
            sys.exit(1)
        log_info("Creating local Docker registry...")
        run_command(
            f"docker run -d --restart=always -p {REGISTRY_PORT}:5000 --name {REGISTRY_NAME} registry:2"
        )


def setup_k3s_network():
    """Setup Docker network for k3s."""
    result = run_command("docker network ls --format '{{{{.Name}}}}'", check=False)
    if "k3s-net" not in result.stdout:
        log_info("Creating Docker network for k3s...")
        run_command("docker network create k3s-net", check=False)
    
    # Connect registry to k3s network
    result = run_command(
        f"docker ps -a --filter 'name={REGISTRY_NAME}' --format '{{{{.Names}}}}'",
        check=False
    )
    if REGISTRY_NAME in result.stdout:
        run_command(f"docker network connect k3s-net {REGISTRY_NAME}", check=False)


def setup_k3s_container():
    """Setup k3s container."""
    result = run_command(f"docker ps -a --format '{{{{.Names}}}}'", check=False)
    
    if CONTAINER_NAME in result.stdout:
        log_info(f"K3s container '{CONTAINER_NAME}' already exists")
        container_deleted = False
        
        # In non-interactive mode (e.g., from justfile), just update kubeconfig
        if not is_interactive():
            log_info("Non-interactive mode: updating kubeconfig from existing container...")
        else:
            log_warn(f"K3s container '{CONTAINER_NAME}' already exists")
            response = input("Do you want to delete and recreate it? (y/N) ")
            if response.lower() == 'y':
                log_info("Stopping and removing existing container...")
                run_command(f"docker stop {CONTAINER_NAME}", check=False)
                run_command(f"docker rm {CONTAINER_NAME}", check=False)
                container_deleted = True
                # Continue to create new container below
            else:
                log_info("Using existing container")
        
        # If container was deleted, skip kubeconfig update and continue to creation
        if container_deleted:
            log_info("Container deleted, will create new container...")
            # Fall through to container creation below
        else:
            # Start container if not running
            run_command(f"docker start {CONTAINER_NAME}", check=False)
            
            # Get kubeconfig from existing container
            log_info("Retrieving kubeconfig from existing container...")
            kube_dir = Path.home() / ".kube"
            kube_dir.mkdir(parents=True, exist_ok=True)
            
            run_command(
                f"docker cp {CONTAINER_NAME}:/etc/rancher/k3s/k3s.yaml {kube_dir}/k3s-{CLUSTER_NAME}.yaml",
                check=False
            )
            
            # Update kubeconfig to use localhost
            kubeconfig_path = kube_dir / f"k3s-{CLUSTER_NAME}.yaml"
            if kubeconfig_path.exists():
                content = kubeconfig_path.read_text()
                content = content.replace("127.0.0.1", "localhost")
                kubeconfig_path.write_text(content)
                
                # Merge kubeconfig into main config
                main_config = kube_dir / "config"
                if main_config.exists():
                    run_command(
                        f"KUBECONFIG={kubeconfig_path}:{main_config} kubectl config view --flatten > {main_config}.new",
                        check=False
                    )
                    if (main_config.with_suffix(".new")).exists():
                        (main_config.with_suffix(".new")).replace(main_config)
                else:
                    shutil.copy(kubeconfig_path, main_config)
                
                # Ensure context is named correctly
                run_command(
                    f"kubectl config rename-context default k3s-{CLUSTER_NAME}",
                    check=False
                )
            
            log_info("✅ K3s cluster is ready!")
            log_info(f"📋 Context name: k3s-{CLUSTER_NAME}")
            
            # Exit early if we're using existing container
            # In interactive mode: user chose not to recreate
            # In non-interactive mode: just update kubeconfig and use existing container
            return
    
    # Create k3s container
    log_info("Creating K3s container...")
    run_command(
        f"docker run -d --name {CONTAINER_NAME} --privileged --restart=unless-stopped "
        f"-p {K3S_PORT}:6443 -v {CONTAINER_NAME}:/var/lib/rancher/k3s "
        f"-v {CONTAINER_NAME}-config:/etc/rancher/k3s --network k3s-net "
        f"rancher/k3s:latest server --disable traefik --write-kubeconfig-mode 644 --tls-san localhost"
    )
    
    # Configure registry access for k3s
    log_info("Configuring registry access...")
    registries_yaml = f"""mirrors:
  "localhost:{REGISTRY_PORT}":
    endpoint:
      - "http://{REGISTRY_NAME}:5000"
configs:
  "localhost:{REGISTRY_PORT}":
    tls:
      insecure_skip_verify: true
"""
    
    run_command(
        f"docker exec {CONTAINER_NAME} sh -c 'mkdir -p /etc/rancher/k3s && cat > /etc/rancher/k3s/registries.yaml'",
        input=registries_yaml,
        check=False
    )
    
    # Create containerd hosts.toml
    log_info("Configuring containerd registry access...")
    hosts_toml = f"""server = "http://{REGISTRY_NAME}:5000"

[host."http://{REGISTRY_NAME}:5000"]
  capabilities = ["pull", "resolve"]
"""
    
    run_command(
        f"docker exec {CONTAINER_NAME} sh -c 'mkdir -p /var/lib/rancher/k3s/agent/etc/containerd/certs.d/localhost:{REGISTRY_PORT} && cat > /var/lib/rancher/k3s/agent/etc/containerd/certs.d/localhost:{REGISTRY_PORT}/hosts.toml'",
        input=hosts_toml,
        check=False
    )
    
    log_info("Restarting k3s to apply registry configuration...")
    run_command(f"docker restart {CONTAINER_NAME}", check=False)
    time.sleep(5)
    
    # Wait for k3s to be ready
    log_info("Waiting for K3s to be ready...")
    for i in range(60):
        result = run_command(
            f"docker exec {CONTAINER_NAME} kubectl get nodes",
            check=False,
            capture_output=True
        )
        if result.returncode == 0:
            log_info("K3s is ready!")
            break
        if i == 59:
            log_error("K3s failed to start after 120 seconds")
            sys.exit(1)
        time.sleep(2)
    
    # Get kubeconfig
    log_info("Retrieving kubeconfig...")
    kube_dir = Path.home() / ".kube"
    kube_dir.mkdir(parents=True, exist_ok=True)
    
    run_command(
        f"docker cp {CONTAINER_NAME}:/etc/rancher/k3s/k3s.yaml {kube_dir}/k3s-{CLUSTER_NAME}.yaml"
    )
    
    # Update kubeconfig to use localhost
    kubeconfig_path = kube_dir / f"k3s-{CLUSTER_NAME}.yaml"
    if kubeconfig_path.exists():
        content = kubeconfig_path.read_text()
        content = content.replace("127.0.0.1", "localhost")
        kubeconfig_path.write_text(content)
    
    # Merge kubeconfig into main config
    main_config = kube_dir / "config"
    if main_config.exists():
        run_command(
            f"KUBECONFIG={kubeconfig_path}:{main_config} kubectl config view --flatten > {main_config}.new",
            check=False
        )
        if (main_config.with_suffix(".new")).exists():
            (main_config.with_suffix(".new")).replace(main_config)
    else:
        shutil.copy(kubeconfig_path, main_config)
    
    # Rename context
    run_command(
        f"kubectl config rename-context default k3s-{CLUSTER_NAME}",
        check=False
    )
    
    log_info(f"✅ K3s cluster '{CLUSTER_NAME}' created successfully!")
    log_info(f"📦 Local registry: {REGISTRY_NAME} (localhost:{REGISTRY_PORT})")
    log_info("🚀 Kubeconfig merged into: ~/.kube/config")
    log_info(f"📋 Context name: k3s-{CLUSTER_NAME}")
    print()
    log_info("To use this cluster:")
    print(f"  kubectl config use-context k3s-{CLUSTER_NAME}")
    print("  kubectl get nodes")


def main():
    """Main setup function."""
    log_info("Checking prerequisites...")
    check_command("docker")
    check_command("kubectl")
    
    check_docker_running()
    setup_registry()
    setup_k3s_network()
    setup_k3s_container()


if __name__ == "__main__":
    main()

