#!/bin/bash

# Build DEB package for VuIO on Debian/Ubuntu systems
# Creates a proper Debian package with systemd integration

set -e

# Configuration
BINARY_PATH="${1:-../../target/x86_64-unknown-linux-gnu/release/vuio}"
OUTPUT_DIR="${2:-../../builds}"
VERSION="${3:-0.1.0}"
ARCHITECTURE="${4:-amd64}"
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
    echo -e "${GREEN}--- DEB Package Build Script ---${NC}"
    echo ""
    echo "Usage: $0 [BINARY_PATH] [OUTPUT_DIR] [VERSION] [ARCHITECTURE]"
    echo ""
    echo "Arguments:"
    echo "  BINARY_PATH   Path to the compiled vuio binary (default: ../../target/x86_64-unknown-linux-gnu/release/vuio)"
    echo "  OUTPUT_DIR    Output directory for DEB file (default: ../../builds)"
    echo "  VERSION       Version number for the package (default: 0.1.0)"
    echo "  ARCHITECTURE  Target architecture (default: amd64)"
    echo ""
    echo "Prerequisites:"
    echo "  - dpkg-deb utility"
    echo "  - fakeroot (recommended)"
    echo ""
}

if [[ "$1" == "--help" || "$1" == "-h" ]]; then
    show_help
    exit 0
fi

# Check prerequisites
echo -e "${YELLOW}--- Checking Prerequisites ---${NC}"

if ! command -v dpkg-deb &> /dev/null; then
    echo -e "${RED}✗ dpkg-deb not found${NC}"
    echo -e "${YELLOW}Please install dpkg-deb: sudo apt-get install dpkg-dev${NC}"
    exit 1
fi

echo -e "${GREEN}✓ dpkg-deb found${NC}"

if [[ ! -f "$BINARY_PATH" ]]; then
    echo -e "${RED}✗ Binary not found at: $BINARY_PATH${NC}"
    echo -e "${YELLOW}Please build the project first or specify correct path${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Binary found at: $BINARY_PATH${NC}"

# Create build environment
echo ""
echo -e "${YELLOW}--- Preparing Build Environment ---${NC}"

TEMP_DIR="temp_deb"
PKG_DIR="$TEMP_DIR/${PACKAGE_NAME}_${VERSION}_${ARCHITECTURE}"

# Clean and create package directory structure
if [[ -d "$TEMP_DIR" ]]; then
    rm -rf "$TEMP_DIR"
fi

mkdir -p "$PKG_DIR"/{DEBIAN,usr/bin,etc/vuio,etc/init.d,var/log/vuio,lib/systemd/system,usr/share/doc/vuio}

# Copy binary
cp "$BINARY_PATH" "$PKG_DIR/usr/bin/vuio"
chmod +x "$PKG_DIR/usr/bin/vuio"

# Create default configuration
cat > "$PKG_DIR/etc/vuio/vuio.toml" << 'EOF'
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
supported_extensions = ["mp4", "mkv", "avi", "ts", "m2ts", "mp3", "flac", "wav", "jpg", "png", "gif"]

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
cat > "$PKG_DIR/lib/systemd/system/vuio.service" << 'EOF'
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

# Create SysV init script
cat > "$PKG_DIR/etc/init.d/vuio" << 'EOF'
#!/bin/sh
### BEGIN INIT INFO
# Provides:          vuio
# Required-Start:    $network $local_fs $remote_fs
# Required-Stop:     $network $local_fs $remote_fs
# Default-Start:     2 3 4 5
# Default-Stop:      0 1 6
# Short-Description: VuIO DLNA Media Server
# Description:       VuIO is a cross-platform DLNA media server
### END INIT INFO

PATH=/sbin:/usr/sbin:/bin:/usr/bin
DESC="VuIO DLNA Media Server"
NAME=vuio
DAEMON=/usr/bin/vuio
DAEMON_ARGS="--config /etc/vuio/vuio.toml --log-file /var/log/vuio/vuio.log"
PIDFILE=/var/run/$NAME.pid
SCRIPTNAME=/etc/init.d/$NAME
USER=vuio
GROUP=vuio

# Exit if the package is not installed
[ -x "$DAEMON" ] || exit 0

# Load the LSB library functions
if [ -f /lib/lsb/init-functions ]; then
    . /lib/lsb/init-functions
fi

do_start()
{
    if [ -f "$PIDFILE" ]; then
        PID=$(cat "$PIDFILE")
        if kill -0 "$PID" 2>/dev/null; then
            return 1 # already running
        fi
        rm -f "$PIDFILE"
    fi

    # Create directories if needed
    mkdir -p /var/log/vuio /var/lib/vuio
    chown -R $USER:$GROUP /var/log/vuio /var/lib/vuio

    if command -v start-stop-daemon >/dev/null; then
        start-stop-daemon --start --quiet --pidfile "$PIDFILE" --chuid $USER:$GROUP --make-pidfile --background --exec "$DAEMON" -- $DAEMON_ARGS || return 2
    else
        # Fallback for systems without start-stop-daemon
        su -s /bin/sh -c "nohup $DAEMON $DAEMON_ARGS >/dev/null 2>&1 & echo \$!" $USER > "$PIDFILE" || return 2
    fi
    return 0
}

