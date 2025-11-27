# Secret Manager Controller Tiltfile
# 
# This Tiltfile uses a standard build pattern:
# 1. Builds Rust binaries on host (cross-compilation)
# 2. Copies binaries to build_artifacts/
# 3. Generates CRD using crdgen binary
# 4. Builds Docker image copying from build_artifacts/
# 5. Deploys to Kubernetes using kustomize
#
# Usage: tilt up
#
# Resources are organized into parallel streams using labels:
# - 'controllers' label: Controller build, deployment, and related resources
# - 'infrastructure' label: FluxCD, ArgoCD, Contour, GitOps setup
# - 'pact' label: Pact broker and mock servers for contract testing
#
# Each stream can build in parallel independently. Use 'tilt up controllers' to run
# only controller resources, 'tilt up infrastructure' for infrastructure, or
# 'tilt up pact' for Pact resources.

# Note: restart_container() is deprecated, but for k8s resources we can use run() to restart
# The restart_process extension is primarily for docker_build resources
# For k8s resources, we'll use run() to send a signal or restart the process

# ====================
# Configuration
# ====================

# Restrict to kind cluster
allow_k8s_contexts(['kind-secret-manager-controller'])

# Configure default registry for Kind cluster
# Tilt will automatically push docker_build images to this registry
# The registry is set up by scripts/setup_kind.py
default_registry('localhost:5000')

# Suppress warnings for images that Tilt correctly substitutes
# Tilt expands 'mock-webhook' to 'localhost:5000/mock-webhook' but the custom_build
# is named 'mock-webhook', which Tilt correctly matches during substitution
# docs-site is built with docker_build and Tilt will substitute it in the deployment
# postgres-manager is built with custom_build and Tilt will substitute it in the deployment
update_settings(suppress_unused_image_warnings=['localhost:5000/mock-webhook', 'docs-site'])

# Get the directory where this Tiltfile is located
# Since the Tiltfile is in the root directory, use '.' for relative paths
CONTROLLER_DIR = '.'
CONTROLLER_NAME = 'secret-manager-controller'
IMAGE_NAME = 'localhost:5000/secret-manager-controller'
BINARY_NAME = 'secret-manager-controller'
# Build for Linux x86_64 (cross-compile for container compatibility)
# Use target path directly, not build_artifacts
# Workspace builds output to root target/ directory
BINARY_PATH = '%s/target/x86_64-unknown-linux-musl/debug/%s' % (CONTROLLER_DIR, BINARY_NAME)
CRDGEN_PATH = '%s/target/x86_64-unknown-linux-musl/debug/crdgen' % CONTROLLER_DIR
# Native crdgen for host execution (CRD generation runs on host, not in container)
CRDGEN_NATIVE_PATH = '%s/target/debug/crdgen' % CONTROLLER_DIR



# ====================
# Build All Rust Binaries
# ====================
# Note: build-all-binaries now also generates and applies the CRD
# Build all binaries (controller, mock servers, webhook) in a single build
# This is more efficient than building each binary separately
# Uses cargo zigbuild on macOS (like microservices) for cross-compilation
local_resource(
    'build-all-binaries',
    cmd='python3 scripts/tilt/build_all_binaries.py',
    deps=[
        '%s/crates/controller/src' % CONTROLLER_DIR,
        '%s/crates/pact-mock-server/src' % CONTROLLER_DIR,
        '%s/crates/paths/src' % CONTROLLER_DIR,
        '%s/crates/controller/Cargo.toml' % CONTROLLER_DIR,
        '%s/crates/pact-mock-server/Cargo.toml' % CONTROLLER_DIR,
        '%s/crates/paths/Cargo.toml' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        '%s/Cargo.lock' % CONTROLLER_DIR,
        './scripts/tilt/build_all_binaries.py',
    ],
    resource_deps=[],
    labels=['controllers',],
    allow_parallel=True,
)

