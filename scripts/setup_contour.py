#!/usr/bin/env python3
"""
Setup Contour Ingress Controller for Kind cluster.

This script deploys Contour Ingress Controller to the Kind cluster.
Contour is a CNCF project that uses Envoy as the data plane.
"""

import os
import subprocess
import sys
import time
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
    """Run a shell command."""
    result = subprocess.run(
        cmd,
        shell=True,
        capture_output=capture_output,
        text=True
    )
    if check and result.returncode != 0:
        log_error(f"Command failed: {cmd}")
        if result.stderr:
            log_error(result.stderr)
        sys.exit(1)
    return result


def check_cluster_exists():
    """Check if Kind cluster exists."""
    result = run_command("kind get clusters", check=False)
    if "secret-manager-controller" not in result.stdout:
        log_error("Kind cluster 'secret-manager-controller' not found")
        log_info("Please create the cluster first: python3 scripts/setup_kind.py")
        sys.exit(1)
    log_info("✅ Kind cluster found")


def check_contour_exists():
    """Check if Contour already exists."""
    result = run_command(
        "kubectl get namespace projectcontour --context kind-secret-manager-controller",
        check=False
    )
    return result.returncode == 0


def check_helm():
    """Check if Helm is installed."""
    result = run_command("helm version", check=False)
    if result.returncode != 0:
        log_error("Helm is not installed. Please install Helm first:")
        log_info("  macOS: brew install helm")
        log_info("  Linux: https://helm.sh/docs/intro/install/")
        sys.exit(1)
    log_info("✅ Helm is installed")


def add_contour_repo():
    """Add Contour Helm repository."""
    log_info("Adding Contour Helm repository...")
    
    # Check if repo already exists
    result = run_command(
        "helm repo list",
        check=False,
        capture_output=True
    )
    
    repo_name = "contour"
    repo_url = "https://projectcontour.github.io/helm-charts/"
    
    if repo_name in result.stdout:
        log_info("Contour repository already added")
        # Update the repo
        run_command(f"helm repo update {repo_name}", check=False)
    else:
        # Add the official Contour Helm repository
        add_cmd = f"helm repo add {repo_name} {repo_url}"
        result = run_command(add_cmd, check=False, capture_output=True)
        if result.returncode != 0:
            log_error(f"Failed to add Contour Helm repository: {result.stderr}")
            return False
        
        # Update repos
        run_command("helm repo update", check=False)
    
    log_info("✅ Contour Helm repository added")
    return True


def install_contour():
    """Install Contour using Helm."""
    log_info("Installing Contour Ingress Controller via Helm...")
    
    # Check if Contour is already installed
    # Helm uses the current kubeconfig context (should be kind-secret-manager-controller)
    result = run_command(
        "helm list -n projectcontour",
        check=False,
        capture_output=True
    )
    
    release_name = "contour"
    if release_name in result.stdout:
        log_info("Contour is already installed via Helm")
        return True
    
    # Install Contour using Helm
    # Configure Envoy to use hostPort for Kind compatibility
    # This allows Envoy to bind directly to host ports 80/443 via Kind port mappings
    # Note: externalIPs not needed with hostPort - port mappings handle external access
    # Docker bridge IPs (172.19.0.x) are not routable from host anyway
    # Helm uses the current kubeconfig context automatically
    install_cmd = (
        f"helm install {release_name} contour/contour "
        "--namespace projectcontour "
        "--create-namespace "
        "--set envoy.useHostPort.http=true "
        "--set envoy.useHostPort.https=true "
        "--set envoy.hostPorts.http=80 "
        "--set envoy.hostPorts.https=443 "
        "--set envoy.service.type=ClusterIP"
    )
    
    result = run_command(install_cmd, check=False, capture_output=True)
    
    if result.returncode != 0:
        log_error(f"Failed to install Contour via Helm: {result.stderr}")
        if result.stdout:
            log_error(f"Output: {result.stdout}")
        return False
    
    log_info("✅ Contour installed via Helm")
    return True


