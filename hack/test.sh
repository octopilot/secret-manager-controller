#!/usr/bin/env bash
# Test command of the pipeline's test leg (skaffold.yaml: BP_TEST_COMMAND) and of
# pr.yml; also runnable locally.
#
# The sops tests assert how sops failures are classified, so they need the sops
# binary. Where it is missing (GitHub-hosted runners) this installs the version the
# release image ships, pinned in dockerfiles/Dockerfile.controller, so that file stays
# the one place the version and checksum are written.
set -euo pipefail
cd "$(dirname "$0")/.."

dockerfile=dockerfiles/Dockerfile.controller
pin() { sed -n "s/^ARG $1=//p" "$dockerfile"; }

if ! command -v sops >/dev/null 2>&1; then
  case "$(uname -m)" in
    x86_64) arch=amd64; sha="$(pin SOPS_SHA256_AMD64)" ;;
    aarch64 | arm64) arch=arm64; sha="$(pin SOPS_SHA256_ARM64)" ;;
    *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;;
  esac
  [ "$(uname -s)" = Linux ] || { echo "sops is not installed; install it to run these tests" >&2; exit 1; }
  version="$(pin SOPS_VERSION)"
  bin="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/smc-tools/bin"
  mkdir -p "$bin"
  echo "Installing sops ${version} (${arch}) to ${bin}"
  curl -fsSL -o "$bin/sops" \
    "https://github.com/getsops/sops/releases/download/v${version}/sops-v${version}.linux.${arch}"
  echo "${sha}  ${bin}/sops" | sha256sum -c -
  chmod 0755 "$bin/sops"
  export PATH="$bin:$PATH"
fi
command -v gpg >/dev/null 2>&1 || { echo "gpg is not installed" >&2; exit 1; }
sops --version --disable-version-check | head -1

# With sops in place, a test that would skip for want of it fails instead.
export SMC_REQUIRE_SOPS=1

if cargo llvm-cov --version >/dev/null 2>&1 && [ -n "${GITHUB_WORKSPACE:-}" ]; then
  # The pipeline's default Rust test command, with coverage.
  mkdir -p "$GITHUB_WORKSPACE/coverage"
  cargo llvm-cov test --workspace --all-features --no-fail-fast --no-clean \
    --lcov --output-path "$GITHUB_WORKSPACE/coverage/lcov.info"
else
  cargo test --workspace --locked --no-fail-fast --lib --bins --tests
fi
