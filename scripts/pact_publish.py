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
    """Wait for Pact broker pod to be ready."""
    print("Waiting for Pact broker to be ready...")
    cmd = [
        "kubectl", "wait",
        "--for=condition=ready",
        "pod",
        "-l", "app=pact-broker",
        "-n", "secret-manager-controller-pact-broker",
        f"--timeout={timeout}s"
    ]
    try:
        run_command(cmd)
        print("✅ Pact broker is ready")
        return True
    except subprocess.CalledProcessError:
        print("❌ Pact broker failed to become ready")
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


def run_pact_tests() -> int:
    """Run Pact contract tests."""
    print("Running Pact contract tests...")
    cmd = ["cargo", "test", "--test", "pact_*", "--no-fail-fast"]
    try:
        result = run_command(cmd, check=False)
        if result.returncode == 0:
            print("✅ Pact tests passed")
        else:
            print(f"⚠️  Pact tests exited with code {result.returncode}")
        return result.returncode
    except Exception as e:
        print(f"❌ Error running Pact tests: {e}")
        return 1


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
                    "pactfoundation/pact-cli:latest",
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
    
    args = parser.parse_args()
    
    port_forward_process = None
    
    try:
        # Wait for broker to be ready
        if not args.skip_wait:
            if not wait_for_pact_broker():
                return 1
        
        # Set up port forwarding
        if not args.skip_port_forward:
            port_forward_process = setup_port_forward(
                "secret-manager-controller-pact-broker",
                "pact-broker",
                9292,
                9292
            )
            
            if not check_port_forward(args.broker_url, args.username, args.password):
                if port_forward_process:
                    port_forward_process.terminate()
                return 1
        
        # Run Pact tests
        test_exit_code = run_pact_tests()
        
        # Publish Pact files if they exist
        pact_dir = Path(args.pact_dir)
        if pact_dir.exists() and any(pact_dir.glob("*.json")):
            branch, commit = get_git_info()
            if not publish_pact_files(
                pact_dir,
                args.broker_url,
                args.username,
                args.password,
                commit,
                branch
            ):
                return 1
        else:
            print("⏭️  No Pact files found to publish")
        
        return test_exit_code
        
    finally:
        # Clean up port forward
        if port_forward_process:
            print("Cleaning up port forward...")
            port_forward_process.terminate()
            try:
                port_forward_process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                port_forward_process.kill()


if __name__ == "__main__":
    sys.exit(main())

