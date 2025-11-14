#!/usr/bin/env bash
# Secret Manager Controller Kind Cluster Setup Script

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
CLUSTER_NAME="secret-manager-controller"
REGISTRY_NAME="secret-manager-controller-registry"
REGISTRY_PORT="5002"

# Helper functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_command() {
    if ! command -v "$1" &> /dev/null; then
        log_error "$1 is not installed. Please install it first."
        exit 1
    fi
}

# Check prerequisites
log_info "Checking prerequisites..."
check_command "docker"
check_command "kind"
check_command "kubectl"

# Create local registry if it doesn't exist
if ! docker ps | grep -q ${REGISTRY_NAME}; then
    log_info "Creating local Docker registry..."
    docker run -d --restart=always \
        -p "${REGISTRY_PORT}:5000" \
        --name "${REGISTRY_NAME}" \
        registry:2
else
    log_info "Local registry already running"
fi

# Create Kind cluster
if kind get clusters | grep -q ${CLUSTER_NAME}; then
    log_warn "Cluster ${CLUSTER_NAME} already exists"
    read -p "Do you want to delete and recreate it? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        log_info "Deleting existing cluster..."
        kind delete cluster --name ${CLUSTER_NAME}
    else
        log_info "Using existing cluster"
        exit 0
    fi
fi

if ! kind get clusters | grep -q ${CLUSTER_NAME}; then
    log_info "Creating Kind cluster..."
    kind create cluster --config kind-config.yaml
    
    # Connect registry to cluster network
    if docker network ls | grep -q "kind"; then
        docker network connect "kind" "${REGISTRY_NAME}" 2>/dev/null || true
    fi
    
    # Configure cluster to use local registry
    kubectl apply -f - <<EOF
apiVersion: v1
kind: ConfigMap
metadata:
  name: local-registry-hosting
  namespace: kube-public
data:
  localRegistryHosting.v1: |
    host: "localhost:${REGISTRY_PORT}"
    help: "https://kind.sigs.k8s.io/docs/user/local-registry/"
EOF
    
    log_info "✅ Kind cluster '${CLUSTER_NAME}' created successfully!"
    log_info "📦 Local registry: ${REGISTRY_NAME} (localhost:${REGISTRY_PORT})"
    log_info "🚀 You can now run 'tilt up' to start the controller"
else
    log_info "✅ Kind cluster '${CLUSTER_NAME}' is ready"
fi

