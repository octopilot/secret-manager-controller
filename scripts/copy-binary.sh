#!/usr/bin/env bash
# Copy binary from build target to artifacts directory
# and create MD5 hash file for Docker rebuild triggers
#
# Usage: copy-binary.sh <target_path> <artifact_path> <binary_name>

set -euo pipefail

if [[ $# -lt 3 ]]; then
  echo "usage: $0 <target_path> <artifact_path> <binary_name>" >&2
  exit 1
fi

target_path="$1"
artifact_path="$2"
binary_name="$3"
hash_path="${artifact_path}.md5"

# Create artifacts directory
mkdir -p build_artifacts

# Check if source binary exists
if [[ ! -f "$target_path" ]]; then
  echo "❌ Error: $target_path not found" >&2
  exit 1
fi

# Delete existing binary from artifacts directory before copying
rm -f "$artifact_path"

# Copy binary to artifacts directory
cp "$target_path" "$artifact_path"

# Create MD5 hash file (triggers Docker rebuilds when binary changes)
md5 -q "$artifact_path" > "$hash_path"

echo "✅ Copied $binary_name"
echo "   Source: $target_path"
echo "   Artifact: $artifact_path"
echo "   Hash: $hash_path"

