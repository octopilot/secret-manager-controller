#!/usr/bin/env python3
"""
Pact test runner and publisher.

This script replaces the shell script in Tiltfile to comply with zero shell script policy.
It runs Pact contract tests and publishes the results to the Pact broker.
"""

import argparse
import base64
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import List, Optional, Tuple


def run_command(cmd: List[str], check: bool = True, capture_output: bool = False) -> subprocess.CompletedProcess:
    """Run a shell command and return the result."""
    print(f"Running: {' '.join(cmd)}")
    try:
        result = subprocess.run(
            cmd,
            check=check,
            capture_output=capture_output,
            text=True
        )
        if capture_output and result.stdout:
            print(result.stdout)
        return result
    except subprocess.CalledProcessError as e:
        if capture_output and e.stderr:
            print(f"Error: {e.stderr}", file=sys.stderr)
        raise


def wait_for_pact_broker(timeout: int = 120) -> bool:
    """Wait for Pact infrastructure deployment to be ready."""
    print("Waiting for Pact infrastructure to be ready...")
    namespace = "secret-manager-controller-pact-broker"
    
    # Wait for deployment to be available (at least one replica ready)
    # This is more reliable than waiting for individual pods, especially during rolling updates
    print("Checking deployment status...")
    deployment_cmd = [
        "kubectl", "wait",
        "--for=condition=available",
        "deployment/pact-infrastructure",
        "-n", namespace,
        f"--timeout={timeout}s"
    ]
    try:
        run_command(deployment_cmd, capture_output=True)
        print("✅ Pact infrastructure deployment is available")
        
        # Verify at least one pod is actually ready (deployment condition can be true before pod is ready)
        print("Verifying pod readiness...")
        # Use a simpler approach: get all pods and check their status
        check_cmd = [
            "kubectl", "get", "pods",
            "-l", "app=pact-infrastructure",
            "-n", namespace,
            "-o", "json"
        ]
        check_result = subprocess.run(check_cmd, capture_output=True, text=True, check=False)
        if check_result.returncode == 0:
            try:
                pods_data = json.loads(check_result.stdout)
                ready_pods = []
                for pod in pods_data.get("items", []):
                    phase = pod.get("status", {}).get("phase", "")
                    conditions = pod.get("status", {}).get("conditions", [])
                    ready_condition = next(
                        (c for c in conditions if c.get("type") == "Ready"),
                        None
                    )
                    if phase == "Running" and ready_condition and ready_condition.get("status") == "True":
                        ready_pods.append(pod.get("metadata", {}).get("name", "unknown"))
                
                if ready_pods:
                    print(f"✅ Found {len(ready_pods)} ready pod(s): {', '.join(ready_pods)}")
                    return True
                else:
                    # If no ready pods found, wait a bit more and check again
                    print("⚠️  Deployment available but no ready pods found, waiting a bit more...")
                    time.sleep(5)
                    check_result = subprocess.run(check_cmd, capture_output=True, text=True, check=False)
                    if check_result.returncode == 0:
                        pods_data = json.loads(check_result.stdout)
                        ready_pods = []
                        for pod in pods_data.get("items", []):
                            phase = pod.get("status", {}).get("phase", "")
                            conditions = pod.get("status", {}).get("conditions", [])
                            ready_condition = next(
                                (c for c in conditions if c.get("type") == "Ready"),
                                None
                            )
                            if phase == "Running" and ready_condition and ready_condition.get("status") == "True":
                                ready_pods.append(pod.get("metadata", {}).get("name", "unknown"))
                        if ready_pods:
                            print(f"✅ Found {len(ready_pods)} ready pod(s): {', '.join(ready_pods)}")
                            return True
                    print("❌ No ready pods found after deployment became available")
                    return False
            except json.JSONDecodeError:
                print("⚠️  Failed to parse pod status, assuming deployment is ready")
                return True  # If we can't parse, trust the deployment condition
        else:
            print("⚠️  Failed to check pod status, assuming deployment is ready")
            return True  # If we can't check pods, trust the deployment condition
    except subprocess.CalledProcessError as e:
        print(f"❌ Pact infrastructure deployment failed to become available: {e}")
        return False


