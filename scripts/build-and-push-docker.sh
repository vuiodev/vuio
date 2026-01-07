#!/usr/bin/env bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Binary name from Cargo.toml
BINARY_NAME="vuio"

# Function to print colored messages
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to show usage
usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Build and push multi-architecture Docker images to GitHub Container Registry.

OPTIONS:
    -t, --tag TAG           Version tag (e.g., v1.0.0) [required]
    -u, --user USER         GitHub username/organization [default: auto-detected from git]
    -r, --registry URL      Container registry URL [default: ghcr.io]
    -p, --platforms ARCH    Platforms to build [default: linux/amd64,linux/arm64]
    --no-cache             Build without cache
    --no-latest            Don't tag as 'latest'
    -h, --help             Show this help message

EXAMPLES:
    # Build and push version v1.0.0
    $0 --tag v1.0.0

    # Build for custom user and platforms
    $0 --tag v1.2.3 --user myuser --platforms linux/amd64,linux/arm64,linux/arm/v7

    # Build without cache
    $0 --tag v2.0.0 --no-cache

REQUIREMENTS:
    - Docker with buildx support
    - Logged in to GitHub Container Registry (ghcr.io)
      Run: echo \$GITHUB_TOKEN | docker login ghcr.io -u USERNAME --password-stdin

EOF
    exit 1
}

# Default values
TAG=""
REGISTRY="ghcr.io"
PLATFORMS="linux/amd64,linux/arm64"
NO_CACHE=""
TAG_LATEST=true

# Auto-detect GitHub username from git remote
GITHUB_USER=$(git config --get remote.origin.url | sed -n 's#.*github.com[:/]\([^/]*\)/.*#\1#p' | tr '[:upper:]' '[:lower:]')
if [ -z "$GITHUB_USER" ]; then
    log_warn "Could not auto-detect GitHub username from git remote"
    GITHUB_USER=""
fi

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--tag)
            TAG="$2"
            shift 2
            ;;
        -u|--user)
            GITHUB_USER="$2"
            shift 2
            ;;
        -r|--registry)
            REGISTRY="$2"
            shift 2
            ;;
        -p|--platforms)
            PLATFORMS="$2"
            shift 2
            ;;
        --no-cache)
            NO_CACHE="--no-cache"
            shift
            ;;
        --no-latest)
            TAG_LATEST=false
            shift
            ;;
        -h|--help)
            usage
            ;;
        *)
            log_error "Unknown option: $1"
            usage
            ;;
    esac
done

# Validate required arguments
if [ -z "$TAG" ]; then
    log_error "Tag is required. Use --tag or -t to specify a version."
    usage
fi

if [ -z "$GITHUB_USER" ]; then
    log_error "GitHub username could not be detected. Use --user or -u to specify."
    usage
fi

# Remove 'v' prefix if present for version number
VERSION="${TAG#v}"

# Construct image name
IMAGE_NAME="${REGISTRY}/${GITHUB_USER}/${BINARY_NAME}"

log_info "=========================================="
log_info "Docker Multi-Arch Build & Push"
log_info "=========================================="
log_info "Project:     ${BINARY_NAME}"
log_info "Tag:         ${TAG}"
log_info "Version:     ${VERSION}"
log_info "Registry:    ${REGISTRY}"
log_info "User:        ${GITHUB_USER}"
log_info "Image:       ${IMAGE_NAME}"
log_info "Platforms:   ${PLATFORMS}"
log_info "Tag latest:  ${TAG_LATEST}"
log_info "=========================================="

# Change to project root
cd "$PROJECT_ROOT"

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    log_error "Docker is not installed or not in PATH"
    exit 1
fi

# Check if buildx is available
if ! docker buildx version &> /dev/null; then
    log_error "Docker buildx is not available. Please install or enable it."
    exit 1
fi

# Check if Dockerfile exists
if [ ! -f "Dockerfile" ]; then
    log_error "Dockerfile not found in project root: $PROJECT_ROOT"
    exit 1
fi

# Create or use existing buildx builder
log_info "Setting up Docker Buildx..."
BUILDER_NAME="vuio-multiarch-builder"

if docker buildx inspect "$BUILDER_NAME" &> /dev/null; then
    log_info "Using existing builder: $BUILDER_NAME"
else
    log_info "Creating new builder: $BUILDER_NAME"
    docker buildx create --name "$BUILDER_NAME" --driver docker-container --use
fi

docker buildx use "$BUILDER_NAME"
docker buildx inspect --bootstrap

# Build tag list
TAGS=()
TAGS+=("-t" "${IMAGE_NAME}:${VERSION}")
TAGS+=("-t" "${IMAGE_NAME}:${TAG}")

# Add semantic version tags
if [[ "$VERSION" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
    MAJOR="${BASH_REMATCH[1]}"
    MINOR="${BASH_REMATCH[2]}"
    TAGS+=("-t" "${IMAGE_NAME}:${MAJOR}.${MINOR}")
    TAGS+=("-t" "${IMAGE_NAME}:${MAJOR}")
fi

# Add latest tag
if [ "$TAG_LATEST" = true ]; then
    TAGS+=("-t" "${IMAGE_NAME}:latest")
fi

log_info "Building and pushing with tags:"
for tag_arg in "${TAGS[@]}"; do
    if [ "$tag_arg" = "-t" ]; then
        continue
    fi
    log_info "  - $tag_arg"
done

# Build and push
log_info "Building multi-architecture Docker image..."
log_info "This may take several minutes..."

docker buildx build \
    --platform "$PLATFORMS" \
    --push \
    "${TAGS[@]}" \
    --build-arg BUILDKIT_INLINE_CACHE=1 \
    --provenance=false \
    $NO_CACHE \
    -f Dockerfile \
    .

if [ $? -eq 0 ]; then
    log_info "=========================================="
    log_info "Successfully built and pushed Docker images!"
    log_info "=========================================="
    log_info "You can pull the image with:"
    log_info "  docker pull ${IMAGE_NAME}:${VERSION}"
    if [ "$TAG_LATEST" = true ]; then
        log_info "  docker pull ${IMAGE_NAME}:latest"
    fi
    log_info ""
    log_info "View on GitHub:"
    log_info "  https://github.com/${GITHUB_USER}/${BINARY_NAME}/pkgs/container/${BINARY_NAME}"
else
    log_error "Docker build failed!"
    exit 1
fi
