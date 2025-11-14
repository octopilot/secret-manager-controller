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
    cmd='''
        # Delete old binaries to force fresh build
        echo "🧹 Cleaning old binaries from target directory..."
        rm -f %s/target/x86_64-unknown-linux-musl/debug/%s
        rm -f %s/target/x86_64-unknown-linux-musl/debug/crdgen
        rm -f %s/target/debug/crdgen
        # Clean Cargo build artifacts for these specific binaries to force rebuild
        # This ensures timestamp changes trigger a rebuild even if source is identical
        echo "🧹 Cleaning Cargo build artifacts..."
        cargo clean -p secret-manager-controller --target x86_64-unknown-linux-musl 2>/dev/null || true
        cargo clean -p secret-manager-controller --bin crdgen --target x86_64-unknown-linux-musl 2>/dev/null || true
        cargo clean -p secret-manager-controller --bin crdgen 2>/dev/null || true
        # Generate fresh timestamp for this build
        BUILD_TIMESTAMP=$(date +%%s)
        BUILD_DATETIME=$(date -u +"%%Y-%%m-%%d %%H:%%M:%%S UTC")
        BUILD_GIT_HASH=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
        BUILD_GIT_DIRTY=$(git diff --quiet && echo "" || echo "-dirty")
        echo "📋 Build info:"
        echo "  Timestamp: $BUILD_TIMESTAMP"
        echo "  DateTime: $BUILD_DATETIME"
        echo "  Git Hash: $BUILD_GIT_HASH$BUILD_GIT_DIRTY"
        # Build Linux binaries for container (cross-compilation) with timestamp
        # Note: Building DEBUG binaries (no --release flag)
        echo "🔨 Building Linux binaries (debug mode)..."
        if ! BUILD_TIMESTAMP=$BUILD_TIMESTAMP BUILD_DATETIME="$BUILD_DATETIME" BUILD_GIT_HASH="$BUILD_GIT_HASH$BUILD_GIT_DIRTY" ./scripts/host-aware-build.sh --bin %s --bin crdgen; then
            echo "❌ Error: Failed to build Linux binaries" >&2
            exit 1
        fi
        # Also build native crdgen for host execution (CRD generation) with timestamp
        echo "🔨 Building native crdgen (debug mode)..."
        if ! BUILD_TIMESTAMP=$BUILD_TIMESTAMP BUILD_DATETIME="$BUILD_DATETIME" BUILD_GIT_HASH="$BUILD_GIT_HASH$BUILD_GIT_DIRTY" cargo build --bin crdgen; then
            echo "❌ Error: Failed to build native crdgen" >&2
            exit 1
        fi
        # Verify binaries were created
        echo "🔍 Verifying binaries were built..."
        BUILD_ERROR=0
        if [ ! -f "%s/target/x86_64-unknown-linux-musl/debug/%s" ]; then
            echo "❌ Error: Binary not found at %s/target/x86_64-unknown-linux-musl/debug/%s" >&2
            BUILD_ERROR=1
        else
            echo "  ✅ %s built successfully" 
        fi
        if [ ! -f "%s/target/x86_64-unknown-linux-musl/debug/crdgen" ]; then
            echo "❌ Error: crdgen not found at %s/target/x86_64-unknown-linux-musl/debug/crdgen" >&2
            BUILD_ERROR=1
        else
            echo "  ✅ crdgen (Linux) built successfully"
        fi
        if [ ! -f "%s/target/debug/crdgen" ]; then
            echo "❌ Error: Native crdgen not found at %s/target/debug/crdgen" >&2
            BUILD_ERROR=1
        else
            echo "  ✅ crdgen (native) built successfully"
        fi
        if [ $BUILD_ERROR -eq 0 ]; then
            echo "✅ Build complete - all binaries verified"
        else
            echo "❌ Build failed - some binaries are missing" >&2
            exit 1
        fi
    ''' % (CONTROLLER_DIR, BINARY_NAME, CONTROLLER_DIR, CONTROLLER_DIR, BINARY_NAME, CONTROLLER_DIR, CONTROLLER_DIR, CONTROLLER_DIR, CONTROLLER_DIR, CONTROLLER_DIR),
    deps=[
        '%s/src' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
        '%s/Cargo.lock' % CONTROLLER_DIR,
        './scripts/host-aware-build.sh',
    ],
    labels=['controllers'],
    allow_parallel=False,
)

# ====================
# Copy Binaries to Artifacts
# ====================
# Copy binaries to build_artifacts directory for Docker builds