# ====================
# Controller Container Packaging
# ====================
# Package controller binary into Docker image
# Depends on unified build-all-binaries resource
# Uses Dockerfile.controller.dev for development (expects pre-built binary)
# Standard build pattern
custom_build(
    IMAGE_NAME,
    'docker build -f dockerfiles/Dockerfile.controller.dev -t %s:tilt . && docker tag %s:tilt $EXPECTED_REF && docker push $EXPECTED_REF' % (
        IMAGE_NAME,
        IMAGE_NAME
    ),
    deps=[
        BINARY_PATH,  # File dependency ensures binary exists before Docker build
        'dockerfiles/Dockerfile.controller.dev',
    ],
    # resource_deps=['build-all-binaries'],  # Wait for unified build
    tag='tilt',
    live_update=[
        sync(BINARY_PATH, '/app/secret-manager-controller'),
        run('kill -HUP 1', trigger=[BINARY_PATH]),
    ],
)

# ====================
# Container Cleanup
# ====================
# Note: container-cleanup resource removed from Tilt - problematic container causing disk space issues has been found
# The Python script (scripts/tilt/cleanup_stopped_containers.py) is kept for manual use if needed

# ====================
# FluxCD Installation
# ====================
# Note: FluxCD is installed by Kind cluster setup (scripts/setup_kind.py)
# and is not managed by Tilt to ensure it's always available

# ====================
# Contour Ingress Installation
# ====================
# Install Contour Ingress Controller for Kind cluster
# Contour is a CNCF project that uses Envoy as the data plane
# Required for ArgoCD ingress access
# Idempotent - can be run multiple times safely

# Disabling contour, not sure its actually needed as we communicate with k8s svc
# local_resource(
#     'ingress-install',
#     cmd='NON_INTERACTIVE=1 python3 scripts/setup_contour.py',
#     deps=[
#         './scripts/setup_contour.py',
#     ],
#     labels=['infrastructure'],
#     allow_parallel=False,
# )

# ====================
# ArgoCD Installation
# ====================
# Note: ArgoCD CRDs are installed by Kind cluster setup (scripts/setup_kind.py)
# and are not managed by Tilt to ensure they're always available

# ArgoCD Ingress
# Access ArgoCD UI via ingress at http://argocd.localhost
# Requires Contour ingress controller (installed via ingress-install)
# Contour uses Envoy as the data plane and exposes it via NodePort
# Default credentials: admin / (get password with: kubectl -n argocd get secret argocd-initial-admin-secret -o jsonpath="{.data.password}" | base64 -d)
# Apply ingress after ArgoCD and Contour are installed
# Wait for Contour HTTPProxy CRD to be ready before applying ingress

# Disabled as we don't need to log into argo
# local_resource(
#     'argocd-ingress-apply',
#     cmd='''
#         # Wait for Contour HTTPProxy CRD to be available
#         echo "Waiting for Contour HTTPProxy CRD to be ready..."
#         timeout=60
#         elapsed=0
#         while [ $elapsed -lt $timeout ]; do
#             if kubectl get crd httpproxies.projectcontour.io >/dev/null 2>&1; then
#                 echo "✅ Contour HTTPProxy CRD is ready"
#                 break
#             fi
#             echo "  Waiting for CRD... (${elapsed}s/${timeout}s)"
#             sleep 2
#             elapsed=$((elapsed + 2))
#         done
        
#         if ! kubectl get crd httpproxies.projectcontour.io >/dev/null 2>&1; then
#             echo "❌ Error: Contour HTTPProxy CRD not found after ${timeout}s"
#             echo "Please ensure Contour is installed: python3 scripts/setup_contour.py"
#             exit 1
#         fi
        
#         # Apply ingress configuration
#         kubectl apply -f gitops/cluster/argocd/ingress.yaml
#     ''',
#     deps=[
#         'gitops/cluster/argocd/ingress.yaml',
#     ],
#     labels=['infrastructure',],
#     resource_deps=['argocd-install'],
#     allow_parallel=False,
# )

# ====================
# Git Credentials Setup
# ====================
# Decrypt git credentials from SOPS-encrypted .env file and create Kubernetes secret
# This allows GitRepository resources to authenticate with private repositories
# Optional - only runs if .env file exists and contains git credentials

