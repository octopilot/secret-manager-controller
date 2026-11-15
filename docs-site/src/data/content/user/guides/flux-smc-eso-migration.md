# Migrate from Flux SOPS Decrypt to SMC + ESO

Guide for teams moving from **Flux Kustomization in-cluster SOPS decryption** to
**Secret Manager Controller (SMC) + External Secrets Operator (ESO)**.

## When to migrate

Migrate when you need:

- Cloud secret manager as the delivery plane (GCP Secret Manager, AWS Secrets Manager, Azure Key Vault)
- Serverless consumers of the same Git-encrypted secrets
- Tamper detection between Git and cloud (`diffDiscovery`)
- A single pipeline for Kubernetes and non-Kubernetes workloads

**Legacy pattern (deprecate):**

```yaml
apiVersion: kustomize.toolkit.fluxcd.io/v1
kind: Kustomization
spec:
  decryption:
    provider: sops
    secretRef:
      name: sops-gpg
  # ... builds Secrets directly in cluster from encrypted git files
```

**Target pattern:**

```
Git (SOPS) → GitOps artifact → SecretManagerConfig → Cloud SM → ESO → Kubernetes Secret
```

## Prerequisites

- [SMC installed](../getting-started/installation.md)
- [External Secrets Operator](https://external-secrets.io/) installed
- `ClusterSecretStore` for your cloud provider (Workload Identity / IRSA preferred)
- SOPS-encrypted files already in Git (see [SOPS Setup](./sops-setup.md))
- Flux `GitRepository` (or Argo `Application`) syncing the app deploy config repo

## Migration steps

### 1. Inventory

Document per environment overlay:

| Item | Discover from repo |
|------|-------------------|
| Kustomize overlay path | `kustomization.yaml` location |
| Secret file names | `application.secrets.env`, `secretGenerator` |
| Flux `GitRepository` name/namespace | cluster gitops |
| Helm `envFrom` / secret references | chart values |
| Existing `decryption.provider: sops` | app Kustomization |

### 2. Install SMC + wire provider

Create `ClusterSecretStore` (or namespaced store) matching SMC `spec.provider`.

### 3. Add SecretManagerConfig per overlay

```yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: <service>-<env>
  namespace: <smc-operator-namespace>
spec:
  sourceRef:
    kind: GitRepository
    name: <app-git-repository>
    namespace: <gitops-namespace>
  provider:
    gcp:
      projectId: <project-id>
  secrets:
    environment: <env>
    kustomizePath: <path-from-repo-root-to-overlay>
    sops:
      enabled: true
      gpgSecretRef:
        name: sops-gpg-key
        namespace: <namespace>
        key: private.key
  diffDiscovery: true
  reconcileInterval: 1m
```

Verify SMC status conditions show successful sync to cloud.

### 4. Add ExternalSecret per workload namespace

See [Helm and ESO naming](./helm-eso-naming.md) for `target.name` alignment.

### 5. Verify parallel run (recommended)

1. Keep Flux decrypt active briefly.
2. Confirm ESO-created Secret matches keys expected by Helm.
3. Roll one non-production environment first.

### 6. Remove legacy Flux decrypt

On the app `Kustomization`, remove:

```yaml
decryption:
  provider: sops
  secretRef:
    name: sops-gpg
```

Ensure Helm no longer depends on Flux-generated Secrets from that path alone.

### 7. Validate rollback plan

- Re-enable `decryption.provider: sops` only if ESO path fails
- Document cloud secret IDs for manual inspection

## diffDiscovery operations

With `diffDiscovery: true`, SMC logs when cloud secrets drift from Git.

**Recommended:**

- Ship controller logs to your observability stack
- Alert on reconciliation errors and diff warnings
- Treat unexpected cloud edits as incidents — Git is source of truth

## Troubleshooting

| Symptom | Check |
|---------|-------|
| SMC not syncing | `GitRepository` artifact path; `kustomizePath` relative to repo root |
| SOPS decrypt fail | `gpgSecretRef` key material; `.sops.yaml` fingerprints |
| ESO empty Secret | `ClusterSecretStore` auth; cloud secret ID from SMC status |
| Helm pod missing env | `ExternalSecret.target.name` vs chart `envFrom.secretRef.name` |
| Dual secrets conflict | Remove Flux decrypt after ESO verified |

## Related

- [Helm and ESO naming](./helm-eso-naming.md)
- [GitOps Integration](./gitops-integration.md)
- [SOPS Setup](./sops-setup.md)
- [CRD Reference](../api-reference/crd-reference.md)