local_resource(
    'secret-manager-controller-copy',
    cmd='''
        # Ensure build_artifacts directory exists
        mkdir -p build_artifacts
        # Delete old binaries to ensure fresh copy
        echo "🧹 Cleaning old binaries from build_artifacts..."
        rm -f %s
        rm -f %s
        # Copy new binaries with error checking
        echo "📋 Copying new binaries..."
        COPY_ERROR=0
        if ! ./scripts/copy-binary.sh %s %s %s; then
            echo "❌ Error: Failed to copy %s" >&2
            COPY_ERROR=1
        fi
        if ! ./scripts/copy-binary.sh %s %s crdgen; then
            echo "❌ Error: Failed to copy crdgen" >&2
            COPY_ERROR=1
        fi
        # Output hashes to verify what was copied
        echo ""
        echo "📊 Binary Hashes (verify what was built):"
        BINARY_OK=0
        CRDGEN_OK=0
        if [ -f "%s" ]; then
            echo "  %s: $(md5 -q %s)"
            echo "    Size: $(stat -f%%z %s) bytes"
            BINARY_OK=1
        else
            echo "  ❌ %s not found!" >&2
            COPY_ERROR=1
        fi
        if [ -f "%s" ]; then
            echo "  crdgen: $(md5 -q %s)"
            echo "    Size: $(stat -f%%z %s) bytes"
            CRDGEN_OK=1
        else
            echo "  ❌ crdgen not found!" >&2
            COPY_ERROR=1
        fi
        # Only report success if both binaries exist
        if [ $COPY_ERROR -eq 0 ] && [ $BINARY_OK -eq 1 ] && [ $CRDGEN_OK -eq 1 ]; then
            echo "✅ Binaries copied successfully"
        else
            echo "❌ Binary copy failed - check errors above" >&2
            exit 1
        fi
    ''' % (ARTIFACT_PATH, CRDGEN_ARTIFACT_PATH, BINARY_PATH, ARTIFACT_PATH, BINARY_NAME, BINARY_NAME, CRDGEN_PATH, CRDGEN_ARTIFACT_PATH, ARTIFACT_PATH, BINARY_NAME, ARTIFACT_PATH, ARTIFACT_PATH, ARTIFACT_PATH, CRDGEN_ARTIFACT_PATH, CRDGEN_ARTIFACT_PATH, CRDGEN_ARTIFACT_PATH),
    deps=[BINARY_PATH, CRDGEN_PATH, './scripts/copy-binary.sh'],
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
    cmd='''
        mkdir -p config/crd
        # Check if native crdgen binary exists
        if [ ! -f "%s" ]; then
            echo "❌ Error: crdgen binary not found at %s" >&2
            echo "   Make sure 'secret-manager-controller-build' has completed" >&2
            exit 1
        fi
        # Use native crdgen binary (runs on host, not in container)
        # Redirect stdout to CRD file, stderr to Tilt logs separately
        # This ensures error messages don't corrupt the CRD file
        RUST_LOG=off "%s" > config/crd/secretmanagerconfig.yaml 2> /tmp/crdgen-stderr.log
        exit_code=$?
        if [ $exit_code -ne 0 ]; then
            echo "❌ Error: CRD generation command failed with exit code $exit_code" >&2
            if [ -s /tmp/crdgen-stderr.log ]; then
                echo "Error output:" >&2
                cat /tmp/crdgen-stderr.log >&2
            fi
            # Don't leave invalid YAML in the CRD file
            rm -f config/crd/secretmanagerconfig.yaml
            exit $exit_code
        fi
        # Validate CRD is valid YAML (must contain apiVersion, kind, or --- after comments)
        # Skip comment lines and check for actual YAML content
        if ! grep -v '^#' config/crd/secretmanagerconfig.yaml | grep -qE '^(apiVersion|kind|---)'; then
            echo "❌ Error: CRD generation failed - file does not contain valid YAML" >&2
            echo "First 10 lines of output:" >&2
            head -10 config/crd/secretmanagerconfig.yaml >&2
            exit 1
        fi
        echo "✅ CRD generated successfully"
        # Delete existing CRD before applying (handles schema changes)
        echo "📋 Deleting existing CRD (if exists)..."
        kubectl delete crd secretmanagerconfigs.secret-management.microscaler.io 2>/dev/null || true
        # Apply CRD to Kubernetes cluster
        echo "📋 Applying CRD to cluster..."
        kubectl apply -f config/crd/secretmanagerconfig.yaml
        apply_exit_code=$?
        if [ $apply_exit_code -eq 0 ]; then
            echo "✅ CRD applied successfully"
        else
            echo "❌ Error: CRD apply failed with exit code $apply_exit_code" >&2
            exit $apply_exit_code
        fi
    ''' % (CRDGEN_NATIVE_PATH, CRDGEN_NATIVE_PATH, CRDGEN_NATIVE_PATH),
    deps=[
        CRDGEN_NATIVE_PATH,
        '%s/src' % CONTROLLER_DIR,
        '%s/Cargo.toml' % CONTROLLER_DIR,
    ],
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
    cmd='''
        echo "🧹 Cleaning up controller pod and image before rebuild..."
        # Delete controller pod (will be recreated by deployment)
        echo "📋 Deleting controller pod..."
        kubectl delete pod -n microscaler-system -l app=secret-manager-controller --ignore-not-found=true 2>&1 || true
        # Delete all versions of the image to force fresh build
        echo "📋 Deleting all image tags..."
        docker rmi %s:tilt 2>/dev/null || true
        # Remove all tilt-* tags (Tilt generates these based on content hash)
        docker images %s --format "{{.Tag}}" | grep "^tilt-" | while read tag; do
            docker rmi %s:$tag 2>/dev/null || true
            docker rmi localhost:5002/%s:$tag 2>/dev/null || true
        done || true
        docker rmi localhost:5002/%s:tilt 2>/dev/null || true
        # Also try to remove from kind's containerd if it's a kind cluster
        echo "📋 Cleaning up kind registry cache..."
        docker exec kind-registry sh -c "rm -rf /var/lib/registry/docker/registry/v2/repositories/%s/" 2>/dev/null || true
        # Force remove dangling images
        docker image prune -f 2>/dev/null || true
        echo "✅ Cleanup complete"
    ''' % (IMAGE_NAME, IMAGE_NAME, IMAGE_NAME, CONTROLLER_NAME, CONTROLLER_NAME, CONTROLLER_NAME),
    deps=[],
    resource_deps=[],
    labels=['controllers'],
    allow_parallel=True,
)

