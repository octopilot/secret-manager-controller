#!/usr/bin/env python3
"""
Troubleshoot docs-site nginx proxy to mock servers.

This script helps diagnose 502 Bad Gateway errors when the docs-site
tries to proxy requests to mock servers.
"""

import subprocess
import sys
import json
from typing import Optional, Dict, List


def run_command(cmd: List[str], check: bool = True) -> subprocess.CompletedProcess:
    """Run a command and return the result."""
    print(f"🔍 Running: {' '.join(cmd)}")
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            check=check
        )
        if result.stdout:
            print(result.stdout)
        return result
    except subprocess.CalledProcessError as e:
        print(f"❌ Error: {e.stderr}")
        if check:
            raise
        return e


def check_pods(namespace: str, label: str) -> bool:
    """Check if pods are running."""
    print(f"\n📦 Checking pods in namespace {namespace} with label {label}")
    result = run_command(
        ["kubectl", "get", "pods", "-n", namespace, "-l", label, "-o", "json"],
        check=False
    )
    
    if result.returncode != 0:
        print(f"❌ Failed to get pods: {result.stderr}")
        return False
    
    try:
        data = json.loads(result.stdout)
        pods = data.get("items", [])
        
        if not pods:
            print(f"❌ No pods found with label {label}")
            return False
        
        all_ready = True
        for pod in pods:
            name = pod["metadata"]["name"]
            status = pod["status"]["phase"]
            containers = pod["status"].get("containerStatuses", [])
            
            ready_containers = sum(1 for c in containers if c.get("ready", False))
            total_containers = len(containers)
            
            print(f"  Pod: {name}")
            print(f"    Status: {status}")
            print(f"    Containers: {ready_containers}/{total_containers} ready")
            
            if status != "Running" or ready_containers != total_containers:
                all_ready = False
                for container in containers:
                    if not container.get("ready", False):
                        state = container.get("state", {})
                        waiting = state.get("waiting", {})
                        if waiting:
                            print(f"    ⚠️  {container['name']}: {waiting.get('reason', 'waiting')}")
        
        return all_ready
    except json.JSONDecodeError:
        print("❌ Failed to parse pod JSON")
        return False


def check_service(namespace: str, service_name: str) -> bool:
    """Check if service exists and has endpoints."""
    print(f"\n🔌 Checking service {service_name} in namespace {namespace}")
    
    # Check service exists
    result = run_command(
        ["kubectl", "get", "svc", "-n", namespace, service_name, "-o", "json"],
        check=False
    )
    
    if result.returncode != 0:
        print(f"❌ Service {service_name} not found: {result.stderr}")
        return False
    
    try:
        data = json.loads(result.stdout)
        spec = data.get("spec", {})
        ports = spec.get("ports", [])
        
        print(f"  Service found")
        for port in ports:
            print(f"    Port: {port.get('port')} -> {port.get('targetPort')}")
        
        # Check endpoints
        endpoints_result = run_command(
            ["kubectl", "get", "endpoints", "-n", namespace, service_name, "-o", "json"],
            check=False
        )
        
        if endpoints_result.returncode == 0:
            endpoints_data = json.loads(endpoints_result.stdout)
            subsets = endpoints_data.get("subsets", [])
            
            if not subsets:
                print(f"  ⚠️  No endpoints found - service has no backing pods")
                return False
            
            total_addresses = sum(len(s.get("addresses", [])) for s in subsets)
            print(f"  ✅ Service has {total_addresses} endpoint(s)")
            return True
        else:
            print(f"  ⚠️  Could not check endpoints")
            return False
            
    except json.JSONDecodeError:
        print("❌ Failed to parse service JSON")
        return False


def check_connectivity(namespace: str, service_name: str, port: int) -> bool:
    """Check if we can connect to the service from within cluster."""
    print(f"\n🌐 Testing connectivity to {service_name}:{port}")
    
    # Try to exec into docs-site pod and test connection
    result = run_command(
        ["kubectl", "get", "pods", "-n", "octopilot-system", "-l", "app=docs-site", "-o", "jsonpath={.items[0].metadata.name}"],
        check=False
    )
    
    if result.returncode != 0 or not result.stdout.strip():
        print("❌ Could not find docs-site pod")
        return False
    
    pod_name = result.stdout.strip()
    print(f"  Using pod: {pod_name}")
    
    # Test DNS resolution
    fqdn = f"{service_name}.{namespace}.svc.cluster.local"
    print(f"  Testing DNS resolution for {fqdn}")
    
    dns_result = run_command(
        ["kubectl", "exec", "-n", "octopilot-system", pod_name, "--", "nslookup", fqdn],
        check=False
    )
    
    if dns_result.returncode != 0:
        print(f"  ❌ DNS resolution failed")
        return False
    
    print(f"  ✅ DNS resolution successful")
    
    # Test HTTP connection
    url = f"http://{fqdn}:{port}/health"
    print(f"  Testing HTTP connection to {url}")
    
    http_result = run_command(
        ["kubectl", "exec", "-n", "octopilot-system", pod_name, "--", "wget", "-q", "-O-", "-T", "5", url],
        check=False
    )
    
    if http_result.returncode == 0:
        print(f"  ✅ HTTP connection successful")
        return True
    else:
        print(f"  ❌ HTTP connection failed: {http_result.stderr}")
        return False


