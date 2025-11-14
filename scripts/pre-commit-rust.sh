#!/usr/bin/env bash
#
# Pre-commit hook for Rust code formatting and checking
# Runs cargo fmt and cargo check on the secret-manager-controller
#
# Usage:
#   This script is called by pre-commit framework automatically
#   Can also be run manually: ./scripts/pre-commit-rust.sh

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONTROLLER_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

# Check if cargo is available
if ! command -v cargo &>/dev/null; then
    log_error "cargo is not installed. Please install Rust: https://rustup.rs/"
    exit 1
fi

cd "$CONTROLLER_DIR"

log_info "Running cargo fmt on secret-manager-controller..."
if ! cargo fmt --check --all; then
    log_error "Code formatting check failed. Run 'cargo fmt' to fix formatting issues."
    log_info "Attempting to auto-format..."
    cargo fmt --all
    log_error "Code has been auto-formatted. Please review changes and commit again."
    exit 1
fi

log_info "Running cargo check on secret-manager-controller..."
if ! cargo check --all-targets; then
    log_error "cargo check failed. Please fix compilation errors before committing."
    exit 1
fi

log_info "Rust code formatting and checks passed!"
exit 0

