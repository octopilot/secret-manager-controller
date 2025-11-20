#!/bin/bash
# Build and publish the base image for Secret Manager Controller
# This image contains all runtime dependencies (git, curl, gnupg, kustomize, sops)
# and is published to gchr.io/microscaler/secret-manager-controller-base-image

set -euo pipefail

IMAGE_NAME="gchr.io/microscaler/secret-manager-controller-base-image"
VERSION="${1:-latest}"
DOCKERFILE="${2:-Dockerfile.base}"

echo "🐳 Building base image: ${IMAGE_NAME}:${VERSION}"
echo "   Using Dockerfile: ${DOCKERFILE}"

# Build the image
docker build \
    -f "${DOCKERFILE}" \
    -t "${IMAGE_NAME}:${VERSION}" \
    -t "${IMAGE_NAME}:latest" \
    .

echo "✅ Base image built successfully"

# Optionally flatten the image to a single layer
if [[ "${FLATTEN:-false}" == "true" ]]; then
    echo "📦 Flattening image to single layer..."
    
    # Create a temporary container from the image
    CONTAINER_ID=$(docker create "${IMAGE_NAME}:${VERSION}")
    
    # Export the container filesystem and import as a new single-layer image
    docker export "${CONTAINER_ID}" | docker import - "${IMAGE_NAME}:${VERSION}-flat"
    docker tag "${IMAGE_NAME}:${VERSION}-flat" "${IMAGE_NAME}:${VERSION}"
    docker tag "${IMAGE_NAME}:${VERSION}-flat" "${IMAGE_NAME}:latest"
    
    # Clean up
    docker rm "${CONTAINER_ID}"
    
    echo "✅ Image flattened successfully"
fi

# Push to registry (if not in dry-run mode)
if [[ "${DRY_RUN:-false}" != "true" ]]; then
    echo "📤 Pushing image to registry..."
    docker push "${IMAGE_NAME}:${VERSION}"
    docker push "${IMAGE_NAME}:latest"
    echo "✅ Image pushed successfully"
else
    echo "🔍 Dry-run mode: Skipping push"
fi

echo "✅ Base image build complete: ${IMAGE_NAME}:${VERSION}"

