# Dockerfile for Axum Pact Mock Server (Rust/Axum)
# Binaries are cross-compiled on host and copied in (matches controller pattern)
# Located at pact-broker level for separation of concerns from controller

FROM debian:bookworm-slim

# Create non-root user
RUN groupadd -g 1001 pact && \
    useradd -u 1001 -g 1001 -m -s /bin/bash pact

# Install runtime dependencies (minimal - Rust binary is statically linked)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app directory with proper permissions for live updates
WORKDIR /app

# Copy pre-built binaries from build_artifacts (x86_64 Linux musl target)
# Note: Binaries must exist before Docker build (ensured by Tilt resource_deps)
# Build context is root, so paths are relative to that
COPY ./build_artifacts/mock-server/gcp-mock-server /app/gcp-mock-server
COPY ./build_artifacts/mock-server/aws-mock-server /app/aws-mock-server
COPY ./build_artifacts/mock-server/azure-mock-server /app/azure-mock-server

# Verify files exist and set permissions (fail fast if files are missing)
RUN test -f /app/gcp-mock-server && \
    test -f /app/aws-mock-server && \
    test -f /app/azure-mock-server && \
    chmod +x /app/gcp-mock-server /app/aws-mock-server /app/azure-mock-server

# Create directories with write permissions for live updates
RUN chmod -R 777 /app

# Switch to non-root user
USER pact

# Expose port
EXPOSE 1234

# Default command (can be overridden in deployment)
# Each deployment will specify which binary to run
CMD ["/app/gcp-mock-server"]

