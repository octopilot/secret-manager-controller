#!/bin/bash
# Update SOPS encryption keys on all encrypted files
# This script finds all SOPS-encrypted files and updates them using sops updatekeys

set -e

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

echo "🔄 Updating SOPS encryption keys..."
echo ""

# Find all potential SOPS files
# Look for files that match SOPS patterns and contain SOPS metadata
find . \
  -type f \
  \( -name "*.secrets.env" -o -name "*.secrets.yaml" -o -name ".env" \) \
  ! -path "*/node_modules/*" \
  ! -path "*/target/*" \
  ! -path "*/.git/*" \
  ! -path "*/.venv/*" \
  ! -path "*/venv/*" \
  ! -path "*/__pycache__/*" \
  | while read -r file; do
    # Check if file is SOPS-encrypted
    if grep -q "sops:" "$file" 2>/dev/null || grep -q "ENC\[" "$file" 2>/dev/null; then
      echo "🔄 Updating keys: $file"
      if sops updatekeys -y "$file" 2>/dev/null; then
        echo "  ✅ Keys updated successfully"
      else
        echo "  ⚠️  Failed to update (may not be a SOPS file or key not available)"
      fi
    fi
  done

echo ""
echo "✅ Key update complete!"
echo ""
echo "Next steps:"
echo "1. Verify files are still decryptable: sops -d <file>"
echo "2. Remove old key from .sops.yaml if update was successful"
echo "3. Commit the updated files"

