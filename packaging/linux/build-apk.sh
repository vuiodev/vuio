#!/bin/bash

# Build APK package for VuIO on Alpine Linux
# Creates a proper Alpine package (.apk)

set -e

BINARY_PATH="${1:-../../target/x86_64-unknown-linux-musl/release/vuio}"
OUTPUT_DIR="${2:-../../builds}"
VERSION="${3:-0.0.43}"
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
    echo -e "${GREEN}--- APK Package Build Script ---${NC}"
    echo "Usage: $0 [BINARY_PATH] [OUTPUT_DIR] [VERSION] [ARCHITECTURE]"
    echo ""
    echo "Arguments:"
    echo "  BINARY_PATH   Path to compiled vuio binary (default: ../../target/x86_64-unknown-linux-musl/release/vuio)"
    echo "  OUTPUT_DIR    Output directory for APK file (default: ../../builds)"
    echo "  VERSION       Version number for the package (default: 0.0.43)"
    echo "  ARCHITECTURE  Target architecture: x86_64 or aarch64 (default: x86_64)"
}

if [[ "$1" == "--help" || "$1" == "-h" ]]; then
    show_help
    exit 0
fi

if [[ ! -f "$BINARY_PATH" ]]; then
    echo -e "${RED}✗ Binary not found at: $BINARY_PATH${NC}"
    exit 1
fi

echo -e "${CYAN}Building Alpine APK package for $ARCHITECTURE (v$VERSION)...${NC}"

# Normalize version for Alpine (e.g., 0.0.43-r0)
APK_VER="${VERSION#v}"
if [[ "$APK_VER" != *-r* ]]; then
    APK_VER="${APK_VER}-r0"
fi

# Resolve OUTPUT_DIR to absolute path before any cd
OUTPUT_DIR="$(cd "$(dirname "$OUTPUT_DIR")" && pwd)/$(basename "$OUTPUT_DIR")"
mkdir -p "$OUTPUT_DIR"

TEMP_DIR=$(mktemp -d)
trap "rm -rf '$TEMP_DIR'" EXIT

mkdir -p "$TEMP_DIR/pkg/usr/bin"
mkdir -p "$TEMP_DIR/pkg/etc/vuio"
mkdir -p "$TEMP_DIR/pkg/etc/init.d"

# Copy binary & config
cp "$BINARY_PATH" "$TEMP_DIR/pkg/usr/bin/vuio"
chmod 755 "$TEMP_DIR/pkg/usr/bin/vuio"

cat > "$TEMP_DIR/pkg/etc/vuio/vuio.toml" << 'EOF'
[server]
port = 8080
interface = "0.0.0.0"
name = "VuIO"

[media]
scan_on_startup = true
watch_changes = true
EOF

# OpenRC init service for Alpine
cat > "$TEMP_DIR/pkg/etc/init.d/vuio" << 'EOF'
#!/sbin/openrc-run

name="vuio"
description="VuIO Media Server"
command="/usr/bin/vuio"
command_background="yes"
pidfile="/run/${RC_SVCNAME}.pid"
output_log="/var/log/vuio.log"
error_log="/var/log/vuio.log"

depend() {
    need net
    after firewall
}
EOF
chmod 755 "$TEMP_DIR/pkg/etc/init.d/vuio"

SIZE=$(du -sb "$TEMP_DIR/pkg" 2>/dev/null | cut -f1 || true)
if [ -z "$SIZE" ]; then
    SIZE=$(du -sk "$TEMP_DIR/pkg" | cut -f1)
    SIZE=$((SIZE * 1024))
fi
BUILD_DATE=$(date +%s)

# Create .PKGINFO
cat > "$TEMP_DIR/.PKGINFO" << EOF
pkgname = $PACKAGE_NAME
pkgver = $APK_VER
pkgdesc = $DESCRIPTION
url = https://github.com/vuiodev/vuio
builddate = $BUILD_DATE
packager = $MAINTAINER
size = $SIZE
arch = $ARCHITECTURE
license = MIT OR Apache-2.0
depend = ca-certificates
EOF

# Build .apk package (tar archive containing control and data tarballs)
APK_FILE="${OUTPUT_DIR}/${PACKAGE_NAME}-${APK_VER}.${ARCHITECTURE}.apk"

cd "$TEMP_DIR"
cp .PKGINFO pkg/
cd pkg
tar -czf "$APK_FILE" .PKGINFO etc usr

echo -e "${GREEN}✓ Successfully created APK package: $APK_FILE${NC}"
