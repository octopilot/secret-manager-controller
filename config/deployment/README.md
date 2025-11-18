# Deployment Configuration

## Default Deployment

The default deployment (`deployment.yaml`) does NOT include Pact mode environment variables. This is the production-ready configuration that connects to real cloud provider APIs.

## Pact Mode (Testing/Development)

To enable Pact mode for testing without cloud accounts, apply the Pact environment variables patch:

### Option 1: kubectl patch (one-time)

```bash
kubectl patch deployment secret-manager-controller -n microscaler-system \
  --patch-file config/deployment/pact-env-patch.yaml
```

### Option 2: Kustomize overlay (recommended for persistent use)

Create a `kustomization.yaml` overlay:

```yaml
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization

resources:
  - ../../config

patches:
  - path: deployment/pact-env-patch.yaml
    target:
      kind: Deployment
      name: secret-manager-controller
```

Then apply:

```bash
kubectl apply -k ./overlays/pact-mode
```

### Option 3: Manual environment variable injection

```bash
kubectl set env deployment/secret-manager-controller -n microscaler-system \
  PACT_MODE=true \
  AWS_SECRETS_MANAGER_ENDPOINT=http://pact-broker.secret-manager-controller-pact-broker.svc.cluster.local:9292 \
  AZURE_KEY_VAULT_ENDPOINT=http://pact-broker.secret-manager-controller-pact-broker.svc.cluster.local:9292
```

## Pact Endpoints

The patch file uses Kubernetes service names for in-cluster communication:
- Service: `pact-broker`
- Namespace: `secret-manager-controller-pact-broker`
- Port: `9292` (default Pact broker port)

Both AWS and Azure endpoints point to the same Pact broker service. If you need separate mock servers for different providers, you can:
1. Deploy additional Pact broker instances on different ports
2. Update the endpoints in `pact-env-patch.yaml` accordingly

## Removing Pact Mode

To disable Pact mode and return to production configuration:

```bash
kubectl set env deployment/secret-manager-controller -n microscaler-system \
  PACT_MODE- \
  AWS_SECRETS_MANAGER_ENDPOINT- \
  AZURE_KEY_VAULT_ENDPOINT-
```