local_resource(
    'git-credentials-setup',
    cmd='python3 scripts/tilt/setup_git_credentials.py --all-environments',
    deps=[
        './scripts/tilt/setup_git_credentials.py',
        '.env',  # Watch for .env file changes
    ],
    labels=['infrastructure'],
    allow_parallel=False,
)

# ====================
# SOPS Private Key Setup
# ====================
# Export GPG private key from local keyring (using .sops.yaml key ID) and create Kubernetes secrets
# This allows the controller to decrypt SOPS-encrypted files
# Creates secrets in all environment namespaces (tilt, dev, stage, prod, microscaler-system)
# Optional - only runs if .sops.yaml exists and GPG key is available locally

local_resource(
    'sops-key-setup',
    cmd='python3 scripts/setup_sops_key.py --all-environments',
    deps=[
        './scripts/setup_sops_key.py',
        '.sops.yaml',  # Watch for .sops.yaml changes
    ],
    labels=['infrastructure'],
    allow_parallel=False,
)

# ====================
# Deploy to Kubernetes
# ====================
# Note: microscaler-system namespace is created by Kind cluster setup (scripts/setup_kind.py)
# and is not managed by Tilt to ensure it's always available

# Note: CRD generation and application is now handled by build-all-binaries
# This ensures the CRD is applied before any resources that use it

# Deploy using kustomize
# Note: CRD is NOT included in kustomize - it's applied separately above to prevent deletion
# Note: FluxCD is installed by Kind cluster setup (scripts/setup_kind.py)
# Note: Namespace is created by Kind cluster setup (scripts/setup_kind.py)
# k8s_yaml doesn't support resource_deps, but the namespace resource runs first
# and kubectl apply is idempotent, so namespace.yaml in kustomize won't cause issues
k8s_yaml(kustomize('%s/config' % CONTROLLER_DIR))

# Configure resource
# Tilt will automatically substitute the image in the deployment
# because custom_build registers the image and Tilt matches it to the deployment
# Note: No port forwarding needed - pods get their own IPs
# Use 'kubectl port-forward' or 'just port-forward' to access metrics
# Note: PVC is created during Kind cluster setup (not managed by Tilt to avoid deletion issues)
k8s_resource(
    CONTROLLER_NAME,
    labels=['controllers'],
    resource_deps=['build-all-binaries', 'sops-key-setup'],
)

# ====================
# Pact Broker Deployment
# ====================
# Deploy Pact Broker for contract testing
# 
# INDEPENDENT: Pact resources operate independently of controller resources.
# They can be started/stopped/managed separately using Tilt labels.
# Use 'tilt up pact' to run only Pact resources, or filter by label in UI.
# Note: Labels on k8s_resource() ensure isolation - controller won't wait for pact resources.

# ====================
# Mock Server Container Packaging
# ====================
# Copy mock server binaries to build_artifacts for Docker packaging
# Depends on unified build-all-binaries resource
local_resource(
    'copy-mock-server-binaries',
    cmd='python3 scripts/tilt/copy_mock_server_binaries.py',
    deps=[
        'target/x86_64-unknown-linux-musl/debug/gcp-mock-server',
        'target/x86_64-unknown-linux-musl/debug/aws-mock-server',
        'target/x86_64-unknown-linux-musl/debug/azure-mock-server',
        'target/x86_64-unknown-linux-musl/debug/webhook',
        './scripts/tilt/copy_mock_server_binaries.py',
    ],
    resource_deps=['build-all-binaries'],  # Wait for unified build
    labels=['pact'],
    allow_parallel=True,
)