do_stop()
{
    if [ -f "$PIDFILE" ]; then
        PID=$(cat "$PIDFILE")
        if kill -0 "$PID" 2>/dev/null; then
            kill -15 "$PID" || kill -9 "$PID"
            rm -f "$PIDFILE"
            return 0
        fi
        rm -f "$PIDFILE"
        return 1
    fi
    if command -v start-stop-daemon >/dev/null; then
        start-stop-daemon --stop --quiet --retry=TERM/30/KILL/5 --exec "$DAEMON"
        return "$?"
    fi
    return 1
}

case "$1" in
  start)
    echo "Starting $DESC..."
    do_start
    ;;
  stop)
    echo "Stopping $DESC..."
    do_stop
    ;;
  status)
    if [ -f "$PIDFILE" ]; then
        PID=$(cat "$PIDFILE")
        if kill -0 "$PID" 2>/dev/null; then
            echo "$NAME is running (pid $PID)"
            exit 0
        fi
        echo "$NAME is not running but pid file exists"
        exit 1
    fi
    echo "$NAME is not running"
    exit 3
    ;;
  restart|force-reload)
    echo "Restarting $DESC..."
    do_stop
    sleep 1
    do_start
    ;;
  *)
    echo "Usage: $SCRIPTNAME {start|stop|status|restart|force-reload}" >&2
    exit 3
    ;;
esac
EOF
chmod +x "$PKG_DIR/etc/init.d/vuio"

# Create control file
cat > "$PKG_DIR/DEBIAN/control" << EOF
Package: $PACKAGE_NAME
Version: $VERSION
Section: net
Priority: optional
Architecture: $ARCHITECTURE
Depends: libc6 (>= 2.17)
Maintainer: $MAINTAINER
Description: $DESCRIPTION
 VuIO is a cross-platform DLNA media server that allows you to share
 your media files with DLNA-compatible devices on your network.
 .
 Features:
  - Cross-platform compatibility (Linux, Windows, macOS)
  - Automatic media discovery and indexing
  - Real-time file system monitoring
  - Configurable via TOML configuration files
  - Systemd and SysVinit integration for service management
Homepage: https://github.com/vuio/vuio
EOF

# Create preinst script
cat > "$PKG_DIR/DEBIAN/preinst" << 'EOF'
#!/bin/bash
set -e

# Stop service if it's running
if [ -d /run/systemd/system ] && command -v systemctl >/dev/null; then
    if systemctl is-active --quiet vuio 2>/dev/null; then
        echo "Stopping VuIO systemd service..."
        systemctl stop vuio || true
    fi
elif [ -x /etc/init.d/vuio ]; then
    echo "Stopping VuIO init.d service..."
    /etc/init.d/vuio stop || true
fi

exit 0
EOF

# Create postinst script
cat > "$PKG_DIR/DEBIAN/postinst" << 'EOF'
#!/bin/bash
set -e

# Create vuio user and group
if ! getent group vuio >/dev/null; then
    echo "Creating vuio group..."
    groupadd --system vuio
fi

if ! getent passwd vuio >/dev/null; then
    echo "Creating vuio user..."
    useradd --system --gid vuio --home-dir /var/lib/vuio \
            --shell /usr/sbin/nologin --comment "VuIO service user" vuio
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

# Set init script permissions if it exists
if [ -f /etc/init.d/vuio ]; then
    chown root:root /etc/init.d/vuio
    chmod 755 /etc/init.d/vuio
fi

# Reload systemd and enable service, or fallback to SysV init
if [ -d /run/systemd/system ] && command -v systemctl >/dev/null; then
    echo "Configuring systemd service..."
    systemctl daemon-reload || true
    systemctl enable vuio || true
    systemctl start vuio || true
    echo "VuIO Server has been installed and started via systemd!"
elif command -v update-rc.d >/dev/null; then
    echo "Configuring SysV init service..."
    update-rc.d vuio defaults || true
    if [ -x /etc/init.d/vuio ]; then
        /etc/init.d/vuio start || true
    fi
    echo "VuIO Server has been installed and started via SysV init!"
else
    echo "VuIO Server installed successfully, but no supported init system was detected."
    echo "You can start the server manually by running:"
    echo "  sudo -u vuio /usr/bin/vuio --config /etc/vuio/vuio.toml"
fi

exit 0
EOF

# Create prerm script
cat > "$PKG_DIR/DEBIAN/prerm" << 'EOF'
#!/bin/bash
set -e

