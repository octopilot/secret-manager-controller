#!/usr/bin/env just --justfile
# Secret Manager Controller Development Justfile

# Set shell for recipes
set shell := ["bash", "-uc"]

# Default recipe to display help
default:
    @just --list --unsorted

# ============================================================================
# Development Environment
# ============================================================================

# Start development environment (K3s + Tilt)
dev-up:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "🚀 Starting Secret Manager Controller development environment (K3s)..."
    
    # Check if Docker is running
    if ! docker info >/dev/null 2>&1; then
        echo "❌ Error: Docker daemon is not running"
        echo "   Please start Docker Desktop and try again"
        exit 1
    fi

    # Check if k3s container exists
    if docker ps -a --filter "name=k3s-secret-manager-controller" --quiet | grep -q .; then
        echo "✅ K3s container 'k3s-secret-manager-controller' already exists"
        docker start k3s-secret-manager-controller 2>/dev/null || true
    else
        # Create K3s cluster
        echo "📦 Creating K3s cluster..."
        chmod +x scripts/setup-k3s.sh
        if ! ./scripts/setup-k3s.sh; then
            echo "❌ Failed to create K3s cluster"
            exit 1
        fi
    fi

    # Set kubeconfig context
    kubectl config use-context k3s-secret-manager-controller 2>/dev/null || {
        echo "⚠️  Warning: Could not set k3s context, using current context"
    }

    
    # Start Tilt
    echo "🎯 Starting Tilt..."
    tilt up


# Stop development environment (K3s + Tilt)
dev-down:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "🛑 Stopping Secret Manager Controller development environment..."
    
    # Stop Tilt
    echo "Stopping Tilt..."
    pkill -f "tilt up" 2>/dev/null || true
    
    # Stop K3s container
    echo "Stopping K3s container..."
    docker stop k3s-secret-manager-controller 2>/dev/null || true
    
    echo "✅ Development environment stopped"
# ============================================================================
# Building
# ============================================================================

# Build all (Rust binary + Docker image)
build: build-rust build-docker

# Build Rust binary (debug)
build-rust:
    @echo "🔨 Building Rust binary..."
    @cargo build

# Build Rust binary (release)
build-release:
    @echo "🔨 Building Rust binary (release)..."
    @cargo build --release


# Build Docker image (development)
build-docker:
    @echo "🐳 Building Docker image..."
    @docker build -f Dockerfile.dev -t localhost:5002/secret-manager-controller:dev .

# Build Docker image (production)
build-docker-prod:
    @echo "🐳 Building Docker image (production)..."
    @docker build -f Dockerfile -t localhost:5002/secret-manager-controller:latest .

# Generate CRD from Rust code
generate-crd:
    @echo "📝 Generating CRD..."
    @cargo run --bin crdgen > config/crd/secretmanagerconfig.yaml
    @echo "✅ CRD generated: config/crd/secretmanagerconfig.yaml"

# Build CLI tool (msmctl)
build-cli:
    @echo "🔨 Building CLI tool (msmctl)..."
    @cargo build --release --bin msmctl
    @echo "✅ CLI built: target/release/msmctl"

# ============================================================================
# Testing
# ============================================================================

# Run all tests
test: test-unit test-pact

# Run unit tests
test-unit:
    @echo "🧪 Running unit tests..."
    @cargo test --lib --no-fail-fast

# Run unit tests with output
test-unit-verbose:
    @echo "🧪 Running unit tests (verbose)..."
    @cargo test --lib -- --nocapture --no-fail-fast

# Run Pact contract tests
test-pact:
    @echo "🧪 Running Pact contract tests..."
    @cargo test --test pact_* --no-fail-fast

# Run specific Pact test suite
# Usage: just test-pact-gcp
test-pact-gcp:
    @echo "🧪 Running GCP Pact tests..."
    @cargo test --test pact_gcp_secret_manager --no-fail-fast

test-pact-aws:
    @echo "🧪 Running AWS Pact tests..."
    @cargo test --test pact_aws_secrets_manager --no-fail-fast

test-pact-azure:
    @echo "🧪 Running Azure Pact tests..."
    @cargo test --test pact_azure_key_vault --no-fail-fast

# Run tests with coverage
test-coverage:
    @echo "🧪 Running tests with coverage..."
    @cargo test --lib --no-fail-fast
    @echo "📊 Coverage report: target/debug/coverage/"