def wait_for_contour():
    """Wait for Contour to be ready."""
    log_info("Waiting for Contour to be ready...")
    
    max_wait = 120  # Wait up to 2 minutes
    for i in range(max_wait):
        # Check if contour controller pods are ready
        contour_result = run_command(
            "kubectl wait --namespace projectcontour "
            "--for=condition=ready pod "
            "--selector=app.kubernetes.io/name=contour "
            "--timeout=10s --context kind-secret-manager-controller",
            check=False
        )
        
        # Check if envoy pods are ready
        envoy_result = run_command(
            "kubectl wait --namespace projectcontour "
            "--for=condition=ready pod "
            "--selector=app.kubernetes.io/name=envoy "
            "--timeout=10s --context kind-secret-manager-controller",
            check=False
        )
        
        if contour_result.returncode == 0 and envoy_result.returncode == 0:
            log_info("✅ Contour controller and Envoy are ready!")
            return True
        
        if i % 10 == 0 and i > 0:
            log_info(f"Still waiting... ({i}/{max_wait}s)")
        
        time.sleep(1)
    
    log_warn("⚠️  Contour did not become ready within 2 minutes, but installation may have succeeded")
    return False


def check_hosts_file():
    """Check and inform about hosts file configuration for Kind ingress."""
    log_info("Checking hosts file configuration...")
    
    hosts_file = "/etc/hosts"
    if not os.path.exists(hosts_file):
        log_warn(f"⚠️  Hosts file not found: {hosts_file}")
        return
    
    # Read hosts file
    try:
        with open(hosts_file, 'r') as f:
            hosts_content = f.read()
    except PermissionError:
        log_warn("⚠️  Cannot read hosts file (permission denied)")
        log_info("   You may need to manually configure hosts file entries")
        return
    
    # Check for kind.local entries
    # Note: /etc/hosts doesn't support wildcards, so we check for common service entries
    # Services will be accessible via {service_name}.kind.local
    common_services = ["argocd.kind.local"]
    missing_services = []
    
    for service in common_services:
        if service not in hosts_content:
            missing_services.append(service)
    
    if missing_services:
        log_info("")
        log_info("📋 Hosts File Configuration Required:")
        log_info("   /etc/hosts doesn't support wildcards, so add entries for each service:")
        log_info("")
        for service in missing_services:
            log_info(f"   127.0.0.1  {service}")
        log_info("")
        log_info("   On macOS/Linux, run:")
        for service in missing_services:
            log_info(f"   sudo sh -c 'echo \"127.0.0.1  {service}\" >> /etc/hosts'")
        log_info("")
        log_info("   Or add all at once:")
        log_info(f"   sudo sh -c 'echo \"127.0.0.1  {' '.join(missing_services)}\" >> /etc/hosts'")
        log_info("")
        log_info("   For wildcard support, consider using dnsmasq or similar DNS solution.")
    else:
        log_info("✅ Required kind.local hosts file entries are present")


def verify_contour():
    """Verify Contour deployment."""
    log_info("Verifying Contour deployment...")
    
    # Check pods
    result = run_command(
        "kubectl get pods -n projectcontour --context kind-secret-manager-controller",
        check=False
    )
    if result.returncode == 0:
        print(result.stdout)
    
    # Check services
    result = run_command(
        "kubectl get svc -n projectcontour --context kind-secret-manager-controller",
        check=False
    )
    if result.returncode == 0:
        print(result.stdout)
    
    log_info("✅ Contour verification complete")


def main():
    """Main function."""
    log_info("Contour Ingress Controller Setup")
    log_info("=" * 50)
    
    # Check prerequisites
    check_cluster_exists()
    check_helm()
    
    # Check if already installed
    if check_contour_exists():
        log_info("projectcontour namespace already exists")
        # Check if NON_INTERACTIVE mode is set (called from Tilt)
        if os.getenv("NON_INTERACTIVE", "").lower() in ("1", "true", "yes"):
            log_info("Using existing Contour installation")
            verify_contour()
            return
        response = input("Do you want to reinstall? (y/N): ")
        if response.lower() != 'y':
            log_info("Skipping installation")
            verify_contour()
            return
    
    # Add Helm repository
    if not add_contour_repo():
        sys.exit(1)
    
    # Install Contour
    if not install_contour():
        sys.exit(1)
    
    # Wait for readiness
    wait_for_contour()
    
    # Verify
    verify_contour()
    
    # Check hosts file configuration
    check_hosts_file()
    
    log_info("")
    log_info("✅ Contour Ingress Controller setup complete!")
    log_info("")
    log_info("📋 Usage:")
    log_info("   Contour is now available and will route traffic based on HTTPProxy resources")
    log_info("   The Envoy service is exposed via hostPort on ports 80/443")
    log_info("")
    log_info("🌐 Access Services:")
    log_info("   Services are accessible via: http://{service_name}.kind.local")
    log_info("   Example: http://argocd.kind.local")
    log_info("")
    log_info("📚 Documentation: https://projectcontour.io/")


if __name__ == "__main__":
    main()

