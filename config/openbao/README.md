# OpenBao provider setup (RBAC + auth)

One-time OpenBao configuration so the Secret Manager Controller can sync secrets
to a KV v2 engine using the Kubernetes auth method. Run these as an OpenBao
admin (`bao` CLI or API). See also the full guide at
`docs-site/.../user/guides/openbao-setup.md`.

Assumptions: controller ServiceAccount is `secret-manager-controller` in
namespace `octopilot-system`; KV v2 mount is `secret`.

## 1. Enable the KV v2 engine (if not already)

```bash
bao secrets enable -path=secret -version=2 kv
```

## 2. Enable and configure the Kubernetes auth method

```bash
bao auth enable kubernetes

# When OpenBao runs inside the same cluster, it can auto-detect the API server
# and use its own ServiceAccount token/CA for TokenReview:
bao write auth/kubernetes/config \
  kubernetes_host="https://${KUBERNETES_SERVICE_HOST}:${KUBERNETES_SERVICE_PORT}"
```

> OpenBao's ServiceAccount must be allowed to call the TokenReview API
> (bind it to the `system:auth-delegator` ClusterRole). The official Helm chart
> does this via `server.authDelegator.enabled=true`.

## 3. Write the least-privilege policy

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

## Notes

- **Token audience / projection.** The controller reads the standard projected
  ServiceAccount token at `/var/run/secrets/kubernetes.io/serviceaccount/token`,
  which is auto-mounted — no extra projected-token volume is required for the
  default TokenReview-based validation. If your OpenBao role sets a specific
  `audience`, mount a projected token with that audience and point the CR's
  `provider.openbao.auth.serviceAccountTokenPath` at it.
- **TLS.** For a TLS-enabled OpenBao with a private CA, set
  `provider.openbao.tlsCaSecretRef` in the SecretManagerConfig to a Kubernetes
  Secret holding the CA (custom-CA support: OB-8).
- **Config store.** Routing `application.properties` to OpenBao is not yet
  supported (planned as OB-14); use `application.secrets` for OpenBao.
