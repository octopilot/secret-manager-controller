# FluxCD Patches

This directory contains Kustomize patches for modifying FluxCD-managed resources to enable integration with the secret-manager-controller.

## Purpose

FluxCD installs resources in the `flux-system` namespace with default configurations that may not allow access from other namespaces. These patches modify FluxCD resources to enable the secret-manager-controller (running in `octopilot-system` namespace) to access FluxCD services.

## Patches

### `networkpolicy-allow-egress.yaml`

Modifies FluxCD's `allow-egress` NetworkPolicy to allow ingress from the `octopilot-system` namespace.

**Why this is needed:**
- The controller needs to download FluxCD artifacts via HTTP from `source-controller` service
- FluxCD's default network policy only allows ingress from `flux-system` namespace
- The controller runs in `octopilot-system` namespace
- Without this patch, artifact downloads will timeout

**What it does:**
- Adds `namespaceSelector` to allow ingress from `octopilot-system` namespace
- Explicitly allows ports 80 (service port) and 9090 (container port)

### Namespace Label

The `octopilot-system` namespace label is defined in `config/namespace.yaml` (DRY principle).

**Why this is needed:**
- The NetworkPolicy patch uses `namespaceSelector.matchLabels.name: octopilot-system`
- This label must exist on the namespace for the selector to match
- The label is included in the main kustomization via `config/namespace.yaml`

## Usage

### Apply All Patches (Recommended)

**Via Kustomize (includes FluxCD installation):**
```bash
# Apply FluxCD installation with patches included
# Namespace label is included from config/namespace.yaml (DRY principle)
kubectl apply -k gitops/cluster/fluxcd/install/
```

**Manual application (if FluxCD already installed):**
```bash
# Apply namespace label (from config/namespace.yaml)
kubectl apply -f config/namespace.yaml

# Apply NetworkPolicy patch
kubectl patch networkpolicy allow-egress -n flux-system \
  --type=strategic \
  --patch-file=gitops/cluster/fluxcd/install/patches/networkpolicy-allow-egress.yaml
```

### Apply Individual Patches

```bash
# Apply namespace label only (from config/namespace.yaml - DRY principle)
kubectl apply -f config/namespace.yaml

# Apply NetworkPolicy patch only (requires namespace label to exist first)
kubectl patch networkpolicy allow-egress -n flux-system \
  --type=strategic \
  --patch-file=gitops/cluster/fluxcd/install/patches/networkpolicy-allow-egress.yaml
```

### Verify Patches Applied

```bash
# Check NetworkPolicy
kubectl get networkpolicy allow-egress -n flux-system -o yaml | grep -A 10 "ingress:"

# Check namespace label
kubectl get namespace octopilot-system -o jsonpath='{.metadata.labels.name}'
```

## When to Apply

These patches should be applied:

1. **After FluxCD installation** - Apply immediately after `flux install`
2. **After FluxCD upgrade** - Reapply if FluxCD is upgraded/reinstalled
3. **As part of cluster setup** - Include in your cluster initialization scripts

## Integration with Tilt

The Tilt setup script (`scripts/tilt/install_fluxcd.py`) applies these patches automatically via Kustomize:

```python
# Installation uses kubectl apply -k which includes patches
kubectl apply -k gitops/cluster/fluxcd/install/
```

Patches are included in the kustomization, so no separate patch application step is needed.

## Troubleshooting

### Artifact Downloads Still Failing

1. **Verify NetworkPolicy is patched:**
   ```bash
   kubectl get networkpolicy allow-egress -n flux-system -o yaml | grep -A 15 "ingress:"
   ```
   Should show `namespaceSelector` with `name: octopilot-system`

2. **Verify namespace label exists:**
   ```bash
   kubectl get namespace octopilot-system -o jsonpath='{.metadata.labels.name}'
   ```
   Should output: `octopilot-system`

3. **Test connectivity from controller pod:**
   ```bash
   CONTROLLER_POD=$(kubectl get pods -n octopilot-system -l app=secret-manager-controller -o jsonpath='{.items[0].metadata.name}')
   kubectl exec -n octopilot-system $CONTROLLER_POD -- \
     curl -v http://source-controller.flux-system.svc.cluster.local/gitrepository/tilt/gitrepository-tilt/<sha>.tar.gz
   ```
   Should return HTTP 200 OK

### Patches Overwritten by FluxCD

If FluxCD is reinstalled or upgraded, these patches may be overwritten. Reapply them:

```bash
# Reapply FluxCD installation with patches
# Namespace label is included from config/namespace.yaml
kubectl apply -k gitops/cluster/fluxcd/install/
```

Consider adding this to your FluxCD upgrade procedures.

## Security Considerations

These patches allow ingress from the `octopilot-system` namespace to FluxCD services. This is necessary for the controller to function, but consider:

- The NetworkPolicy still restricts access to specific ports (80, 9090)
- Only pods in `octopilot-system` namespace can access FluxCD services
- The controller has RBAC permissions to access FluxCD resources

If you need stricter security, consider:
- Using a dedicated service account for the controller
- Adding podSelector to further restrict which pods can access FluxCD
- Using network policies at the cluster level for additional defense in depth

