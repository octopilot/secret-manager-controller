# Minimal runtime-only Dockerfile for Secret Manager Controller (Tilt development)
# Binary is cross-compiled on host (Apple Silicon -> x86_64 Linux) and copied in

ARG TARGETPLATFORM=linux/amd64
FROM --platform=${TARGETPLATFORM} alpine:3.19

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    libgcc

# Create app directory with proper permissions for live updates
WORKDIR /app

# Copy pre-built binary from staging directory (x86_64 Linux musl target)
# Note: Binary must exist before Docker build (ensured by Tilt resource_deps)
# Build context is the controller directory, so path is relative to that
COPY ./target/x86_64-unknown-linux-musl/debug/secret-manager-controller /app/secret-manager-controller
RUN chmod +x /app/secret-manager-controller

# Create directories with write permissions for live updates
RUN chmod -R 777 /app

# Expose metrics port
EXPOSE 8080

# Set runtime environment
ENV RUST_BACKTRACE=1
ENV RUST_LOG=info

# Run the controller
ENTRYPOINT ["/app/secret-manager-controller"]

