#!/usr/bin/env bash
# Secret Manager Controller K3s Setup Script

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
CLUSTER_NAME="secret-manager-controller"
CONTAINER_NAME="k3s-${CLUSTER_NAME}"
REGISTRY_NAME="secret-manager-controller-registry"
REGISTRY_PORT="5002"
K3S_PORT="6443"

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
check_command "kubectl"

# Check if Docker is running
if ! docker info >/dev/null 2>&1; then
    log_error "Docker daemon is not running"
    echo "   Please start Docker Desktop and try again"
    exit 1
fi

# Check if registry container exists (running or stopped)
if docker ps -a --filter "name=${REGISTRY_NAME}" --quiet | grep -q .; then
    log_info "Registry container '${REGISTRY_NAME}' already exists"
    # Check if it's running
    if docker ps --filter "name=${REGISTRY_NAME}" --quiet | grep -q .; then
        log_info "Registry container is already running"
    else
        # Try to start it
        log_info "Starting existing registry container..."
        if ! docker start ${REGISTRY_NAME} 2>/dev/null; then
            log_warn "Failed to start existing registry (port may be in use)"
            log_info "Removing failed container..."
            docker rm ${REGISTRY_NAME} 2>/dev/null || true
            # Check if port is available
            if lsof -i :${REGISTRY_PORT} >/dev/null 2>&1 || netstat -an 2>/dev/null | grep -q ":${REGISTRY_PORT}.*LISTEN"; then
                log_error "Port ${REGISTRY_PORT} is already in use by another process"
                log_info "Please stop the process using port ${REGISTRY_PORT} or use a different port"
                exit 1
            fi
            log_info "Creating new registry container..."
            docker run -d --restart=always \
                -p "${REGISTRY_PORT}:5000" \
                --name "${REGISTRY_NAME}" \
                registry:2 || {
                log_error "Failed to create registry"
                exit 1
            }
        fi
    fi
else
    # Check if port is available before creating
    if lsof -i :${REGISTRY_PORT} >/dev/null 2>&1 || netstat -an 2>/dev/null | grep -q ":${REGISTRY_PORT}.*LISTEN"; then
        log_error "Port ${REGISTRY_PORT} is already in use"
        log_info "Port ${REGISTRY_PORT} is being used by:"
        lsof -i :${REGISTRY_PORT} 2>/dev/null || docker ps --format "{{.Names}}: {{.Ports}}" | grep ${REGISTRY_PORT} || true
        log_info "Please stop the process using port ${REGISTRY_PORT} or modify REGISTRY_PORT in the script"
        exit 1
    fi
    log_info "Creating local Docker registry..."
    docker run -d --restart=always \
        -p "${REGISTRY_PORT}:5000" \
        --name "${REGISTRY_NAME}" \
        registry:2 || {
        log_error "Failed to create registry"
        exit 1
    }
fi

# Create Docker network for k3s if it doesn't exist
if ! docker network ls | grep -q "^.*k3s-net.*"; then
    log_info "Creating Docker network for k3s..."
    docker network create k3s-net 2>/dev/null || true
fi

# Connect registry to k3s network (if registry exists)
if docker ps -a --filter "name=${REGISTRY_NAME}" --quiet | grep -q .; then
    docker network connect k3s-net ${REGISTRY_NAME} 2>/dev/null || true
fi