def check_nginx_config(pod_name: str) -> bool:
    """Check nginx configuration in the pod."""
    print(f"\n⚙️  Checking nginx configuration in {pod_name}")
    
    result = run_command(
        ["kubectl", "exec", "-n", "octopilot-system", pod_name, "--", "cat", "/etc/nginx/conf.d/default.conf"],
        check=False
    )
    
    if result.returncode != 0:
        print(f"❌ Failed to read nginx config: {result.stderr}")
        return False
    
    config = result.stdout
    
    # Check for GCP proxy configuration
    if "/api/mock-servers/gcp/" in config:
        print("  ✅ GCP proxy location found")
        
        # Check if using trailing slash (correct pattern)
        if "proxy_pass http://gcp-mock-server" in config and "/;" in config.split("proxy_pass")[1].split("\n")[0]:
            print("  ✅ Using correct proxy_pass pattern with trailing slash")
        else:
            print("  ⚠️  Proxy configuration may need trailing slash")
    else:
        print("  ❌ GCP proxy location not found in config")
        return False
    
    return True


def main():
    """Main troubleshooting function."""
    print("=" * 60)
    print("Docs-Site Proxy Troubleshooting")
    print("=" * 60)
    
    issues = []
    
    # Check mock server pods
    print("\n" + "=" * 60)
    print("STEP 1: Check Mock Server Pods")
    print("=" * 60)
    if not check_pods("secret-manager-controller-pact-broker", "app=pact-infrastructure"):
        issues.append("Mock server pods are not running or not ready")
    
    # Check GCP mock server service
    print("\n" + "=" * 60)
    print("STEP 2: Check GCP Mock Server Service")
    print("=" * 60)
    if not check_service("secret-manager-controller-pact-broker", "gcp-mock-server"):
        issues.append("GCP mock server service is not available")
    
    # Check docs-site pod
    print("\n" + "=" * 60)
    print("STEP 3: Check Docs-Site Pod")
    print("=" * 60)
    result = run_command(
        ["kubectl", "get", "pods", "-n", "octopilot-system", "-l", "app=docs-site", "-o", "jsonpath={.items[0].metadata.name}"],
        check=False
    )
    
    if result.returncode == 0 and result.stdout.strip():
        pod_name = result.stdout.strip()
        print(f"  ✅ Docs-site pod found: {pod_name}")
        
        # Check nginx config
        if not check_nginx_config(pod_name):
            issues.append("Nginx configuration issue detected")
    else:
        issues.append("Docs-site pod not found")
        pod_name = None
    
    # Check connectivity
    if pod_name:
        print("\n" + "=" * 60)
        print("STEP 4: Check Connectivity")
        print("=" * 60)
        if not check_connectivity("secret-manager-controller-pact-broker", "gcp-mock-server", 1234):
            issues.append("Cannot connect to GCP mock server from docs-site pod")
    
    # Summary
    print("\n" + "=" * 60)
    print("SUMMARY")
    print("=" * 60)
    
    if not issues:
        print("✅ All checks passed! The proxy should be working.")
        print("\nIf you're still seeing 502 errors:")
        print("  1. Rebuild the docs-site image: Tilt should auto-rebuild")
        print("  2. Restart the docs-site pod: kubectl delete pod -n octopilot-system -l app=docs-site")
        print("  3. Check nginx logs: kubectl logs -n octopilot-system -l app=docs-site")
    else:
        print("❌ Issues found:")
        for issue in issues:
            print(f"  - {issue}")
        
        print("\nRecommended actions:")
        print("  1. Ensure mock servers are running:")
        print("     kubectl get pods -n secret-manager-controller-pact-broker")
        print("  2. Check mock server logs:")
        print("     kubectl logs -n secret-manager-controller-pact-broker -l app=pact-infrastructure -c gcp-mock-server")
        print("  3. Rebuild docs-site image (Tilt should auto-rebuild on nginx config changes)")
        print("  4. Restart docs-site pod:")
        print("     kubectl delete pod -n octopilot-system -l app=docs-site")
        print("  5. Check nginx error logs:")
        print("     kubectl logs -n octopilot-system -l app=docs-site")
    
    return 0 if not issues else 1


if __name__ == "__main__":
    sys.exit(main())

