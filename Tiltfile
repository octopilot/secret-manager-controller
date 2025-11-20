# Secret Manager Controller Tiltfile
# 
# This Tiltfile matches PriceWhisperer's build pattern:
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
update_settings(suppress_unused_image_warnings=['localhost:5000/mock-webhook'])

# Get the directory where this Tiltfile is located
# Since the Tiltfile is in the controller directory, use '.' for relative paths
CONTROLLER_DIR = '.'
CONTROLLER_NAME = 'secret-manager-controller'
IMAGE_NAME = 'localhost:5000/secret-manager-controller'
BINARY_NAME = 'secret-manager-controller'
# Build for Linux x86_64 (cross-compile for container compatibility)
BINARY_PATH = '%s/target/x86_64-unknown-linux-musl/debug/%s' % (CONTROLLER_DIR, BINARY_NAME)
CRDGEN_PATH = '%s/target/x86_64-unknown-linux-musl/debug/crdgen' % CONTROLLER_DIR
# Native crdgen for host execution (CRD generation runs on host, not in container)
CRDGEN_NATIVE_PATH = '%s/target/debug/crdgen' % CONTROLLER_DIR
ARTIFACT_PATH = 'build_artifacts/%s' % BINARY_NAME
CRDGEN_ARTIFACT_PATH = 'build_artifacts/crdgen'



# ====================
# Build and Copy Rust Binaries
# ====================
# Build binaries on host (cross-compilation) and copy to build_artifacts

local_resource(
    'secret-manager-controller-build-and-copy',
    cmd='python3 scripts/tilt/build_and_copy_binaries.py',
    deps=[
        '%s/src' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        '%s/Cargo.lock' % CONTROLLER_DIR,
        './scripts/host_aware_build.py',
        './scripts/copy_binary.py',
        './scripts/tilt/build_and_copy_binaries.py',
    ],
    env={
        'CONTROLLER_DIR': CONTROLLER_DIR,
        'BINARY_NAME': BINARY_NAME,
    },
    labels=['controllers'],
    allow_parallel=False,
)

# ====================
# CRD Generation
# ====================
# Generate CRD using crdgen binary from build_artifacts

local_resource(
    'secret-manager-controller-crd-gen',
    cmd='python3 scripts/tilt/generate_crd.py',
    deps=[
        CRDGEN_NATIVE_PATH,
        '%s/src' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        './scripts/tilt/generate_crd.py',
    ],
    env={
        'CONTROLLER_DIR': CONTROLLER_DIR,
    },
    resource_deps=['secret-manager-controller-build-and-copy'],
    labels=['controllers'],
    allow_parallel=True,
)

# ====================
# Docker Build
# ====================
# Build Docker image using custom_build (matches PriceWhisperer pattern)
# Note: docker_build.py handles image cleanup before building

custom_build(
    IMAGE_NAME,
    'python3 scripts/tilt/docker_build.py',
    deps=[
        ARTIFACT_PATH,
        CRDGEN_ARTIFACT_PATH,
        'dockerfiles/Dockerfile.controller.dev',
        './scripts/tilt/docker_build.py',
    ],
    env={
        'IMAGE_NAME': IMAGE_NAME,
        'CONTROLLER_NAME': CONTROLLER_NAME,
        'CONTROLLER_DIR': CONTROLLER_DIR,
    },
    tag='tilt',
    live_update=[
        # Sync the updated binary into the running container
        sync(ARTIFACT_PATH, '/app/secret-manager-controller'),
        # Restart the process to pick up the new binary
        # For k8s resources, we use run() to send SIGTERM which triggers graceful shutdown
        # Kubernetes will automatically restart the pod when the container exits
        # This replaces the deprecated restart_container() function
        # Note: The controller handles SIGTERM gracefully and will exit cleanly
        # run() takes a command string and optional echo_off boolean
        run('kill -TERM 1'),
    ],
    skips_local_docker=False,
)

# ====================
# Container Cleanup
# ====================
# Clean up stopped Docker containers after controller builds complete
# This prevents Docker from being overwhelmed by stopped containers from Tilt builds
# Runs as a one-shot cleanup after each controller build