# Check if k3s container already exists
if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    log_warn "K3s container '${CONTAINER_NAME}' already exists"
    read -p "Do you want to delete and recreate it? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        log_info "Stopping and removing existing container..."
        docker stop ${CONTAINER_NAME} 2>/dev/null || true
        docker rm ${CONTAINER_NAME} 2>/dev/null || true
    else
        log_info "Using existing container"
        docker start ${CONTAINER_NAME} 2>/dev/null || true
        # Get kubeconfig from existing container
        log_info "Retrieving kubeconfig from existing container..."
        mkdir -p ~/.kube
        docker cp ${CONTAINER_NAME}:/etc/rancher/k3s/k3s.yaml ~/.kube/k3s-${CLUSTER_NAME}.yaml 2>/dev/null || {
            log_error "Failed to retrieve kubeconfig"
            exit 1
        }
        # Update kubeconfig
        sed -i.bak "s/127.0.0.1/localhost/g" ~/.kube/k3s-${CLUSTER_NAME}.yaml 2>/dev/null || \
        sed -i '' "s/127.0.0.1/localhost/g" ~/.kube/k3s-${CLUSTER_NAME}.yaml 2>/dev/null || true
        kubectl config view --flatten > ~/.kube/config.tmp 2>/dev/null || true
        if [ -f ~/.kube/config.tmp ]; then
            KUBECONFIG=~/.kube/k3s-${CLUSTER_NAME}.yaml:~/.kube/config.tmp kubectl config view --flatten > ~/.kube/config.new 2>/dev/null || true
            mv ~/.kube/config.new ~/.kube/config 2>/dev/null || true
            rm ~/.kube/config.tmp 2>/dev/null || true
        fi
        kubectl config use-context default 2>/dev/null || kubectl config set-context default --cluster=default --user=default 2>/dev/null || true
        log_info "✅ K3s cluster is ready!"
        exit 0
    fi
fi

# Create k3s container
log_info "Creating K3s container..."
docker run -d \
    --name ${CONTAINER_NAME} \
    --privileged \
    --restart=unless-stopped \
    -p ${K3S_PORT}:6443 \
    -v ${CONTAINER_NAME}:/var/lib/rancher/k3s \
    -v ${CONTAINER_NAME}-config:/etc/rancher/k3s \
    --network k3s-net \
    rancher/k3s:latest \
    server \
    --disable traefik \
    --write-kubeconfig-mode 644 \
    --tls-san localhost \
    --private-registry "http://${REGISTRY_NAME}:5000"

# Wait for k3s to be ready
log_info "Waiting for K3s to be ready..."
for i in {1..60}; do
    if docker exec ${CONTAINER_NAME} kubectl get nodes >/dev/null 2>&1; then
        log_info "K3s is ready!"
        break
    fi
    if [ $i -eq 60 ]; then
        log_error "K3s failed to start after 120 seconds"
        exit 1
    fi
    sleep 2
done

# Get kubeconfig
log_info "Retrieving kubeconfig..."
mkdir -p ~/.kube
docker cp ${CONTAINER_NAME}:/etc/rancher/k3s/k3s.yaml ~/.kube/k3s-${CLUSTER_NAME}.yaml 2>/dev/null || {
    log_error "Failed to retrieve kubeconfig"
    exit 1
}

# Update kubeconfig to use localhost and set context name
sed -i.bak "s/127.0.0.1/localhost/g" ~/.kube/k3s-${CLUSTER_NAME}.yaml 2>/dev/null || \
sed -i '' "s/127.0.0.1/localhost/g" ~/.kube/k3s-${CLUSTER_NAME}.yaml 2>/dev/null || true

# Merge kubeconfig into main config
if [ -f ~/.kube/config ]; then
    KUBECONFIG=~/.kube/k3s-${CLUSTER_NAME}.yaml:~/.kube/config kubectl config view --flatten > ~/.kube/config.new 2>/dev/null || true
    mv ~/.kube/config.new ~/.kube/config 2>/dev/null || true
else
    cp ~/.kube/k3s-${CLUSTER_NAME}.yaml ~/.kube/config
fi

# Rename context
kubectl config rename-context default k3s-${CLUSTER_NAME} 2>/dev/null || true

log_info "✅ K3s cluster '${CLUSTER_NAME}' created successfully!"
log_info "📦 Local registry: ${REGISTRY_NAME} (localhost:${REGISTRY_PORT})"
log_info "🚀 Kubeconfig merged into: ~/.kube/config"
log_info "📋 Context name: k3s-${CLUSTER_NAME}"
log_info ""
log_info "To use this cluster:"
log_info "  kubectl config use-context k3s-${CLUSTER_NAME}"
log_info "  kubectl get nodes"

