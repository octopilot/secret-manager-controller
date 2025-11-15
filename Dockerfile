# Production Dockerfile for Secret Manager Controller
# Multi-stage build that compiles the Rust binary inside Docker
# This is used for production builds with docker buildx

# Stage 1: Build the Rust binary
ARG BUILDPLATFORM=linux/amd64
FROM --platform=$BUILDPLATFORM rust:1.82-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy Cargo files first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY config ./config

# Build the binary in release mode
RUN cargo build --release --bin secret-manager-controller

# Stage 2: Runtime image
ARG TARGETPLATFORM=linux/amd64
FROM --platform=$TARGETPLATFORM debian:bookworm-slim

# Install runtime dependencies
# git: Required for ArgoCD support (cloning repositories)
# kustomize: Required for Kustomize Build Mode
RUN apt-get update && apt-get install -y \
    ca-certificates \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/* && \
    # Install kustomize
    KUSTOMIZE_VERSION=5.8.0 && \
    curl -L "https://github.com/kubernetes-sigs/kustomize/releases/download/kustomize%2Fv${KUSTOMIZE_VERSION}/kustomize_v${KUSTOMIZE_VERSION}_linux_amd64.tar.gz" | \
    tar -xz -C /usr/local/bin && \
    chmod +x /usr/local/bin/kustomize

WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /build/target/release/secret-manager-controller /app/secret-manager-controller
RUN chmod +x /app/secret-manager-controller

# Expose metrics port
EXPOSE 5000

# Set runtime environment
ENV RUST_BACKTRACE=1
ENV RUST_LOG=info

# Run the controller
ENTRYPOINT ["/app/secret-manager-controller"]

