#!/bin/sh

# FreeBSD package builder for VuIO
# Creates a .txz package using pkg-create

set -e

# Configuration
BINARY_PATH="${1}"
OUTPUT_DIR="${2:-../builds}"
VERSION="${3:-0.1.0}"
ARCH="${4:-amd64}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Package information
PACKAGE_NAME="vuio"
PACKAGE_MAINTAINER="VuIO Team <support@vuio.dev>"
PACKAGE_DESCRIPTION="VuIO DLNA Media Server - A modern, efficient DLNA/UPnP media server"
PACKAGE_WEBSITE="https://github.com/vuio/vuio"
PACKAGE_LICENSE="MIT"

# Validate inputs
if [ -z "$BINARY_PATH" ] || [ ! -f "$BINARY_PATH" ]; then
    echo -e "${RED}Error: Binary path not provided or file does not exist: $BINARY_PATH${NC}"
    exit 1
fi

if [ -z "$OUTPUT_DIR" ]; then
    echo -e "${RED}Error: Output directory not provided${NC}"
    exit 1
fi

echo -e "${CYAN}--- FreeBSD Package Builder ---${NC}"
echo "Binary: $BINARY_PATH"
echo "Output: $OUTPUT_DIR"
echo "Version: $VERSION"
echo "Architecture: $ARCH"
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Create temporary directory for package contents
TEMP_DIR=$(mktemp -d)
PKG_ROOT="$TEMP_DIR/pkg_root"
mkdir -p "$PKG_ROOT"

# Set up directory structure
mkdir -p "$PKG_ROOT/usr/local/bin"
mkdir -p "$PKG_ROOT/usr/local/etc/vuio"
mkdir -p "$PKG_ROOT/usr/local/share/vuio"
mkdir -p "$PKG_ROOT/usr/local/etc/rc.d"

echo -e "${YELLOW}Setting up package structure...${NC}"

# Copy binary
cp "$BINARY_PATH" "$PKG_ROOT/usr/local/bin/vuio"
chmod 755 "$PKG_ROOT/usr/local/bin/vuio"

# Create default configuration file
cat > "$PKG_ROOT/usr/local/etc/vuio/config.toml" << 'EOF'
# VuIO Configuration File
# This is the default configuration for VuIO DLNA Media Server

[server]
name = "VuIO Media Server"
port = 8080
host = "0.0.0.0"

[media]
library_paths = ["/usr/home/media", "/mnt/media"]
scan_interval = 3600
watch_for_changes = true

[network]
interface = "auto"
multicast_address = "239.255.255.250"

[database]
path = "/var/db/vuio/media.db"

[logging]
level = "info"
file = "/var/log/vuio/vuio.log"
EOF

# Create rc.d service script
cat > "$PKG_ROOT/usr/local/etc/rc.d/vuio" << 'EOF'
#!/bin/sh

# PROVIDE: vuio
# REQUIRE: DAEMON NETWORKING
# KEYWORD: shutdown

. /etc/rc.subr

name="vuio"
rcvar="vuio_enable"

command="/usr/local/bin/vuio"
command_args="--config /usr/local/etc/vuio/config.toml"
pidfile="/var/run/vuio.pid"

vuio_user="vuio"
vuio_group="vuio"

start_precmd="vuio_prestart"

vuio_prestart()
{
    # Create user and group if they don't exist
    if ! pw groupshow "$vuio_group" >/dev/null 2>&1; then
        pw groupadd "$vuio_group"
    fi
    
    if ! pw usershow "$vuio_user" >/dev/null 2>&1; then
        pw useradd "$vuio_user" -g "$vuio_group" -h - -s /usr/sbin/nologin -c "VuIO Media Server"
    fi
    
    # Create necessary directories
    install -d -o "$vuio_user" -g "$vuio_group" /var/db/vuio
    install -d -o "$vuio_user" -g "$vuio_group" /var/log/vuio
    
    # Set permissions on config file
    chown "$vuio_user:$vuio_group" /usr/local/etc/vuio/config.toml
    chmod 640 /usr/local/etc/vuio/config.toml
}

