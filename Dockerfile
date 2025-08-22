# ---- Builder Stage ----
# Use an official Rust image with Alpine for smaller static builds.
FROM rust:1-alpine AS builder

# Install build dependencies for static linking.
RUN apk add --no-cache musl-dev perl build-base

WORKDIR /app

# Declare TARGETARCH for this stage
ARG TARGETARCH

# Copy all source files
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build the application directly
RUN if [ "$TARGETARCH" = "amd64" ]; then \
        export RUST_TARGET="x86_64-unknown-linux-musl"; \
    elif [ "$TARGETARCH" = "arm64" ]; then \
        export RUST_TARGET="aarch64-unknown-linux-musl"; \
    else \
        echo "Unsupported architecture: $TARGETARCH" && exit 1; \
    fi && \
    rustup target add $RUST_TARGET && \
    cargo build --release --target $RUST_TARGET && \
    cp target/$RUST_TARGET/release/vuio /tmp/vuio

# ---- Final Stage ----
FROM alpine:latest

# Install runtime dependencies (ca-certificates is good practice).
RUN apk add --no-cache ca-certificates

# Create a non-root user and group for security.
RUN addgroup -S vuio && adduser -S vuio -G vuio

# Create directories for the application, config, and media.
# These should be mounted as volumes in production.
RUN mkdir -p /app /config /media && chown -R vuio:vuio /app /config /media

# Copy the binary from the builder stage
COPY --from=builder --chown=vuio:vuio /tmp/vuio /app/vuio

# Make sure the binary is executable
RUN chmod +x /app/vuio

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