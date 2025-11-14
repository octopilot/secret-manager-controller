#!/usr/bin/env bash
set -euo pipefail

# Usage:
#   scripts/host-aware-build.sh [extra cargo args...]
#
# Selects the correct build strategy based on host OS/arch:
# - macOS: cargo zigbuild --target x86_64-unknown-linux-musl
# - Linux x86_64: cargo build --target x86_64-unknown-linux-musl with musl-gcc linker
#
# Builds the secret-manager-controller binary for Linux musl target

os_name=$(uname -s || echo unknown)
arch=$(uname -m || echo unknown)

use_zigbuild=true
if [[ ${os_name} == Linux && ${arch} == x86_64 ]]; then
  use_zigbuild=false
fi

if [[ ${use_zigbuild} == true ]]; then
  # macOS: Use cargo zigbuild (handles OpenSSL cross-compilation automatically)
  exec cargo zigbuild --target x86_64-unknown-linux-musl "$@"
else
  # Linux x86_64: Use musl-gcc linker
  exec env CC_x86_64_unknown_linux_musl=musl-gcc \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc \
    cargo build --target x86_64-unknown-linux-musl "$@"
fi

