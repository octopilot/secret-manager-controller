# MSMCTL CLI

`msmctl` (Microscaler Secret Manager Controller) is a command-line tool for interacting with the Secret Manager Controller running in Kubernetes. Similar to `fluxctl`, it provides commands to trigger reconciliations, view status, and manage SecretManagerConfig resources.

## Installation

### Build from Source

```bash
# Build the CLI tool
cargo build --bin msmctl

# Build release version
cargo build --release --bin msmctl
```

The binary will be available at:
- Debug build: `target/debug/msmctl`
- Release build: `target/release/msmctl`

### Install to Local Bin

```bash
# Using just (recommended)
just install-cli

# Or manually
mkdir -p ~/.local/bin
cp target/release/msmctl ~/.local/bin/
```

Make sure `~/.local/bin` is in your `PATH`.

### Prerequisites

- Kubernetes cluster with Secret Manager Controller deployed
- `kubectl` configured with access to the cluster
- RBAC permissions to read/update SecretManagerConfig resources

## Authentication

`msmctl` uses Kubernetes authentication primitives:

- **kubeconfig**: Uses the default kubeconfig (`~/.kube/config`) or `KUBECONFIG` environment variable
- **Service Account**: When running in-cluster, uses the pod's service account token
- **Client Certificates**: Supports client certificate authentication from kubeconfig

No additional authentication is required - `msmctl` leverages Kubernetes' built-in security mechanisms.

## Commands

### `msmctl reconcile`

Trigger a manual reconciliation for a SecretManagerConfig resource.

**Usage:**
```bash
msmctl reconcile secretmanagerconfig <name> [--namespace <namespace>] [--force]
```

**Arguments:**
- `secretmanagerconfig` (or `smc`): Resource type (required)
- `<name>`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- `--namespace, -n`: Namespace of the resource (defaults to current context namespace)
- `--force`: Force reconciliation by deleting and waiting for GitOps to recreate the resource (useful when resources get stuck)

**Examples:**
```bash
# Trigger reconciliation in default namespace
msmctl reconcile secretmanagerconfig myapp-dev-secrets

# Trigger reconciliation in specific namespace
msmctl reconcile secretmanagerconfig myapp-dev-secrets --namespace mysystem

# Using short form 'smc'
msmctl reconcile smc myapp-dev-secrets

# Force reconciliation (delete and wait for GitOps recreation)
msmctl reconcile secretmanagerconfig myapp-dev-secrets --namespace mysystem --force
```

**How it works:**
- **Normal mode**: Updates the `secret-management.octopilot.io/reconcile` annotation with a timestamp. The controller watches for annotation changes and triggers reconciliation. This is a Kubernetes-native approach that doesn't require HTTP endpoints.
- **Force mode (`--force`)**: 
  1. Deletes the SecretManagerConfig resource
  2. Waits for GitOps (Flux/ArgoCD) to recreate it (up to 5 minutes)
  3. Shows progress logs during the wait
  4. Once recreated, triggers reconciliation
  5. Provides command to view reconciliation logs

**Force mode output:**
```
🔄 Force reconciliation mode enabled
   Resource: mysystem/myapp-dev-secrets

🗑️  Deleting SecretManagerConfig 'mysystem/myapp-dev-secrets'...

⏳ Waiting for GitOps to recreate resource...
   (This may take a few moments depending on GitOps sync interval)
   ⏳ Still waiting... (10s elapsed)
   ⏳ Still waiting... (20s elapsed)
   ✅ Resource recreated (generation: 1)

⏳ Waiting for resource to stabilize...

🔄 Triggering reconciliation for SecretManagerConfig 'mysystem/myapp-dev-secrets'...
✅ Reconciliation triggered successfully
   Resource: mysystem/myapp-dev-secrets
   Timestamp: 1702567890

📊 Watching reconciliation logs...
   (Use 'kubectl logs -n octopilot-system -l app=secret-manager-controller --tail=50 -f' to see detailed logs)
```

### `msmctl list`

List all SecretManagerConfig resources.

**Usage:**
```bash
msmctl list secretmanagerconfig [--namespace <namespace>]
```

**Arguments:**
- `secretmanagerconfig` (or `smc`): Resource type (required)

**Options:**
- `--namespace, -n`: Namespace to list resources in (defaults to all namespaces)

