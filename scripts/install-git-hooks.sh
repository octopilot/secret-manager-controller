#!/bin/bash
# Install Git hooks for the secret-manager-controller project

set -e

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

echo "Installing Git hooks..."

# Copy commit-msg hook
if [ -f ".git/hooks/commit-msg" ]; then
    echo "⚠️  commit-msg hook already exists. Backing up..."
    mv .git/hooks/commit-msg .git/hooks/commit-msg.backup
fi

# The commit-msg hook should already be in .git/hooks/ from our setup
# But we'll ensure it's executable
chmod +x .git/hooks/commit-msg

# Ensure pre-commit hook is executable
if [ -f ".git/hooks/pre-commit" ]; then
    chmod +x .git/hooks/pre-commit
fi

echo "✅ Git hooks installed successfully!"
echo ""
echo "The following hooks are now active:"
echo "  - commit-msg: Validates conventional commit messages"
echo "  - pre-commit: Runs GitHub Actions workflow validation, SOPS encryption check, and Rust formatting"
echo ""
echo "To test the commit-msg hook, try:"
echo "  git commit --allow-empty -m 'test: this is a valid commit message'"
echo ""
