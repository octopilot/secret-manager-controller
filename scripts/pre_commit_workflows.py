#!/usr/bin/env python3
"""
Pre-commit hook for GitHub Actions workflow validation.

Validates that all GitHub Actions workflow files (.github/workflows/*.yml) are valid YAML
and can be parsed by GitHub Actions.

Usage:
    This script is called by git pre-commit hook automatically
    Can also be run manually: python3 scripts/pre_commit_workflows.py
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path


def log_info(msg):
    """Print info message."""
    print(f"[INFO] {msg}")


def log_error(msg):
    """Print error message."""
    print(f"[ERROR] {msg}", file=sys.stderr)


def validate_workflow_file(workflow_path: Path) -> bool:
    """
    Validate a GitHub Actions workflow file.
    
    Uses GitHub's actionlint tool if available, otherwise falls back to basic YAML validation.
    """
    # Check if actionlint is available (GitHub's official workflow linter)
    actionlint = shutil.which("actionlint")
    
    if actionlint:
        log_info(f"Validating {workflow_path.name} with actionlint...")
        result = subprocess.run(
            [actionlint, "-color", str(workflow_path)],
            capture_output=True,
            text=True
        )
        
        if result.returncode != 0:
            log_error(f"Workflow validation failed for {workflow_path.name}:")
            if result.stdout:
                print(result.stdout, file=sys.stderr)
            if result.stderr:
                print(result.stderr, file=sys.stderr)
            return False
        return True
    else:
        # Fallback: basic YAML syntax check using Python
        log_info(f"Validating {workflow_path.name} (basic YAML check)...")
        try:
            import yaml
            with open(workflow_path, 'r') as f:
                yaml.safe_load(f)
            log_info(f"✅ {workflow_path.name} is valid YAML")
            log_info("💡 Install actionlint for more comprehensive validation: https://github.com/rhymond/actionlint")
            return True
        except ImportError:
            log_error("PyYAML not available. Install with: pip install pyyaml")
            log_error("Or install actionlint: https://github.com/rhymond/actionlint")
            return False
        except yaml.YAMLError as e:
            log_error(f"Invalid YAML in {workflow_path.name}: {e}")
            return False
        except Exception as e:
            log_error(f"Error validating {workflow_path.name}: {e}")
            return False


def main():
    """Main pre-commit function."""
    script_dir = Path(__file__).parent
    repo_root = script_dir.parent
    
    workflows_dir = repo_root / ".github" / "workflows"
    
    if not workflows_dir.exists():
        log_info("No .github/workflows directory found, skipping workflow validation")
        sys.exit(0)
    
    log_info("Validating GitHub Actions workflow files...")
    
    workflow_files = list(workflows_dir.glob("*.yml")) + list(workflows_dir.glob("*.yaml"))
    
    if not workflow_files:
        log_info("No workflow files found, skipping validation")
        sys.exit(0)
    
    all_valid = True
    for workflow_file in sorted(workflow_files):
        if not validate_workflow_file(workflow_file):
            all_valid = False
    
    if not all_valid:
        log_error("Workflow validation failed. Please fix the errors above before committing.")
        sys.exit(1)
    
    log_info("✅ All workflow files are valid!")
    sys.exit(0)


if __name__ == "__main__":
    main()

