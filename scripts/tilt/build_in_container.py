#!/usr/bin/env python3
"""
Container-based Rust workspace builder for Tilt local development.

Replaces the host-side `cargo zigbuild` cross-compilation approach with a
build that runs inside the official Rust Docker image.  This means:

  - No Rust toolchain required on the developer's macOS/host.
  - Builds are genuinely Linux x86_64 — no cross-compilation surprises.
  - The Cargo registry cache lives in a named Docker volume (fast after
    the first build regardless of macOS filesystem virtualisation).
  - The `target/` directory is a bind-mounted host path so that the
    compiled binaries are accessible to the dev Dockerfiles and to the
    host-side CRD application step.

Volume layout
─────────────
  smc-cargo-registry  (named volume)  → /root/.cargo/registry
  smc-cargo-git       (named volume)  → /root/.cargo/git
  $(pwd)/target       (bind mount)    → /workspace/target

Named volumes stay inside Docker's VM and are NOT accessible from the
macOS host.  The target/ bind-mount IS accessible so dev Dockerfiles
can COPY from it and kubectl can apply the generated CRD.

Target: x86_64-unknown-linux-musl
──────────────────────────────────
We keep the musl target (static linking) for consistency with the
existing dev Dockerfiles and copy_mock_server_binaries.py.  The musl
toolchain is installed once inside the builder image on first run and
cached via the named volumes.

Usage (from repo root):
  python3 scripts/tilt/build_in_container.py [--release] [--skip-crd]
"""

import argparse
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path


# ── Configuration ─────────────────────────────────────────────────────────────

# Rust builder image used for container-based compilation.
#
# Default: official rust:stable-bookworm — always has the current stable Rust
#   release so edition2024 (stabilised in 1.85) and future features work.
#   musl-tools and the musl Rust target are installed at runtime (see below).
#
# Override: set OP_RUST_BUILDER to use the octopilot pre-baked base image once
#   it has been rebuilt against rust:stable and includes musl-tools.
#   e.g.  OP_RUST_BUILDER=ghcr.io/octopilot/rust-builder-base-image:latest
#
# Why not default to the octopilot image?
#   The published ghcr.io/octopilot/rust-builder-base-image:latest was built
#   against rust:1.82 which predates edition2024 support (requires 1.85+).
#   Until that image is rebuilt and pushed, the official image is the safe default.
RUST_BUILDER_IMAGE = os.environ.get(
    "OP_RUST_BUILDER",
    "rust:stable-bookworm",
)

# Cargo target — musl for static binaries compatible with alpine-based dev images.
CARGO_TARGET = "x86_64-unknown-linux-musl"

# Named volumes for the Cargo caches (survive container restarts, stay in Docker VM).
CARGO_REGISTRY_VOLUME = "smc-cargo-registry"
CARGO_GIT_VOLUME = "smc-cargo-git"

# Binaries that build_all_binaries used to produce.
CONTROLLER_BINARIES = [
    "secret-manager-controller",
    "crdgen",
    "msmctl",
]

PACT_BINARIES = [
    "gcp-mock-server",
    "aws-mock-server",
    "azure-mock-server",
    "webhook",
    "manager",
    "postgres-manager",
]


# ── Helpers ───────────────────────────────────────────────────────────────────

def run(cmd: str, *, check: bool = True) -> subprocess.CompletedProcess:
    """Run a shell command, streaming output to stdout/stderr."""
    result = subprocess.run(cmd, shell=True, text=True)
    if check and result.returncode != 0:
        print(f"❌ Command failed (exit {result.returncode}): {cmd}", file=sys.stderr)
        sys.exit(result.returncode)
    return result


def ensure_volumes() -> None:
    """Create named Docker volumes if they do not already exist."""
    for vol in (CARGO_REGISTRY_VOLUME, CARGO_GIT_VOLUME):
        result = subprocess.run(
            f"docker volume inspect {vol}",
            shell=True, capture_output=True,
        )
        if result.returncode != 0:
            print(f"📦 Creating Docker volume '{vol}'...")
            run(f"docker volume create {vol}")


def get_build_info() -> dict:
    """Collect build metadata injected into the binary via build.rs."""
    timestamp = str(int(time.time()))
    dt = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    try:
        git_hash = subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"], text=True
        ).strip()
        dirty = subprocess.run(["git", "diff", "--quiet"], capture_output=True)
        if dirty.returncode != 0:
            git_hash += "-dirty"
    except Exception:
        git_hash = "unknown"
    return {"timestamp": timestamp, "datetime": dt, "git_hash": git_hash}


