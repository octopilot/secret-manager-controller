# Secret Manager Controller

**The Missing Bridge Between GitOps and Serverless**

The Secret Manager Controller unlocks serverless migration and delivers massive FinOps savings by bridging SOPS-encrypted secrets from Git to cloud-native secret stores. Move workloads to serverless, shrink your Kubernetes footprint, and cut cloud costs—all while preserving your GitOps workflow.

## Why Secret Manager Controller?

- **GitOps-native secrets** for both Kubernetes and serverless workloads
- **Unified secrets pipeline** from Git to cloud providers
- **Multi-cloud support** — GCP, AWS, and Azure from one controller
- **SOPS compatible** — preserves your existing encryption workflow
- **Eliminates manual scripts** — no more secret drift or copy/paste workflows

## Supported Cloud Providers

- ✅ **Google Cloud Platform** — Secret Manager
- ✅ **Amazon Web Services** — Secrets Manager
- ✅ **Microsoft Azure** — Key Vault

## The FinOps Imperative

Finance and platform leadership are mandating cost optimization initiatives:
- **Lower cloud spend** through serverless adoption
- **Reduce infrastructure overhead** by shrinking Kubernetes clusters
- **Improve elasticity** with serverless auto-scaling
- **Push workloads to serverless** where viable
- **Minimize cluster footprints** to only what can't run serverless

**Kubernetes resizing helps—but not nearly enough.** The real savings come from running workloads on the most optimal platform for that system.

## The Blocking Problem

### SOPS Works for Kubernetes, But Not Serverless

For pure Kubernetes deployments, SOPS just worked:
- Secrets encrypted in Git with SOPS
- Deployed as Kubernetes Secrets/ConfigMaps
- Simple workflows with Git log audits
- Fully GitOps-aligned

**But serverless platforms cannot consume Kubernetes Secrets.**

- **Cloud Run** → requires GCP Secret Manager / Parameter Store
- **Cloud Functions** → requires GCP Secret Manager
- **AWS Lambda** → requires AWS Secrets Manager / Parameter Store
- **Azure Functions** → requires Azure Key Vault
- **Batch/ETL workloads** → require cloud-native secret stores

### The Result: Two Parallel Worlds

When secrets exist only inside Kubernetes (encrypted via SOPS), you're left with:
- ❌ **Two parallel worlds of secrets** (K8s vs. serverless)
- ❌ **No unified pipeline** between GitOps and serverless
- ❌ **No GitOps → Serverless bridge**
- ❌ **Massive friction** for teams wanting to migrate workloads

### The Hidden Opportunity Cost

Because SOPS-to-serverless wasn't supported, teams resorted to:
- Complex shell scripts and manual secret updates
- Half-automated CI pipelines
- Copy/pasting secrets into cloud consoles
- Out-of-sync secrets between GitOps and serverless
- Per-team rewrites of deployment logic

**This increases:**
- **Cognitive load** on developers and SREs
- **Security risk** from manual processes
- **Human error** from copy/paste workflows
- **Operational cost** from maintaining parallel systems
- **Time-to-benefit** for simple migrations

**The lack of a unified secret delivery mechanism was holding organizations back from achieving real FinOps savings.**

## The Strategic Goal

Move relevant workloads to serverless. Reduce Kubernetes footprint to what can't run on serverless. Achieve real FinOps savings.

**Blocked by one missing capability:** *"How do we take SOPS-encrypted secrets in Git and push them into cloud-native secret stores?"*

**This is the missing piece.**

## The Solution: Secret Manager Controller

**A unified bridge between GitOps and Serverless**

Secret Manager Controller reads SOPS-encrypted secrets from Git, decrypts them securely inside Kubernetes, and pushes them into cloud-native secret managers:

- ✔ **Google Secret Manager**
- ✔ **AWS Secrets Manager**
- ✔ **Azure Key Vault**
- *(Future: Parameter Stores + Runtime Config)*

### What It Enables

#### 1. Serverless Workloads Can Use Secrets Directly

Cloud Run, Cloud Functions, serverless jobs, and batch workloads now fetch secrets from the official cloud providers' APIs—no Kubernetes required.

#### 2. Kubernetes Workloads Stay GitOps-Native

Through ExternalSecrets, Kubernetes still consumes secrets from cloud stores—not local Secrets. One unified pipeline for both platforms.

#### 3. One Unified Workflow

Developers keep using:
- **Git** for version control
- **Pull requests** for review
- **SOPS encryption** for security
- **GitOps principles** for automation

**All with zero changes to developer workflow.**

And secrets flow to:
- Kubernetes
- Serverless
- Batch jobs
- Legacy apps
- CI/CD pipelines

### Massive FinOps Wins