**Examples:**
```bash
# List all resources in all namespaces
msmctl list secretmanagerconfig

# List resources in specific namespace
msmctl list secretmanagerconfig --namespace mysystem

# Using short form 'smc'
msmctl list smc
```

**Output:**
```
NAME                           NAMESPACE            SUSPEND      READY           SECRETS SYNCED 
-------------------------------------------------------------------------------------
test-sops-config               default              No           False           -              
test-sops-config-prod          default              No           False           -              
test-sops-config-stage         default              No           False           -              
```

**Note:** The `SUSPEND` column shows whether reconciliation is paused for each resource.

### `msmctl status`

Show detailed status of a SecretManagerConfig resource.

**Usage:**
```bash
msmctl status secretmanagerconfig <name> [--namespace <namespace>]
```

**Arguments:**
- `secretmanagerconfig` (or `smc`): Resource type (required)
- `<name>`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- `--namespace, -n`: Namespace of the resource (defaults to current context namespace)

**Examples:**
```bash
# Show status in default namespace
msmctl status secretmanagerconfig myapp-dev-secrets --namespace mysystem

# Using short form 'smc'
msmctl status smc myapp-dev-secrets
```

**Output:**
```
SecretManagerConfig: mysystem/myapp-dev-secrets

Phase: Synced
Description: Successfully synced 5 secrets
Ready: True

Conditions:
  - Type: Ready
    Status: True
    Reason: ReconciliationSucceeded
    Message: Successfully synced 5 secrets
    Last Transition: 2024-01-15T10:30:00Z

Status:
  Last Sync Time: 2024-01-15T10:30:00Z
  Secrets Count: 5
  Suspended: false
  Git Pulls Suspended: false
```

### `msmctl suspend`

Suspend reconciliation for a SecretManagerConfig resource.

**Usage:**
```bash
msmctl suspend secretmanagerconfig <name> [--namespace <namespace>]
```

**Arguments:**
- `secretmanagerconfig` (or `smc`): Resource type (required)
- `<name>`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- `--namespace, -n`: Namespace of the resource (defaults to current context namespace)

**Examples:**
```bash
# Suspend reconciliation
msmctl suspend secretmanagerconfig test-sops-config --namespace default

# Using short form 'smc'
msmctl suspend smc test-sops-config
```

**What it does:**
- Sets the `secret-management.octopilot.io/suspend` annotation to `"true"`
- Controller will skip reconciliation for this resource
- Manual reconciliation via `msmctl reconcile` will also be blocked

**To resume:**
```bash
msmctl resume secretmanagerconfig test-sops-config --namespace default
```

### `msmctl resume`

Resume reconciliation for a SecretManagerConfig resource.

**Usage:**
```bash
msmctl resume secretmanagerconfig <name> [--namespace <namespace>]
```

**Arguments:**
- `secretmanagerconfig` (or `smc`): Resource type (required)
- `<name>`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- `--namespace, -n`: Namespace of the resource (defaults to current context namespace)

**Examples:**
```bash
# Resume reconciliation
msmctl resume secretmanagerconfig test-sops-config --namespace default

# Using short form 'smc'
msmctl resume smc test-sops-config
```

**What it does:**
- Removes the `secret-management.octopilot.io/suspend` annotation
- Controller will resume normal reconciliation

### `msmctl suspend-git-pulls`

Suspend Git repository pulls for a SecretManagerConfig resource.

**Usage:**
```bash
msmctl suspend-git-pulls secretmanagerconfig <name> [--namespace <namespace>]
```

**Arguments:**
- `secretmanagerconfig` (or `smc`): Resource type (required)
- `<name>`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- `--namespace, -n`: Namespace of the resource (defaults to current context namespace)

**Examples:**
```bash
# Suspend Git pulls
msmctl suspend-git-pulls secretmanagerconfig test-sops-config --namespace default

# Using short form 'smc'
msmctl suspend-git-pulls smc test-sops-config
```

**What it does:**
- Sets the `secret-management.octopilot.io/suspend-git-pulls` annotation to `"true"`
- Controller will stop checking for updates from the Git repository
- Existing secrets will continue to be reconciled, but new changes from Git will be ignored

**To resume:**
```bash
msmctl resume-git-pulls secretmanagerconfig test-sops-config --namespace default
```

