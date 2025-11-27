#!/usr/bin/env python3
"""Validate all SecretManagerConfig CRs meet CRD pattern requirements."""

import re
import sys
from pathlib import Path
import yaml

# Pattern definitions from CRD
GCP_LOCATION_PATTERN = re.compile(r"^[a-z]+-[a-z]+[0-9]+$")
AWS_REGION_PATTERN = re.compile(
    r"^[a-z]{2}-[a-z]+-[0-9]+$|^[a-z]{2}-gov-[a-z]+-[0-9]+$|^[a-z]{2}-iso-[a-z]+-[0-9]+$|^cn-[a-z]+-[0-9]+$|^local$"
)
AZURE_LOCATION_PATTERN = re.compile(r"^[a-z]+[0-9]*$")


def validate_gcp_location(location: str) -> tuple[bool, str]:
    """Validate GCP location format."""
    location_lower = location.lower().strip()
    if not location_lower:
        return False, "GCP location cannot be empty"
    if GCP_LOCATION_PATTERN.match(location_lower):
        return True, ""
    return False, f"GCP location '{location}' does not match pattern ^[a-z]+-[a-z]+[0-9]+$ (e.g., us-central1)"


def validate_aws_region(region: str) -> tuple[bool, str]:
    """Validate AWS region format."""
    region_lower = region.lower().strip()
    if not region_lower:
        return False, "AWS region cannot be empty"
    if AWS_REGION_PATTERN.match(region_lower):
        return True, ""
    return False, f"AWS region '{region}' does not match pattern (e.g., us-east-1, us-gov-west-1, cn-north-1)"


def validate_azure_location(location: str) -> tuple[bool, str]:
    """Validate Azure location format."""
    location_lower = location.lower().strip()
    if not location_lower:
        return False, "Azure location cannot be empty"
    if AZURE_LOCATION_PATTERN.match(location_lower):
        return True, ""
    return False, f"Azure location '{location}' does not match pattern ^[a-z]+[0-9]*$ (e.g., eastus, westus2)"


def validate_cr_file(file_path: Path) -> list[tuple[str, bool, str]]:
    """Validate a single CR file."""
    errors = []
    
    try:
        with open(file_path, "r") as f:
            content = yaml.safe_load(f)
        
        if not content or "spec" not in content:
            return [("file_structure", False, "Invalid YAML structure or missing spec")]
        
        provider = content.get("spec", {}).get("provider", {})
        
        # Check GCP
        if "gcp" in provider:
            gcp_config = provider["gcp"]
            if "location" not in gcp_config:
                errors.append(("gcp.location", False, "GCP location field is missing (required)"))
            else:
                location = gcp_config["location"]
                is_valid, error_msg = validate_gcp_location(location)
                errors.append(("gcp.location", is_valid, error_msg if not is_valid else f"Valid: {location}"))
        
        # Check AWS
        if "aws" in provider:
            aws_config = provider["aws"]
            if "region" not in aws_config:
                errors.append(("aws.region", False, "AWS region field is missing (required)"))
            else:
                region = aws_config["region"]
                is_valid, error_msg = validate_aws_region(region)
                errors.append(("aws.region", is_valid, error_msg if not is_valid else f"Valid: {region}"))
        
        # Check Azure
        if "azure" in provider:
            azure_config = provider["azure"]
            if "location" not in azure_config:
                errors.append(("azure.location", False, "Azure location field is missing (required)"))
            else:
                location = azure_config["location"]
                is_valid, error_msg = validate_azure_location(location)
                errors.append(("azure.location", is_valid, error_msg if not is_valid else f"Valid: {location}"))
    
    except yaml.YAMLError as e:
        errors.append(("yaml_parse", False, f"Failed to parse YAML: {e}"))
    except Exception as e:
        errors.append(("error", False, f"Unexpected error: {e}"))
    
    return errors


def main():
    """Main validation function."""
    repo_root = Path(__file__).parent.parent
    gitops_dir = repo_root / "gitops" / "cluster"
    
    # Find all SecretManagerConfig files
    cr_files = []
    for pattern in ["argocd/**/secretmanagerconfig*.yaml", "fluxcd/**/secretmanagerconfig*.yaml"]:
        cr_files.extend(gitops_dir.glob(pattern))
    
    if not cr_files:
        print("❌ No SecretManagerConfig files found!")
        sys.exit(1)
    
    print(f"🔍 Validating {len(cr_files)} SecretManagerConfig files...\n")
    
    all_valid = True
    total_errors = 0
    
    for cr_file in sorted(cr_files):
        relative_path = cr_file.relative_to(repo_root)
        errors = validate_cr_file(cr_file)
        
        has_errors = any(not is_valid for _, is_valid, _ in errors)
        if has_errors:
            all_valid = False
            print(f"❌ {relative_path}")
            for field, is_valid, msg in errors:
                if not is_valid:
                    print(f"   • {field}: {msg}")
                    total_errors += 1
        else:
            # Show valid fields
            print(f"✅ {relative_path}")
            for field, is_valid, msg in errors:
                if is_valid:
                    print(f"   • {field}: {msg}")
    
    print(f"\n{'='*60}")
    if all_valid:
        print(f"✅ All {len(cr_files)} CRs are valid!")
        sys.exit(0)
    else:
        print(f"❌ Found {total_errors} validation error(s) in {len(cr_files)} CR(s)")
        sys.exit(1)


if __name__ == "__main__":
    main()

