# OpenBao Setup Guide

Configure the Secret Manager Controller to sync secrets to an in-cluster
[OpenBao](https://openbao.org) KV v2 secrets engine using the Kubernetes auth
method.

## Overview

OpenBao is the Linux Foundation fork of HashiCorp Vault and is API-compatible
with it. The controller stores each secret as a single `value` field under
`<mount>/data/<name>` in a KV v2 engine, and authenticates with the OpenBao
**Kubernetes auth method** — the controller's ServiceAccount JWT is exchanged
for a short-lived OpenBao token, so no static credentials are stored.

`disable`/`enable` map to KV v2 version **soft-delete**/**undelete**
(reversible); `delete` removes all versions.

## Prerequisites

- OpenBao running in the cluster (for example at `openbao.openbao.svc:8200`).
- A KV v2 engine and the Kubernetes auth method (configured below).
- The controller installed, with ServiceAccount `secret-manager-controller` in
  namespace `octopilot-system`.
- OpenBao's own ServiceAccount bound to `system:auth-delegator` so it can call
  the Kubernetes TokenReview API (the official OpenBao Helm chart does this via
  `server.authDelegator.enabled=true`).

## 1. Enable the KV v2 engine

```bash
bao secrets enable -path=secret -version=2 kv
```

## 2. Enable and configure Kubernetes auth

```bash
bao auth enable kubernetes

bao write auth/kubernetes/config \
  kubernetes_host="https://${KUBERNETES_SERVICE_HOST}:${KUBERNETES_SERVICE_PORT}"
```

## 3. Create a least-privilege policy

Save as `controller-policy.hcl` (also in `config/openbao/controller-policy.hcl`):

```hcl
path "secret/data/*"     { capabilities = ["create", "update", "read"] }
path "secret/delete/*"   { capabilities = ["update"] }
path "secret/undelete/*" { capabilities = ["update"] }
path "secret/metadata/*" { capabilities = ["read", "list", "delete"] }
```

```bash
bao policy write secret-manager-controller ./controller-policy.hcl
```

## 4. Create the role bound to the controller ServiceAccount

```bash
bao write auth/kubernetes/role/secret-manager-controller \
  bound_service_account_names=secret-manager-controller \
  bound_service_account_namespaces=octopilot-system \
  policies=secret-manager-controller \
  ttl=1h
```

## 5. Create a SecretManagerConfig

```yaml
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: myapp-dev-secrets-openbao
  namespace: mysystem
spec:
  sourceRef:
    kind: GitRepository
    name: mysystem-manifests
    namespace: flux-system
  provider:
    type: openbao
    openbao:
      address: https://openbao.openbao.svc:8200
      mount: secret
      auth:
        authType: kubernetes
        role: secret-manager-controller
  secrets:
    environment: dev
    kustomizePath: microservices/myapp/deployment-configuration/profiles/dev
    prefix: myapp-dev
```

See `examples/idam-dev-openbao-secret-manager-config.yaml` for a fully-commented
version.

## Configuration reference

| Field | Required | Description |
| --- | --- | --- |
| `address` | yes | OpenBao API base URL (`http(s)://host:8200`). |
| `mount` | no (default `secret`) | KV v2 mount name. |
| `namespace` | no | OpenBao namespace (`X-Vault-Namespace`, OpenBao 2.6+). |
| `tlsCaSecretRef` | no | Kubernetes Secret + key holding a custom CA for TLS. |
| `auth.authType` | yes | Must be `kubernetes`. |
| `auth.role` | yes | OpenBao Kubernetes auth role bound to the controller SA. |
| `auth.serviceAccountTokenPath` | no | Override the projected token path. |

There is no `location`/`region` field — OpenBao is a single endpoint.

## Verifying

After applying the CR, check the resource status and confirm a secret landed:

```bash
kubectl get secretmanagerconfig myapp-dev-secrets-openbao -n mysystem
bao kv get secret/myapp-dev/<key>
```

## Troubleshooting

- **`permission denied` / 403** — the controller re-authenticates once on 403;
  a persistent 403 means the role/policy doesn't grant the KV v2 paths above, or
  the role's `bound_service_account_*` don't match the controller SA/namespace.
- **`sealed` / 503** — OpenBao is sealed or on standby; the controller treats
  this as retryable and requeues. Unseal OpenBao.
- **TLS errors** — for a private CA, set `provider.openbao.tlsCaSecretRef`.
- **`application.properties` not synced** — config-store routing to OpenBao is
  not yet supported; use `application.secrets`.
