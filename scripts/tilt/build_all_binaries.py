#!/usr/bin/env python3
"""
Build all Rust binaries for Tilt development.

Builds all binaries (controller, mock servers, webhook) in a single cargo build.
This is more efficient than building each binary separately.
"""

import os
import platform
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path


def run_command(cmd, check=True, capture_output=True, env=None):
    """Run a command and return the result."""
    result = subprocess.run(
        cmd,
        shell=True,
        capture_output=capture_output,
        text=True,
        env=env
    )
    if capture_output:
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr and result.returncode != 0:
            print(result.stderr, end="", file=sys.stderr)
    if check and result.returncode != 0:
        sys.exit(result.returncode)
    return result


def get_git_hash():
    """Get git hash for build info."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            capture_output=True,
            text=True,
            check=True
        )
        git_hash = result.stdout.strip()
        
        # Check if git is dirty
        diff_result = subprocess.run(
            ["git", "diff", "--quiet"],
            capture_output=True
        )
        dirty_suffix = "-dirty" if diff_result.returncode != 0 else ""
        return f"{git_hash}{dirty_suffix}"
    except Exception:
        return "unknown"


def main():
    """Build all binaries."""
    print("🔨 Building all Rust binaries...")
    
    # Generate build info (required by build.rs)
    build_timestamp = str(int(time.time()))
    build_datetime = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    build_git_hash = get_git_hash()
    
    print(f"📋 Build info:")
    print(f"  Timestamp: {build_timestamp}")
    print(f"  DateTime: {build_datetime}")
    print(f"  Git Hash: {build_git_hash}")
    
    os_name = platform.system()
    arch = platform.machine()
    
    target = "x86_64-unknown-linux-musl"
    
    # Set up build environment with build info
    build_env = os.environ.copy()
    build_env["BUILD_TIMESTAMP"] = build_timestamp
    build_env["BUILD_DATETIME"] = build_datetime
    build_env["BUILD_GIT_HASH"] = build_git_hash
    
    # Build all binaries in one go using workspace
    # Add strip flags to reduce binary size (debug builds are still large)
    print("🔨 Building workspace binaries (debug mode with size optimization)...")
    
    # Add RUSTFLAGS for stripping debug symbols (reduces binary size)
    build_env["RUSTFLAGS"] = "-C link-arg=-s"
    
    if os_name == "Darwin":
        # macOS: Use cargo zigbuild (like microservices)
        print("  Using cargo-zigbuild for cross-compilation (macOS)")
        result = run_command(
            f"cargo zigbuild --target {target} --workspace --bins",
            check=False,
            env=build_env
        )
        if result.returncode != 0:
            print("❌ Build failed", file=sys.stderr)
            sys.exit(1)
    elif os_name == "Linux" and arch == "x86_64":
        # Linux x86_64: Use musl-gcc linker
        print("  Using musl-gcc linker (Linux x86_64)")
        build_env["CC_x86_64_unknown_linux_musl"] = "musl-gcc"
        build_env["CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER"] = "musl-gcc"
        result = subprocess.run(
            ["cargo", "build", "--target", target, "--workspace", "--bins"],
            env=build_env,
            capture_output=True,
            text=True
        )
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr:
            print(result.stderr, end="", file=sys.stderr)
        if result.returncode != 0:
            print("❌ Build failed", file=sys.stderr)
            sys.exit(1)
    else:
        # Fallback: Try regular cargo build
        print(f"  Using standard cargo build (OS: {os_name}, Arch: {arch})")
        result = run_command(
            f"cargo build --target {target} --workspace --bins",
            check=False,
            env=build_env
        )
        if result.returncode != 0:
            print("❌ Build failed", file=sys.stderr)
            sys.exit(1)
    
    # Verify binaries exist and define binary list
    target_dir = Path(f"target/{target}/debug")
    binaries = [
        "secret-manager-controller",
        "crdgen",
        "msmctl",
        "gcp-mock-server",
        "aws-mock-server",
        "azure-mock-server",
        "webhook",
        "manager",
        "postgres-manager",
    ]
    
    # Strip binaries to further reduce size
    # Note: For cross-compiled binaries (Linux from macOS), the macOS strip tool can't process them
    # RUSTFLAGS="-C link-arg=-s" already strips during build, so manual strip is optional
    # Only attempt manual strip for native builds (not cross-compiled)
    os_name = platform.system()
    is_cross_compile = target != f"{arch}-unknown-{os_name.lower()}-{('gnu' if os_name == 'Linux' else 'darwin')}"
    
    if not is_cross_compile:
        print("📦 Stripping binaries to reduce size...")
        strip_cmd = shutil.which("strip")
        if strip_cmd:
            for binary in binaries:
                binary_path = target_dir / binary
                if binary_path.exists():
                    result = subprocess.run(
                        [strip_cmd, str(binary_path)],
                        check=False,
                        capture_output=True,
                        text=True
                    )
                    if result.returncode == 0:
                        print(f"  ✅ Stripped {binary}")
                    # Silently ignore errors (binary may already be stripped or incompatible)
        else:
            print("  ⚠️  strip command not found, skipping (binaries already optimized with RUSTFLAGS)")
    else:
        print("📦 Skipping manual strip (cross-compiled binaries - RUSTFLAGS already handles stripping)")
    
    print("🔍 Verifying binaries were built...")
    all_exist = True
    for binary in binaries:
        binary_path = target_dir / binary
        if binary_path.exists():
            size = binary_path.stat().st_size
            print(f"  ✅ {binary}: {size:,} bytes")
        else:
            print(f"  ❌ {binary}: NOT FOUND", file=sys.stderr)
            all_exist = False
    
    if not all_exist:
        print("❌ Build failed: Some binaries not found", file=sys.stderr)
        sys.exit(1)
    
    print("✅ All binaries built successfully!")
    
    # Generate and apply CRD
    print("📋 Generating SecretManagerConfig CRD...")
    crd_output_path = Path("config/crd/secretmanagerconfig.yaml")
    crd_output_path.parent.mkdir(parents=True, exist_ok=True)
    
    # Determine which crdgen binary to use based on platform
    # On Linux x86_64 (CI), use the cross-compiled binary we just built
    # On macOS/other platforms, prefer native build but fallback to cross-compiled
    os_name = platform.system()
    arch = platform.machine()
    
    if os_name == "Linux" and arch == "x86_64":
        # CI/Linux: Use the cross-compiled binary we just built
        crdgen_path = target_dir / "crdgen"
        print(f"  Using cross-compiled crdgen for Linux x86_64: {crdgen_path}")
    else:
        # macOS/other: Try native first, then fallback to cross-compiled
        native_crdgen = Path("target/debug/crdgen")
        if native_crdgen.exists():
            crdgen_path = native_crdgen
            print(f"  Using native crdgen: {crdgen_path}")
        else:
            # Fallback to cross-compiled
            crdgen_path = target_dir / "crdgen"
            print(f"  Using cross-compiled crdgen: {crdgen_path}")
    
    if not crdgen_path.exists():
        # If cross-compiled doesn't exist and we're not on Linux, try building native
        if os_name != "Linux" or arch != "x86_64":
            print("⚠️  crdgen not found, building native version...")
            build_result = run_command(
                "cargo build -p controller --bin crdgen",
                check=False
            )
            if build_result.returncode == 0:
                native_crdgen = Path("target/debug/crdgen")
                if native_crdgen.exists():
                    crdgen_path = native_crdgen
                    print(f"  Using newly built native crdgen: {crdgen_path}")
                else:
                    print("❌ crdgen binary not found after build", file=sys.stderr)
                    sys.exit(1)
            else:
                print("❌ Failed to build native crdgen", file=sys.stderr)
                sys.exit(1)
        else:
            print(f"❌ crdgen binary not found at {crdgen_path}", file=sys.stderr)
            sys.exit(1)
    
    print(f"  Running crdgen: {crdgen_path}")
    result = run_command(
        f"{crdgen_path} > {crd_output_path}",
        check=False
    )
    
    if result.returncode != 0:
        print("❌ Failed to generate CRD", file=sys.stderr)
        sys.exit(1)
    
    print(f"✅ CRD generated: {crd_output_path}")
    
    # Apply CRD to cluster
    print("📤 Applying CRD to cluster...")
    
    # Check if cluster is accessible
    cluster_check = run_command(
        "kubectl cluster-info --request-timeout=5s",
        check=False,
        capture_output=True
    )
    
    if cluster_check.returncode != 0:
        print("⚠️  Cluster not accessible - skipping CRD apply", file=sys.stderr)
        print("   CRD file generated but not applied. Apply manually when cluster is ready:", file=sys.stderr)
        print(f"   kubectl apply -f {crd_output_path}", file=sys.stderr)
        return
    
    # Apply/update CRD (idempotent - updates if changed, no-op if same)
    # Note: CRD may already be installed from cluster setup (setup_kind.py)
    # This ensures we have the latest version if the code has changed
    apply_result = run_command(
        f"kubectl apply -f {crd_output_path}",
        check=False,
        capture_output=True
    )
    
    if apply_result.returncode != 0:
        # Try with --validate=false as fallback (for cases where cluster is starting up)
        print("  ⚠️  Standard apply failed, trying with --validate=false...")
        apply_result = run_command(
            f"kubectl apply -f {crd_output_path} --validate=false",
            check=False,
            capture_output=True
        )
        
        if apply_result.returncode != 0:
            print("❌ Failed to apply CRD", file=sys.stderr)
            if apply_result.stderr:
                print(apply_result.stderr, file=sys.stderr)
            sys.exit(1)
        else:
            print("✅ CRD applied (with --validate=false)")
    else:
        print("✅ CRD applied successfully")
    
    # Wait for CRD to be established before continuing
    # This prevents "no matches for kind" errors when resources are applied too quickly
    print("⏳ Waiting for CRD to be established...")
    crd_name = "secretmanagerconfigs.secret-management.octopilot.io"
    max_attempts = 30  # Wait up to 1 minute
    
    for i in range(max_attempts):
        wait_result = run_command(
            f"kubectl wait --for=condition=established crd {crd_name} --timeout=2s",
            check=False,
            capture_output=True
        )
        
        if wait_result.returncode == 0:
            print("✅ CRD is established and ready")
            break
        
        if i < max_attempts - 1:
            time.sleep(2)
    else:
        print("⚠️  Warning: CRD not established after 60 seconds, but continuing anyway", file=sys.stderr)
        print("   Resources may fail to apply if CRD is not ready", file=sys.stderr)


if __name__ == "__main__":
    main()

