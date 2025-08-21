FROM scratch as binaries

COPY dist/amd64/dlnasrv /amd64/dlnasrv
COPY dist/arm64/dlnasrv /arm64/dlnasrv

FROM alpine:latest

# TARGETARCH is a build argument automatically provided by Docker Buildx.
# It will be 'amd64' or 'arm64' depending on the platform being built.
ARG TARGETARCH

# Install runtime dependencies. ca-certificates is good practice.
RUN apk add --no-cache ca-certificates

# Create a non-root user and group for security
RUN addgroup -S appgroup && adduser -S appuser -G appgroup

# Set the working directory
WORKDIR /app

# Copy the correct pre-compiled binary from the 'binaries' stage
# into the final image, based on the target architecture.
COPY --from=binaries /${TARGETARCH}/dlnasrv /app/dlnasrv

# Set ownership for the application binary and directory
RUN chown -R appuser:appgroup /app

# Create a directory for media files and set ownership
# This directory can be mounted as a volume from the host.
RUN mkdir /media && chown appuser:appgroup /media

# Switch to the non-root user for added security
USER appuser

# Expose the web server port (TCP) and the SSDP discovery port (UDP)
EXPOSE 8080/tcp
EXPOSE 1900/udp

# Set the entrypoint for the container.
# The CMD specifies the default arguments, which can be overridden.
ENTRYPOINT ["/app/dlnasrv"]
CMD ["--media-dir", "/media", "--port", "8080"]