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
    python3 scripts/dev_up.py

# Stop development environment (K3s + Tilt)
dev-down:
    python3 scripts/dev_down.py
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
    python3 scripts/undeploy.py

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
    python3 scripts/status.py

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
    python3 scripts/check_deps.py

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

