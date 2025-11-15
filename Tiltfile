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

# ====================
# Configuration
# ====================

# Restrict to k3s cluster
allow_k8s_contexts(['k3s-secret-manager-controller'])

# Get the directory where this Tiltfile is located
# Since the Tiltfile is in the controller directory, use '.' for relative paths
CONTROLLER_DIR = '.'
CONTROLLER_NAME = 'secret-manager-controller'
IMAGE_NAME = 'localhost:5002/secret-manager-controller'
BINARY_NAME = 'secret-manager-controller'
# Build for Linux x86_64 (cross-compile for container compatibility)
BINARY_PATH = '%s/target/x86_64-unknown-linux-musl/debug/%s' % (CONTROLLER_DIR, BINARY_NAME)
CRDGEN_PATH = '%s/target/x86_64-unknown-linux-musl/debug/crdgen' % CONTROLLER_DIR
# Native crdgen for host execution (CRD generation runs on host, not in container)
CRDGEN_NATIVE_PATH = '%s/target/debug/crdgen' % CONTROLLER_DIR
ARTIFACT_PATH = 'build_artifacts/%s' % BINARY_NAME
CRDGEN_ARTIFACT_PATH = 'build_artifacts/crdgen'

# ====================
# Code Quality Checks
# ====================
# Run formatting and linting checks
# Disabled for now
# local_resource(
#     'secret-manager-controller-fmt-check',
#     cmd='''
#         echo "🎨 Checking code formatting..."
#         cargo fmt --all -- --check || {
#             echo "❌ Formatting check failed. Run 'cargo fmt' to fix."
#             exit 1
#         }
#         echo "✅ Formatting check passed"
#     ''',
#     deps=[
#         '%s/src' % CONTROLLER_DIR,
#         '%s/Cargo.toml' % CONTROLLER_DIR,
#     ],
#     resource_deps=[],
#     labels=['code-quality'],
#     allow_parallel=True,
# )

# local_resource(
#     'secret-manager-controller-clippy',
#     cmd='''
#         echo "🔍 Running clippy..."
#         cargo clippy --all-targets --all-features -- -D warnings || {
#             echo "❌ Clippy check failed. Fix the warnings above."
#             exit 1
#         }
#         echo "✅ Clippy check passed"
#     ''',
#     deps=[
#         '%s/src' % CONTROLLER_DIR,
#         '%s/Cargo.toml' % CONTROLLER_DIR,
#         '%s/Cargo.lock' % CONTROLLER_DIR,
#     ],
#     resource_deps=[],
#     labels=['code-quality'],
#     allow_parallel=True,
# )


# ====================
# Build Rust Binaries
# ====================
# Build both controller and crdgen binaries on host (cross-compilation)

local_resource(
    'secret-manager-controller-build',
    cmd='python3 scripts/tilt/build_binaries.py',
    deps=[
        '%s/src' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        '%s/Cargo.lock' % CONTROLLER_DIR,
        './scripts/host_aware_build.py',
        './scripts/tilt/build_binaries.py',
    ],
    env={
        'CONTROLLER_DIR': CONTROLLER_DIR,
        'BINARY_NAME': BINARY_NAME,
    },
    labels=['controllers'],
    allow_parallel=False,
)

# ====================
# Copy Binaries to Artifacts
# ====================
# Copy binaries to build_artifacts directory for Docker builds

local_resource(
    'secret-manager-controller-copy',
    cmd='python3 scripts/tilt/copy_binaries.py',
    deps=[BINARY_PATH, CRDGEN_PATH, './scripts/copy_binary.py', './scripts/tilt/copy_binaries.py'],
    env={
        'CONTROLLER_DIR': CONTROLLER_DIR,
        'BINARY_NAME': BINARY_NAME,
    },
    resource_deps=['secret-manager-controller-build'],
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
    resource_deps=['secret-manager-controller-build'],
    labels=['controllers'],
    allow_parallel=True,
)

# ====================
# Docker Build
# ====================
# Build Docker image using custom_build (matches PriceWhisperer pattern)
# Delete pod and image before building to force fresh rebuild

local_resource(
    'secret-manager-controller-cleanup',
    cmd='python3 scripts/tilt/cleanup.py',
    deps=['./scripts/tilt/cleanup.py'],
    env={
        'IMAGE_NAME': IMAGE_NAME,
        'CONTROLLER_NAME': CONTROLLER_NAME,
    },
    resource_deps=[],
    labels=['controllers'],
    allow_parallel=True,
)

custom_build(
    IMAGE_NAME,
    'python3 scripts/tilt/docker_build.py',
    deps=[
        ARTIFACT_PATH,
        CRDGEN_ARTIFACT_PATH,
        '%s/Dockerfile.dev' % CONTROLLER_DIR,
        './scripts/tilt/docker_build.py',
    ],
    env={
        'IMAGE_NAME': IMAGE_NAME,
        'CONTROLLER_NAME': CONTROLLER_NAME,
        'CONTROLLER_DIR': CONTROLLER_DIR,
    },
    tag='tilt',
    live_update=[
        sync(ARTIFACT_PATH, '/app/secret-manager-controller'),
        run('kill -HUP 1', trigger=[ARTIFACT_PATH]),
    ],
    skips_local_docker=False,
)

# ====================
# Deploy to Kubernetes
# ====================
# Deploy using kustomize
# Note: CRD file must exist before kustomize runs (generated by crd-gen resource)

k8s_yaml(kustomize('%s/config' % CONTROLLER_DIR))

# Configure resource
# Tilt will automatically substitute the image in the deployment
# because custom_build registers the image and Tilt matches it to the deployment
# Note: No port forwarding needed - pods get their own IPs
# Use 'kubectl port-forward' or 'just port-forward' to access metrics
k8s_resource(
    CONTROLLER_NAME,
    labels=['controllers'],
    resource_deps=['secret-manager-controller-copy', 'secret-manager-controller-crd-gen', 'secret-manager-controller-cleanup'],
)

# ====================
# Pact Broker Deployment
# ====================
# Deploy Pact Broker for contract testing

k8s_yaml(kustomize('pact-broker/k8s'))

k8s_resource(
    'pact-broker',
    labels=['pact'],
    port_forwards=['9292:9292'],
)

# ====================
# Pact Contract Publishing
# ====================
# Run Pact tests and publish contracts to broker

local_resource(
    'pact-tests-and-publish',
    cmd='python3 scripts/pact_publish.py',
    deps=[
        '%s/tests' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        'scripts/pact_publish.py',
    ],
    resource_deps=['pact-broker'],
    labels=['pact'],
    allow_parallel=False,
)

# ====================
# Test Resource Management
# ====================
# Delete and reapply test SecretManagerConfig resource
# Independent resource - can be run separately for testing

local_resource(
    'test-resource-reset',
    cmd='python3 scripts/tilt/reset_test_resource.py',
    deps=[
        'examples/test-sops-config.yaml',
        './scripts/tilt/reset_test_resource.py',
    ],
    resource_deps=[],
    labels=['test'],
    allow_parallel=True,
)