def setup_port_forward(namespace: str, service: str, local_port: int, remote_port: int) -> Optional[subprocess.Popen]:
    """Set up port forwarding in the background."""
    print(f"Setting up port forwarding {local_port}:{remote_port}...")
    log_file = open("/tmp/pact-port-forward.log", "w")
    process = subprocess.Popen(
        [
            "kubectl", "port-forward",
            "-n", namespace,
            f"service/{service}",
            f"{local_port}:{remote_port}"
        ],
        stdout=log_file,
        stderr=subprocess.STDOUT
    )
    # Give port forward time to establish
    time.sleep(3)
    return process


def check_port_forward(url: str, username: str, password: str) -> bool:
    """Check if port forward is working."""
    print(f"Checking port forward at {url}...")
    try:
        # Create basic auth header
        credentials = base64.b64encode(f"{username}:{password}".encode()).decode()
        req = urllib.request.Request(url)
        req.add_header("Authorization", f"Basic {credentials}")
        
        # Try to connect with a short timeout
        with urllib.request.urlopen(req, timeout=5) as response:
            if response.status == 200:
                print("✅ Port forward is working")
                return True
            else:
                print(f"⚠️  Port forward check returned status {response.status}")
                return False
    except urllib.error.URLError as e:
        print(f"❌ Port forward check failed: {e}")
        return False
    except Exception as e:
        print(f"❌ Port forward check failed: {e}")
        return False


def check_manager_health(manager_url: str, timeout: int = 30) -> Tuple[bool, dict]:
    """Check the manager's /health endpoint.
    
    Returns (is_ready, health_data) where is_ready is True if broker is healthy and pacts are published.
    """
    print(f"Checking manager health at {manager_url}/health...")
    
    max_attempts = timeout // 2  # Check every 2 seconds
    attempt = 0
    
    while attempt < max_attempts:
        try:
            req = urllib.request.Request(f"{manager_url}/health")
            with urllib.request.urlopen(req, timeout=5) as response:
                if response.status == 200:
                    health_data = json.loads(response.read().decode())
                    broker_healthy = health_data.get("broker_healthy", False)
                    pacts_published = health_data.get("pacts_published", False)
                    status = health_data.get("status", "unknown")
                    
                    print(f"  Manager status: {status}")
                    print(f"  Broker healthy: {broker_healthy}")
                    print(f"  Pacts published: {pacts_published}")
                    
                    # Ready if both broker is healthy and pacts are published
                    is_ready = broker_healthy and pacts_published
                    
                    if is_ready:
                        print("✅ Manager indicates all components are ready and pacts are published")
                        return (True, health_data)
                    else:
                        if attempt % 5 == 0:  # Log every 5 attempts
                            print(f"  ⏳ Waiting for manager to be ready... (attempt {attempt + 1}/{max_attempts})")
                else:
                    if attempt % 5 == 0:
                        print(f"  ⚠️  Manager health check returned status {response.status}")
        except urllib.error.URLError as e:
            if attempt % 5 == 0:
                print(f"  ⏳ Manager not yet accessible: {e} (attempt {attempt + 1}/{max_attempts})")
        except Exception as e:
            if attempt % 5 == 0:
                print(f"  ⚠️  Error checking manager health: {e} (attempt {attempt + 1}/{max_attempts})")
        
        attempt += 1
        if attempt < max_attempts:
            time.sleep(2)
    
    print(f"❌ Manager health check timed out after {timeout} seconds")
    return (False, {})