local_resource(
    'container-cleanup',
    cmd='python3 scripts/tilt/cleanup_stopped_containers.py',
    deps=[
        './scripts/tilt/cleanup_stopped_containers.py',
    ],
    labels=['controllers'],
    allow_parallel=True,
    # Run after controller deployment completes to clean up stopped containers
    # This prevents accumulation of stopped containers from Docker builds
    # Depend on the Kubernetes deployment resource which uses the image
    resource_deps=[CONTROLLER_NAME],
)

# ====================
# FluxCD Installation
# ====================
# Install FluxCD in the cluster before deploying the controller
# This ensures GitRepository CRDs and source-controller are available
# Idempotent - can be run multiple times safely

local_resource(
    'fluxcd-install',
    cmd='python3 scripts/tilt/install_fluxcd.py',
    deps=[
        './scripts/tilt/install_fluxcd.py',
    ],
    labels=['infrastructure'],
    allow_parallel=False,
)

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
# Install ArgoCD in the cluster before deploying Applications
# This ensures Application CRDs and controllers are available
# Idempotent - can be run multiple times safely

local_resource(
    'argocd-install',
    cmd='python3 scripts/tilt/install_argocd.py',
    deps=[
        './scripts/tilt/install_argocd.py',
    ],
    labels=['infrastructure'],
    resource_deps=['fluxcd-install'],
    allow_parallel=False,
)

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
    resource_deps=['fluxcd-install'],
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
    resource_deps=['fluxcd-install'],
    allow_parallel=False,
)

# ====================
# Deploy to Kubernetes
# ====================
# Ensure namespace exists before applying kustomize
# Kustomize may try to apply namespace-scoped resources before namespace is created
local_resource(
    'ensure-microscaler-system-namespace',
    cmd='kubectl apply -f config/namespace.yaml',
    deps=[
        'config/namespace.yaml',
    ],
    labels=['infrastructure'],
    allow_parallel=False,
)

# Deploy using kustomize
# Note: CRD file must exist before kustomize runs (generated by crd-gen resource)
# Note: FluxCD should be installed first (fluxcd-install resource)
# Note: Namespace is created by ensure-microscaler-system-namespace resource above
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
    resource_deps=['secret-manager-controller-build-and-copy', 'secret-manager-controller-crd-gen', 'fluxcd-install', 'sops-key-setup', 'ensure-microscaler-system-namespace'],
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

# Build Pact mock server binaries on host (cross-compilation)
# Separate build resources for each binary - simpler and more targeted
local_resource(
    'gcp-mock-server-build',
    cmd='python3 scripts/tilt/build_mock_server_binary.py gcp-mock-server',
    deps=[
        'pact-broker/mock-server/src/lib.rs',
        'pact-broker/mock-server/src/bin/gcp.rs',
        'pact-broker/mock-server/Cargo.toml',
        'pact-broker/mock-server/Cargo.lock',
        './scripts/host_aware_build.py',
        './scripts/copy_binary.py',
        './scripts/tilt/build_mock_server_binary.py',
    ],
    labels=['pact'],
    allow_parallel=True,
)

local_resource(
    'aws-mock-server-build',
    cmd='python3 scripts/tilt/build_mock_server_binary.py aws-mock-server',
    deps=[
        'pact-broker/mock-server/src/lib.rs',
        'pact-broker/mock-server/src/bin/aws.rs',
        'pact-broker/mock-server/Cargo.toml',
        'pact-broker/mock-server/Cargo.lock',
        './scripts/host_aware_build.py',
        './scripts/copy_binary.py',
        './scripts/tilt/build_mock_server_binary.py',
    ],
    labels=['pact'],
    allow_parallel=True,
)

local_resource(
    'azure-mock-server-build',
    cmd='python3 scripts/tilt/build_mock_server_binary.py azure-mock-server',
    deps=[
        'pact-broker/mock-server/src/lib.rs',
        'pact-broker/mock-server/src/bin/azure.rs',
        'pact-broker/mock-server/Cargo.toml',
        'pact-broker/mock-server/Cargo.lock',
        './scripts/host_aware_build.py',
        './scripts/copy_binary.py',
        './scripts/tilt/build_mock_server_binary.py',
    ],
    labels=['pact'],
    allow_parallel=True,
)

