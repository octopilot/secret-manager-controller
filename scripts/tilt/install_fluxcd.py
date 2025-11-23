#!/usr/bin/env python3
"""
Install FluxCD source-controller and notification-controller in Kubernetes cluster.

This script installs a minimal FluxCD setup:
- All Flux CRDs
- source-controller deployment and all services, RBAC, etc.
- notification-controller deployment and all services, RBAC, etc.

We exclude:
- helm-controller
- kustomize-controller

This provides "enough flux" for GitRepository and notification support.
"""

import subprocess
import sys
import tempfile
import yaml
from pathlib import Path


def run_command(cmd, check=True, capture_output=True, shell=False):
    """Run a shell command and return the result."""
    if isinstance(cmd, str) and not shell:
        cmd = cmd.split()
    
    result = subprocess.run(
        cmd,
        shell=shell,
        capture_output=capture_output,
        text=True
    )
    if check and result.returncode != 0:
        print(f"Error: Command failed: {' '.join(cmd) if isinstance(cmd, list) else cmd}", file=sys.stderr)
        if result.stderr:
            print(result.stderr, file=sys.stderr)
        sys.exit(1)
    return result


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_warn(msg):
    """Print warning message."""
    print(f"[WARN] {msg}", file=sys.stderr)


def log_error(msg):
    """Print error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def check_flux_cli():
    """Check if flux CLI is installed."""
    result = run_command("which flux", check=False, capture_output=True)
    if result.returncode != 0:
        log_error("flux CLI not found - required for installation")
        log_error("  Install with: brew install fluxcd/tap/flux")
        return False
    return True


def check_fluxcd_installed():
    """Check if FluxCD source-controller and notification-controller are already installed."""
    # Check namespace
    ns_check = run_command(
        "kubectl get namespace flux-system",
        check=False,
        capture_output=True
    )
    if ns_check.returncode != 0:
        return False
    
    # Check if source-controller is running
    source_check = run_command(
        "kubectl get pods -n flux-system -l app=source-controller --field-selector=status.phase=Running",
        check=False,
        capture_output=True
    )
    source_running = source_check.returncode == 0 and "source-controller" in source_check.stdout
    
    # Check if notification-controller is running
    notification_check = run_command(
        "kubectl get pods -n flux-system -l app=notification-controller --field-selector=status.phase=Running",
        check=False,
        capture_output=True
    )
    notification_running = notification_check.returncode == 0 and "notification-controller" in notification_check.stdout
    
    if source_running and notification_running:
        log_info("✅ FluxCD source-controller and notification-controller are already installed (running)")
        return True
    
    return False


def filter_flux_manifests(manifests_text):
    """Filter Flux manifests to keep only source-controller and notification-controller.
    
    Removes:
    - helm-controller deployment and related resources
    - kustomize-controller deployment and related resources
    
    Keeps:
    - All CRDs
    - source-controller deployment, service, RBAC, etc.
    - notification-controller deployment, service, RBAC, etc.
    - Shared resources (NetworkPolicy, ResourceQuota, etc.)
    - ClusterRoles/ClusterRoleBindings that reference source or notification controllers
    """
    # Split YAML documents
    documents = []
    current_doc = []
    
    for line in manifests_text.split('\n'):
        if line.strip() == '---':
            if current_doc:
                documents.append('\n'.join(current_doc))
                current_doc = []
        else:
            current_doc.append(line)
    
    if current_doc:
        documents.append('\n'.join(current_doc))
    
    # Filter documents
    filtered_docs = []
    
    for doc in documents:
        if not doc.strip():
            continue
        
        try:
            # Parse YAML to check resource type and name
            data = yaml.safe_load(doc)
            if not data:
                continue
            
            kind = data.get('kind', '')
            metadata = data.get('metadata', {})
            name = metadata.get('name', '')
            labels = metadata.get('labels', {})
            app_label = labels.get('app', '')
            
            # Keep all CRDs
            if kind == 'CustomResourceDefinition':
                filtered_docs.append(doc)
                continue
            
            # Keep namespace
            if kind == 'Namespace' and name == 'flux-system':
                filtered_docs.append(doc)
                continue
            
            # Keep shared resources (NetworkPolicy, ResourceQuota, etc.)
            if kind in ['NetworkPolicy', 'ResourceQuota', 'LimitRange']:
                filtered_docs.append(doc)
                continue
            
            # Skip helm-controller specific resources
            if app_label == 'helm-controller' or name == 'helm-controller':
                continue
            
            # Skip kustomize-controller specific resources
            if app_label == 'kustomize-controller' or name == 'kustomize-controller':
                continue
            
            # Handle ClusterRoleBinding - filter out helm and kustomize controller subjects
            if kind == 'ClusterRoleBinding':
                subjects = data.get('subjects', [])
                
                # Filter out helm and kustomize controller subjects
                filtered_subjects = [
                    sub for sub in subjects
                    if sub.get('name') not in ['helm-controller', 'kustomize-controller']
                    and 'helm-controller' not in str(sub.get('name', ''))
                    and 'kustomize-controller' not in str(sub.get('name', ''))
                ]
                
                # If no subjects remain after filtering, skip this binding entirely
                if not filtered_subjects:
                    continue
                
                # If subjects were filtered, update the document
                if len(filtered_subjects) < len(subjects):
                    data['subjects'] = filtered_subjects
                    # Re-serialize the document
                    import io
                    output = io.StringIO()
                    yaml.dump(data, output, default_flow_style=False, sort_keys=False, allow_unicode=True)
                    doc = output.getvalue()
                
                filtered_docs.append(doc)
                continue
            
            # Handle ClusterRole - check if it's helm or kustomize specific
            if kind == 'ClusterRole':
                # Skip if name contains helm or kustomize (but not if it's a shared role)
                if 'helm-controller' in name.lower() and name != 'flux-edit-flux-system' and name != 'flux-view-flux-system':
                    continue
                if 'kustomize-controller' in name.lower() and name != 'flux-edit-flux-system' and name != 'flux-view-flux-system':
                    continue
                # Keep shared roles and source/notification specific roles
                filtered_docs.append(doc)
                continue
            
            # Keep source-controller resources
            if app_label == 'source-controller' or name.startswith('source-controller'):
                filtered_docs.append(doc)
                continue
            
            # Keep notification-controller resources
            if app_label == 'notification-controller' or name.startswith('notification-controller'):
                filtered_docs.append(doc)
                continue
            
            # Keep ServiceAccount, Role, RoleBinding if they're for source or notification
            if kind in ['ServiceAccount', 'Role', 'RoleBinding']:
                # Check if name or labels indicate source or notification controller
                if 'source-controller' in name or 'notification-controller' in name:
                    filtered_docs.append(doc)
                    continue
                # Skip if it's for helm or kustomize
                if 'helm-controller' in name or 'kustomize-controller' in name:
                    continue
                # Keep others (might be shared)
                filtered_docs.append(doc)
                continue
            
            # Keep Deployment, Service, etc. if they're for source or notification
            if kind in ['Deployment', 'Service', 'ServiceMonitor']:
                if app_label in ['source-controller', 'notification-controller']:
                    filtered_docs.append(doc)
                    continue
                # Skip helm and kustomize
                if app_label in ['helm-controller', 'kustomize-controller']:
                    continue
            
            # Keep other resources by default (ConfigMap, Secret, etc.) unless they're clearly helm/kustomize
            if 'helm-controller' not in name.lower() and 'kustomize-controller' not in name.lower():
                filtered_docs.append(doc)
                continue
            
        except yaml.YAMLError as e:
            log_warn(f"⚠️  Failed to parse YAML document: {e}")
            # Skip unparseable documents
            continue
    
    return '\n---\n'.join(filtered_docs)


def install_fluxcd():
    """Install FluxCD source-controller and notification-controller (minimal installation).
    
    Uses `flux install --export` to get all manifests, then filters to keep only:
    - All CRDs
    - source-controller and notification-controller deployments and related resources
    """
    log_info("Installing FluxCD (minimal: source-controller + notification-controller)...")
    log_info("Note: helm-controller and kustomize-controller are excluded")
    
    # Check if flux CLI is available
    if not check_flux_cli():
        return False
    
    # Get all Flux manifests
    log_info("Exporting Flux manifests...")
    result = run_command(
        ["flux", "install", "--export"],
        check=False,
        capture_output=True
    )
    
    if result.returncode != 0:
        log_error(f"Failed to export Flux manifests: {result.stderr}")
        return False
    
    # Filter manifests to keep only source-controller and notification-controller
    log_info("Filtering manifests (removing helm-controller and kustomize-controller)...")
    filtered_manifests = filter_flux_manifests(result.stdout)
    
    # Write filtered manifests to temporary file
    with tempfile.NamedTemporaryFile(mode='w', suffix='.yaml', delete=False) as tmp_file:
        tmp_file.write(filtered_manifests)
        tmp_path = tmp_file.name
    
    try:
        # Apply filtered manifests
        log_info("Applying filtered Flux manifests...")
        apply_result = run_command(
            ["kubectl", "apply", "-f", tmp_path],
            check=False,
            capture_output=True
        )
        
        if apply_result.returncode != 0:
            log_error(f"Failed to apply Flux manifests: {apply_result.stderr}")
            if apply_result.stdout:
                log_error(f"Output: {apply_result.stdout}")
            return False
        
        log_info("✅ FluxCD manifests applied")
        
        # Wait for source-controller to be ready
        log_info("Waiting for source-controller to be ready...")
        wait_result = run_command(
            "kubectl wait --for=condition=ready pod -l app=source-controller -n flux-system --timeout=120s",
            check=False,
            capture_output=True
        )
        
        if wait_result.returncode == 0:
            log_info("✅ source-controller is ready!")
        else:
            log_warn("⚠️  source-controller not ready after 120 seconds, but installation may have succeeded")
        
        # Wait for notification-controller to be ready
        log_info("Waiting for notification-controller to be ready...")
        wait_result = run_command(
            "kubectl wait --for=condition=ready pod -l app=notification-controller -n flux-system --timeout=120s",
            check=False,
            capture_output=True
        )
        
        if wait_result.returncode == 0:
            log_info("✅ notification-controller is ready!")
        else:
            log_warn("⚠️  notification-controller not ready after 120 seconds, but installation may have succeeded")
        
        # Configure source-controller to watch all namespaces
        log_info("Configuring source-controller to watch all namespaces...")
        result = run_command(
            "kubectl get deployment source-controller -n flux-system -o jsonpath='{.spec.template.spec.containers[0].args}'",
            check=False,
            capture_output=True
        )
        
        if result.returncode == 0 and "--watch-all-namespaces=true" not in result.stdout:
            patch_result = run_command(
                "kubectl patch deployment source-controller -n flux-system --type='json' -p='[{\"op\": \"add\", \"path\": \"/spec/template/spec/containers/0/args/-\", \"value\": \"--watch-all-namespaces=true\"}]'",
                check=False,
                capture_output=True
            )
            
            if patch_result.returncode == 0:
                log_info("✅ Configured source-controller to watch all namespaces")
                log_info("Waiting for source-controller to restart...")
                import time
                time.sleep(5)
                
                # Wait for the new pod to be ready
                for i in range(30):
                    result = run_command(
                        "kubectl wait --for=condition=ready pod -l app=source-controller -n flux-system --timeout=10s",
                        check=False,
                        capture_output=True
                    )
                    if result.returncode == 0:
                        log_info("✅ source-controller restarted and ready with multi-namespace support")
                        break
                    time.sleep(2)
            else:
                log_warn(f"⚠️  Failed to configure source-controller: {patch_result.stderr}")
        else:
            if result.returncode == 0 and "--watch-all-namespaces=true" in result.stdout:
                log_info("✅ source-controller already configured to watch all namespaces")
        
        # Verify CRDs exist
        crds_to_check = [
            "gitrepositories.source.toolkit.fluxcd.io",
            "alerts.notification.toolkit.fluxcd.io",
            "providers.notification.toolkit.fluxcd.io",
        ]
        
        for crd in crds_to_check:
            result = run_command(
                f"kubectl get crd {crd}",
                check=False,
                capture_output=True
            )
            if result.returncode == 0:
                log_info(f"✅ CRD {crd} is installed")
            else:
                log_warn(f"⚠️  CRD {crd} not found")
        
        return True
        
    finally:
        # Clean up temporary file
        try:
            Path(tmp_path).unlink()
        except Exception:
            pass


def main():
    """Main function."""
    log_info("FluxCD Installation Script")
    log_info("=" * 50)
    log_info("Installing minimal FluxCD: source-controller + notification-controller")
    log_info("")
    
    # Check if already installed
    is_installed = check_fluxcd_installed()
    
    if is_installed:
        log_info("FluxCD is already installed. Verifying configuration...")
        # Still configure multi-namespace support if needed
        result = run_command(
            "kubectl get deployment source-controller -n flux-system -o jsonpath='{.spec.template.spec.containers[0].args}'",
            check=False,
            capture_output=True
        )
        if result.returncode == 0 and "--watch-all-namespaces=true" not in result.stdout:
            log_info("Configuring source-controller for multi-namespace support...")
            patch_result = run_command(
                "kubectl patch deployment source-controller -n flux-system --type='json' -p='[{\"op\": \"add\", \"path\": \"/spec/template/spec/containers/0/args/-\", \"value\": \"--watch-all-namespaces=true\"}]'",
                check=False,
                capture_output=True
            )
            if patch_result.returncode == 0:
                log_info("✅ Configured source-controller to watch all namespaces")
            else:
                log_warn(f"⚠️  Failed to configure: {patch_result.stderr}")
        else:
            if result.returncode == 0 and "--watch-all-namespaces=true" in result.stdout:
                log_info("✅ source-controller already configured for multi-namespace")
        log_info("")
        log_info("✅ FluxCD installation check complete!")
        return
    
    # Install FluxCD
    if not install_fluxcd():
        sys.exit(1)
    
    log_info("")
    log_info("✅ FluxCD installation complete!")
    log_info("📋 Installed components:")
    log_info("  - All Flux CRDs")
    log_info("  - source-controller (deployment, service, RBAC)")
    log_info("  - notification-controller (deployment, service, RBAC)")
    log_info("📋 Excluded components:")
    log_info("  - helm-controller")
    log_info("  - kustomize-controller")


if __name__ == "__main__":
    main()
