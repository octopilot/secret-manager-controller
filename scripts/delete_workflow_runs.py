#!/usr/bin/env python3
"""
Delete all workflow runs from GitHub Actions.

⚠️  ONE-TIME CLEANUP SCRIPT ⚠️
This script is intended for one-time cleanup of workflow run history to prepare
the project for showcasing. After this cleanup, normal workflow runs will
accumulate as expected.

This script uses the GitHub API to delete all workflow runs from the repository.
Useful for cleaning up build history after squashing commits or preparing for
public showcase.

Requirements:
    - GitHub Personal Access Token (PAT) with 'repo' and 'actions:write' permissions
    - Set GITHUB_TOKEN environment variable or pass via --token flag

Usage:
    # Using environment variable
    export GITHUB_TOKEN=ghp_your_token_here
    python3 scripts/delete_workflow_runs.py

    # Using command line flag
    python3 scripts/delete_workflow_runs.py --token ghp_your_token_here

    # Dry run (list runs without deleting) - RECOMMENDED FIRST STEP
    python3 scripts/delete_workflow_runs.py --dry-run

    # Delete specific workflow runs only
    python3 scripts/delete_workflow_runs.py --workflow ci.yml
"""

import argparse
import os
import sys
import time
from typing import List, Optional

try:
    import requests
except ImportError:
    print("Error: 'requests' library is required. Install it with: pip install requests")
    sys.exit(1)


# GitHub API base URL
GITHUB_API_BASE = "https://api.github.com"
REPO = "microscaler/secret-manager-controller"


def get_github_token(token: Optional[str] = None) -> str:
    """Get GitHub token from environment or argument."""
    if token:
        return token
    
    env_token = os.environ.get("GITHUB_TOKEN")
    if env_token:
        return env_token
    
    print("Error: GitHub token is required.")
    print("Set GITHUB_TOKEN environment variable or use --token flag")
    print("\nTo create a token:")
    print("1. Go to https://github.com/settings/tokens")
    print("2. Generate new token (classic)")
    print("3. Select 'repo' and 'workflow' scopes")
    print("4. Copy the token and use it with this script")
    sys.exit(1)


def make_request(
    method: str,
    url: str,
    token: str,
    params: Optional[dict] = None,
    max_retries: int = 3,
) -> requests.Response:
    """Make HTTP request with retry logic and rate limit handling."""
    headers = {
        "Authorization": f"token {token}",
        "Accept": "application/vnd.github.v3+json",
    }
    
    for attempt in range(max_retries):
        try:
            if method.upper() == "GET":
                response = requests.get(url, headers=headers, params=params, timeout=30)
            elif method.upper() == "DELETE":
                response = requests.delete(url, headers=headers, timeout=30)
            else:
                raise ValueError(f"Unsupported HTTP method: {method}")
            
            # Handle rate limiting
            if response.status_code == 403:
                rate_limit_reset = response.headers.get("X-RateLimit-Reset")
                if rate_limit_reset:
                    reset_time = int(rate_limit_reset)
                    wait_time = max(0, reset_time - int(time.time())) + 1
                    print(f"⚠️  Rate limit exceeded. Waiting {wait_time} seconds...")
                    time.sleep(wait_time)
                    continue
            
            return response
            
        except requests.exceptions.RequestException as e:
            if attempt < max_retries - 1:
                wait_time = 2 ** attempt  # Exponential backoff
                print(f"⚠️  Request failed (attempt {attempt + 1}/{max_retries}): {e}")
                print(f"    Retrying in {wait_time} seconds...")
                time.sleep(wait_time)
            else:
                raise
    
    raise Exception("Max retries exceeded")


def get_all_workflow_runs(
    token: str,
    workflow_id: Optional[str] = None,
) -> List[dict]:
    """Fetch all workflow runs for the repository."""
    runs = []
    page = 1
    per_page = 100
    
    if workflow_id:
        url = f"{GITHUB_API_BASE}/repos/{REPO}/actions/workflows/{workflow_id}/runs"
    else:
        url = f"{GITHUB_API_BASE}/repos/{REPO}/actions/runs"
    
    print(f"📋 Fetching workflow runs from {REPO}...")
    
    while True:
        params = {"per_page": per_page, "page": page}
        response = make_request("GET", url, token, params=params)
        
        if response.status_code != 200:
            print(f"❌ Failed to fetch workflow runs: {response.status_code}")
            print(f"   Response: {response.text}")
            return runs
        
        data = response.json()
        workflow_runs = data.get("workflow_runs", [])
        
        if not workflow_runs:
            break
        
        runs.extend(workflow_runs)
        print(f"   Found {len(runs)} runs so far...")
        
        # Check if there are more pages
        if len(workflow_runs) < per_page:
            break
        
        page += 1
        time.sleep(0.5)  # Be nice to the API
    
    return runs


def delete_workflow_run(run_id: int, token: str) -> bool:
    """Delete a specific workflow run by ID."""
    url = f"{GITHUB_API_BASE}/repos/{REPO}/actions/runs/{run_id}"
    response = make_request("DELETE", url, token)
    
    if response.status_code == 204:
        return True
    elif response.status_code == 404:
        print(f"   ⚠️  Run {run_id} not found (may have been already deleted)")
        return True  # Consider 404 as success (already deleted)
    else:
        print(f"   ❌ Failed to delete run {run_id}: {response.status_code}")
        if response.text:
            print(f"      Response: {response.text}")
        return False