load_rc_config $name
run_rc_command "$1"
EOF

chmod 755 "$PKG_ROOT/usr/local/etc/rc.d/vuio"

# Create package manifest
cat > "$TEMP_DIR/manifest" << EOF
name: $PACKAGE_NAME
version: $VERSION
origin: multimedia/vuio
comment: $PACKAGE_DESCRIPTION
desc: |
  VuIO is a modern, efficient DLNA/UPnP media server written in Rust.
  
  Features:
  - Fast media scanning and indexing
  - Real-time file system monitoring
  - Low memory footprint
  - Cross-platform compatibility
  - Modern web interface
  
  This package includes the VuIO server binary, default configuration,
  and FreeBSD service integration.

maintainer: $PACKAGE_MAINTAINER
www: $PACKAGE_WEBSITE
abi: FreeBSD:13:$ARCH
arch: $ARCH
prefix: /usr/local
licenselogic: single
licenses: [$PACKAGE_LICENSE]
deps: {}
categories: [multimedia]
EOF

# Create plist (file list)
echo -e "${YELLOW}Generating file list...${NC}"
(cd "$PKG_ROOT" && find . -type f -o -type l | sed 's,^\./,,' | sort) > "$TEMP_DIR/plist"

# Create the package
PACKAGE_FILE="$OUTPUT_DIR/${PACKAGE_NAME}-${VERSION}-freebsd-${ARCH}.txz"

echo -e "${YELLOW}Creating FreeBSD package...${NC}"

if command -v pkg >/dev/null 2>&1; then
    # Use pkg-create if available
    pkg create -r "$PKG_ROOT" -m "$TEMP_DIR/manifest" -p "$TEMP_DIR/plist" -o "$OUTPUT_DIR"
    
    # Rename to our desired format
    if [ -f "$OUTPUT_DIR/${PACKAGE_NAME}-${VERSION}.txz" ]; then
        mv "$OUTPUT_DIR/${PACKAGE_NAME}-${VERSION}.txz" "$PACKAGE_FILE"
    fi
else
    # Fallback: create tar.xz manually
    echo -e "${YELLOW}pkg tool not available, creating tar.xz package...${NC}"
    
    # Create package metadata
    mkdir -p "$PKG_ROOT/+COMPACT_MANIFEST"
    cp "$TEMP_DIR/manifest" "$PKG_ROOT/+COMPACT_MANIFEST/"
    cp "$TEMP_DIR/plist" "$PKG_ROOT/+COMPACT_MANIFEST/"
    
    # Create the archive
    (cd "$PKG_ROOT" && tar -cJf "$PACKAGE_FILE" .)
fi

# Clean up
rm -rf "$TEMP_DIR"

if [ -f "$PACKAGE_FILE" ]; then
    echo -e "${GREEN}✓ Package created successfully: $PACKAGE_FILE${NC}"
    echo -e "${CYAN}Package size: $(du -h "$PACKAGE_FILE" | cut -f1)${NC}"
    echo ""
    echo -e "${CYAN}Installation instructions:${NC}"
    echo "1. Copy package to FreeBSD system"
    echo "2. Install: sudo pkg add $PACKAGE_FILE"
    echo "3. Enable service: sudo sysrc vuio_enable=YES"
    echo "4. Configure: edit /usr/local/etc/vuio/config.toml"
    echo "5. Start service: sudo service vuio start"
    echo ""
    echo -e "${CYAN}Package contents:${NC}"
    echo "- Binary: /usr/local/bin/vuio"
    echo "- Config: /usr/local/etc/vuio/config.toml"
    echo "- Service: /usr/local/etc/rc.d/vuio"
    echo "- Database: /var/db/vuio/ (created on first run)"
    echo "- Logs: /var/log/vuio/ (created on first run)"
else
    echo -e "${RED}✗ Package creation failed${NC}"
    exit 1
fi