def check_pacts_published(broker_url: str, username: str, password: str) -> bool:
    """Check if pacts are published in the broker.
    
    Returns True if at least one pact is found, False otherwise.
    """
    print("Checking if pacts are published in the broker...")
    
    # List of providers we expect to have pacts
    providers = [
        "GCP-Secret-Manager",
        "AWS-Secrets-Manager",
        "AWS-Parameter-Store",
        "Azure-Key-Vault",
        "Azure-App-Configuration",
        "GCP-Parameter-Manager",
    ]
    consumer = "Secret-Manager-Controller"
    
    found_pacts = 0
    
    for provider in providers:
        # Check for latest pact between consumer and provider
        # URL format: /pacts/provider/{provider}/consumer/{consumer}/latest
        url = f"{broker_url}/pacts/provider/{provider}/consumer/{consumer}/latest"
        
        try:
            # Create basic auth header
            credentials = base64.b64encode(f"{username}:{password}".encode()).decode()
            req = urllib.request.Request(url)
            req.add_header("Authorization", f"Basic {credentials}")
            
            # Try to fetch the pact
            with urllib.request.urlopen(req, timeout=10) as response:
                if response.status == 200:
                    found_pacts += 1
                    print(f"  ✅ Found pact for {provider}")
                else:
                    print(f"  ⏭️  No pact found for {provider} (status: {response.status})")
        except urllib.error.HTTPError as e:
            if e.code == 404:
                print(f"  ⏭️  No pact found for {provider}")
            else:
                print(f"  ⚠️  Error checking {provider}: {e.code}")
        except Exception as e:
            print(f"  ⚠️  Error checking {provider}: {e}")
    
    if found_pacts > 0:
        print(f"✅ Found {found_pacts} published pact(s) in the broker")
        return True
    else:
        print("❌ No pacts found in the broker")
        return False


def run_pact_tests() -> int:
    """Run Pact contract tests."""
    print("Running Pact contract tests...")
    # Run tests sequentially to avoid environment variable conflicts
    # Integration tests share environment variables (PACT_MODE, endpoint URLs, etc.)
    # and must run one at a time to prevent interference
    # Cargo test --test doesn't support glob patterns, so we need to run each test file individually
    pact_test_files = [
        "pact_aws_parameter_store",
        "pact_aws_secrets_manager",
        "pact_azure_app_configuration",
        "pact_azure_key_vault",
        "pact_gcp_parameter_manager",
        "pact_gcp_secret_manager",
        "pact_provider_integration_aws",
        "pact_provider_integration_azure",
        "pact_provider_integration_gcp",
    ]
    
    failed_tests = []
    for test_file in pact_test_files:
        print(f"\n📋 Running {test_file} tests...")
        cmd = ["cargo", "test", "--test", test_file, "--no-fail-fast", "--", "--test-threads=1"]
        try:
            result = run_command(cmd, check=False)
            if result.returncode != 0:
                print(f"⚠️  {test_file} tests failed with exit code {result.returncode}")
                failed_tests.append(test_file)
            else:
                print(f"✅ {test_file} tests passed")
        except Exception as e:
            print(f"❌ Error running {test_file} tests: {e}")
            failed_tests.append(test_file)
    
    if failed_tests:
        print(f"\n❌ {len(failed_tests)} test file(s) failed: {', '.join(failed_tests)}")
        return 1
    else:
        print("\n✅ All Pact tests passed")
        return 0


def get_git_info() -> Tuple[str, str]:
    """Get git branch and commit hash."""
    try:
        branch_result = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            capture_output=True,
            text=True,
            check=False
        )
        branch = branch_result.stdout.strip() if branch_result.returncode == 0 else "main"
        
        commit_result = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            capture_output=True,
            text=True,
            check=False
        )
        commit = commit_result.stdout.strip() if commit_result.returncode == 0 else "dev"
        
        return branch, commit
    except Exception:
        return "main", "dev"


def find_pact_files(pact_dir: Path, provider_name: str) -> List[Path]:
    """Find Pact files for a specific provider."""
    pattern = f"*{provider_name}*.json"
    return list(pact_dir.glob(pattern))


