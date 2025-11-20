#!/bin/bash
# Flatten a Docker image to a single layer
# This reduces image size and simplifies the image structure
# Usage: ./scripts/flatten_image.sh <source-image> <target-image>

set -euo pipefail

SOURCE_IMAGE="${1:-}"
TARGET_IMAGE="${2:-}"

if [[ -z "${SOURCE_IMAGE}" || -z "${TARGET_IMAGE}" ]]; then
    echo "Usage: $0 <source-image> <target-image>"
    echo "Example: $0 gchr.io/microscaler/secret-manager-controller-base-image:latest gchr.io/microscaler/secret-manager-controller-base-image:latest-flat"
    exit 1
fi

echo "📦 Flattening image: ${SOURCE_IMAGE} -> ${TARGET_IMAGE}"

# Create a temporary container from the source image
echo "   Creating temporary container..."
CONTAINER_ID=$(docker create "${SOURCE_IMAGE}")

# Export the container filesystem and import as a new single-layer image
echo "   Exporting and importing as single layer..."
docker export "${CONTAINER_ID}" | docker import - "${TARGET_IMAGE}"

# Clean up
echo "   Cleaning up temporary container..."
docker rm "${CONTAINER_ID}"

echo "✅ Image flattened successfully: ${TARGET_IMAGE}"
echo "   You can now push it with: docker push ${TARGET_IMAGE}"

