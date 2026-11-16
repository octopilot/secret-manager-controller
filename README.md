<p align="center"><img src="website/images/hero-banner.png" alt="Secret Manager Controller" width="800" /></p>

[![CI](https://github.com/octopilot/secret-manager-controller/actions/workflows/ci.yml/badge.svg)](https://github.com/octopilot/secret-manager-controller/actions)
[![Dependabot](https://github.com/octopilot/secret-manager-controller/actions/workflows/dependabot-automerge.yml/badge.svg)](https://github.com/octopilot/secret-manager-controller/actions)
[![Docs](https://github.com/octopilot/secret-manager-controller/actions/workflows/docs-site.yml/badge.svg)](https://octopilot.github.io/secret-manager-controller/#/)
[![Crate](https://img.shields.io/crates/v/secret-manager-controller.svg)](https://crates.io/crates/secret-manager-controller)
[![License: PolyForm Shield 1.0.0](https://img.shields.io/badge/License-PolyForm%20Shield%201.0.0-blue.svg)](https://polyformproject.org/licenses/shield/1.0.0)

**The Missing Bridge Between GitOps and Serverless**

A Kubernetes controller that syncs SOPS-encrypted secrets from GitOps repositories (FluxCD/ArgoCD) to cloud-native secret stores (GCP Secret Manager, AWS Secrets Manager, Azure Key Vault), enabling serverless migration while preserving your GitOps workflow.

## Why This Exists

Platform teams optimization of cloud footprints through serverless adoption. The problem? **SOPS works for Kubernetes, but not serverless.**

When secrets exist only inside Kubernetes (encrypted via SOPS), you're left with:
- ❌ Two parallel worlds of secrets (K8s vs. serverless)
- ❌ No unified pipeline between GitOps and serverless
- ❌ Massive friction for teams wanting to migrate workloads
- ❌ Hidden opportunity costs from manual secret management

**The lack of a unified secret delivery mechanism was holding organizations back from achieving real FinOps savings.**

## What It Does

Secret Manager Controller reads SOPS-encrypted secrets from Git, decrypts them securely inside Kubernetes, and pushes them into cloud-native secret managers:

- ✔ **Google Secret Manager**
- ✔ **AWS Secrets Manager**
- ✔ **Azure Key Vault**
- ✔ **OpenBao** (in-cluster, KV v2)

This enables:
- ✅ **Serverless migration** — Unlock workloads previously blocked by secret management
- ✅ **Reduced cloud bill** — Shrink Kubernetes footprint, move to serverless
- ✅ **Unified workflow** — One pipeline for K8s and serverless
- ✅ **GitOps-first** — Preserve your existing SOPS + Git workflow

## Quick Start

```bash
# Apply CRD
kubectl apply -f https://raw.githubusercontent.com/octopilot/secret-manager-controller/main/config/crd/secretmanagerconfig.yaml

# Deploy controller
kubectl apply -k https://github.com/octopilot/secret-manager-controller/config/
```

See the [Installation Guide](https://octopilot.github.io/secret-manager-controller/#/user/getting-started/installation) for detailed setup instructions.

## SOPS keys

The controller decrypts with the `sops` binary. Put the private key in a Secret named
`sops-private-key` (key `private-key`) in the namespace of your `SecretManagerConfig`s,
or in the controller's own namespace to share it with every namespace that has none:

- **age**: the identity (`AGE-SECRET-KEY-1...`), as it appears in an age key file. Comment
  lines and several identities are fine.
- **GPG**: the ASCII-armored private key.

A namespace's key is used only for the configs in that namespace.

A repository may keep files encrypted to an age recipient next to files encrypted to a
GPG key. Give the controller both private keys and it decrypts either kind: put them in
separate fields of the Secret (`private-key`, `key`, `age-key`, `gpg-key` and `pgp-key`
are all read), or one after the other in a single field.

Name the files to sync with `spec.secrets.files` when they are not
`deployment-configuration/profiles/<env>/application.secrets.env`:

```yaml
spec:
  secrets:
    environment: dev
    prefix: myapp-dev
    files:
      - deployment-configuration/profiles/dev/myapp/runtime/application.secrets.env
```

## Pruning

Removing a key from a secret file leaves its secret in the provider. To have the
controller delete secrets it synced earlier that its files no longer define, annotate the
`SecretManagerConfig`:

```yaml
metadata:
  annotations:
    secret-management.octopilot.io/prune: "true"
```

Pruning is off unless the annotation is `"true"`. Only secrets recorded in that config's
own status are deleted, never anything found by name in the provider; a commented-out
key is disabled, not pruned; and nothing is pruned when the files define no secrets at
all. The controller's identity needs permission to delete secrets. The annotation takes
effect at the next reconcile.

Kustomize mode (`kustomizePath`) does not decrypt SOPS files and fails if its output
contains ciphertext; use `files` for encrypted secrets.

## Documentation

📚 **Comprehensive documentation is available at: [octopilot.github.io/secret-manager-controller](https://octopilot.github.io/secret-manager-controller)**

### Getting Started
- [Installation](https://octopilot.github.io/secret-manager-controller/#/user/getting-started/installation) - Deploy to your Kubernetes cluster
- [Quick Start](https://octopilot.github.io/secret-manager-controller/#/user/getting-started/quick-start) - Create your first SecretManagerConfig
- [Configuration](https://octopilot.github.io/secret-manager-controller/#/user/getting-started/configuration) - Configure your cloud provider

### Key Guides
- [Architecture Overview](https://octopilot.github.io/secret-manager-controller/#/user/architecture/overview) - Understand how it works
- [Serverless Integration](https://octopilot.github.io/secret-manager-controller/#/user/architecture/serverless-integration) - Deploy to CloudRun, Lambda, Functions
- [GitOps Integration](https://octopilot.github.io/secret-manager-controller/#/user/guides/gitops-integration) - Integrate with FluxCD or ArgoCD
- [SOPS Setup](https://octopilot.github.io/secret-manager-controller/#/user/guides/sops-setup) - Encrypt secrets in Git

### Provider Setup
- [GCP Setup](https://octopilot.github.io/secret-manager-controller/#/user/guides/gcp-setup) - Google Cloud Platform configuration
- [AWS Setup](https://octopilot.github.io/secret-manager-controller/#/user/guides/aws-setup) - Amazon Web Services configuration
- [Azure Setup](https://octopilot.github.io/secret-manager-controller/#/user/guides/azure-setup) - Microsoft Azure configuration
- [OpenBao Setup](https://octopilot.github.io/secret-manager-controller/#/user/guides/openbao-setup) - In-cluster OpenBao (KV v2) configuration

### API Reference
- [CRD Reference](https://octopilot.github.io/secret-manager-controller/#/user/api-reference/crd-reference) - Complete CRD documentation
- [Configuration Options](https://octopilot.github.io/secret-manager-controller/#/user/api-reference/configuration-options) - All configuration parameters
- [Provider APIs](https://octopilot.github.io/secret-manager-controller/#/user/api-reference/provider-apis) - Cloud provider API details

### CLI Tool
- [MSMCTL CLI](https://octopilot.github.io/secret-manager-controller/#/user/guides/msmctl-cli) - Command-line tool for managing the controller

## Features

- **GitOps-Agnostic** - Works with FluxCD, ArgoCD, or any GitOps tool
- **Multi-Cloud Support** - GCP, AWS, Azure, and in-cluster OpenBao from one controller
- **SOPS Integration** - Automatically decrypts SOPS-encrypted secrets
- **Kustomize Support** - Extracts secrets from Kustomize-built configurations
- **Workload Identity** - Uses Workload Identity/IRSA by default (no credential management)
- **GitOps-Driven** - Git is the source of truth; cloud providers are synced automatically

## Contributing

We welcome contributions! Please see our [Contributing Guide](https://octopilot.github.io/secret-manager-controller/#/contributor/contributing/contributing-guide) for details.

For development setup, see:
- [Development Setup](https://octopilot.github.io/secret-manager-controller/#/contributor/development/setup)
- [Tilt Integration](https://octopilot.github.io/secret-manager-controller/#/contributor/development/tilt-integration)
- [Testing Guide](https://octopilot.github.io/secret-manager-controller/#/contributor/testing/testing-guide)

## License

Licensed under the [PolyForm Shield License 1.0.0](https://polyformproject.org/licenses/shield/1.0.0).

Copyright 2024 Octopilot (https://octopilot.app)

You may use, modify, and distribute this software for any purpose **except**
building products or services that compete with Octopilot or any products
Octopilot provides using this software. See the [LICENSE](LICENSE) and
[NOTICE](NOTICE) files for the full terms.

---

**Questions?** Check out our [troubleshooting guide](https://octopilot.github.io/secret-manager-controller/#/user/tutorials/troubleshooting) or explore the full [documentation site](https://octopilot.github.io/secret-manager-controller).