With secrets no longer locked inside Kubernetes:

- ✅ **Easy migration** of dozens of workloads to serverless
- ✅ **Significant reduction** in cluster size
- ✅ **Lower operational overhead** from unified workflows
- ✅ **No re-engineering** of secrets pipelines
- ✅ **Less toil** for developers & SREs

**The cost savings multiply across every team.**

### Technical Benefits

- **Centralizes secret workflows** — eliminates script sprawl and manual secret drift
- **Supports multi-cloud providers** — GCP, AWS, Azure from one controller
- **Clear audit trail** — everything begins in Git with full history
- **Safe, encrypted, GitOps-first design** — SOPS encryption preserved
- **Fully compatible** with GitHub Actions, Flux, ArgoCD, Kustomize

**Teams only encrypt → commit → let automation do the rest.**

## Before vs. After

### Before: SOPS → Kubernetes Only

```
SOPS → Kubernetes Secret → pods use secret
❌ No path to serverless
❌ Workarounds everywhere
❌ Teams blocked from serverless migration
```

### After: One Unified Secret Delivery Pipeline

```
SOPS → Git → Secret Manager Controller → Cloud Secret Manager
                                              ↓
                                    ┌─────────┴─────────┐
                                    ↓                   ↓
                                  K8s              Serverless
```

**One workflow. Multiple platforms. Zero friction.**

## The One Remaining Dependency

### A Small Kubernetes Control Cluster Is Still Needed

Even when teams migrate fully to serverless, they still require a minimal Kubernetes control-plane cluster to run:

- **FluxCD / ArgoCD** — GitOps engine for source-of-truth reconciliation
- **Terraform or Crossplane (Upbound)** — Cloud API orchestration
- **Deployment automation** — Cloud Run, Cloud Functions, Pub/Sub, SQL, Storage, IAM lifecycle management

**The remaining cluster is smaller, cost-effective, and strategically valuable.**

This is normal in a modern cloud-native architecture:
- It's not a workload cluster
- It's not scaled to application traffic
- It's an "infrastructure brain," not a "serverless runtime"

### Why This Dependency Is Acceptable

Most teams will always have some Kubernetes workloads:
- Stateful services
- Daemons or sidecars
- Custom networking components
- Internal APIs
- Specialized workloads poorly suited for serverless

**The cluster pays for itself** by enabling:
- Unified GitOps across serverless + K8s
- Terraform or Crossplane-driven cloud API automation
- Central control of secrets + config
- Reduced operational overhead through consolidation
- **Enormous serverless-driven cost reductions elsewhere**

## Summary: Why Secret Manager Controller Is a Must

Secret Manager Controller enables:

- ✅ **Serverless migration** — Unlock workloads previously blocked by secret management
- ✅ **Reduced cloud bill** — Shrink Kubernetes footprint, move to serverless
- ✅ **Clear security model** — SOPS encryption, Git audit trail, automated sync
- ✅ **Centralized operational control** — One workflow for K8s and serverless
- ✅ **Better developer ergonomics** — No workflow changes, just commit and deploy

**This is the missing engine that unlocks your organization's next wave of optimization.**

### Why Undertake This?

- **Cut K8S reliance** — Move workloads to serverless where viable
- **Cut wasted infrastructure** — Reduce cluster sizes and operational overhead
- **Preserve GitOps** — Keep your existing SOPS + Git workflow
- **Gain flexibility** — Deploy to any platform without secret management rewrites

### Start Migrating

- **Complex business logic** → Keep in Kubernetes
- **Batch jobs** → Migrate to Cloud Run
- **Static APIs** → Migrate to Cloud Run + ASM
- **ETL & scheduled workloads** → Migrate to Cloud Scheduler + Cloud Run Jobs

**This is the path forward.**

## Get Started

Ready to unify your secret management? Get started in minutes:

1. **[Install the Controller](./getting-started/installation.md)** - Deploy to your Kubernetes cluster
2. **[Quick Start Guide](./getting-started/quick-start.md)** - Create your first SecretManagerConfig
3. **[Configure Your Provider](./getting-started/configuration.md)** - Set up GCP, AWS, or Azure integration

### Learn More

- **[Architecture Overview](./architecture/overview.md)** - Understand how it works
- **[Serverless Integration](./architecture/serverless-integration.md)** - Deploy to CloudRun, Lambda, Functions
- **[GitOps Integration](./guides/gitops-integration.md)** - Integrate with FluxCD or ArgoCD
- **[SOPS Setup](./guides/sops-setup.md)** - Encrypt secrets in Git

---

**Questions?** Check out our [troubleshooting guide](./tutorials/troubleshooting.md) or explore the [API reference](./api-reference/crd-reference.md).