custom_build(
    IMAGE_NAME,
    '''
    # Cleanup before build
    echo "🧹 Cleaning up old images..."
    docker rmi %s:tilt 2>/dev/null || true
    docker images %s --format "{{.Tag}}" | grep "^tilt-" | while read tag; do
        docker rmi %s:$tag 2>/dev/null || true
        docker rmi localhost:5002/%s:$tag 2>/dev/null || true
    done || true
    docker rmi localhost:5002/%s:tilt 2>/dev/null || true
    docker exec kind-registry sh -c "rm -rf /var/lib/registry/docker/registry/v2/repositories/%s/" 2>/dev/null || true
    # Build with timestamp
    TIMESTAMP=$(date +%%s) && docker build --no-cache -f %s/Dockerfile.dev -t %s:tilt-$TIMESTAMP %s && docker tag %s:tilt-$TIMESTAMP $EXPECTED_REF && docker push $EXPECTED_REF
    ''' % (
        IMAGE_NAME, IMAGE_NAME, IMAGE_NAME, CONTROLLER_NAME, CONTROLLER_NAME, CONTROLLER_NAME,
        CONTROLLER_DIR, IMAGE_NAME, CONTROLLER_DIR, IMAGE_NAME
    ),
    deps=[
        ARTIFACT_PATH,
        CRDGEN_ARTIFACT_PATH,
        '%s/Dockerfile.dev' % CONTROLLER_DIR,
    ],
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
    cmd='''
        echo "🔄 Resetting test SecretManagerConfig resource..."
        # Delete existing resource (ignore errors if it doesn't exist)
        echo "📋 Deleting existing resource (if exists)..."
        kubectl delete secretmanagerconfig test-sops-config --ignore-not-found=true
        # Wait a moment for deletion to complete
        sleep 1
        # Apply the resource
        echo "📋 Applying test SecretManagerConfig resource..."
        kubectl apply -f examples/test-sops-config.yaml
        apply_exit_code=$?
        if [ $apply_exit_code -eq 0 ]; then
            echo "✅ Test resource applied successfully"
            echo "📋 Resource: test-sops-config"
            echo "📋 Namespace: default"
        else
            echo "❌ Error: Failed to apply test resource (exit code: $apply_exit_code)" >&2
            exit $apply_exit_code
        fi
    ''',
    deps=[
        'examples/test-sops-config.yaml',
    ],
    resource_deps=[],
    labels=['test'],
    allow_parallel=True,
)
