# ArgoCD CRDs

This directory contains ArgoCD CRDs exported from a Kind cluster.

## Contents

- `applications.argoproj.io.yaml` - Application CRD (main CRD used by controller)
- `applicationsets.argoproj.io.yaml` - ApplicationSet CRD
- `appprojects.argoproj.io.yaml` - AppProject CRD
- `kustomization.yaml` - Kustomize file for easy application

## Usage

### Apply All CRDs

```bash
kubectl apply -k pact-broker/argocd/
```

### Apply Individual CRD

```bash
kubectl apply -f pact-broker/argocd/applications.argoproj.io.yaml
```

## Purpose

These CRDs are the minimal installation needed for the secret-manager-controller to work with ArgoCD Application resources. The controller clones repos itself using the git binary, so we only need the CRDs, not the full ArgoCD installation (server, controllers, etc.).

## Exporting CRDs

To re-export CRDs from a cluster:

```bash
# Export all ArgoCD CRDs
kubectl get crd -o name | grep argoproj | while read crd; do
  name=$(echo $crd | cut -d'/' -f2)
  kubectl get crd $name -o yaml > "pact-broker/argocd/${name}.yaml"
done
```

## Notes

- These CRDs are cluster-scoped resources
- They can be applied to any cluster that needs ArgoCD Application support
- The controller only uses the `Application` CRD, but we include all ArgoCD CRDs for completeness

