#!/bin/bash
# Semver version bump script for base images
# Usage: ./scripts/semver_bump.sh <image-name> <major|minor|patch>
# Example: ./scripts/semver_bump.sh controller-base-image patch

set -euo pipefail

IMAGE_NAME="${1:-}"
BUMP_TYPE="${2:-patch}"

if [[ -z "${IMAGE_NAME}" ]]; then
    echo "Usage: $0 <image-name> <major|minor|patch>"
    echo "Example: $0 controller-base-image patch"
    exit 1
fi

if [[ ! "${BUMP_TYPE}" =~ ^(major|minor|patch)$ ]]; then
    echo "Error: Bump type must be 'major', 'minor', or 'patch'"
    exit 1
fi

# Get the current version from git tags
# Format: <image-name>-v<major>.<minor>.<patch>
CURRENT_TAG=$(git tag -l "${IMAGE_NAME}-v*" | sort -V | tail -1 || echo "")

if [[ -z "${CURRENT_TAG}" ]]; then
    # No existing tag, start at v0.1.0
    MAJOR=0
    MINOR=1
    PATCH=0
else
    # Extract version from tag (e.g., controller-base-image-v1.2.3 -> 1.2.3)
    VERSION="${CURRENT_TAG#${IMAGE_NAME}-v}"
    IFS='.' read -r MAJOR MINOR PATCH <<< "${VERSION}"
fi

# Bump version based on type
case "${BUMP_TYPE}" in
    major)
        MAJOR=$((MAJOR + 1))
        MINOR=0
        PATCH=0
        ;;
    minor)
        MINOR=$((MINOR + 1))
        PATCH=0
        ;;
    patch)
        PATCH=$((PATCH + 1))
        ;;
esac

NEW_VERSION="${MAJOR}.${MINOR}.${PATCH}"
NEW_TAG="${IMAGE_NAME}-v${NEW_VERSION}"

echo "Current version: ${CURRENT_TAG:-none}"
echo "New version: ${NEW_TAG}"
echo "Bump type: ${BUMP_TYPE}"

# Create and push the tag
read -p "Create and push tag ${NEW_TAG}? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    git tag "${NEW_TAG}"
    git push origin "${NEW_TAG}"
    echo "✅ Tag ${NEW_TAG} created and pushed"
    echo "Version: ${NEW_VERSION}"
else
    echo "Cancelled"
    exit 1
fi

