#!/usr/bin/env python3
"""
Quick fix script to update containerd registry configuration on existing Kind cluster.

This fixes the issue where containerd tries to resolve 'secret-manager-controller-registry'
as a hostname, which fails. Instead, we use the registry container's IP address.
"""

import subprocess
import sys

REGISTRY_NAME = "secret-manager-controller-registry"


def run_command(cmd, check=False, capture_output=True, input=None):
    """Run a command and return the result."""
    kwargs = {
        'shell': isinstance(cmd, str),
        'capture_output': capture_output,
        'text': True,
        'check': check,
    }
    if input is not None:
        kwargs['input'] = input
    result = subprocess.run(cmd, **kwargs)
    return result


def get_registry_ip():
    """Get the registry container's IP address on the kind network."""
    # Get the registry container's IP on the kind network
    result = run_command(
        f"docker inspect {REGISTRY_NAME} --format='{{{{range $key, $value := .NetworkSettings.Networks}}}}{{{{if eq $key \"kind\"}}}}{{{{.IPAddress}}}}{{{{end}}}}{{{{end}}}}'",
        check=False,
        capture_output=True
    )
    if result.returncode == 0 and result.stdout.strip():
        ip = result.stdout.strip()
        if ip:
            return ip
    
    # Fallback: try to get any IP
    result = run_command(
        f"docker inspect {REGISTRY_NAME} --format='{{{{.NetworkSettings.IPAddress}}}}'",
        check=False,
        capture_output=True
    )
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip()
    
    return None


def main():
    """Main fix function."""
    print("🔧 Fixing registry configuration on Kind cluster...")
    
    # Ensure registry is connected to kind network
    print("📋 Ensuring registry is connected to kind network...")
    result = run_command(
        "docker network inspect kind --format='{{range .Containers}}{{.Name}}{{\"\\n\"}}{{end}}'",
        check=False,
        capture_output=True
    )
    
    if REGISTRY_NAME not in result.stdout:
        print(f"  Connecting {REGISTRY_NAME} to kind network...")
        result = run_command(f"docker network connect kind {REGISTRY_NAME}", check=False)
        if result.returncode != 0:
            print(f"  ❌ Failed to connect registry to kind network: {result.stderr}", file=sys.stderr)
            sys.exit(1)
        print("  ✅ Registry connected to kind network")
    else:
        print("  ✅ Registry already connected to kind network")
    
    # Get registry IP
    registry_ip = get_registry_ip()
    if not registry_ip:
        print("  ❌ Could not determine registry IP address", file=sys.stderr)
        sys.exit(1)
    
    print(f"  📍 Registry IP on kind network: {registry_ip}")
    registry_endpoint = f"http://{registry_ip}:5000"
    
    # Get all node names
    result = run_command("kubectl get nodes -o jsonpath='{.items[*].metadata.name}'", check=True)
    nodes = result.stdout.strip().split()
    
    # Containerd config patch
    containerd_patch = f"""
[plugins."io.containerd.grpc.v1.cri".registry.mirrors."localhost:5000"]
  endpoint = ["{registry_endpoint}"]
"""
    
    # Update containerd config on each node
    for node in nodes:
        print(f"📋 Updating containerd config on node: {node}")
        
        # Read current containerd config
        read_cmd = f"docker exec {node} cat /etc/containerd/config.toml"
        result = run_command(read_cmd, check=False, capture_output=True)
        
        if result.returncode != 0:
            print(f"  ⚠️  Could not read containerd config on {node}, skipping", file=sys.stderr)
            continue
        
        config_content = result.stdout
        
        # Remove existing localhost:5000 mirror config if present
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
            print(f"  ⚠️  Could not write containerd config on {node}", file=sys.stderr)
            continue
        
        # Restart containerd
        print(f"  🔄 Restarting containerd on {node}...")
        result = run_command(f"docker exec {node} systemctl restart containerd", check=False)
        if result.returncode != 0:
            print(f"  ⚠️  Could not restart containerd on {node}", file=sys.stderr)
            continue
        
        print(f"  ✅ Updated containerd config on {node}")
    
    print("\n✅ Registry configuration fixed!")
    print(f"   Registry endpoint: {registry_endpoint}")
    print("   You may need to restart pods to pick up the new configuration.")


if __name__ == "__main__":
    main()

