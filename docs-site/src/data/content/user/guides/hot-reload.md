# Hot Reload Guide

Learn how to configure hot reload so your applications can pick up secret changes without pod restarts.

## Overview

By default, Kubernetes does **not** automatically restart pods when Secrets or ConfigMaps are updated. The Secret Manager Controller supports hot reload functionality that allows applications to detect and apply secret changes without requiring pod restarts.

## How Hot Reload Works

Hot reload enables applications to detect secret changes through:

1. **File Watchers** — Applications watch mounted secret files for changes
2. **SIGHUP Signals** — Applications listen for SIGHUP to reload configuration
3. **HTTP Endpoints** — Applications expose HTTP endpoints to trigger reloads
4. **Polling** — Applications periodically check for secret updates

## Configuration

### Enable Hot Reload

Enable hot reload in your `SecretManagerConfig`:

```yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-service-secrets
spec:
  hotReload:
    enabled: true
  # ... rest of configuration
```

### Controller-Level Hot Reload

Configure hot reload at the controller level:

```yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: my-service-secrets
spec:
  hotReload:
    enabled: true
    configMapName: secret-manager-controller-config
    configMapNamespace: octopilot-system
```

**Fields:**
- `enabled` (boolean): Enable hot reload functionality
- `configMapName` (string): Name of the ConfigMap to watch for changes
- `configMapNamespace` (string): Namespace where the ConfigMap exists

## Application Support

### File Watchers

Applications that watch mounted files automatically detect changes:

```yaml
apiVersion: v1
kind: Pod
spec:
  containers:
  - name: app
    volumeMounts:
    - name: secrets
      mountPath: /etc/secrets
    # Application watches /etc/secrets for file changes
  volumes:
  - name: secrets
    secret:
      secretName: my-secrets
```

### SIGHUP Signal

Applications that listen for SIGHUP can reload configuration:

```python
import signal
import os

def reload_config(signum, frame):
    # Reload configuration from files
    load_secrets()

signal.signal(signal.SIGHUP, reload_config)
```

### HTTP Endpoints

Applications can expose HTTP endpoints to trigger reloads:

```python
from flask import Flask

app = Flask(__name__)

@app.route('/reload', methods=['POST'])
def reload():
    # Reload secrets
    load_secrets()
    return {'status': 'reloaded'}, 200
```

### Polling

Applications can periodically check for secret updates:

```python
import time
import hashlib

def check_secrets():
    current_hash = hash_secrets()
    if current_hash != last_hash:
        reload_secrets()
        last_hash = current_hash

while True:
    check_secrets()
    time.sleep(60)  # Check every minute
```

## Limitations

### Kubernetes Behavior

**Important:** Kubernetes does NOT automatically restart pods when Secrets or ConfigMaps are updated. This is by design:

- Secrets are mounted as files at pod startup
- Updates to Secrets do not trigger pod restarts
- Applications must actively watch for changes

### Not All Applications Support Hot Reload

Hot reload requires application-level support:

- ✅ **Supported**: Applications with file watchers, SIGHUP handlers, or HTTP reload endpoints
- ❌ **Not Supported**: Applications that only read secrets at startup
- ❌ **Not Supported**: Applications without reload mechanisms

### File System Updates

When a Secret is updated:

1. The Secret object in Kubernetes is updated
2. The mounted file system may not immediately reflect changes
3. Applications must actively detect changes (file watchers, polling, etc.)

## Best Practices

### 1. Use File Watchers When Possible

File watchers are the most reliable method for detecting changes:

```yaml
# Application watches mounted files
volumeMounts:
- name: secrets
  mountPath: /etc/secrets
  readOnly: true
```

### 2. Implement Graceful Reloads

Ensure reloads don't disrupt service:

- Validate new configuration before applying
- Use atomic updates when possible
- Handle reload errors gracefully

### 3. Monitor Reload Success

Track reload events in your application:

```python
def reload_secrets():
    try:
        new_secrets = load_secrets()
        validate_secrets(new_secrets)
        apply_secrets(new_secrets)
        log.info("Secrets reloaded successfully")
    except Exception as e:
        log.error(f"Failed to reload secrets: {e}")
        # Keep using old secrets
```

### 4. Test Hot Reload

Verify hot reload works in your environment:

1. Update a secret in your Git repository
2. Wait for controller to sync
3. Verify application detects and applies changes
4. Check application logs for reload events

## Troubleshooting

### Hot Reload Not Working

**Check if hot reload is enabled:**

```bash
kubectl get secretmanagerconfig <name> -o yaml | grep -A 5 hotReload
```

**Verify application supports hot reload:**

- Check if application has file watchers
- Verify SIGHUP handlers are registered
- Test HTTP reload endpoints

### Secrets Not Updating

**Check if secrets are being synced:**

```bash
kubectl get secretmanagerconfig <name> -o jsonpath='{.status.lastSyncTime}'
```

**Verify mounted files are updated:**

```bash
kubectl exec <pod-name> -- cat /etc/secrets/<secret-name>
```

### Application Not Detecting Changes

**Common causes:**

1. Application doesn't support hot reload
2. File watchers not configured correctly
3. Secrets mounted as environment variables (not files)
4. Application caches secrets in memory

**Solutions:**

- Use file mounts instead of environment variables
- Implement file watchers or polling
- Add SIGHUP signal handlers
- Expose HTTP reload endpoints

## Alternatives to Hot Reload

If your application doesn't support hot reload:

### 1. Manual Restart

Restart pods manually when secrets change:

```bash
kubectl rollout restart deployment <name> -n <namespace>
```

### 2. Use Reloader

Use [Stakater Reloader](https://github.com/stakater/Reloader) to automatically restart pods:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  annotations:
    secret.reloader.stakater.com/reload: "my-secrets"
```

### 3. Application-Level Polling

Implement polling in your application to check for secret updates:

```python
# Poll every 5 minutes
while True:
    if secrets_changed():
        reload_secrets()
    time.sleep(300)
```

## Examples

### Python Application with File Watcher

```python
from watchdog.observers import Observer
from watchdog.events import FileSystemEventHandler

class SecretHandler(FileSystemEventHandler):
    def on_modified(self, event):
        if event.src_path.endswith('.env'):
            reload_secrets()

observer = Observer()
observer.schedule(SecretHandler(), '/etc/secrets', recursive=False)
observer.start()
```

### Go Application with SIGHUP

```go
package main

import (
    "os"
    "os/signal"
    "syscall"
)

func main() {
    sigChan := make(chan os.Signal, 1)
    signal.Notify(sigChan, syscall.SIGHUP)
    
    go func() {
        for range sigChan {
            reloadSecrets()
        }
    }()
    
    // ... rest of application
}
```

## Next Steps

- [Troubleshooting Guide](../tutorials/troubleshooting.md) - Common issues and solutions
- [Configuration Guide](../getting-started/configuration.md) - All configuration options
- [CRD Reference](../api-reference/crd-reference.md) - Complete API reference

