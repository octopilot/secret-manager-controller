# Troubleshooting Guide

Common issues, error messages, and solutions for the Secret Manager Controller.

## Controller Not Running

### Check Pod Status

```bash
kubectl get pods -n octopilot-system
```

### Check Logs

```bash
kubectl logs -n octopilot-system -l app=secret-manager-controller
```

### Common Causes

- Missing RBAC permissions
- Image pull errors
- Resource constraints

---

## ❌ GitRepository Not Found

### Error Message

```
WARN GitRepository my-repo not found yet, waiting for watch event
ERROR sourceRef GitRepository my-repo not found in namespace flux-system
```

### Diagnosis

```bash
# Check if GitRepository exists
kubectl get gitrepository -n flux-system

# Check GitRepository status
kubectl describe gitrepository <name> -n <namespace>
```

### Solutions

1. **Verify namespace**: Ensure the GitRepository exists in the namespace specified in `sourceRef.namespace`
2. **Check FluxCD installation**: Verify FluxCD is installed and running
   ```bash
   kubectl get pods -n flux-system
   ```
3. **Verify RBAC**: Ensure the controller has permissions to read GitRepository resources
   ```bash
   kubectl auth can-i get gitrepositories -n flux-system --as=system:serviceaccount:octopilot-system:secret-manager-controller
   ```
4. **Check GitRepository status**: Ensure the GitRepository has successfully synced
   ```bash
   kubectl get gitrepository <name> -n <namespace> -o yaml
   ```

---

## ❌ SOPS Key Not Found

### Error Message

```
ERROR unable to read SOPS key from kubernetes secret
ERROR sops decryption failed: no GPG key found
WARN SOPS decryption skipped: GPG key secret not found
```

### Diagnosis

```bash
# Check if SOPS GPG secret exists
kubectl get secret sops-gpg-key -n <namespace>

# Verify secret contains the key
kubectl get secret sops-gpg-key -n <namespace> -o jsonpath='{.data.private\.key}' | base64 -d
```

### Solutions

1. **Create the SOPS GPG secret** in the same namespace as your SecretManagerConfig:
   ```bash
   kubectl create secret generic sops-gpg-key \
     --from-file=private.key=/path/to/private.key \
     -n <namespace>
   ```

2. **Verify secret reference** in your SecretManagerConfig matches:
   ```yaml
   spec:
     secretsConfig:
       sops:
         enabled: true
         gpgSecretRef:
           name: sops-gpg-key
           namespace: <namespace>
           key: private.key
   ```

3. **Check secret permissions**: Ensure the controller ServiceAccount can read the secret
   ```bash
   kubectl auth can-i get secret sops-gpg-key -n <namespace> --as=system:serviceaccount:octopilot-system:secret-manager-controller
   ```

---

## ❌ Cloud Provider Auth Failure

### AWS Error

```
ERROR aws::auth: permission denied
ERROR AccessDeniedException: User is not authorized to perform: secretsmanager:PutSecretValue
```

### GCP Error

```
ERROR gcp::auth: permission denied
ERROR Permission denied (403): The caller does not have permission
```

### Azure Error

```
ERROR azure::auth: authentication failed
ERROR 401 Unauthorized: Access token is missing or invalid
```

### Solutions

#### AWS

1. **Verify IAM role/credentials**: Check that the IAM role has the required permissions
2. **Check IRSA setup** (if using EKS):
   ```bash
   kubectl get serviceaccount secret-manager-controller -n octopilot-system -o yaml
   ```
3. **Verify region**: Ensure the region in your SecretManagerConfig matches your AWS credentials

#### GCP

1. **Verify Workload Identity** (if using GKE):
   ```bash
   kubectl get serviceaccount secret-manager-controller -n octopilot-system -o yaml
   ```
2. **Check service account permissions**: Ensure the GCP service account has `roles/secretmanager.admin` or `roles/secretmanager.secretAccessor`
3. **Verify project ID**: Ensure the project ID in your SecretManagerConfig is correct