def publish_pact_files(
    pact_dir: Path,
    broker_url: str,
    username: str,
    password: str,
    version: str,
    branch: str
) -> bool:
    """Publish Pact files to the broker."""
    providers = ["gcp", "aws", "aws-parameter-store", "azure", "azure-app-configuration"]
    provider_names = ["GCP-Secret-Manager", "AWS-Secrets-Manager", "AWS-Parameter-Store", "Azure-Key-Vault", "Azure-App-Configuration"]
    
    timestamp = int(time.time())
    
    for provider, provider_name in zip(providers, provider_names):
        pact_files = find_pact_files(pact_dir, provider_name)
        
        if not pact_files:
            print(f"⏭️  No Pact files found for {provider_name}")
            continue
        
        print(f"📦 Publishing Pact files for {provider_name}...")
        
        # Create temporary directory for this provider's pacts
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            
            # Copy pact files to temp directory
            for pact_file in pact_files:
                shutil.copy2(pact_file, temp_path / pact_file.name)
            
            # Create provider-specific version
            provider_version = f"{provider}-{branch}-{version}-{timestamp}"
            
            # Check if pact-broker CLI is available
            pact_cli_available = shutil.which("pact-broker") is not None
            
            if pact_cli_available:
                # Use local Pact CLI
                cmd = [
                    "pact-broker", "publish",
                    str(temp_path),
                    "--consumer-app-version", provider_version,
                    "--branch", branch,
                    "--broker-base-url", broker_url,
                    "--broker-username", username,
                    "--broker-password", password
                ]
                try:
                    run_command(cmd)
                    print(f"✅ Published {provider_name} contracts")
                except subprocess.CalledProcessError as e:
                    print(f"❌ Failed to publish {provider_name} contracts: {e}")
                    return False
            else:
                # Use Docker image for Pact CLI
                # Determine if we're on Linux (need --network host) or macOS/Windows (use host.docker.internal)
                is_linux = platform.system() == "Linux"
                
                temp_dir_base = temp_path.name
                docker_broker_url = broker_url.replace("localhost", "host.docker.internal") if not is_linux else broker_url
                
                cmd = [
                    "docker", "run", "--rm",
                ]
                
                if is_linux:
                    # On Linux, use --network host to access localhost
                    cmd.extend(["--network", "host"])
                else:
                    # On macOS/Windows, use host.docker.internal
                    cmd.extend(["--add-host=host.docker.internal:host-gateway"])
                
                cmd.extend([
                    "-v", f"{temp_path}:/pacts/{temp_dir_base}",
                    "-w", f"/pacts/{temp_dir_base}",
                    "docker.io/microscaler/pact-cli:latest",
                    "publish", ".",
                    "--consumer-app-version", provider_version,
                    "--branch", branch,
                    "--broker-base-url", docker_broker_url,
                    "--broker-username", username,
                    "--broker-password", password
                ])
                try:
                    run_command(cmd)
                    print(f"✅ Published {provider_name} contracts")
                except subprocess.CalledProcessError as e:
                    print(f"❌ Failed to publish {provider_name} contracts: {e}")
                    if e.stderr:
                        print(f"Error details: {e.stderr}", file=sys.stderr)
                    return False
    
    return True