# Stop and disable service
if [ -d /run/systemd/system ] && command -v systemctl >/dev/null; then
    echo "Stopping and disabling VuIO systemd service..."
    systemctl disable vuio || true
    systemctl stop vuio || true
elif [ -x /etc/init.d/vuio ]; then
    echo "Stopping and disabling VuIO init.d service..."
    /etc/init.d/vuio stop || true
    if command -v update-rc.d >/dev/null; then
        update-rc.d -f vuio remove || true
    fi
fi

exit 0
EOF

# Create postrm script
cat > "$PKG_DIR/DEBIAN/postrm" << 'EOF'
#!/bin/bash
set -e

case "$1" in
    purge)
        # Remove user and group
        if getent passwd vuio >/dev/null; then
            echo "Removing vuio user..."
            userdel vuio || true
        fi
        
        if getent group vuio >/dev/null; then
            echo "Removing vuio group..."
            groupdel vuio || true
        fi
        
        # Remove data directories
        rm -rf /var/lib/vuio
        rm -rf /var/log/vuio
        
        # Remove configuration
        rm -rf /etc/vuio
        rm -f /etc/init.d/vuio
        ;;
    remove)
        # Reload systemd if present
        if [ -d /run/systemd/system ] && command -v systemctl >/dev/null; then
            systemctl daemon-reload || true
        fi
        ;;
esac

exit 0
EOF

# Create copyright file
cat > "$PKG_DIR/usr/share/doc/vuio/copyright" << 'EOF'
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: vuio
Upstream-Contact: VuIO <vuio@vuio.dev>
Source: https://github.com/vuio/vuio

Files: *
Copyright: 2025 VuIO Contributors
License: MIT

License: MIT
 Permission is hereby granted, free of charge, to any person obtaining a copy
 of this software and associated documentation files (the "Software"), to deal
 in the Software without restriction, including without limitation the rights
 to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 copies of the Software, and to permit persons to whom the Software is
 furnished to do so, subject to the following conditions:
 .
 The above copyright notice and this permission notice shall be included in all
 copies or substantial portions of the Software.
 .
 THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 SOFTWARE.
EOF

# Create changelog
cat > "$PKG_DIR/usr/share/doc/vuio/changelog.Debian" << EOF
$PACKAGE_NAME ($VERSION-1) unstable; urgency=low

  * VuIO Server
  * Cross-platform DLNA media server
  * Systemd integration
  * Real-time file system monitoring

 -- $MAINTAINER  $(date -R)
EOF

# Compress changelog
gzip -9 "$PKG_DIR/usr/share/doc/vuio/changelog.Debian"

# Make scripts executable
chmod +x "$PKG_DIR/DEBIAN"/{preinst,postinst,prerm,postrm}

# Calculate installed size
INSTALLED_SIZE=$(du -sk "$PKG_DIR" | cut -f1)
echo "Installed-Size: $INSTALLED_SIZE" >> "$PKG_DIR/DEBIAN/control"

echo -e "${GREEN}✓ Build environment prepared${NC}"

# Build the package
echo ""
echo -e "${YELLOW}--- Building DEB Package ---${NC}"

DEB_FILE="${PACKAGE_NAME}_${VERSION}_${ARCHITECTURE}.deb"
mkdir -p "$OUTPUT_DIR"
FINAL_DEB="$OUTPUT_DIR/$DEB_FILE"

echo "Creating DEB package..."
if command -v fakeroot &> /dev/null; then
    fakeroot dpkg-deb --build "$PKG_DIR" "$FINAL_DEB"
else
    dpkg-deb --build "$PKG_DIR" "$FINAL_DEB"
fi

echo -e "${GREEN}✓ DEB package created successfully: $FINAL_DEB${NC}"

# Show file info
if [[ -f "$FINAL_DEB" ]]; then
    FILE_SIZE=$(du -h "$FINAL_DEB" | cut -f1)
    echo ""
    echo -e "${CYAN}Package Details:${NC}"
    echo "  File: $(basename "$FINAL_DEB")"
    echo "  Size: $FILE_SIZE"
    echo "  Path: $FINAL_DEB"
    
    # Show package info
    echo ""
    echo -e "${CYAN}Package Information:${NC}"
    dpkg-deb --info "$FINAL_DEB"
fi

# Cleanup
echo ""
echo -e "${YELLOW}--- Cleaning Up ---${NC}"
rm -rf "$TEMP_DIR"
echo -e "${GREEN}✓ Cleanup completed${NC}"

echo ""
echo -e "${GREEN}--- DEB Build Complete ---${NC}"
echo ""
echo "To install the package:"
echo "  sudo dpkg -i \"$FINAL_DEB\""
echo "  sudo apt-get install -f  # Fix any dependency issues"
echo ""
echo "To remove the package:"
echo "  sudo apt-get remove $PACKAGE_NAME"
echo ""
echo "To purge the package (remove config files):"
echo "  sudo apt-get purge $PACKAGE_NAME"