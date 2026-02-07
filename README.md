<p align="center"><img src="website/images/hero-banner.png" alt="Secret Manager Controller" width="800" /></p>

[![CI](https://github.com/octopilot/secret-manager-controller/actions/workflows/ci.yml/badge.svg)](https://github.com/octopilot/secret-manager-controller/actions)
[![Dependabot](https://github.com/octopilot/secret-manager-controller/actions/workflows/dependabot-automerge.yml/badge.svg)](https://github.com/octopilot/secret-manager-controller/actions)
[![Docs](https://github.com/octopilot/secret-manager-controller/actions/workflows/docs-site.yml/badge.svg)](https://octopilot.github.io/secret-manager-controller/#/)
[![Crate](https://img.shields.io/crates/v/secret-manager-controller.svg)](https://crates.io/crates/secret-manager-controller)

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

### API Reference
- [CRD Reference](https://octopilot.github.io/secret-manager-controller/#/user/api-reference/crd-reference) - Complete CRD documentation
- [Configuration Options](https://octopilot.github.io/secret-manager-controller/#/user/api-reference/configuration-options) - All configuration parameters
- [Provider APIs](https://octopilot.github.io/secret-manager-controller/#/user/api-reference/provider-apis) - Cloud provider API details

### CLI Tool
- [MSMCTL CLI](https://octopilot.github.io/secret-manager-controller/#/user/guides/msmctl-cli) - Command-line tool for managing the controller

## Features

- **GitOps-Agnostic** - Works with FluxCD, ArgoCD, or any GitOps tool
- **Multi-Cloud Support** - GCP, AWS, and Azure from one controller
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

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.

---

**Questions?** Check out our [troubleshooting guide](https://octopilot.github.io/secret-manager-controller/#/user/tutorials/troubleshooting) or explore the full [documentation site](https://octopilot.github.io/secret-manager-controller).
