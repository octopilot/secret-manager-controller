#!/usr/bin/env python3
"""
Install ArgoCD CRDs in Kubernetes cluster.

This script installs only the ArgoCD CRDs (minimal installation) from the local
pact-broker/argocd directory. The CRDs are used by the secret-manager-controller
to read Application resources. We don't need the full ArgoCD installation since
the controller clones repos itself.
"""

import os
import subprocess
import sys
import time


def run_command(cmd, check=True, capture_output=True):
    """Run a shell command and return the result."""
    result = subprocess.run(
        cmd,
        shell=True,
        capture_output=capture_output,
        text=True
    )
    if check and result.returncode != 0:
        print(f"Error: Command failed: {cmd}", file=sys.stderr)
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


def check_argocd_installed():
    """Check if ArgoCD CRDs are already installed in the cluster."""
    # Check if all required CRDs exist
    required_crds = [
        "applications.argoproj.io",
        "applicationsets.argoproj.io",
        "appprojects.argoproj.io"
    ]
    
    all_installed = True
    for crd in required_crds:
        crd_result = run_command(
            f"kubectl get crd {crd}",
            check=False,
            capture_output=True
        )
        
        if crd_result.returncode != 0:
            all_installed = False
            break
    
    if all_installed:
        log_info("✅ ArgoCD CRDs are already installed")
        return True
    
    return False


def install_argocd():
    """Install ArgoCD CRDs from local pact-broker/argocd directory.
    
    We only need the CRDs since the controller clones repos itself.
    We don't need the full ArgoCD installation (server, controllers, etc.).
    This is much faster than downloading from remote URLs.
    """
    log_info("Installing ArgoCD CRDs (minimal installation)...")
    log_info("Note: Only CRDs are installed, not full ArgoCD")
    log_info("      This is sufficient since the controller clones repos itself")
    
    # Get the script directory to find the CRD directory
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.dirname(os.path.dirname(script_dir))
    crd_dir = os.path.join(project_root, "pact-broker", "argocd")
    
    if not os.path.exists(crd_dir):
        log_error(f"CRD directory not found: {crd_dir}")
        log_error("Please ensure pact-broker/argocd directory exists with CRD files")
        return False
    
    if not os.path.exists(os.path.join(crd_dir, "kustomization.yaml")):
        log_error(f"kustomization.yaml not found in {crd_dir}")
        return False
    
    log_info(f"📦 Applying CRDs from: {crd_dir}")
    
    # Apply CRDs individually (more reliable than kustomize for large CRDs)
    crd_files = [
        "applications.argoproj.io.yaml",
        "applicationsets.argoproj.io.yaml",
        "appprojects.argoproj.io.yaml"
    ]
    
    all_applied = True
    for crd_file in crd_files:
        crd_path = os.path.join(crd_dir, crd_file)
        if not os.path.exists(crd_path):
            log_error(f"CRD file not found: {crd_path}")
            all_applied = False
            continue
        
        log_info(f"Applying {crd_file}...")
        
        # First try apply (for new CRDs)
        result = run_command(
            f"kubectl apply -f {crd_path}",
            check=False,
            capture_output=True
        )
        
        if result.returncode != 0:
            # If it failed because CRD already exists or needs update, use replace --force
            if "already exists" in result.stderr.lower() or "AlreadyExists" in result.stderr or "must be specified for an update" in result.stderr:
                log_info(f"  {crd_file} already exists, replacing...")
                # Use replace --force to update existing CRD without resourceVersion
                result = run_command(
                    f"kubectl replace --force -f {crd_path}",
                    check=False,
                    capture_output=True
                )
                
                if result.returncode != 0:
                    log_error(f"Failed to replace {crd_file}: {result.stderr}")
                    all_applied = False
                else:
                    log_info(f"  ✅ {crd_file} replaced successfully")
            else:
                log_error(f"Failed to install {crd_file}: {result.stderr}")
                all_applied = False
        else:
            log_info(f"  ✅ {crd_file} applied successfully")
    
    if not all_applied:
        return False
    
    log_info("✅ ArgoCD CRD manifests applied")
    log_info("Waiting for CRDs to be established...")
    
    # Wait for all CRDs to be established
    required_crds = [
        "applications.argoproj.io",
        "applicationsets.argoproj.io",
        "appprojects.argoproj.io"
    ]
    
    max_attempts = 30  # Wait up to 1 minute
    for i in range(max_attempts):
        all_established = True
        for crd in required_crds:
            result = run_command(
                f"kubectl wait --for=condition=established crd {crd} --timeout=10s",
                check=False,
                capture_output=True
            )
            
            if result.returncode != 0:
                all_established = False
                break
        
        if all_established:
            log_info("✅ All ArgoCD CRDs are established!")
            break
        
        if i < max_attempts - 1:
            log_info(f"Waiting for CRDs to be established... ({i+1}/{max_attempts})")
            time.sleep(2)
        else:
            log_warn("Some CRDs not established after 60 seconds, but installation may have succeeded")
    
    # Verify all CRDs exist
    all_ready = True
    for crd in required_crds:
        result = run_command(
            f"kubectl get crd {crd}",
            check=False,
            capture_output=True
        )
        
        if result.returncode == 0:
            log_info(f"✅ {crd} is installed and ready")
        else:
            log_warn(f"⚠️  {crd} not found - this may cause issues")
            all_ready = False
    
    return all_ready


def main():
    """Main function."""
    log_info("ArgoCD CRD Installation Script")
    log_info("=" * 50)
    
    # Check if already installed
    is_installed = check_argocd_installed()
    
    if is_installed:
        log_info("")
        log_info("✅ ArgoCD CRD installation check complete!")
        return
    
    # Install ArgoCD CRDs
    if not install_argocd():
        sys.exit(1)
    
    log_info("")
    log_info("✅ ArgoCD CRD installation complete!")
    log_info("📋 Next steps:")
    log_info("  1. Create Application resources in your environment namespaces")
    log_info("  2. Create SecretManagerConfig resources that reference them")
    log_info("  3. Verify Applications are created: kubectl get application -A")
    log_info("")
    log_info("💡 Note: ArgoCD Applications can be created in any namespace")
    log_info("   The secret-manager-controller will clone repositories from Application specs")


if __name__ == "__main__":
    main()