#### Azure

1. **Verify managed identity or service principal**: Check authentication method
2. **Check Key Vault access policies**: Ensure the identity has `Get`, `Set`, `List` permissions
3. **Verify Key Vault URL format**: Should be `https://<vault-name>.vault.azure.net/`

---

## ❌ Artifact Checksum Mismatch

### Error Message

```
ERROR artifact SHA256 verification failed
ERROR artifact checksum mismatch: expected abc123, got def456
WARN artifact integrity check failed, retrying
```

### Diagnosis

```bash
# Check GitRepository artifact status
kubectl get gitrepository <name> -n <namespace> -o yaml | grep -A 5 artifact

# Check controller logs for artifact details
kubectl logs -n octopilot-system -l app=secret-manager-controller | grep artifact
```

### Solutions

1. **Wait for Flux to commit new artifact**: The checksum mismatch often occurs when Flux is still processing a new commit
2. **Verify GitRepository is synced**: Ensure Flux has successfully synced the repository
   ```bash
   kubectl get gitrepository <name> -n <namespace>
   ```
3. **Check for concurrent updates**: Multiple controllers updating the same artifact can cause checksum issues
4. **Force reconciliation**: Delete and recreate the SecretManagerConfig if the issue persists

---

## ❌ Secrets Not Created in Cloud Provider

### Error Message

```
WARN secret not created: provider returned error
ERROR failed to create secret in cloud provider
```

### Diagnosis

```bash
# Check SecretManagerConfig status
kubectl get secretmanagerconfig <name> -n <namespace> -o yaml

# Check controller logs
kubectl logs -n octopilot-system -l app=secret-manager-controller | grep -i secret
```

### Common Issues

1. **Authentication Failed**
   - Verify provider credentials
   - Check IAM/role permissions
   - Ensure credentials secret exists

2. **Secret Not Found in Source**
   - Verify secret exists in Git repository
   - Check secret key/path in Kustomize overlay
   - Verify SOPS decryption succeeded (if using SOPS)

3. **Network Issues**
   - Check cluster network connectivity to cloud provider
   - Verify VPC endpoints (if using private networking)
   - Check firewall rules

4. **Provider-Specific Issues**
   - **AWS**: Verify Secrets Manager is enabled in the region
   - **GCP**: Ensure Secret Manager API is enabled in the project
   - **Azure**: Verify Key Vault exists and is accessible

---

## ❌ Secrets Not Updating

### Error Message

```
WARN secret not updated: no changes detected
INFO reconciliation skipped: secret unchanged
```

### Diagnosis

```bash
# Check reconciliation interval
kubectl get secretmanagerconfig <name> -n <namespace> -o yaml | grep reconcileInterval

# Check last sync time
kubectl get secretmanagerconfig <name> -n <namespace> -o jsonpath='{.status.lastSyncTime}'
```

### Solutions

1. **Check update policy**: Ensure secrets are configured to update
2. **Verify reconciliation interval**: Check that `reconcileInterval` is set appropriately
   ```yaml
   spec:
     reconcileInterval: 1m  # Check every minute
   ```
3. **Force reconciliation**: Delete and recreate the SecretManagerConfig:
   ```bash
   kubectl delete secretmanagerconfig <name> -n <namespace>
   kubectl apply -f config.yaml
   ```
4. **Check GitRepository sync**: Ensure Flux has synced the latest changes from Git

---

## ❌ Hot Reload Not Working

### Error Message

```
WARN hot reload not configured
INFO hot reload disabled for this workload
```

### Diagnosis

```bash
# Check if hot reload is enabled
kubectl get secretmanagerconfig <name> -n <namespace> -o yaml | grep -A 5 hotReload

# Check controller logs
kubectl logs -n octopilot-system -l app=secret-manager-controller | grep -i reload
```

### Solutions

