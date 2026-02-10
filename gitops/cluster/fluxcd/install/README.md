# FluxCD Installation with Patches

This directory contains FluxCD installation manifests with patches to enable integration with the secret-manager-controller.

## Structure

```
gitops/cluster/fluxcd/install/
├── fluxcd-install.yaml          # FluxCD installation manifests (from flux install --export)
├── kustomization.yaml           # Kustomize configuration (includes FluxCD + patches)
├── README.md                    # This file
└── patches/
    ├── networkpolicy-allow-egress.yaml  # NetworkPolicy patch
    └── README.md                        # Patches documentation

Note: Namespace label is defined in config/namespace.yaml (DRY principle)
```

## Usage

### Install FluxCD with Patches (Recommended)

```bash
# Install FluxCD with all patches applied
# This includes the namespace label from config/namespace.yaml (DRY principle)
kubectl apply -k gitops/cluster/fluxcd/install/
```

### What Gets Installed

1. **FluxCD Components** (from `fluxcd-install.yaml`):
   - CustomResourceDefinitions (GitRepository, Kustomization, HelmRelease, etc.)
   - RBAC resources (ServiceAccounts, Roles, RoleBindings, ClusterRoles, ClusterRoleBindings)
   - Deployments (source-controller, kustomize-controller, helm-controller, notification-controller)
   - Services
   - NetworkPolicies

2. **Patches Applied**:
   - NetworkPolicy `allow-egress` is patched to allow ingress from `octopilot-system` namespace
   - This enables the secret-manager-controller to download FluxCD artifacts

3. **Additional Resources**:
   - Namespace label for `octopilot-system` (from `config/namespace.yaml` - DRY principle)

## Regenerating FluxCD Manifests

If you need to regenerate the FluxCD installation manifests:

```bash
# Export current FluxCD installation
flux install --export --namespace=flux-system > gitops/cluster/fluxcd/install/fluxcd-install.yaml
```

**Note:** After regenerating, verify that the patches still apply correctly:
```bash
kubectl kustomize gitops/cluster/fluxcd/install/ | grep -A 20 "kind: NetworkPolicy" | grep -A 20 "name: allow-egress"
```

## Integration with Tilt

The Tilt setup script (`scripts/tilt/install_fluxcd.py`) uses this kustomization automatically:

```python
# Installation uses kubectl apply -k which includes patches
kubectl apply -k gitops/cluster/fluxcd/install/
```

Patches are included automatically, so no separate patch application step is needed.

## Troubleshooting

### NetworkPolicy Patch Not Applied

If the NetworkPolicy patch doesn't apply correctly:

1. **Verify FluxCD is installed:**
   ```bash
   kubectl get networkpolicy allow-egress -n flux-system
   ```

2. **Apply patch manually:**
   ```bash
   kubectl patch networkpolicy allow-egress -n flux-system \
     --type=strategic \
     --patch-file=gitops/cluster/fluxcd/install/patches/networkpolicy-allow-egress.yaml
   ```

3. **Verify namespace label exists:**
   ```bash
   kubectl get namespace octopilot-system -o jsonpath='{.metadata.labels.name}'
   ```
   Should output: `octopilot-system`

### Kustomize Errors

If you see namespace transformation conflicts:

- The namespace label is included from `config/namespace.yaml` via the kustomization
- Kustomize handles namespace transformation automatically
- If issues occur, verify the namespace exists: `kubectl get namespace octopilot-system`

## See Also

- [Patches README](patches/README.md) - Detailed documentation about the patches
- [FluxCD Multi-Namespace Configuration](../FLUXCD_MULTI_NAMESPACE.md) - How to configure FluxCD to watch multiple namespaces