# Build Pact mock server Docker image (Rust/Axum)
# Binaries are built by build-all-binaries and copied by copy-mock-server-binaries
# Use custom_build for better dependency control
# Note: Image name is 'pact-mock-server' (without registry prefix)
# Tilt will automatically prepend default_registry('localhost:5000') when substituting
custom_build(
    'pact-mock-server',
    'python3 scripts/tilt/docker_build_mock_server.py',
    deps=[
        'build_artifacts/mock-server/gcp-mock-server',
        'build_artifacts/mock-server/aws-mock-server',
        'build_artifacts/mock-server/azure-mock-server',
        'build_artifacts/mock-server/webhook',
        'dockerfiles/Dockerfile.pact-mock-server',
        './scripts/tilt/docker_build_mock_server.py',
    ],
    # File dependencies in deps ensure binaries exist before build
    env={
        'IMAGE_NAME': 'localhost:5000/pact-mock-server',
    },
    tag='tilt',
    live_update=[
        # Sync the updated binary into the running container
        # Each deployment uses a different binary, so we sync all three
        sync('build_artifacts/mock-server/gcp-mock-server', '/app/gcp-mock-server'),
        sync('build_artifacts/mock-server/aws-mock-server', '/app/aws-mock-server'),
        sync('build_artifacts/mock-server/azure-mock-server', '/app/azure-mock-server'),
        sync('build_artifacts/mock-server/webhook', '/app/webhook'),
        # Send SIGHUP to restart the process (graceful restart)
        # The process will pick up the new binary on restart
        run('kill -HUP 1'),
    ],
    skips_local_docker=False,
)

# Build webhook server Docker image (separate from mock servers)
# Note: Image name is 'mock-webhook' (without registry prefix)
# Tilt will automatically prepend default_registry('localhost:5000') when substituting
custom_build(
    'mock-webhook',
    'python3 scripts/tilt/docker_build_webhook.py',
    deps=[
        'build_artifacts/mock-server/webhook',
        'dockerfiles/Dockerfile.pact-webhook',
        './scripts/tilt/docker_build_webhook.py',
    ],
    # File dependencies in deps ensure binaries exist before build
    env={
        'IMAGE_NAME': 'localhost:5000/mock-webhook',
    },
    tag='tilt',
    live_update=[
        sync('build_artifacts/mock-server/webhook', '/app/webhook'),
        run('kill -HUP 1'),
    ],
    skips_local_docker=False,
)

# Build postgres-manager Docker image (separate from mock servers)
# Note: Image name is 'postgres-manager' (without registry prefix)
# Tilt will automatically prepend default_registry('localhost:5000') when substituting
custom_build(
    'postgres-manager',
    'python3 scripts/tilt/docker_build_postgres_manager.py',
    deps=[
        'build_artifacts/mock-server/postgres-manager',
        'dockerfiles/Dockerfile.postgres-manager',
        './scripts/tilt/docker_build_postgres_manager.py',
    ],
    # File dependencies in deps ensure binaries exist before build
    # Note: No env needed - script uses EXPECTED_REF from Tilt
    tag='tilt',
    live_update=[
        sync('build_artifacts/mock-server/postgres-manager', '/app/postgres-manager'),
        run('kill -HUP 1'),
    ],
    skips_local_docker=False,
)

# Apply Pact infrastructure Kubernetes resources
# Order matters: ConfigMap is created first, then populated, then deployment
k8s_yaml(kustomize('pact-broker/k8s'))

# ====================
# Populate Pact ConfigMap
# ====================
# Populate the pact-contracts ConfigMap from local pact files in target/pacts/
# This runs whenever pact files are generated or changed
# The ConfigMap is then used by the manager sidecar to publish contracts
local_resource(
    'populate-pact-configmap',
    cmd='python3 scripts/tilt/populate_pact_configmap.py',
    deps=[
        'scripts/tilt/populate_pact_configmap.py',
        'target/pacts',  # Watch the entire directory for changes
    ],
    labels=['pact'],
    resource_deps=[],  # Can run independently, but should run before pact-infrastructure
    allow_parallel=False,
)