1. **Enable hot reload** in your SecretManagerConfig:
   ```yaml
   spec:
     hotReload:
       enabled: true
   ```
2. **Verify workload supports hot reload**: Not all workloads support hot reload (file watchers, SIGHUP, HTTP endpoints)
3. **Check ConfigMap updates**: Ensure the ConfigMap is being updated by the controller
   ```bash
   kubectl get configmap -n <namespace>
   kubectl describe configmap <name> -n <namespace>
   ```

---

## ❌ Pod Did Not Restart After Secret Update

### Common Misconception

**Kubernetes does NOT automatically restart pods when Secrets or ConfigMaps are updated.**

### Solutions

1. **Use hot reload**: Enable hot reload in your SecretManagerConfig if your application supports it
2. **Manual restart**: Restart the pod/deployment manually:
   ```bash
   kubectl rollout restart deployment <name> -n <namespace>
   ```
3. **Use external tools**: Use tools like Reloader or Stakater Reloader to automatically restart pods on Secret/ConfigMap changes
4. **Application-level reload**: Configure your application to watch for file changes or listen for SIGHUP signals

---

## Provider-Specific Issues

### AWS

- **Verify IAM role/credentials**: Check that the IAM role has `secretsmanager:PutSecretValue` and `secretsmanager:CreateSecret` permissions
- **Check region configuration**: Ensure the region in your SecretManagerConfig matches your AWS credentials region
- **Ensure Secrets Manager is enabled**: Verify Secrets Manager is enabled in the specified region

### Azure

- **Verify managed identity or service principal**: Check authentication method and credentials
- **Check Key Vault URL format**: Should be `https://<vault-name>.vault.azure.net/`
- **Ensure Key Vault access policies**: Verify the identity has `Get`, `Set`, `List` permissions on the Key Vault

### GCP

- **Verify service account permissions**: Ensure the GCP service account has `roles/secretmanager.admin` or `roles/secretmanager.secretAccessor`
- **Check project ID**: Ensure the project ID in your SecretManagerConfig is correct
- **Ensure Secret Manager API is enabled**: Verify the Secret Manager API is enabled in your GCP project

---

## Debugging Tips

### Enable Debug Logging

Add debug logging to your SecretManagerConfig:

```yaml
spec:
  logging:
    reconciliation: DEBUG
    secrets: DEBUG
    provider: DEBUG
    sops: DEBUG
```

### Check Controller Logs

```bash
# Follow logs in real-time
kubectl logs -f -n octopilot-system -l app=secret-manager-controller

# Check logs for specific errors
kubectl logs -n octopilot-system -l app=secret-manager-controller | grep -i error
```

### Verify SecretManagerConfig Status

```bash
# Get full status
kubectl get secretmanagerconfig <name> -n <namespace> -o yaml

# Check conditions
kubectl get secretmanagerconfig <name> -n <namespace> -o jsonpath='{.status.conditions}'
```

### Test Provider Connectivity

```bash
# Test AWS (if using AWS CLI)
aws secretsmanager list-secrets --region us-east-1

# Test GCP (if using gcloud)
gcloud secrets list --project=my-project

# Test Azure (if using Azure CLI)
az keyvault secret list --vault-name my-vault
```

---

## Getting Help

If you're still experiencing issues:

1. **Check controller logs** for detailed error messages
2. **Review SecretManagerConfig status** for condition details
3. **Verify provider credentials** and permissions
4. **Check network connectivity** to cloud providers
5. **Review the [Architecture Overview](../architecture/overview.md)** to understand the flow
6. **Check [Provider Setup Guides](../guides/aws-setup.md)** for provider-specific configuration

---

## Next Steps

- [Basic Usage](./basic-usage.md) - Learn the basics
- [Advanced Scenarios](./advanced-scenarios.md) - Advanced configurations
- [Provider Setup Guides](../guides/aws-setup.md) - Provider-specific setup

