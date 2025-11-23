# Troubleshooting Guide

Common issues and solutions.

## Controller Not Running

### Check Pod Status

```bash
kubectl get pods -n microscaler-system
```

### Check Logs

```bash
kubectl logs -n microscaler-system -l app=secret-manager-controller
```

### Common Causes

- Missing RBAC permissions
- Image pull errors
- Resource constraints

## Secrets Not Created

### Verify SecretManagerConfig

```bash
kubectl get secretmanagerconfig -A
kubectl describe secretmanagerconfig <name> -n <namespace>
```

### Check Status

The status field shows:
- Last sync time
- Error messages
- Secret count

### Common Issues

1. **Authentication Failed**
   - Verify provider credentials
   - Check IAM/role permissions
   - Ensure credentials secret exists

2. **Secret Not Found**
   - Verify secret exists in provider
   - Check secret key/path
   - Verify permissions include the secret

3. **Network Issues**
   - Check cluster network connectivity
   - Verify VPC endpoints (if using)
   - Check firewall rules

## Secrets Not Updating

### Check Update Policy

```yaml
spec:
  updatePolicy: Always  # Ensure this is set
```

### Verify Reconciliation

Check controller logs for reconciliation events.

### Force Reconciliation

Delete and recreate the SecretManagerConfig:

```bash
kubectl delete secretmanagerconfig <name> -n <namespace>
kubectl apply -f config.yaml
```

## Provider-Specific Issues

### AWS

- Verify IAM role/credentials
- Check region configuration
- Ensure Secrets Manager is enabled in region

### Azure

- Verify managed identity or service principal
- Check Key Vault URL format
- Ensure Key Vault access policies

### GCP

- Verify service account permissions
- Check project ID
- Ensure Secret Manager API is enabled

## Getting Help

- Check controller logs
- Review SecretManagerConfig status
- Verify provider credentials
- Check network connectivity

## Next Steps

- [Basic Usage](./basic-usage.md)
- [Advanced Scenarios](./advanced-scenarios.md)

