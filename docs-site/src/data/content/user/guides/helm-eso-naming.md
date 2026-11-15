# Helm and ESO Secret Naming

Conventions for aligning **Helm charts**, **External Secrets Operator (ESO)**, and
**Secret Manager Controller (SMC)** so workloads receive the correct environment variables.

## Problem

Three names participate in the chain:

1. **Kustomize generator name** in Git (`secretGenerator.name`)
2. **Cloud secret ID** after SMC sync (provider-specific)
3. **Kubernetes Secret name** created by ESO (`ExternalSecret.spec.target.name`)
4. **Helm chart reference** (`envFrom.secretRef.name` or `valuesFrom` ConfigMap/Secret)

A mismatch at any step causes pods to start with missing or stale credentials.

## Recommended convention

Use a stable, predictable pattern (adjust prefix to your org):

```
<app>-<env>-secrets
```

Examples (illustrative only):

| Layer | Name |
|-------|------|
| Kustomize `secretGenerator` | `<app>-sops-secrets` (git-side; may differ) |
| Cloud secret (SMC) | `<app>-<env>-secrets` (discover from SMC status/logs) |
| ESO `target.name` | `<app>-<env>-secrets` |
| Helm `envFrom.secretRef.name` | `<app>-<env>-secrets` |

**DO** use `disableNameSuffixHash: true` on generators when Helm `valuesFrom` requires exact ConfigMap names.

**DON'T** rely on hash-suffixed secret names with Helm `valuesFrom` unless your Flux/Helm pipeline resolves hashes explicitly.

## ExternalSecret template

```yaml
apiVersion: external-secrets.io/v1
kind: ExternalSecret
metadata:
  name: <app>-<env>-externalsecret
  namespace: <workload-namespace>
spec:
  refreshInterval: 1m
  secretStoreRef:
    kind: ClusterSecretStore
    name: <discover-from-cluster>
  target:
    name: <app>-<env>-secrets
    creationPolicy: Owner
  dataFrom:
    - extract:
        key: <cloud-secret-id-from-smc>
```

### Discovering `cloud-secret-id`

- Inspect SMC controller logs after reconciliation
- Use provider CLI (`gcloud secrets list`, etc.) with labels if your platform adds them
- Document the mapping in the service README or gitops overlay

## Helm chart alignment

### envFrom pattern

```yaml
# values.yaml (illustrative)
deployment:
  injectEnv: true
  envFromSecret: <app>-<env>-secrets
```

Chart template:

```yaml
envFrom:
  - secretRef:
      name: {{ .Values.deployment.envFromSecret }}
```

### valuesFrom ConfigMap (non-secret config)

Keep **secrets** on ESO path; **non-secret** config may still use:

```yaml
valuesFrom:
  - kind: ConfigMap
    name: <app>-helm-config
```

Ensure ConfigMap name stability matches your kustomize `generatorOptions`.

## One SMC per service vs monorepo

| Approach | When |
|----------|------|
| **One SecretManagerConfig per service/env** | Clear blast radius; independent reconcile |
| **One SMC per repo with multiple paths** | Small number of services; shared GitRepository |

For monorepos, use distinct `kustomizePath` per overlay and separate `ExternalSecret` per workload namespace.

## Rotation

1. Update SOPS file in Git; commit.
2. SMC syncs cloud secret.
3. ESO `refreshInterval` picks up change (or force reconcile).
4. Restart pods if the application does not reload secrets (discover chart behavior).

Coordinate GPG key rotation with [SOPS Setup](./sops-setup.md) and CI re-encrypt workflows.

## Related

- [Migrate Flux decrypt to SMC + ESO](./flux-smc-eso-migration.md)
- [GitOps Integration](./gitops-integration.md)
- [Application Files](./application-files.md)
