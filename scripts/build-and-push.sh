#!/bin/bash
#
# Build and push Secret Manager Controller Docker image
#
# This script builds the controller using docker buildx for production deployment.
# It uses Dockerfile which builds the binary inside Docker (multi-stage build).
# Dockerfile.dev is used for development (Tilt) and expects a pre-built binary.
#
# Usage:
#   ./scripts/build-and-push.sh [image-tag] [registry]
#
# Examples:
#   ./scripts/build-and-push.sh
#   ./scripts/build-and-push.sh v1.0.0
#   ./scripts/build-and-push.sh v1.0.0 ghcr.io/microscaler

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONTROLLER_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
PROJECT_ROOT="$(cd "${CONTROLLER_DIR}/../../.." && pwd)"

# Configuration
IMAGE_NAME="secret-manager-controller"
DEFAULT_TAG="latest"
DEFAULT_REGISTRY="ghcr.io/microscaler"

# Parse arguments
TAG="${1:-${DEFAULT_TAG}}"
REGISTRY="${2:-${DEFAULT_REGISTRY}}"
FULL_IMAGE_NAME="${REGISTRY}/${IMAGE_NAME}:${TAG}"

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "${BLUE}[STEP]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    log_step "Checking prerequisites..."
    
    if ! command -v docker &>/dev/null; then
        log_error "Docker is not installed"
        exit 1
    fi
    
    if ! docker buildx version &>/dev/null; then
        log_error "Docker buildx is not available"
        log_info "Install buildx: https://docs.docker.com/buildx/working-with-buildx/"
        exit 1
    fi
    
    log_info "Prerequisites check passed"
}

# Setup buildx builder
setup_buildx() {
    log_step "Setting up buildx builder..."
    
    # Create builder if it doesn't exist
    if ! docker buildx ls | grep -q "secret-manager-builder"; then
        log_info "Creating buildx builder..."
        docker buildx create --name secret-manager-builder --use || {
            log_warn "Builder may already exist, using existing..."
            docker buildx use secret-manager-builder
        }
    else
        log_info "Using existing buildx builder"
        docker buildx use secret-manager-builder
    fi
    
    # Bootstrap builder
    docker buildx inspect --bootstrap
}

# Build and push image
build_and_push() {
    log_step "Building and pushing image: ${FULL_IMAGE_NAME}"
    
    cd "${CONTROLLER_DIR}"
    
    # Use Dockerfile (production multi-stage build) for production builds
    # Dockerfile.dev is for development (Tilt) and expects pre-built binary
    DOCKERFILE="${CONTROLLER_DIR}/Dockerfile"
    
    if [ ! -f "${DOCKERFILE}" ]; then
        log_error "Dockerfile not found at ${DOCKERFILE}"
        exit 1
    fi
    
    log_info "Building with docker buildx..."
    log_info "  Image: ${FULL_IMAGE_NAME}"
    log_info "  Dockerfile: ${DOCKERFILE} (production multi-stage build)"
    log_info "  Platform: linux/amd64"
    
    # Build and push using buildx
    docker buildx build \
        --platform linux/amd64 \
        --file "${DOCKERFILE}" \
        --tag "${FULL_IMAGE_NAME}" \
        --push \
        --progress=plain \
        "${CONTROLLER_DIR}"
    
    log_info "✅ Image built and pushed successfully!"
    log_info "   ${FULL_IMAGE_NAME}"
}

# Main execution
main() {
    log_info "Building and pushing Secret Manager Controller"
    echo ""
    
    check_prerequisites
    setup_buildx
    build_and_push
    
    echo ""
    log_info "✅ Build and push complete!"
    log_info "   Image: ${FULL_IMAGE_NAME}"
}

main "$@"

