#!/usr/bin/env python3
"""
Show cluster and controller status.

Displays controller pods, SecretManagerConfig resources, and CRD status.
Replaces embedded shell script in justfile.
"""

import subprocess
import sys


def run_kubectl_command(cmd, fallback_msg):
    """Run a kubectl command and return output or fallback message."""
    try:
        result = subprocess.run(
            cmd,
            shell=True,
            capture_output=True,
            text=True,
            check=False
        )
        if result.returncode == 0 and result.stdout.strip():
            return result.stdout.strip()
        return fallback_msg
    except Exception:
        return fallback_msg


def main():
    """Main status display function."""
    print("📊 Cluster Status:")
    print()
    
    print("📦 Controller Pods:")
    pods_output = run_kubectl_command(
        "kubectl get pods -n octopilot-system -l app=secret-manager-controller 2>/dev/null",
        "No pods found"
    )
    print(pods_output)
    print()
    
    print("📋 SecretManagerConfig Resources:")
    configs_output = run_kubectl_command(
        "kubectl get secretmanagerconfig --all-namespaces 2>/dev/null",
        "No SecretManagerConfig resources found"
    )
    print(configs_output)
    print()
    
    print("🔧 CRD Status:")
    # Try both possible CRD names (old and new)
    crd_output = run_kubectl_command(
        "kubectl get crd secretmanagerconfigs.secret-management.octopilot.io 2>/dev/null",
        ""
    )
    if not crd_output or crd_output == "" or "not found" in crd_output.lower():
        crd_output = run_kubectl_command(
            "kubectl get crd secretmanagerconfigs.secret-management.octopilot.io 2>/dev/null",
            "CRD not found"
        )
    print(crd_output)


if __name__ == "__main__":
    main()