def main() -> int:
    """Main entry point."""
    parser = argparse.ArgumentParser(description="Run Pact tests and publish contracts")
    parser.add_argument(
        "--broker-url",
        default="http://localhost:9292",
        help="Pact broker URL (default: http://localhost:9292)"
    )
    parser.add_argument(
        "--username",
        default="pact",
        help="Pact broker username (default: pact)"
    )
    parser.add_argument(
        "--password",
        default="pact",
        help="Pact broker password (default: pact)"
    )
    parser.add_argument(
        "--pact-dir",
        default="target/pacts",
        help="Directory containing Pact files (default: target/pacts)"
    )
    parser.add_argument(
        "--skip-wait",
        action="store_true",
        help="Skip waiting for broker to be ready"
    )
    parser.add_argument(
        "--skip-port-forward",
        action="store_true",
        help="Skip port forwarding setup (assumes broker is accessible)"
    )
    parser.add_argument(
        "--allow-test-failures",
        action="store_true",
        help="Allow publishing even if tests fail (useful when fixing tests)"
    )
    parser.add_argument(
        "--manager-port",
        type=int,
        default=8081,
        help="Manager health port (default: 8081)"
    )
    parser.add_argument(
        "--manager-timeout",
        type=int,
        default=120,
        help="Timeout in seconds for waiting for manager to be ready (default: 120)"
    )
    
    args = parser.parse_args()
    
    port_forward_process = None
    
    try:
        # Wait for broker to be ready
        if not args.skip_wait:
            if not wait_for_pact_broker():
                return 1
        
        # Set up port forwarding for broker and manager
        manager_port_forward = None
        if not args.skip_port_forward:
            # Port forward for Pact broker
            broker_port_forward = setup_port_forward(
                "secret-manager-controller-pact-broker",
                "pact-broker",
                9292,
                9292
            )
            
            if not check_port_forward(args.broker_url, args.username, args.password):
                if broker_port_forward:
                    broker_port_forward.terminate()
                return 1
            
            # Port forward for manager health endpoint
            # Manager is in the same pod, so we can port-forward to the pod directly
            print("Setting up port forwarding for manager health endpoint...")
            namespace = "secret-manager-controller-pact-broker"
            
            # Get the pod name for the manager
            get_pod_cmd = [
                "kubectl", "get", "pods",
                "-l", "app=pact-infrastructure",
                "-n", namespace,
                "-o", "jsonpath={.items[0].metadata.name}"
            ]
            pod_result = subprocess.run(get_pod_cmd, capture_output=True, text=True, check=False)
            if pod_result.returncode == 0 and pod_result.stdout.strip():
                pod_name = pod_result.stdout.strip()
                manager_port_forward = setup_port_forward(
                    namespace,
                    pod_name,  # Port-forward to pod directly
                    args.manager_port,
                    8081  # Manager's health port
                )
                port_forward_process = broker_port_forward  # Keep broker port forward for cleanup
            else:
                print("⚠️  Could not find pact-infrastructure pod for manager port-forward")
                print("   Will try to connect to manager via service if available")
                manager_port_forward = None
                port_forward_process = broker_port_forward
        
        # Check manager health (this checks broker health and pacts published)
        print("\n" + "=" * 60)
        print("Checking manager health status...")
        print("=" * 60)
        
        manager_url = f"http://localhost:{args.manager_port}"
        manager_ready, health_data = check_manager_health(manager_url, timeout=args.manager_timeout)
        
        if not manager_ready:
            print("\n" + "=" * 60)
            print("❌ Manager indicates components are not ready")
            print("=" * 60)
            broker_healthy = health_data.get("broker_healthy", False)
            pacts_published = health_data.get("pacts_published", False)
            
            if not broker_healthy:
                print("\n⚠️  Broker is not healthy yet.")
                print("   The pact-infrastructure deployment may still be initializing.")
            if not pacts_published:
                print("\n⚠️  Pacts are not yet published.")
                print("   The manager may still be publishing contracts.")
            
            print("\nWait for the manager to indicate readiness, then run this script again.")
            print("\nTo check manager status:")
            print("  kubectl get deployment pact-infrastructure -n secret-manager-controller-pact-broker")
            print("  kubectl logs deployment/pact-infrastructure -c manager -n secret-manager-controller-pact-broker")
            print(f"  curl http://localhost:{args.manager_port}/health")
            if port_forward_process:
                port_forward_process.terminate()
            if manager_port_forward:
                manager_port_forward.terminate()
            return 1
        
        print("\n" + "=" * 60)
        print("✅ Manager indicates all components are ready and pacts are published")
        print("=" * 60 + "\n")
        
        # Run Pact tests
        test_exit_code = run_pact_tests()
        
        # Publish Pact files if they exist
        pact_dir = Path(args.pact_dir)
        publish_success = True
        if pact_dir.exists() and any(pact_dir.glob("*.json")):
            branch, commit = get_git_info()
            publish_success = publish_pact_files(
                pact_dir,
                args.broker_url,
                args.username,
                args.password,
                commit,
                branch
            )
            if not publish_success:
                return 1
        else:
            print("⏭️  No Pact files found to publish")
        
        # Return test exit code, unless --allow-test-failures is set
        if args.allow_test_failures:
            if test_exit_code != 0:
                print(f"⚠️  Tests failed (exit code {test_exit_code}) but --allow-test-failures is set, continuing...")
            return 0 if publish_success else 1
        else:
            return test_exit_code
        
    finally:
        # Clean up port forwards
        if port_forward_process:
            print("Cleaning up port forwards...")
            port_forward_process.terminate()
            try:
                port_forward_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                port_forward_process.kill()
        if 'manager_port_forward' in locals() and manager_port_forward:
            manager_port_forward.terminate()
            try:
                manager_port_forward.wait(timeout=5)
            except subprocess.TimeoutExpired:
                manager_port_forward.kill()


if __name__ == "__main__":
    sys.exit(main())