# Combined Pact Infrastructure Deployment
# All Pact components (broker, mock servers, webhook) are now in a single deployment
# This significantly reduces startup time and simplifies orchestration
# 
# Setup order:
# 1. ConfigMap is created by k8s_yaml
# 2. Deployment starts with init containers that populate ConfigMap and publish contracts
# 3. All services are ready
k8s_resource(
    'pact-infrastructure',
    labels=['pact'],
    port_forwards=[
        '9292:9292',  # Pact broker
        '1234:1234',  # AWS mock server
        '1235:1235',  # GCP mock server
        '1236:1236',  # Azure mock server
        '1237:1237',  # Mock webhook
        '1238:1238',  # Manager health endpoint
    ],
    resource_deps=['populate-pact-configmap'],  # Wait for ConfigMap to be populated
    # Tilt automatically detects image dependencies from k8s_yaml
    # The deployment references 'pact-mock-server' and 'mock-webhook' images
    # which are built by custom_build resources above
    # All services (pact-broker, aws-mock-server, gcp-mock-server, azure-mock-server, mock-webhook)
    # are part of this single deployment, accessed via their respective services
    # Contract publishing is handled by the manager sidecar which reads from the ConfigMap
)

# Populate the postgres-migrations ConfigMap from local migration SQL files
# This runs whenever migration files are added or changed
# The ConfigMap is then used by the postgres-manager sidecar to run migrations
local_resource(
    'populate-migrations-configmap',
    cmd='python3 scripts/tilt/populate_migrations_configmap.py',
    deps=[
        'scripts/tilt/populate_migrations_configmap.py',
        'crates/pact-mock-server/migrations',  # Watch the entire migrations directory for changes
    ],
    labels=['pact'],
    resource_deps=[],  # Can run independently, but should run before postgres
    allow_parallel=False,
)

# PostgreSQL Database for Mock Servers
# Provides persistent storage for secrets across pod restarts
# Each provider uses a separate schema (gcp, aws, azure) for isolation
# Includes a migration manager sidecar that watches ConfigMap and runs migrations automatically
k8s_resource(
    'postgres',
    labels=['pact'],
    port_forwards=[
        '5432:5432',  # PostgreSQL database
        '1239:1239',  # Postgres manager health endpoint
    ],
    resource_deps=['populate-migrations-configmap'],  # Wait for ConfigMap to be populated
    # Tilt automatically detects image dependency from k8s_yaml and substitutes 'localhost:5000/postgres-manager' 
    # with the built image reference (localhost:5000/postgres-manager:tilt-{hash})
    # PostgreSQL is deployed via k8s_yaml above (pact-broker/k8s/postgres-deployment.yaml)
    # The postgres-manager sidecar watches ConfigMap and runs migrations automatically once postgres is ready
)

# ====================
# Run Pact tests
# 
# INDEPENDENT: Only depends on pact-broker, not on controller resources.
# Can run independently: 'tilt up pact' or filter by 'pact' label.

local_resource(
    'pact-tests',
    cmd='python3 scripts/pact_tests.py',
    deps=[
        '%s/tests' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        'scripts/pact_tests.py',
    ],
    resource_deps=['pact-infrastructure'],  # Wait for infrastructure
    labels=['pact'],
    allow_parallel=False,
)

# ====================
# Pact Mode Configuration
# ====================
# Pact mode environment variables are applied via kustomize patch
# See config/kustomization.yaml patches section
# This routes cloud provider requests to Pact mock servers instead of real APIs
# Enabled by default for local development/testing without cloud accounts
#
# The patch uses Kubernetes service names:
#   - http://pact-broker.secret-manager-controller-pact-broker.svc.cluster.local:9292
#
# To disable Pact mode, remove the patch from config/kustomization.yaml

# ====================
# GitOps Activation
# ====================
# Activate GitOps resources for tilt environment
# Applies GitRepository and other GitOps resources, triggering reconciliation
# Applied after git-credentials are set up to ensure secret exists if GitRepository references it

# GitOps Activation - FluxCD (default for tilt environment)
local_resource(
    'gitops-activate-fluxcd',
    cmd='kubectl apply -k gitops/cluster/fluxcd/env/tilt',
    deps=[
        'gitops/cluster/fluxcd/env/tilt/gitrepository.yaml',
        'gitops/cluster/fluxcd/env/tilt/secretmanagerconfig.yaml',
        'gitops/cluster/fluxcd/env/tilt/kustomization.yaml',
    ],
    labels=['infrastructure', ],
    resource_deps=['git-credentials-setup'],
    allow_parallel=False,
)

