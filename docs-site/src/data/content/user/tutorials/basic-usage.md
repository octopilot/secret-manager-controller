# Basic Usage Tutorial

Learn how to use Secret Manager Controller with a simple example.

## Step 1: Install the Controller

See [Installation Guide](../getting-started/installation.md) for installation instructions.

## Step 2: Configure Provider

Set up your cloud provider credentials. For AWS:

```bash
kubectl create secret generic aws-credentials \
  --from-literal=AWS_ACCESS_KEY_ID=your-key \
  --from-literal=AWS_SECRET_ACCESS_KEY=your-secret \
  --from-literal=AWS_REGION=us-east-1 \
  -n octopilot-system
```

## Step 3: Create SecretManagerConfig

Create a configuration file:

```yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-app-secrets
  namespace: default
spec:
  provider: aws
  region: us-east-1
  credentials:
    secretRef:
      name: aws-credentials
      namespace: octopilot-system
  secrets:
    - name: database-password
      key: /myapp/database/password
```

Apply it:

```bash
kubectl apply -f secret-config.yaml
```

## Step 4: Verify

Check that the Kubernetes Secret was created:

```bash
kubectl get secret my-app-secrets -n default
kubectl get secret my-app-secrets -n default -o yaml
```

## Step 5: Use in Your Application

Reference the secret in your deployment:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: my-app
spec:
  template:
    spec:
      containers:
      - name: app
        env:
        - name: DB_PASSWORD
          valueFrom:
            secretKeyRef:
              name: my-app-secrets
              key: database-password
```

## Next Steps

- [Advanced Scenarios](./advanced-scenarios.md)
- [Troubleshooting](./troubleshooting.md)

