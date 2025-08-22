# Docker Deployment Guide for VuIO

This guide covers different ways to deploy VuIO using Docker, addressing common issues with DLNA/UPnP networking and file permissions.

## Quick Start

### 1. Basic Deployment (Host Networking)

The simplest way to run VuIO with proper multicast support:

```bash
# Create directories
mkdir -p ./vuio-config ./my-media

# Find your user/group IDs
echo "Your PUID: $(id -u)"
echo "Your PGID: $(id -g)"

# Update docker-compose.yml with your IDs, then:
docker-compose up -d
```

### 2. Check Container Status

```bash
# View logs
docker-compose logs -f vuio

# Check if container is running
docker-compose ps

# Access container shell for debugging
docker-compose exec vuio sh
```

## User/Group ID Mapping

### Why This Matters

Docker containers run with their own user namespace. Without proper ID mapping:
- Files created by the container may have wrong ownership
- You might get permission denied errors
- Media files might not be accessible

### Setting Up PUID/PGID

1. **Find your IDs:**
   ```bash
   id -u  # Your user ID (PUID)
   id -g  # Your group ID (PGID)
   ```

2. **Update docker-compose.yml:**
   ```yaml
   environment:
     - PUID=1000  # Replace with your user ID
     - PGID=1000  # Replace with your group ID
   ```

3. **For existing containers:**
   ```bash
   # Remove old container and recreate
   docker-compose down
   docker-compose up -d
   ```

## Network Configuration

### Option 1: Host Networking (Recommended)

**Best for:** Most home networks, easiest setup

```yaml
network_mode: "host"
```

**Pros:**
- Multicast/broadcast packets work properly
- SSDP discovery works out of the box
- No port mapping needed

**Cons:**
- Container uses host's network stack directly
- Less network isolation

### Option 2: Bridge Networking with Port Mapping

**Best for:** When host networking isn't available

```yaml
ports:
  - "8080:8080/tcp"
  - "1900:1900/udp"
```

**Note:** SSDP discovery may not work properly due to multicast limitations.

### Option 3: Macvlan Networking (Advanced)

**Best for:** Users who need container to appear as separate network device

Use `docker-compose.macvlan.yml` for this setup.

**Setup Steps:**

1. **Create macvlan network:**
   ```bash
   # Find your network interface
   ip route | grep default
   
   # Create macvlan network (adjust for your network)
   docker network create -d macvlan \
     --subnet=192.168.1.0/24 \
     --gateway=192.168.1.1 \
     --ip-range=192.168.1.240/28 \
     -o parent=eth0 \
     vuio-macvlan
   ```

2. **Update environment:**
   ```yaml
   - VUIO_SSDP_INTERFACE=eth0  # Match your parent interface
   ```

3. **Deploy:**
   ```bash
   docker-compose -f docker-compose.macvlan.yml up -d
   ```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PUID` | 1000 | User ID for file ownership |
| `PGID` | 1000 | Group ID for file ownership |
| `VUIO_PORT` | 8080 | HTTP server port |
| `VUIO_SERVER_NAME` | VuIO | DLNA server name |
| `VUIO_BIND_INTERFACE` | 0.0.0.0 | Server bind interface |
| `VUIO_SSDP_INTERFACE` | Auto | SSDP multicast interface |
| `VUIO_MEDIA_DIR` | /media | Media directory path |

### Volume Mapping

```yaml
volumes:
  - ./vuio-config:/config    # Configuration and database
  - ./my-media:/media        # Your media files
```

**Important:** Ensure the host directories exist and have proper permissions.

## Troubleshooting

### Permission Issues

**Symptoms:**
- "Permission denied" errors
- Wrong file ownership
- Can't write to config directory

**Solutions:**
1. Set correct PUID/PGID values
2. Check host directory permissions:
   ```bash
   ls -la ./vuio-config ./my-media
   sudo chown -R $(id -u):$(id -g) ./vuio-config ./my-media
   ```

### DLNA Discovery Not Working

**Symptoms:**
- VuIO starts but devices can't find it
- No SSDP announcements
- Multicast issues

**Solutions:**
1. Use host networking mode
2. Check firewall settings:
   ```bash
   # Allow DLNA ports
   sudo ufw allow 1900/udp
   sudo ufw allow 8080/tcp
   ```
3. Verify interface selection:
   ```bash
   # List available interfaces
   docker exec vuio-server ip addr show
   ```
4. Set specific interface:
   ```yaml
   - VUIO_SSDP_INTERFACE=eth0  # Use your actual interface
   ```

### Configuration File Issues

**Symptoms:**
- "Invalid TOML" errors
- Missing UUID field
- Configuration validation fails

**Solutions:**
1. Delete config file and restart:
   ```bash
   rm ./vuio-config/config.toml
   docker-compose restart vuio
   ```
2. Check container logs:
   ```bash
   docker-compose logs vuio | tail -20
   ```
3. Verify environment variables:
   ```bash
   docker-compose exec vuio env | grep VUIO
   ```

### Network Debugging

**Check container networking:**
```bash
# View network configuration
docker exec vuio-server ip addr show
docker exec vuio-server ip route show

# Test multicast capability
docker exec vuio-server ping -c 3 239.255.255.250

# Check listening ports
docker exec vuio-server netstat -ln
```

### Log Analysis

**View detailed logs:**
```bash
# Follow logs in real-time
docker-compose logs -f vuio

# Filter for specific issues
docker-compose logs vuio | grep -E "(ERROR|WARN|Failed)"

# Check initialization
docker-compose logs vuio | grep -E "(Generated|Configuration|UUID)"
```

## Common Deployment Patterns

### Home Server Setup

```yaml
version: '3.8'
services:
  vuio:
    image: ghcr.io/vuiodev/vuio:latest
    container_name: vuio-server
    restart: unless-stopped
    network_mode: "host"
    volumes:
      - /home/user/vuio-config:/config
      - /mnt/media:/media
    environment:
      - PUID=1000
      - PGID=1000
      - VUIO_SERVER_NAME=Home Media Server
      - VUIO_PORT=8080
```

### Multi-Network Setup

```yaml
version: '3.8'
services:
  vuio:
    image: ghcr.io/vuiodev/vuio:latest
    container_name: vuio-server
    restart: unless-stopped
    networks:
      - media-network
      - management
    environment:
      - VUIO_SSDP_INTERFACE=eth1  # Specific interface for media network
```

### Development Setup

```yaml
version: '3.8'
services:
  vuio:
    build: .  # Build from local Dockerfile
    container_name: vuio-dev
    volumes:
      - ./vuio-config:/config
      - ./test-media:/media
      - ./src:/app/src:ro  # Mount source for development
    environment:
      - VUIO_SERVER_NAME=VuIO Development
      - RUST_LOG=debug  # Enable debug logging
```

## Security Considerations

1. **User Privileges:** Container runs as non-root user by default
2. **Network Exposure:** Only necessary ports are exposed
3. **File Permissions:** Proper PUID/PGID mapping prevents privilege escalation
4. **Volume Mounts:** Use read-only mounts where possible

## Performance Tuning

### For Large Media Libraries

```yaml
environment:
  - VUIO_SCAN_ON_STARTUP=false  # Disable initial scan for faster startup
```