# GitOps Activation - ArgoCD (optional, for testing ArgoCD support)
local_resource(
    'gitops-activate-argocd',
    cmd='kubectl apply -k gitops/cluster/argocd/env/tilt',
    deps=[
        'gitops/cluster/argocd/env/tilt/application.yaml',
        'gitops/cluster/argocd/env/tilt/secretmanagerconfig.yaml',
        'gitops/cluster/argocd/env/tilt/kustomization.yaml',
    ],
    labels=['infrastructure', ],
    resource_deps=['git-credentials-setup'],
    allow_parallel=False,
)

# Also load via k8s_yaml for Tilt to track the resources
# Using top-level kustomization.yaml as entrypoint
# This includes namespaces and all environment configurations
# Note: allow_duplicates=True is safe - Kubernetes handles idempotent applies gracefully
# Note: microscaler-system namespace is created by Kind cluster setup, not by GitOps
# Kubernetes handles idempotent applies gracefully, so duplicates are safe
# CRITICAL: Must wait for build-all-binaries to ensure CRD is applied before SecretManagerConfig resources
# We use a local_resource to apply gitops resources after CRD is ready
local_resource(
    'apply-gitops-cluster',
    cmd='kubectl apply -k gitops/cluster',
    deps=[
        'gitops/cluster',  # Watch for changes in gitops resources
    ],
    labels=['infrastructure'],
    resource_deps=['build-all-binaries'],  # Wait for CRD to be generated and applied
    allow_parallel=False,
)

# Also load via k8s_yaml for Tilt to track the resources (for UI visibility)
# This allows Tilt to show the resources in the UI, but they're actually applied by the local_resource above
k8s_yaml(kustomize('gitops/cluster'), allow_duplicates=True,)

# ====================
# Test Resource Management
# ====================
# Install/update CRD (if changed) and apply test SecretManagerConfig resource
# Independent resource - can be run separately for testing
# Note: CRD is applied (not deleted) - kubectl apply handles install/update automatically

local_resource(
    'test-resource-update',
    cmd='python3 scripts/tilt/reset_test_resource.py',
    deps=[
        'gitops/cluster/env/tilt/secretmanagerconfig.yaml',
        'gitops/cluster/env/stage/secretmanagerconfig.yaml',
        'gitops/cluster/env/prod/secretmanagerconfig.yaml',
        'config/crd/secretmanagerconfig.yaml',
        './scripts/tilt/reset_test_resource.py',
    ],
    env={
        'CONTROLLER_DIR': CONTROLLER_DIR,
    },
    resource_deps=['build-all-binaries'],
    labels=['controllers'],
    allow_parallel=True,
)

# ====================
# Documentation Site
# ====================

# Build search index for documentation site
# This runs independently so developers can rebuild the search index
# without rebuilding the entire Docker image
local_resource(
    'build-docs-search-index',
    cmd='cd docs-site && ([ -d node_modules ] || npm install) && npm run build:search-index',
    deps=[
        'docs-site/package.json',
        'docs-site/package-lock.json',
        'docs-site/scripts/build-search-index.ts',
        'docs-site/src/data/sections.ts',
        'docs-site/src/data/content',  # Watch all content files
    ],
    labels=['docs'],
    allow_parallel=True,
)

# Build documentation site Docker image
# Tilt will watch docs-site/ for changes and rebuild
# Note: The search index is built as part of 'npm run build' in the Dockerfile
# but can also be built independently via build-docs-search-index resource
docker_build(
    'docs-site',
    '.',
    dockerfile='./dockerfiles/Dockerfile.docs-site',
    platform='linux/amd64',
    only=[
        './docs-site',
        './dockerfiles/Dockerfile.docs-site',
        './dockerfiles/nginx.docs-site.conf',
    ],
    ignore=[
        'docs-site/node_modules',
        'docs-site/dist',
        'docs-site/.git',
    ],
)

# Documentation site service (ClusterIP with port forward)
k8s_resource(
    'docs-site',
    port_forwards='8800:80',
    labels=['docs'],
    resource_deps=['build-docs-search-index'],  # Ensure search index is built before deployment
)
