# Production Dockerfile for Secret Manager Controller
# Multi-stage build that compiles the Rust binary inside Docker
# This is used for production builds with docker buildx

# Stage 1: Build the Rust binary
ARG BUILDPLATFORM=linux/amd64
FROM --platform=$BUILDPLATFORM rust:1.82-bookworm AS builder

# Build arguments for build.rs
ARG BUILD_GIT_HASH=unknown
ARG BUILD_TIMESTAMP
ARG BUILD_DATETIME

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Configure git to fetch full history for git dependencies
# This ensures Cargo can access branches and commits from git dependencies
# Required when using git dependencies with branches or specific commits
RUN git config --global --add safe.directory '*' && \
    git config --global init.defaultBranch main

WORKDIR /build

# Copy Cargo files first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Copy build script (needed for build-time git hash)
COPY build.rs ./

# Copy source code
COPY src ./src
COPY config ./config

# Build the binary in release mode
# Pass build-time environment variables to build.rs
# Use --locked to ensure Cargo.lock is respected and transitive dependencies resolve correctly
# CARGO_NET_GIT_FETCH_WITH_CLI=true forces Cargo to use git CLI instead of libgit2
# This ensures full git clones and better compatibility with git dependencies
RUN BUILD_GIT_HASH=${BUILD_GIT_HASH} \
    BUILD_TIMESTAMP=${BUILD_TIMESTAMP} \
    BUILD_DATETIME=${BUILD_DATETIME} \
    CARGO_NET_GIT_FETCH_WITH_CLI=true \
    cargo build --release --locked --bin secret-manager-controller

# Stage 2: Runtime image
FROM debian:bookworm-slim

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

