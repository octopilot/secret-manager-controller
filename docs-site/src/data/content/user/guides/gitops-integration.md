# GitOps Integration

The Secret Manager Controller is GitOps-agnostic and works with both FluxCD and ArgoCD.

## Supported GitOps Tools

### FluxCD

The controller integrates with FluxCD's `GitRepository` CRD and source-controller.

**Requirements:**
- FluxCD source-controller installed
- `GitRepository` resource created
- Artifacts available in `/tmp/flux-source-*` directories

**Example GitRepository:**

```yaml
apiVersion: source.toolkit.fluxcd.io/v1
kind: GitRepository
metadata:
  name: my-secrets-repo
  namespace: octopilot-system
spec:
  url: https://github.com/your-org/your-secrets-repo
  interval: 5m
  ref:
    branch: main
  secretRef:
    name: git-credentials  # Optional: for private repos
```

**Reference in SecretManagerConfig:**

```yaml
spec:
  sourceRef:
    kind: GitRepository
    name: my-secrets-repo
    namespace: octopilot-system
```

### ArgoCD

The controller integrates with ArgoCD's `Application` CRD.

**Requirements:**
- ArgoCD installed
- `Application` resource created
- Repository accessible from controller

**Example Application:**

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: my-secrets-app
  namespace: argocd
spec:
  project: default
  source:
    repoURL: https://github.com/your-org/your-secrets-repo
    targetRevision: main
    path: .
  destination:
    server: https://kubernetes.default.svc
    namespace: default
```

**Reference in SecretManagerConfig:**

```yaml
spec:
  sourceRef:
    kind: Application
    name: my-secrets-app
    namespace: argocd
```

## Repository Structure

Your Git repository should be organized like this:

```
your-secrets-repo/
├── microservices/
│   └── my-service/
│       └── deployment-configuration/
│           └── profiles/
│               ├── dev/
│               │   ├── kustomization.yaml
│               │   └── secrets.yaml  # SOPS-encrypted
│               ├── staging/
│               │   ├── kustomization.yaml
│               │   └── secrets.yaml
│               └── prod/
│                   ├── kustomization.yaml
│                   └── secrets.yaml
└── application.properties  # Optional: for config stores
```

## Kustomize Path Configuration

The `kustomizePath` in your SecretManagerConfig should point to the Kustomize overlay:

```yaml
spec:
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
```

This path is relative to the repository root.

## Private Repositories

### FluxCD

For private repositories, create a Kubernetes Secret with Git credentials:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: git-credentials
  namespace: octopilot-system
type: Opaque
stringData:
  username: your-username
  password: your-token-or-password
```

Reference it in your GitRepository:

```yaml
spec:
  secretRef:
    name: git-credentials
```

### ArgoCD

Configure repository credentials in ArgoCD:

```bash
argocd repo add https://github.com/your-org/private-repo \
  --username your-username \
  --password your-token
```

Or use SSH keys:

```bash
argocd repo add git@github.com:your-org/private-repo.git \
  --ssh-private-key-path ~/.ssh/id_rsa
```

## Branch and Tag Support

### FluxCD

Specify branch or tag in GitRepository:

```yaml
spec:
  ref:
    branch: main
    # OR
    tag: v1.0.0
    # OR
    commit: abc123def456
```

### ArgoCD

Specify target revision in Application:

```yaml
spec:
  source:
    targetRevision: main  # branch, tag, or commit
```

## Update Intervals

### GitRepository Pull Interval

How often the controller checks for Git updates:

```yaml
spec:
  gitRepositoryPullInterval: 5m  # Default: 5m, minimum: 1m
```

**Recommendation:** 5 minutes or greater to avoid Git API rate limits.

### Reconcile Interval

How often the controller reconciles secrets:

```yaml
spec:
  reconcileInterval: 1m  # Default: 1m
```

## Troubleshooting

### GitRepository Not Found

**Error:** `GitRepository "my-repo" not found`

**Solution:**
1. Verify the GitRepository exists:
   ```bash
   kubectl get gitrepository -n octopilot-system
   ```
2. Check the `sourceRef` name and namespace match
3. Ensure FluxCD source-controller is running

### Artifacts Not Available

**Error:** `No artifacts found for GitRepository`

**Solution:**
1. Check source-controller logs:
   ```bash
   kubectl logs -n flux-system -l app=source-controller
   ```
2. Verify GitRepository status:
   ```bash
   kubectl describe gitrepository my-repo -n octopilot-system
   ```
3. Check artifact directory exists:
   ```bash
   kubectl exec -n flux-system -l app=source-controller -- ls -la /tmp/flux-source-*
   ```

### ArgoCD Application Not Found

**Error:** `Application "my-app" not found`

**Solution:**
1. Verify the Application exists:
   ```bash
   kubectl get application -n argocd
   ```
2. Check the `sourceRef` name and namespace match
3. Ensure ArgoCD is running and can access the repository

## Best Practices

1. **Use Kustomize Overlays**: Organize secrets by environment using Kustomize
2. **SOPS Encryption**: Encrypt all secrets in Git (see [SOPS Setup](./sops-setup.md))
3. **Separate Repositories**: Consider separate repos for secrets vs. application code
4. **Branch Protection**: Use branch protection rules for production secrets
5. **Audit Logging**: Enable Git audit logs for secret changes
6. **Access Control**: Limit who can push to secrets repositories

## Next Steps

- [SOPS Setup](./sops-setup.md) - Encrypt secrets in Git
- [Provider Setup Guides](./aws-setup.md) - Configure cloud providers
- [Configuration Reference](../getting-started/configuration.md) - Complete configuration guide
