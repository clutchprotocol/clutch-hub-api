# Multi-stage Dockerfile for Clutch Hub API
# Build arguments for flexibility  
ARG RUST_VERSION=1.86

#==============================================================================
# Builder Stage - Build the Rust application
#==============================================================================
FROM rust:${RUST_VERSION}-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app user for security
RUN groupadd -g 1000 clutch && \
    useradd -r -u 1000 -g clutch -s /bin/sh clutch

WORKDIR /usr/src/clutch-hub-api

# Copy dependency files for better caching
COPY Cargo.toml Cargo.lock ./

# Create dummy source and build dependencies for better caching
# Then delete the dummy binary artifacts so the real build creates fresh binary
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -f target/release/clutch-hub-api target/release/clutch-hub-api.d && \
    rm -rf target/release/deps/clutch_hub_api* target/release/.fingerprint/clutch-hub-api-* && \
    rm -rf src

# Copy actual source code
COPY src ./src
COPY config ./config

# Build the final binary in release mode
RUN cargo build --release --bin clutch-hub-api

# Verify the binary was created
RUN ls -la target/release/ && \
    file target/release/clutch-hub-api

#==============================================================================
# Runtime Stage - Minimal Debian image
#==============================================================================
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -g 1000 clutch && \
    useradd -r -u 1000 -g clutch -s /bin/sh clutch

# Create directories with proper permissions
RUN mkdir -p /usr/local/bin /app/config && \
    chown -R clutch:clutch /app

# Copy the binary from builder stage
COPY --from=builder /usr/src/clutch-hub-api/target/release/clutch-hub-api /usr/local/bin/clutch-hub-api
COPY --from=builder /usr/src/clutch-hub-api/config /app/config

# Set permissions and switch to non-root user
RUN chmod +x /usr/local/bin/clutch-hub-api
USER clutch

# Set working directory
WORKDIR /app

# Expose the API port
EXPOSE 3000

# Health check for container monitoring
HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://127.0.0.1:3000/health || exit 1

# Set the default command
CMD ["clutch-hub-api", "--env", "default"]
