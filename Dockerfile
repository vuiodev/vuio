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

# Install runtime dependencies including shadow for usermod/groupmod
RUN apk add --no-cache ca-certificates shadow su-exec

# Create a non-root user and group for security with default IDs
RUN addgroup -g 1000 vuio && adduser -u 1000 -G vuio -s /bin/sh -D vuio

# Create directories for the application, config, and media
RUN mkdir -p /app /config /media

# Copy the binary from the builder stage
COPY --from=builder /tmp/vuio /app/vuio

# Make sure the binary is executable
RUN chmod +x /app/vuio

# Set default environment variables for user/group IDs
ENV PUID=1000
ENV PGID=1000

# Set default environment variables for configuration
ENV VUIO_PORT=8080
ENV VUIO_MEDIA_DIR=/media
ENV VUIO_BIND_INTERFACE=0.0.0.0
# Can be "Auto", "All", or a specific interface name e.g., "eth0"
ENV VUIO_SSDP_INTERFACE=Auto
ENV VUIO_SERVER_NAME=VuIO

# Copy the enhanced entrypoint script
COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# Expose ports for HTTP server (TCP) and SSDP (UDP)
EXPOSE 8080/tcp
EXPOSE 1900/udp

# Use root for entrypoint to allow user switching
USER root
WORKDIR /app

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
# Default command to run the application
CMD ["/app/vuio", "--config", "/config/config.toml"]