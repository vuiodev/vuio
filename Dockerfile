# ---- Builder Stage ----
# Use an official Rust image with Alpine for smaller static builds.
FROM rust:1-alpine AS builder

# Install build dependencies for static linking.
RUN apk add --no-cache musl-dev

WORKDIR /app

# Set target platform based on build architecture. This allows for cross-compilation.
ARG TARGETARCH

# Cache dependencies by building a dummy project.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    TARGETPLATFORM=$(if [ "$TARGETARCH" = "amd64" ]; then echo "x86_64-unknown-linux-musl"; elif [ "$TARGETARCH" = "arm64" ]; then echo "aarch64-unknown-linux-musl"; else exit 1; fi) && \
    rustup target add $TARGETPLATFORM && \
    cargo build --release --target $TARGETPLATFORM

# Copy the actual source code and build the application.
# This will re-use the cached dependency layers from the previous step.
COPY src ./src
RUN TARGETPLATFORM=$(if [ "$TARGETARCH" = "amd64" ]; then echo "x86_64-unknown-linux-musl"; elif [ "$TARGETARCH" = "arm64" ]; then echo "aarch64-unknown-linux-musl"; else exit 1; fi) && \
    # Remove final crate artifact to ensure the crate source is recompiled
    rm -f target/$TARGETPLATFORM/release/deps/vuio* && \
    cargo build --release --target $TARGETPLATFORM

# ---- Final Stage ----
FROM alpine:latest

# Install runtime dependencies (ca-certificates is good practice).
RUN apk add --no-cache ca-certificates

# Create a non-root user and group for security.
RUN addgroup -S vuio && adduser -S vuio -G vuio

# Create directories for the application, config, and media.
# These should be mounted as volumes in production.
RUN mkdir -p /app /config /media && chown -R vuio:vuio /app /config /media

# Copy the binary from the builder stage based on target architecture.
ARG TARGETARCH
COPY --from=builder --chown=vuio:vuio /app/target/x86_64-unknown-linux-musl/release/vuio /app/vuio-amd64
COPY --from=builder --chown=vuio:vuio /app/target/aarch64-unknown-linux-musl/release/vuio /app/vuio-arm64

# Select the correct binary for the current architecture
RUN if [ "$TARGETARCH" = "amd64" ]; then mv /app/vuio-amd64 /app/vuio; \
    elif [ "$TARGETARCH" = "arm64" ]; then mv /app/vuio-arm64 /app/vuio; fi

# Switch to the non-root user.
USER vuio
WORKDIR /app

# Expose ports for HTTP server (TCP) and SSDP (UDP). Default values can be overridden by environment variables.
EXPOSE 8080/tcp
EXPOSE 1900/udp

# Set default environment variables for configuration.
ENV VUIO_PORT=8080
ENV VUIO_MEDIA_DIR=/media
ENV VUIO_BIND_INTERFACE="0.0.0.0"
# Can be "Auto", "All", or a specific interface name e.g., "eth0"
ENV VUIO_SSDP_INTERFACE="Auto" 
ENV VUIO_SERVER_NAME="VuIO"

# The entrypoint script will generate the config from environment variables on start.
COPY --chown=vuio:vuio docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]

# Default command to run the application.
CMD ["/app/vuio", "--config", "/config/config.toml"]