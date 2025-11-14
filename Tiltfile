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

CONTROLLER_DIR = './hack/controllers/secret-manager-controller'
CONTROLLER_NAME = 'secret-manager-controller'
IMAGE_NAME = 'localhost:5001/pricewhisperer-secret-manager-controller'
BINARY_NAME = 'secret-manager-controller'
BINARY_PATH = '%s/target/x86_64-unknown-linux-musl/debug/%s' % (CONTROLLER_DIR, BINARY_NAME)

# ====================
# CRD Generation
# ====================
# Regenerate CRD whenever Rust code changes

local_resource(
    'secret-manager-controller-crd-gen',
    cmd='''
        echo "🔄 Regenerating SecretManagerConfig CRD..."
        cd hack/controllers/secret-manager-controller
        cargo run --bin crdgen 2>/dev/null > config/crd/secretmanagerconfig.yaml
        echo "✅ CRD regenerated: config/crd/secretmanagerconfig.yaml"
    ''',
    deps=[
        '%s/src' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        '%s/Cargo.lock' % CONTROLLER_DIR,
    ],
    resource_deps=[],
    labels=['controllers'],
    allow_parallel=True,
)

# ====================
# Build Rust Binary
# ====================
# Build the controller binary for local development (debug build for faster iteration)

local_resource(
    'secret-manager-controller-build',
    cmd='''
        echo "🔨 Building secret-manager-controller..."
        cd hack/controllers/secret-manager-controller
        cargo build --target x86_64-unknown-linux-musl
        echo "✅ Build complete: target/x86_64-unknown-linux-musl/debug/secret-manager-controller"
    ''',
    deps=[
        '%s/src' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        '%s/Cargo.lock' % CONTROLLER_DIR,
    ],
    resource_deps=[],
    labels=['controllers'],
    allow_parallel=True,
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

k8s_yaml(kustomize('%s/config' % CONTROLLER_DIR))

# Update deployment image and configure resource
k8s_resource(
    CONTROLLER_NAME,
    image=IMAGE_NAME,
    port_forwards='8080:8080',  # Metrics port
    labels=['controllers'],
    resource_deps=['secret-manager-controller-build', 'secret-manager-controller-crd-gen'],
)

