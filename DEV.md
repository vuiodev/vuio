# Development Guide

## Docker Build and Publish

The project includes a script for building and publishing multi-architecture Docker images to GitHub Container Registry.

### Script Location

`scripts/build-and-push-docker.sh`
`./scripts/build-and-push-docker.sh --tag v0.0.22 --platforms linux/amd64,linux/arm64`

### Features

- **Multi-architecture support**: Builds for `linux/amd64` and `linux/arm64` by default
- **Automatic tagging**: Creates semantic version tags (e.g., `v1.2.3`, `1.2.3`, `1.2`, `1`, `latest`)
- **GitHub Container Registry**: Publishes to `ghcr.io` with auto-detection of your GitHub username
- **Build optimizations**: Supports cache control and custom platforms
- **User-friendly**: Color-coded output, progress messages, and comprehensive error handling

### Platform Support

The Docker build uses `rust:1-alpine` as the base image, which limits the supported platforms:

- ✅ `linux/amd64` - Supported
- ✅ `linux/arm64` - Supported

### Prerequisites

Before running the script, you need to:

1. **Log in to GitHub Container Registry**:
```bash
echo $GITHUB_TOKEN | docker login ghcr.io -u YOUR_USERNAME --password-stdin
```

2. **Ensure Docker Buildx is available** (usually comes with Docker Desktop or Docker 19.03+)

### Usage

```bash
# Basic usage - build and push a version
./scripts/build-and-push-docker.sh --tag v1.0.0

# Build for single platform only (faster for testing)
./scripts/build-and-push-docker.sh --tag v1.2.3 --platforms linux/amd64

# Build without cache
./scripts/build-and-push-docker.sh --tag v2.0.0 --no-cache

# Skip 'latest' tag
./scripts/build-and-push-docker.sh --tag v1.0.0-beta --no-latest

# Custom GitHub user/org
./scripts/build-and-push-docker.sh --tag v1.0.0 --user myorganization

# View help
./scripts/build-and-push-docker.sh --help
```

### Script Options

| Option | Description | Default |
|--------|-------------|---------|
| `-t, --tag TAG` | Version tag (e.g., v1.0.0) | **Required** |
| `-u, --user USER` | GitHub username/organization | Auto-detected from git |
| `-r, --registry URL` | Container registry URL | `ghcr.io` |
| `-p, --platforms ARCH` | Platforms to build | `linux/amd64,linux/arm64` |
| `--no-cache` | Build without cache | Uses cache |
| `--no-latest` | Don't tag as 'latest' | Tags as latest |
| `-h, --help` | Show help message | - |

### What the Script Does

1. **Auto-detects** GitHub username from git remote origin
2. **Validates** required arguments and checks for Docker/buildx
3. **Creates or reuses** a buildx builder instance for multi-arch builds
4. **Builds** the Docker image for specified platforms using the Alpine-based Dockerfile
5. **Tags** the image with semantic versioning (major, minor, patch, latest)
6. **Pushes** all tags to GitHub Container Registry

### Example Output

```
==========================================
Docker Multi-Arch Build & Push
==========================================
Project:     vuio
Tag:         v1.0.0
Version:     1.0.0
Registry:    ghcr.io
User:        yourusername
Image:       ghcr.io/yourusername/vuio
Platforms:   linux/amd64,linux/arm64
Tag latest:  true
==========================================
```

### Pulling Published Images

After successful publication, you can pull images with:

```bash
# Specific version
docker pull ghcr.io/yourusername/vuio:1.0.0

# Latest version
docker pull ghcr.io/yourusername/vuio:latest

# Major version
docker pull ghcr.io/yourusername/vuio:1
```

### Troubleshooting

**Authentication errors**: Ensure you're logged in to ghcr.io with a valid GitHub token that has `write:packages` permission

**Buildx not found**: Update Docker to version 19.03+ or install buildx plugin

**Platform not supported**: Check that QEMU is set up for cross-platform builds:
```bash
docker run --privileged --rm tonistiigi/binfmt --install all
```