local_resource(
    'webhook-build',
    cmd='python3 scripts/tilt/build_mock_server_binary.py webhook',
    deps=[
        'pact-broker/mock-server/src/bin/webhook.rs',
        'pact-broker/mock-server/Cargo.toml',
        'pact-broker/mock-server/Cargo.lock',
        './scripts/host_aware_build.py',
        './scripts/copy_binary.py',
        './scripts/tilt/build_mock_server_binary.py',
    ],
    labels=['pact'],
    allow_parallel=True,
)

# Build Pact mock server Docker image (Rust/Axum)
# Binaries are built on host and copied in (matches controller pattern)
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

k8s_yaml(kustomize('pact-broker/k8s'))

k8s_resource(
    'pact-broker',
    labels=['pact'],
    port_forwards=['9292:9292'],
    # No resource_deps - completely independent from controllers
)

k8s_resource(
    'gcp-mock-server',
    labels=['pact'],
    resource_deps=['gcp-mock-server-build'],
    # Tilt automatically substitutes image from custom_build('pact-mock-server')
    # The image name 'pact-mock-server' in the YAML matches the custom_build name
)

k8s_resource(
    'aws-mock-server',
    labels=['pact'],
    resource_deps=['aws-mock-server-build'],
    # Tilt automatically substitutes image from custom_build('pact-mock-server')
    # The image name 'pact-mock-server' in the YAML matches the custom_build name
)

k8s_resource(
    'azure-mock-server',
    labels=['pact'],
    resource_deps=['azure-mock-server-build'],
    # Tilt automatically substitutes image from custom_build('pact-mock-server')
    # The image name 'pact-mock-server' in the YAML matches the custom_build name
)

k8s_resource(
    'mock-webhook',
    labels=['pact'],
    resource_deps=['webhook-build'],
    port_forwards=['8080:8080'],
    # Tilt automatically substitutes image from custom_build('mock-webhook')
    # The image name 'mock-webhook' in the YAML matches the custom_build name
    # Warning about 'localhost:5000/mock-webhook' can be ignored - Tilt will substitute correctly
)

# ====================
# Pact Contract Publishing
# ====================
# Run Pact tests and publish contracts to broker
# 
# INDEPENDENT: Only depends on pact-broker, not on controller resources.
# Can run independently: 'tilt up pact' or filter by 'pact' label.

local_resource(
    'pact-tests-and-publish',
    cmd='python3 scripts/pact_publish.py',
    deps=[
        '%s/tests' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        'scripts/pact_publish.py',
    ],
    resource_deps=['pact-broker'],  # Only depends on pact-broker, not controllers
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
        'gitops/cluster/fluxcd/env/tilt/namespace.yaml',
        'gitops/cluster/fluxcd/env/tilt/gitrepository.yaml',
        'gitops/cluster/fluxcd/env/tilt/secretmanagerconfig.yaml',
        'gitops/cluster/fluxcd/env/tilt/kustomization.yaml',
    ],
    labels=['infrastructure', ],
    resource_deps=['git-credentials-setup', 'fluxcd-install'],
    allow_parallel=False,
)

# GitOps Activation - ArgoCD (optional, for testing ArgoCD support)
local_resource(
    'gitops-activate-argocd',
    cmd='kubectl apply -k gitops/cluster/argocd/env/tilt',
    deps=[
        'gitops/cluster/argocd/env/tilt/namespace.yaml',
        'gitops/cluster/argocd/env/tilt/application.yaml',
        'gitops/cluster/argocd/env/tilt/secretmanagerconfig.yaml',
        'gitops/cluster/argocd/env/tilt/kustomization.yaml',
    ],
    labels=['infrastructure', ],
    resource_deps=['git-credentials-setup', 'argocd-install'],
    allow_parallel=False,
)

# Also load via k8s_yaml for Tilt to track the resources
# Note: Both kustomizations include namespace.yaml, so we allow duplicates
# Kubernetes handles idempotent applies gracefully
k8s_yaml(kustomize('gitops/cluster/fluxcd/env/tilt'))
k8s_yaml(kustomize('gitops/cluster/argocd/env/tilt'), allow_duplicates=True)

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
    resource_deps=['secret-manager-controller-crd-gen'],
    labels=['controllers'],
    allow_parallel=True,
)