# ============================================================================
# Code Quality
# ============================================================================

# Format code
fmt:
    @echo "🎨 Formatting code..."
    @cargo fmt

# Check formatting
fmt-check:
    @echo "🎨 Checking code formatting..."
    @cargo fmt -- --check

# Lint code
lint:
    @echo "🔍 Linting code..."
    @cargo clippy -- -D warnings

# Lint and fix
lint-fix:
    @echo "🔍 Linting and fixing code..."
    @cargo clippy --fix --allow-dirty --allow-staged

# Audit dependencies
audit:
    @echo "🔒 Auditing dependencies..."
    @cargo audit

# Check code (compile without building)
check:
    @echo "✅ Checking code..."
    @cargo check --all-targets

# Validate all (format, lint, check, tests)
validate: fmt-check lint check test-unit
    @echo "✅ All validations passed!"

# ============================================================================
# Deployment
# ============================================================================

# Deploy to Kubernetes (using kustomize)
deploy:
    @echo "🚀 Deploying to Kubernetes..."
    @kubectl apply -k config/
    @echo "✅ Deployed to microscaler-system namespace"

# Deploy CRD only
deploy-crd:
    @echo "📝 Deploying CRD..."
    @kubectl apply -f config/crd/secretmanagerconfig.yaml
    @echo "✅ CRD deployed"

# Undeploy from Kubernetes
undeploy:
    @echo "🗑️ Undeploying from Kubernetes..."
    @kubectl delete -k config/ || true
    @echo "✅ Undeployed"

# ============================================================================
# Utilities
# ============================================================================

# Clean build artifacts
clean:
    @echo "🧹 Cleaning build artifacts..."
    @cargo clean
    @echo "✅ Cleaned"

# Show cluster and controller status
status:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "📊 Cluster Status:"
    echo ""
    echo "📦 Controller Pods:"
    kubectl get pods -n microscaler-system -l app=secret-manager-controller 2>/dev/null || echo "No pods found"
    echo ""
    echo "📋 SecretManagerConfig Resources:"
    kubectl get secretmanagerconfig --all-namespaces 2>/dev/null || echo "No SecretManagerConfig resources found"
    echo ""
    echo "🔧 CRD Status:"
    kubectl get crd secretmanagerconfigs.secretmanager.microscaler.io 2>/dev/null || echo "CRD not found"

# Show controller logs
logs:
    @echo "📜 Controller logs..."
    @kubectl logs -n microscaler-system -l app=secret-manager-controller --tail=100 -f

# Show controller logs (all containers)
logs-all:
    @echo "📜 Controller logs (all containers)..."
    @kubectl logs -n microscaler-system -l app=secret-manager-controller --tail=100 -f --all-containers=true

# Port forward to controller metrics
port-forward:
    @echo "🔌 Port forwarding to controller metrics (5000)..."
    @kubectl port-forward -n microscaler-system svc/secret-manager-controller-metrics 5000:5000

# ============================================================================
# Dependencies & Tools
# ============================================================================

# Check prerequisites
check-deps:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Checking dependencies..."
    command -v docker >/dev/null 2>&1 || { echo "❌ docker is required but not installed."; exit 1; }
    echo "Installing Tilt..."
    curl -fsSL https://raw.githubusercontent.com/tilt-dev/tilt/master/scripts/install.sh | bash
    echo "Installing Just..."
    curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to ~/.local/bin
    echo "✅ Tools installed!"

# ============================================================================
# CLI Tool (msmctl)
# ============================================================================

# Install CLI tool to local bin
install-cli: build-cli
    @echo "📦 Installing msmctl to ~/.local/bin..."
    @mkdir -p ~/.local/bin
    @cp target/release/msmctl ~/.local/bin/
    @echo "✅ msmctl installed! Make sure ~/.local/bin is in your PATH"

# Run CLI tool (development)
# Usage: just cli reconcile --name my-secrets
cli *args:
    @cargo run --bin msmctl -- {{args}}

# ============================================================================
# Documentation
# ============================================================================

# Generate documentation
docs:
    @echo "📚 Generating documentation..."
    @cargo doc --no-deps --open

# Generate documentation (without opening)
docs-build:
    @echo "📚 Building documentation..."
    @cargo doc --no-deps
    @echo "✅ Documentation built: target/doc/"

