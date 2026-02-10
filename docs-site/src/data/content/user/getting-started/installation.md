# Installation

This guide will help you install the Secret Manager Controller in your Kubernetes cluster.

## Prerequisites

- Kubernetes cluster (v1.20+)
- `kubectl` configured to access your cluster
- Helm 3.x (optional, for Helm installation)
- GitOps tool installed (FluxCD or ArgoCD) - see [GitOps Integration Guide](../guides/gitops-integration.md)

## Installation Methods

### Method 1: Using kubectl (Recommended)

1. **Apply the CRD:**

```bash
kubectl apply -f https://raw.githubusercontent.com/octopilot/secret-manager-controller/main/config/crd/secretmanagerconfig.yaml
```

2. **Apply the controller manifests:**

```bash
kubectl apply -k https://github.com/octopilot/secret-manager-controller/config/
```

This will create:
- The `octopilot-system` namespace
- ServiceAccount, Role, and RoleBinding for the controller
- Deployment for the controller

### Method 2: Using Helm

```bash
# Add the Helm repository
helm repo add secret-manager-controller https://octopilot.github.io/secret-manager-controller
helm repo update

# Install the controller
helm install secret-manager-controller secret-manager-controller/secret-manager-controller
```

## Verify Installation

Check that the controller is running:

```bash
kubectl get pods -n octopilot-system
```

You should see the `secret-manager-controller` pod in `Running` state:

```
NAME                                      READY   STATUS    RESTARTS   AGE
secret-manager-controller-xxxxxxxxxx-xxx  1/1     Running   0          1m
```

Check the controller logs:

```bash
kubectl logs -n octopilot-system -l app=secret-manager-controller --tail=50
```

## Cloud Provider Setup

Before using the controller, you'll need to configure authentication for your cloud provider:

- **GCP**: Set up [Workload Identity](https://cloud.google.com/kubernetes-engine/docs/how-to/workload-identity) or service account key
- **AWS**: Configure [IRSA (IAM Roles for Service Accounts)](https://docs.aws.amazon.com/eks/latest/userguide/iam-roles-for-service-accounts.html) or access keys
- **Azure**: Set up [Workload Identity](https://learn.microsoft.com/en-us/azure/aks/workload-identity) or service principal

See the provider-specific setup guides:
- [AWS Setup Guide](../guides/aws-setup.md)
- [Azure Setup Guide](../guides/azure-setup.md)
- [GCP Setup Guide](../guides/gcp-setup.md)

## Next Steps

- [Quick Start Guide](./quick-start.md) - Get up and running in minutes
- [Configuration](./configuration.md) - Learn about configuration options
- [Architecture Overview](../architecture/overview.md) - Understand how it works
