#!/bin/bash
# Helper script to add git credentials to SOPS-encrypted .env file
# This script uses sops to edit the encrypted file directly

set -e

ENV_FILE="${1:-.env}"

if [ ! -f "$ENV_FILE" ]; then
    echo "Error: .env file not found: $ENV_FILE"
    exit 1
fi

echo "🔐 Adding git credentials to SOPS-encrypted .env file"
echo ""
echo "Choose authentication method:"
echo "1) HTTPS (token-based)"
echo "2) SSH (private key)"
read -p "Enter choice [1 or 2]: " choice

case $choice in
    1)
        echo "Choose token type:"
        echo "1) GitHub token (GITHUB_TOKEN)"
        echo "2) Generic Git token (GIT_TOKEN)"
        read -p "Enter choice [1 or 2]: " token_type
        
        read -sp "Enter Git token: " token
        echo ""
        
        # Use sops to edit the encrypted file
        if [ "$token_type" = "1" ]; then
            sops --set "[\"GITHUB_TOKEN\"] \"$token\"" "$ENV_FILE"
            echo "✅ Added GitHub token to $ENV_FILE"
        else
            sops --set "[\"GIT_TOKEN\"] \"$token\"" "$ENV_FILE"
            echo "✅ Added Git token to $ENV_FILE"
        fi
        ;;
    2)
        echo "Paste your SSH private key (press Enter, then paste, then Ctrl+D on empty line):"
        ssh_key=$(cat)
        
        # Use sops to edit the encrypted file
        sops --set "[\"GIT_SSH_KEY\"] \"$ssh_key\"" "$ENV_FILE"
        
        echo "✅ Added SSH git credentials to $ENV_FILE"
        ;;
    *)
        echo "Invalid choice"
        exit 1
        ;;
esac

echo ""
echo "📋 Next steps:"
echo "1. Run: python3 scripts/tilt/setup_git_credentials.py"
echo "2. Or let Tilt run it automatically when you run 'tilt up'"
echo "3. Update GitRepository to reference the secret:"
echo "   spec:"
echo "     secretRef:"
echo "       name: git-credentials"

