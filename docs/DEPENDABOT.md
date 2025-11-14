# Dependabot Configuration

This document describes the Dependabot setup for automatically updating cloud provider dependencies in the secret-manager-controller.

## Overview

Dependabot is configured to:
- **Automatically check** for dependency updates weekly (Mondays at 9:00 AM)
- **Group related updates** (GCP, AWS, Azure, Kubernetes) to reduce PR noise
- **Auto-merge** PRs when tests and builds pass
- **Require manual review** for major version updates

## Configuration Files

### `.github/dependabot.yml`

Main Dependabot configuration:
- Monitors Rust dependencies in `hack/controllers/secret-manager-controller`
- Groups cloud provider SDKs together
- Labels PRs with `automerge` for automatic merging
- Ignores major version updates (require manual review)

### `.github/workflows/test-secret-manager-controller.yml`

CI workflow that runs on every PR:
- **Test job**: Runs cargo fmt, clippy, build, unit tests, and Pact tests
- **Lint job**: Runs cargo audit for security vulnerabilities
- Both jobs must pass for automerge to proceed

### `.github/workflows/dependabot-automerge.yml`

Automerge workflow that:
- Waits for required status checks to pass (`test` and `lint` jobs)
- Automatically merges Dependabot PRs with the `automerge` label
- Uses squash merge for clean commit history

## How It Works

1. **Weekly Check**: Every Monday at 9:00 AM, Dependabot checks for updates
2. **PR Creation**: Creates PRs for minor/patch updates, grouped by provider
3. **CI Runs**: GitHub Actions runs tests and linting
4. **Automerge**: Once all checks pass, the automerge workflow merges the PR

## Dependency Groups

Updates are grouped to reduce PR noise:

- **gcp-dependencies**: All `google-cloud-*` packages
- **aws-dependencies**: All `aws-sdk-*`, `aws-config`, `aws-credential-types`
- **azure-dependencies**: All `azure_*` packages
- **kube-dependencies**: All `kube*` and `k8s-openapi` packages

## Major Version Updates

Major version updates are **ignored** by Dependabot and require manual review. This ensures:
- Breaking changes are reviewed before merging
- Migration guides can be consulted
- Team can coordinate on major updates

## Manual Override

If you need to manually merge a Dependabot PR:
1. Remove the `automerge` label
2. Review the changes
3. Merge manually when ready

## Troubleshooting

### Automerge Not Working

Check:
1. PR has `automerge` label
2. All required status checks pass (`test` and `lint`)
3. No merge conflicts
4. Branch protection rules allow automerge

### Tests Failing

If tests fail on a Dependabot PR:
1. Review the error logs
2. Check if the dependency update introduced breaking changes
3. Either fix the code or remove the `automerge` label to prevent auto-merge

### Security Vulnerabilities

If `cargo audit` finds vulnerabilities:
1. Review the security advisory
2. Update to a patched version if available
3. Remove `automerge` label if manual intervention needed

## Configuration Details

### Update Schedule

- **Interval**: Weekly
- **Day**: Monday
- **Time**: 09:00 UTC

### PR Limits

- Maximum 10 open PRs at once
- Prevents PR spam while keeping dependencies updated

### Commit Messages

- Prefix: `chore`
- Includes scope (package name)
- Format: `chore(deps): bump google-cloud-secretmanager-v1 from 1.1 to 1.2`

## Best Practices

1. **Review Weekly**: Check Dependabot PRs weekly to ensure they're merging correctly
2. **Monitor Failures**: If automerge fails, investigate why tests are failing
3. **Major Updates**: Manually create PRs for major version updates with proper migration
4. **Security**: Review `cargo audit` results regularly