### `msmctl resume-git-pulls`

Resume Git repository pulls for a SecretManagerConfig resource.

**Usage:**
```bash
msmctl resume-git-pulls secretmanagerconfig <name> [--namespace <namespace>]
```

**Arguments:**
- `secretmanagerconfig` (or `smc`): Resource type (required)
- `<name>`: Name of the SecretManagerConfig resource (required, positional)

**Options:**
- `--namespace, -n`: Namespace of the resource (defaults to current context namespace)

**Examples:**
```bash
# Resume Git pulls
msmctl resume-git-pulls secretmanagerconfig test-sops-config --namespace default

# Using short form 'smc'
msmctl resume-git-pulls smc test-sops-config
```

**What it does:**
- Removes the `secret-management.octopilot.io/suspend-git-pulls` annotation
- Controller will resume checking for updates from the Git repository

### `msmctl install`

Install the Secret Manager Controller in a Kubernetes cluster.

**Usage:**
```bash
msmctl install [--namespace <namespace>] [--export]
```

**Options:**
- `--namespace, -n`: Namespace to install the controller in (default: `octopilot-system`)
- `--export`: Export manifests instead of applying them

**Examples:**
```bash
# Install to default namespace
msmctl install

# Install to custom namespace
msmctl install --namespace my-namespace

# Export manifests without installing
msmctl install --export
```

**What it installs:**
- CRD: `SecretManagerConfig` Custom Resource Definition
- Namespace: `octopilot-system` (or specified namespace)
- ServiceAccount, Role, RoleBinding: RBAC resources
- Deployment: Controller deployment

### `msmctl check`

Check the installation and prerequisites of the Secret Manager Controller.

**Usage:**
```bash
msmctl check [--pre]
```

**Options:**
- `--pre`: Check prerequisites only (Kubernetes version, CRDs, etc.)

**Examples:**
```bash
# Full check
msmctl check

# Prerequisites only
msmctl check --pre
```

**What it checks:**
- Kubernetes version compatibility
- CRD availability
- Controller deployment status
- RBAC permissions
- Controller health

## Resource Types

The following resource types are supported:

- `secretmanagerconfig` (or `smc`): SecretManagerConfig resource

## Short Forms

You can use `smc` as a short form for `secretmanagerconfig` in all commands:

```bash
# These are equivalent:
msmctl list secretmanagerconfig
msmctl list smc

msmctl reconcile secretmanagerconfig my-secrets
msmctl reconcile smc my-secrets
```

## RBAC Requirements

The user/service account running `msmctl` needs the following permissions:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: msmctl-user
rules:
- apiGroups: ["secret-management.octopilot.io"]
  resources: ["secretmanagerconfigs"]
  verbs: ["get", "list", "watch", "update", "patch", "delete"]
```

## Troubleshooting

### Command Not Found

If `msmctl` is not found, ensure it's in your PATH:

```bash
# Check if it's installed
which msmctl

# Add to PATH if needed
export PATH="$HOME/.local/bin:$PATH"
```

### Permission Denied

If you get permission errors, check your RBAC permissions:

```bash
# Check if you can list SecretManagerConfig resources
kubectl get secretmanagerconfigs --all-namespaces

# Check your current context
kubectl config current-context
```

### Resource Not Found

If a resource is not found, check the namespace:

```bash
# List all resources
msmctl list secretmanagerconfig

# Check specific namespace
msmctl list secretmanagerconfig --namespace <namespace>
```

## Examples

### Batch Operations

List all resources and check their status:

```bash
for config in $(msmctl list secretmanagerconfig --namespace mysystem | awk 'NR>2 {print $1}'); do
  msmctl status secretmanagerconfig "$config" --namespace mysystem
done
```

### Force Reconciliation for Stuck Resources

If a resource is stuck and not reconciling:

```bash
msmctl reconcile secretmanagerconfig my-secrets --namespace default --force
```

This will delete and wait for GitOps to recreate the resource, then trigger reconciliation.

## Next Steps

- [Quick Start Guide](../getting-started/quick-start.md) - Get started with the controller
- [Configuration Reference](../getting-started/configuration.md) - Learn about configuration options
- [Troubleshooting](../tutorials/troubleshooting.md) - Common issues and solutions

