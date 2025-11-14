# Production Dockerfile for Secret Manager Controller
# Multi-stage build that compiles the Rust binary inside Docker
# This is used for production builds with docker buildx

# Stage 1: Build the Rust binary
ARG BUILDPLATFORM=linux/amd64
FROM --platform=$BUILDPLATFORM rust:1.75-alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    musl-gcc \
    git \
    curl \
    openssl-dev \
    openssl-libs-static \
    pkgconfig

# Install cargo-zigbuild for cross-compilation
RUN cargo install cargo-zigbuild

# Install musl target
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /build

# Copy Cargo files first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY config ./config

# Build the binary in release mode
# Use zigbuild for cross-compilation (works on any platform)
RUN cargo zigbuild --release --target x86_64-unknown-linux-musl

# Stage 2: Runtime image
ARG TARGETPLATFORM=linux/amd64
FROM --platform=${TARGETPLATFORM} alpine:3.19

# Install runtime dependencies
# git: Required for ArgoCD support (cloning repositories)
# kustomize: Required for Kustomize Build Mode
RUN apk add --no-cache \
    ca-certificates \
    libgcc \
    git \
    curl && \
    # Install kustomize
    curl -s "https://raw.githubusercontent.com/kubernetes-sigs/kustomize/master/hack/install_kustomize.sh" | bash && \
    mv kustomize /usr/local/bin/ && \
    chmod +x /usr/local/bin/kustomize

WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/secret-manager-controller /app/secret-manager-controller
RUN chmod +x /app/secret-manager-controller

# Expose metrics port
EXPOSE 8080

# Set runtime environment
ENV RUST_BACKTRACE=1
ENV RUST_LOG=info

# Run the controller
ENTRYPOINT ["/app/secret-manager-controller"]