def get_workflow_name(run: dict) -> str:
    """Get workflow name from run data."""
    workflow_name = run.get("name", "Unknown")
    workflow_path = run.get("path", "")
    if workflow_path:
        return f"{workflow_name} ({workflow_path})"
    return workflow_name


def main():
    parser = argparse.ArgumentParser(
        description="Delete all workflow runs from GitHub Actions",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Delete all workflow runs
  python3 scripts/delete_workflow_runs.py

  # Dry run (list without deleting)
  python3 scripts/delete_workflow_runs.py --dry-run

  # Delete runs for specific workflow
  python3 scripts/delete_workflow_runs.py --workflow ci.yml

  # Use token from command line
  python3 scripts/delete_workflow_runs.py --token ghp_xxxxxxxxxxxx
        """
    )
    parser.add_argument(
        "--token",
        type=str,
        help="GitHub Personal Access Token (or set GITHUB_TOKEN env var)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="List workflow runs without deleting them",
    )
    parser.add_argument(
        "--workflow",
        type=str,
        help="Only delete runs for specific workflow file (e.g., 'ci.yml')",
    )
    parser.add_argument(
        "--confirm",
        action="store_true",
        help="Skip confirmation prompt (use with caution!)",
    )
    
    args = parser.parse_args()
    
    token = get_github_token(args.token)
    
    # Get workflow ID if filtering by workflow
    workflow_id = None
    if args.workflow:
        # Get workflow ID from workflow file path
        workflows_url = f"{GITHUB_API_BASE}/repos/{REPO}/actions/workflows"
        response = make_request("GET", workflows_url, token)
        if response.status_code == 200:
            workflows_data = response.json().get("workflows", [])
            # Try to find workflow by path (e.g., '.github/workflows/ci.yml')
            workflow_path = args.workflow
            if not workflow_path.startswith(".github/workflows/"):
                workflow_path = f".github/workflows/{workflow_path}"
            
            for wf in workflows_data:
                wf_path = wf.get("path", "")
                if wf_path.endswith(args.workflow) or wf_path == workflow_path:
                    workflow_id = str(wf["id"])
                    print(f"📋 Found workflow: {wf.get('name', 'Unknown')} (ID: {workflow_id})")
                    break
        
        if not workflow_id:
            print(f"⚠️  Warning: Could not find workflow '{args.workflow}'")
            print("   Will filter runs by workflow path after fetching all runs...")
    
    # Fetch all workflow runs
    runs = get_all_workflow_runs(token, workflow_id)
    
    if not runs:
        print("✅ No workflow runs found.")
        return
    
    # Filter by workflow if specified
    if args.workflow and not workflow_id:
        # Filter by workflow path
        runs = [
            r for r in runs
            if r.get("path", "").endswith(args.workflow)
        ]
    
    print(f"\n📊 Found {len(runs)} workflow run(s) to {'list' if args.dry_run else 'delete'}")
    
    if args.dry_run:
        print("\n📋 Workflow runs (dry run - not deleting):")
        print("-" * 80)
        for run in runs[:20]:  # Show first 20
            run_id = run["id"]
            status = run.get("status", "unknown")
            conclusion = run.get("conclusion", "unknown")
            workflow_name = get_workflow_name(run)
            created_at = run.get("created_at", "unknown")
            print(f"  {run_id:10} | {status:10} | {conclusion:15} | {workflow_name}")
            print(f"             | Created: {created_at}")
        
        if len(runs) > 20:
            print(f"\n  ... and {len(runs) - 20} more runs")
        
        print("\n✅ Dry run complete. Use without --dry-run to actually delete.")
        return
    
    # Confirmation prompt
    if not args.confirm:
        print("\n" + "=" * 80)
        print("⚠️  ONE-TIME CLEANUP WARNING ⚠️")
        print("=" * 80)
        print("This will PERMANENTLY delete all workflow runs!")
        print("This is intended as a one-time cleanup to prepare the project for showcasing.")
        print("After this cleanup, normal workflow runs will accumulate as expected.")
        print("=" * 80)
        print(f"\nRepository: {REPO}")
        print(f"Runs to delete: {len(runs)}")
        if args.workflow:
            print(f"Workflow filter: {args.workflow}")
        print("\n⚠️  This action is IRREVERSIBLE!")
        print("   Deleted workflow runs and their logs cannot be restored.")
        
        response = input("\nType 'DELETE ALL' to confirm deletion: ")
        if response != "DELETE ALL":
            print("❌ Cancelled. You must type exactly 'DELETE ALL' to proceed.")
            return
    
    # Delete workflow runs
    print(f"\n🗑️  Deleting {len(runs)} workflow run(s)...")
    deleted = 0
    failed = 0
    
    for i, run in enumerate(runs, 1):
        run_id = run["id"]
        workflow_name = get_workflow_name(run)
        status = run.get("status", "unknown")
        
        print(f"[{i}/{len(runs)}] Deleting run {run_id} ({workflow_name}, status: {status})...", end=" ")
        
        if delete_workflow_run(run_id, token):
            deleted += 1
            print("✅")
        else:
            failed += 1
            print("❌")
        
        # Rate limiting: be nice to the API
        if i < len(runs):
            time.sleep(0.5)
    
    # Summary
    print("\n" + "=" * 80)
    print("📊 Summary:")
    print(f"   Total runs: {len(runs)}")
    print(f"   ✅ Deleted: {deleted}")
    if failed > 0:
        print(f"   ❌ Failed: {failed}")
    print("=" * 80)
    
    if failed == 0:
        print("✅ All workflow runs deleted successfully!")
    else:
        print(f"⚠️  Completed with {failed} failure(s)")


if __name__ == "__main__":
    main()

