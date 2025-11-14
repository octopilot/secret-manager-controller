# Secret Manager Controller Tiltfile
# 
# This Tiltfile:
# 1. Generates CRD on changes to Rust code
# 2. Builds the Rust binary
# 3. Builds Docker image
# 4. Deploys to Kubernetes using kustomize

# ====================
# Configuration
# ====================

# Restrict to kind cluster (independent cluster for secret-manager-controller)
allow_k8s_contexts(['kind-secret-manager-controller'])

# Get the directory where this Tiltfile is located
# Since the Tiltfile is in the controller directory, use '.' for relative paths
CONTROLLER_DIR = '.'
CONTROLLER_NAME = 'secret-manager-controller'
IMAGE_NAME = 'localhost:5002/secret-manager-controller'
BINARY_NAME = 'secret-manager-controller'
BINARY_PATH = '%s/target/x86_64-unknown-linux-musl/debug/%s' % (CONTROLLER_DIR, BINARY_NAME)

# ====================
# CRD Generation
# ====================
# Regenerate CRD using the built crdgen binary (not cargo run)

CRDGEN_BINARY_PATH = '%s/target/x86_64-unknown-linux-musl/debug/crdgen' % CONTROLLER_DIR

local_resource(
    'secret-manager-controller-crd-gen',
    cmd='''
        echo "🔄 Regenerating SecretManagerConfig CRD..."
        if [ ! -f %s ]; then
            echo "❌ CRD gen binary not found: %s"
            echo "   Waiting for build to complete..."
            exit 1
        fi
        %s 2>/dev/null > config/crd/secretmanagerconfig.yaml
        echo "✅ CRD regenerated: config/crd/secretmanagerconfig.yaml"
    ''' % (CRDGEN_BINARY_PATH, CRDGEN_BINARY_PATH, CRDGEN_BINARY_PATH),
    deps=[
        CRDGEN_BINARY_PATH,
    ],
    resource_deps=['secret-manager-controller-build'],  # Must wait for build to complete
    labels=['controllers'],
    allow_parallel=False,  # Must run after build, before k8s deployment
)

# ====================
# Build Rust Binaries
# ====================
# Build all three binaries: controller, crdgen, and msmctl
# Uses cargo zigbuild on macOS (like PriceWhisperer microservices) for cross-compilation
# Host-aware build selection via shell script (matches BRRTRouter pattern)
build_cmd = '%s/scripts/host-aware-build.sh' % CONTROLLER_DIR

local_resource(
    'secret-manager-controller-build',
    cmd='''
        echo "🔨 Building secret-manager-controller binaries..."
        echo "   Building: secret-manager-controller, crdgen, msmctl"
        # Build all binaries at once using --bins flag (builds all [[bin]] targets)
        %s --bins
        if [ ! -f target/x86_64-unknown-linux-musl/debug/secret-manager-controller ]; then
            echo "❌ Build failed: secret-manager-controller binary not found"
            exit 1
        fi
        if [ ! -f target/x86_64-unknown-linux-musl/debug/crdgen ]; then
            echo "❌ Build failed: crdgen binary not found"
            exit 1
        fi
        if [ ! -f target/x86_64-unknown-linux-musl/debug/msmctl ]; then
            echo "❌ Build failed: msmctl binary not found"
            exit 1
        fi
        echo "✅ Build complete:"
        echo "   - secret-manager-controller"
        echo "   - crdgen"
        echo "   - msmctl"
    ''' % build_cmd,
    deps=[
        '%s/src' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        '%s/Cargo.lock' % CONTROLLER_DIR,
        '%s/scripts/host-aware-build.sh' % CONTROLLER_DIR,
    ],
    resource_deps=[],
    labels=['controllers'],
    allow_parallel=False,  # Must run first, before CRD gen
)

# ====================
# Docker Build
# ====================
# Build Docker image using custom_build for live updates

custom_build(
    IMAGE_NAME,
    'docker build -f %s/Dockerfile.dev -t %s:tilt %s && docker tag %s:tilt $EXPECTED_REF && docker push $EXPECTED_REF' % (
        CONTROLLER_DIR,
        IMAGE_NAME,
        CONTROLLER_DIR,
        IMAGE_NAME
    ),
    deps=[
        BINARY_PATH,
        '%s/Dockerfile.dev' % CONTROLLER_DIR,
    ],
    resource_deps=['secret-manager-controller-build'],  # Must wait for build to complete
    tag='tilt',
    live_update=[
        sync(BINARY_PATH, '/app/secret-manager-controller'),
        run('kill -HUP 1', trigger=[BINARY_PATH]),
    ],
)

# ====================
# Deploy to Kubernetes
# ====================
# Deploy using kustomize
# Note: k8s_yaml runs kustomize when dependencies are ready
# The k8s_resource dependency ensures CRD gen completes before deployment

k8s_yaml(kustomize('%s/config' % CONTROLLER_DIR))

# Configure resource
# Tilt will automatically substitute the image in the deployment
# because custom_build registers the image and Tilt matches it to the deployment
# Note: No port forwarding needed - pods get their own IPs
# Use 'kubectl port-forward' or 'just port-forward' to access metrics
# Dependencies: Strict serial order - build -> CRD gen -> k8s deployment
k8s_resource(
    CONTROLLER_NAME,
    labels=['controllers'],
    resource_deps=['secret-manager-controller-crd-gen'],  # CRD gen depends on build, so this ensures both complete
)

