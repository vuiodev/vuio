#!/bin/bash

# Build Arch Linux package for VuIO
# Creates a proper Arch package (.pkg.tar.zst) with PKGBUILD

set -e

# Configuration
BINARY_PATH="${1:-../../target/x86_64-unknown-linux-gnu/release/vuio}"
OUTPUT_DIR="${2:-../../builds}"
VERSION="${3:-0.1.0}"
ARCHITECTURE="${4:-x86_64}"
PACKAGE_NAME="vuio"
MAINTAINER="VuIO <vuio@vuio.dev>"
DESCRIPTION="Cross-platform DLNA media server"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

function show_help() {
    echo -e "${GREEN}--- Arch Linux Package Build Script ---${NC}"
    echo ""
    echo "Usage: $0 [BINARY_PATH] [OUTPUT_DIR] [VERSION] [ARCHITECTURE]"
    echo ""
    echo "Arguments:"
    echo "  BINARY_PATH   Path to the compiled vuio binary (default: ../../target/x86_64-unknown-linux-gnu/release/vuio)"
    echo "  OUTPUT_DIR    Output directory for package file (default: ../../builds)"
    echo "  VERSION       Version number for the package (default: 0.1.0)"
    echo "  ARCHITECTURE  Target architecture (default: x86_64)"
    echo ""
    echo "Prerequisites:"
    echo "  - makepkg utility (from pacman-contrib or base-devel)"
    echo ""
}

if [[ "$1" == "--help" || "$1" == "-h" ]]; then
    show_help
    exit 0
fi

if [[ ! -f "$BINARY_PATH" ]]; then
    echo -e "${RED}✗ Binary not found at: $BINARY_PATH${NC}"
    echo -e "${YELLOW}Please build the project first or specify correct path${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Binary found at: $BINARY_PATH${NC}"

# Create build environment
echo ""
echo -e "${YELLOW}--- Preparing Build Environment ---${NC}"

TEMP_DIR="temp_arch"
PKG_DIR="$TEMP_DIR/vuio-arch"

# Clean and create package directory structure
if [[ -d "$TEMP_DIR" ]]; then
    rm -rf "$TEMP_DIR"
fi

mkdir -p "$PKG_DIR"

# Copy binary to build folder
cp "$BINARY_PATH" "$PKG_DIR/vuio"
chmod +x "$PKG_DIR/vuio"

# Create default configuration
cat > "$PKG_DIR/vuio.toml" << 'EOF'
# VuIO Server Configuration
# This is the default configuration file for VuIO

[server]
port = 8080
interface = "0.0.0.0"
name = "Vuio"

[network]
ssdp_port = 1900
interface_selection = "auto"
multicast_ttl = 4
announce_interval_seconds = 30

[media]
scan_on_startup = true
watch_for_changes = true
supported_extensions = ["mp4", "mkv", "avi", "mp3", "flac", "wav", "jpg", "png", "gif"]

[[media.directories]]
path = "/home/media/Videos"
recursive = true

[[media.directories]]
path = "/home/media/Music"
recursive = true

[[media.directories]]
path = "/home/media/Pictures"
recursive = true

[database]
vacuum_on_startup = false
backup_enabled = true
EOF

# Create systemd service file
cat > "$PKG_DIR/vuio.service" << 'EOF'
[Unit]
Description=VuIO Media Server
Documentation=https://github.com/vuio/vuio
After=network.target
Wants=network.target

[Service]
Type=simple
User=vuio
Group=vuio
ExecStart=/usr/bin/vuio --config /etc/vuio/vuio.toml
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=vuio

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/vuio /var/lib/vuio
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true

# Network settings
IPAddressDeny=any
IPAddressAllow=localhost
IPAddressAllow=link-local
IPAddressAllow=multicast

[Install]
WantedBy=multi-user.target
EOF

# Create vuio.install script
cat > "$PKG_DIR/vuio.install" << 'EOF'
post_install() {
    # Create group if not exists
    if ! getent group vuio >/dev/null; then
        echo "Creating vuio group..."
        groupadd --system vuio
    fi

    # Create user if not exists
    if ! getent passwd vuio >/dev/null; then
        echo "Creating vuio user..."
        useradd --system --gid vuio --home-dir /var/lib/vuio \
                --shell /usr/bin/nologin --comment "VuIO service user" vuio
    fi

    # Create directories and set permissions
    mkdir -p /var/lib/vuio
    mkdir -p /var/log/vuio

    chown vuio:vuio /var/lib/vuio
    chown vuio:vuio /var/log/vuio
    chmod 755 /var/lib/vuio
    chmod 755 /var/log/vuio

    # Set configuration file permissions
    chown root:vuio /etc/vuio/vuio.toml
    chmod 640 /etc/vuio/vuio.toml
}

post_upgrade() {
    post_install
}

post_remove() {
    # Arch packaging policy recommends keeping system user/group,
    # but we will print a cleanup message for convenience
    echo "VuIO package removed. The 'vuio' system user/group and /var/lib/vuio files were kept."
    echo "To completely clean up, run:"
    echo "  userdel vuio"
    echo "  groupdel vuio"
    echo "  rm -rf /var/lib/vuio /var/log/vuio /etc/vuio"
}
EOF

# Create PKGBUILD file
cat > "$PKG_DIR/PKGBUILD" << EOF
# Maintainer: $MAINTAINER
pkgname=vuio-bin
pkgver=$VERSION
pkgrel=1
pkgdesc="$DESCRIPTION"
arch=('$ARCHITECTURE')
url="https://github.com/vuio/vuio"
license=('MIT')
depends=('glibc')
provides=('vuio')
conflicts=('vuio')
install=vuio.install
backup=('etc/vuio/vuio.toml')

package() {
    # Install binary
    install -Dm755 "\${srcdir}/vuio" "\${pkgdir}/usr/bin/vuio"

    # Install configuration
    install -Dm640 "\${srcdir}/vuio.toml" "\${pkgdir}/etc/vuio/vuio.toml"

    # Install systemd service
    install -Dm644 "\${srcdir}/vuio.service" "\${pkgdir}/usr/lib/systemd/system/vuio.service"
}
EOF

echo -e "${GREEN}✓ Build environment prepared${NC}"

# Build the package
echo ""
echo -e "${YELLOW}--- Building Arch Linux Package ---${NC}"

mkdir -p "$OUTPUT_DIR"
FINAL_OUTPUT_DIR=$(cd "$OUTPUT_DIR" && pwd)

cd "$PKG_DIR"

if command -v makepkg &> /dev/null; then
    echo "Running makepkg..."
    # Build package using makepkg
    makepkg -f -p PKGBUILD --noconfirm
    
    # Find built package file
    PKG_FILE=$(find . -name "*.pkg.tar.zst" -type f)
    if [[ -n "$PKG_FILE" ]]; then
        cp "$PKG_FILE" "$FINAL_OUTPUT_DIR/"
        echo -e "${GREEN}✓ Arch package created successfully: $FINAL_OUTPUT_DIR/\$(basename "\$PKG_FILE")${NC}"
    else
        echo -e "${RED}✗ Package file not found after makepkg${NC}"
        exit 1
    fi
else
    echo -e "${YELLOW}! makepkg not found. Preparing the build directory only.${NC}"
    echo "Arch packaging directory prepared at: $PWD"
    echo "To build on an Arch Linux machine, run:"
    echo "  cd $PWD"
    echo "  makepkg -f"
fi

cd - > /dev/null

# Clean up
if command -v makepkg &> /dev/null; then
    rm -rf "$TEMP_DIR"
fi

echo ""
echo -e "${GREEN}--- Arch Build Complete ---${NC}"