# ── Main ──────────────────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release", action="store_true",
                        help="Build in release mode (slower, optimised)")
    parser.add_argument("--skip-crd", action="store_true",
                        help="Skip CRD generation and kubectl apply")
    parser.add_argument("--skip-apply", action="store_true",
                        help="Generate CRD but do not kubectl apply")
    parser.add_argument("--purge-cache", action="store_true",
                        help=(
                            "Delete the smc-cargo-registry and smc-cargo-git "
                            "Docker volumes before building.  Use this when the "
                            "cached dependency index becomes incompatible with a "
                            "newer Rust toolchain (e.g. after upgrading the "
                            "builder image from rust:1.82 to rust:stable)."
                        ))
    args = parser.parse_args()

    if args.purge_cache:
        print("🗑  Purging Cargo cache volumes...")
        for vol in (CARGO_REGISTRY_VOLUME, CARGO_GIT_VOLUME):
            result = subprocess.run(
                f"docker volume rm {vol}",
                shell=True, capture_output=True, text=True,
            )
            if result.returncode == 0:
                print(f"  ✅ Removed volume '{vol}'")
            else:
                print(f"  ⚠️  Volume '{vol}' not found or already removed")

    workspace = Path.cwd()
    profile = "release" if args.release else "debug"
    target_dir = workspace / "target" / CARGO_TARGET / profile

    info = get_build_info()
    print("🔨 Building Rust workspace binaries in container...")
    print(f"  Image:     {RUST_BUILDER_IMAGE}")
    print(f"  Target:    {CARGO_TARGET}")
    print(f"  Profile:   {profile}")
    print(f"  Git hash:  {info['git_hash']}")

    # ── Create named volumes ───────────────────────────────────────────────
    ensure_volumes()

    # ── Cargo build inside Linux container ───────────────────────────────
    # Named volumes for the Cargo cache stay inside Docker and are fast.
    # The workspace (including target/) is a bind mount so compiled
    # binaries are visible on the host.
    profile_flag = "--release" if args.release else ""

    build_cmd = (
        f"docker run --rm "
        f"-v '{workspace}:/workspace' "
        f"-w /workspace "
        f"-v {CARGO_REGISTRY_VOLUME}:/root/.cargo/registry "
        f"-v {CARGO_GIT_VOLUME}:/root/.cargo/git "
        # Pass build metadata to build.rs via environment
        f"-e BUILD_TIMESTAMP='{info['timestamp']}' "
        f"-e BUILD_DATETIME='{info['datetime']}' "
        f"-e BUILD_GIT_HASH='{info['git_hash']}' "
        f"-e CARGO_NET_GIT_FETCH_WITH_CLI=true "
        f"{RUST_BUILDER_IMAGE} "
        f"sh -c '"
        # Install musl-tools and add the musl Rust target.
        # These are idempotent: if OP_RUST_BUILDER points to the octopilot base
        # image (which pre-installs them), apt-get and rustup skip silently.
        # With the default rust:stable-bookworm image they are installed fresh.
        # apt-get is run quietly (-qq) so it doesn't flood the build log.
        f"apt-get update -qq && "
        f"apt-get install -y --no-install-recommends musl-tools -qq 2>/dev/null && "
        f"rustup target add {CARGO_TARGET} 2>/dev/null && "
        f"cargo build {profile_flag} --workspace --bins --target {CARGO_TARGET}"
        f"'"
    )

    print("🔨 Running cargo build inside container (Cargo cache is shared)...")
    run(build_cmd)

    # ── Verify outputs ─────────────────────────────────────────────────────
    all_ok = True
    for binary in CONTROLLER_BINARIES + PACT_BINARIES:
        path = target_dir / binary
        if path.exists():
            print(f"  ✅ {binary}: {path.stat().st_size:,} bytes")
        else:
            print(f"  ❌ {binary}: NOT FOUND at {path}", file=sys.stderr)
            all_ok = False

    if not all_ok:
        print("❌ Build failed: some binaries missing", file=sys.stderr)
        sys.exit(1)

    print("✅ All binaries built successfully!")

    # ── CRD generation ─────────────────────────────────────────────────────
    # crdgen is a Linux binary; run it inside a container so it executes on
    # macOS without cross-running Linux ELFs.
    if args.skip_crd:
        print("⏭  Skipping CRD generation (--skip-crd)")
        return

    crd_output = workspace / "config" / "crd" / "secretmanagerconfig.yaml"
    crd_output.parent.mkdir(parents=True, exist_ok=True)
    crdgen_path = f"/workspace/target/{CARGO_TARGET}/{profile}/crdgen"

    print("📋 Generating SecretManagerConfig CRD...")
    crd_cmd = (
        f"docker run --rm "
        f"-v '{workspace}:/workspace' "
        f"-w /workspace "
        f"{RUST_BUILDER_IMAGE} "
        f"sh -c '{crdgen_path} > /workspace/config/crd/secretmanagerconfig.yaml'"
    )
    run(crd_cmd)
    print(f"  ✅ CRD written to {crd_output}")

    # ── kubectl apply ──────────────────────────────────────────────────────
    if args.skip_apply:
        print("⏭  Skipping kubectl apply (--skip-apply)")
        return

    cluster = run(
        "kubectl cluster-info --request-timeout=3s",
        check=False,
    )
    if cluster.returncode != 0:
        print("⚠️  Cluster not reachable — skipping CRD apply", file=sys.stderr)
        print(f"   Apply manually:  kubectl apply -f {crd_output}", file=sys.stderr)
        return

    run(f"kubectl apply -f {crd_output}")
    print("✅ CRD applied to cluster")

    # Wait for CRD to be established
    crd_name = "secretmanagerconfigs.secret-management.octopilot.io"
    wait = run(
        f"kubectl wait --for=condition=established crd {crd_name} --timeout=60s",
        check=False,
    )
    if wait.returncode == 0:
        print("✅ CRD is established and ready")
    else:
        print("⚠️  CRD may not be fully established yet — continuing anyway",
              file=sys.stderr)


if __name__ == "__main__":
    main()
