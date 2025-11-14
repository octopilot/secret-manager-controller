#!/usr/bin/env bash
set -euo pipefail

# Extract CRD from Docker image and clean it
# Usage: extract-crd.sh <image-name> <output-path>

if [[ $# -lt 2 ]]; then
    echo "usage: $0 <image-name> <output-path>" >&2
    exit 1
fi

IMAGE_NAME="$1"
OUTPUT_PATH="$2"
CONTAINER_NAME="crd-extract-$$"

# Ensure output directory exists
mkdir -p "$(dirname "$OUTPUT_PATH")"

# Extract CRD from Docker image
docker create --name "$CONTAINER_NAME" "$IMAGE_NAME" > /dev/null 2>&1 || true
docker cp "$CONTAINER_NAME:/config/crd/secretmanagerconfig.yaml" "$OUTPUT_PATH.tmp" || {
    echo "❌ Error: Failed to extract CRD from Docker image" >&2
    docker rm "$CONTAINER_NAME" > /dev/null 2>&1 || true
    exit 1
}
docker rm "$CONTAINER_NAME" > /dev/null 2>&1 || true

# Clean ANSI escape sequences and control characters
# Remove ANSI escape sequences: ESC[ followed by numbers and letters ending with m
# Also remove other control characters (0x00-0x1F) except newlines (0x0A) and carriage returns (0x0D)
if command -v perl >/dev/null 2>&1; then
    # Use perl for more reliable pattern matching
    perl -pe 's/\x1b\[[0-9;]*m//g; s/[\x00-\x08\x0B-\x0C\x0E-\x1F]//g' < "$OUTPUT_PATH.tmp" > "$OUTPUT_PATH"
else
    # Fallback: use sed (less reliable for ANSI sequences)
    sed 's/\x1b\[[0-9;]*m//g; s/[\x00-\x08\x0B-\x0C\x0E-\x1F]//g' < "$OUTPUT_PATH.tmp" > "$OUTPUT_PATH" || {
        echo "⚠️  Warning: Failed to clean CRD, using original" >&2
        cp "$OUTPUT_PATH.tmp" "$OUTPUT_PATH"
    }
fi

# Validate it's valid YAML (must start with apiVersion or kind)
if ! head -1 "$OUTPUT_PATH" | grep -qE '^(apiVersion|kind|---)'; then
    echo "❌ Error: Extracted file does not appear to be valid YAML" >&2
    echo "First line: $(head -1 "$OUTPUT_PATH")" >&2
    echo "File appears to contain logs instead of YAML. Check Dockerfile CRD generation step." >&2
    rm -f "$OUTPUT_PATH.tmp" "$OUTPUT_PATH"
    exit 1
fi

# Clean up temp file
rm -f "$OUTPUT_PATH.tmp"

echo "✅ CRD extracted and cleaned: $OUTPUT_PATH"

