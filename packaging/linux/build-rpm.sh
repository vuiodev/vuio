#!/bin/bash

# Build RPM package for VuIO on RedHat/SUSE systems
# Creates a proper RPM package with systemd integration

set -e

# Configuration
BINARY_PATH="${1:-../../target/x86_64-unknown-linux-gnu/release/vuio}"
OUTPUT_DIR="${2:-../../builds}"
VERSION="${3:-0.1.0}"
RELEASE="${4:-1}"
ARCHITECTURE="${5:-x86_64}"
PACKAGE_NAME="vuio"
SUMMARY="Cross-platform DLNA media server"
DESCRIPTION="VuIO is a cross-platform DLNA media server that allows you to share your media files with DLNA-compatible devices on your network."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

function show_help() {
    echo -e "${GREEN}--- RPM Package Build Script ---${NC}"
    echo ""
    echo "Usage: $0 [BINARY_PATH] [OUTPUT_DIR] [VERSION] [RELEASE] [ARCHITECTURE]"
    echo ""
    echo "Arguments:"
    echo "  BINARY_PATH   Path to the compiled vuio binary (default: ../../target/x86_64-unknown-linux-gnu/release/vuio)"
    echo "  OUTPUT_DIR    Output directory for RPM file (default: ../../builds)"
    echo "  VERSION       Version number for the package (default: 0.1.0)"
    echo "  RELEASE       Release number (default: 1)"
    echo "  ARCHITECTURE  Target architecture (default: x86_64)"
    echo ""
    echo "Prerequisites:"
    echo "  - rpmbuild utility"
    echo "  - rpm-build package"
    echo ""
}

if [[ "$1" == "--help" || "$1" == "-h" ]]; then
    show_help
    exit 0
fi

# Check prerequisites
echo -e "${YELLOW}--- Checking Prerequisites ---${NC}"

if ! command -v rpmbuild &> /dev/null; then
    echo -e "${RED}✗ rpmbuild not found${NC}"
    echo -e "${YELLOW}Please install rpm-build package:${NC}"
    echo -e "${YELLOW}  RHEL/CentOS/Fedora: sudo dnf install rpm-build${NC}"
    echo -e "${YELLOW}  SUSE: sudo zypper install rpm-build${NC}"
    exit 1
fi

echo -e "${GREEN}✓ rpmbuild found${NC}"

if [[ ! -f "$BINARY_PATH" ]]; then
    echo -e "${RED}✗ Binary not found at: $BINARY_PATH${NC}"
    echo -e "${YELLOW}Please build the project first or specify correct path${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Binary found at: $BINARY_PATH${NC}"

# Create build environment
echo ""
echo -e "${YELLOW}--- Preparing Build Environment ---${NC}"

TEMP_DIR="temp_rpm"
RPM_ROOT="$TEMP_DIR/rpmbuild"

# Clean and create RPM build directory structure
if [[ -d "$TEMP_DIR" ]]; then
    rm -rf "$TEMP_DIR"
fi

mkdir -p "$RPM_ROOT"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}

# Create source tarball
SOURCE_DIR="$TEMP_DIR/${PACKAGE_NAME}-${VERSION}"
mkdir -p "$SOURCE_DIR"/{bin,etc/vuio,etc/init.d,lib/systemd/system}

# Copy binary
cp "$BINARY_PATH" "$SOURCE_DIR/bin/vuio"
chmod +x "$SOURCE_DIR/bin/vuio"

# Create default configuration
cat > "$SOURCE_DIR/etc/vuio/vuio.toml" << 'EOF'
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
cat > "$SOURCE_DIR/lib/systemd/system/vuio.service" << 'EOF'
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
cat > "$SOURCE_DIR/etc/init.d/vuio" << 'EOF'
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
    elif [ -f /etc/rc.d/init.d/functions ]; then
        # Use RHEL daemon function
        . /etc/rc.d/init.d/functions
        daemon --user=$USER --pidfile=$PIDFILE "$DAEMON $DAEMON_ARGS >/dev/null 2>&1 &"
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
    elif [ -f /etc/rc.d/init.d/functions ]; then
        . /etc/rc.d/init.d/functions
        killproc -p "$PIDFILE" "$DAEMON"
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
chmod +x "$SOURCE_DIR/etc/init.d/vuio"

# Create source tarball
cd "$TEMP_DIR"
tar -czf "$RPM_ROOT/SOURCES/${PACKAGE_NAME}-${VERSION}.tar.gz" "${PACKAGE_NAME}-${VERSION}"
cd - > /dev/null

# Create RPM spec file
cat > "$RPM_ROOT/SPECS/${PACKAGE_NAME}.spec" << EOF
Name:           $PACKAGE_NAME
Version:        $VERSION
Release:        $RELEASE%{?dist}
Summary:        $SUMMARY
License:        MIT
URL:            https://github.com/vuio/vuio
Source0:        %{name}-%{version}.tar.gz
BuildArch:      $ARCHITECTURE

Requires(pre):  shadow-utils
%{?systemd_requires}

%description
$DESCRIPTION

Features:
- Cross-platform compatibility (Linux, Windows, macOS)
- Automatic media discovery and indexing
- Real-time file system monitoring
- Configurable via TOML configuration files
- Systemd and SysVinit integration for service management

%prep
%setup -q

%build
# Nothing to build, binary is pre-compiled

%install
rm -rf %{buildroot}

# Create directory structure
mkdir -p %{buildroot}%{_bindir}
mkdir -p %{buildroot}%{_sysconfdir}/vuio
mkdir -p %{buildroot}%{_initddir}
mkdir -p %{buildroot}%{_unitdir}
mkdir -p %{buildroot}%{_localstatedir}/lib/vuio
mkdir -p %{buildroot}%{_localstatedir}/log/vuio

# Install files
install -m 755 bin/vuio %{buildroot}%{_bindir}/vuio
install -m 640 etc/vuio/vuio.toml %{buildroot}%{_sysconfdir}/vuio/vuio.toml
install -m 755 etc/init.d/vuio %{buildroot}%{_initddir}/vuio
install -m 644 lib/systemd/system/vuio.service %{buildroot}%{_unitdir}/vuio.service

%pre
# Create vuio user and group
getent group vuio >/dev/null || groupadd -r vuio
getent passwd vuio >/dev/null || \
    useradd -r -g vuio -d %{_localstatedir}/lib/vuio -s /sbin/nologin \
    -c "VuIO service user" vuio
exit 0

%post
# Set directory permissions
chown vuio:vuio %{_localstatedir}/lib/vuio
chown vuio:vuio %{_localstatedir}/log/vuio
chmod 755 %{_localstatedir}/lib/vuio
chmod 755 %{_localstatedir}/log/vuio

# Set configuration file permissions
chown root:vuio %{_sysconfdir}/vuio/vuio.toml
chmod 640 %{_sysconfdir}/vuio/vuio.toml

# Set SysV init script permissions
if [ -f %{_initddir}/vuio ]; then
    chown root:root %{_initddir}/vuio
    chmod 755 %{_initddir}/vuio
fi

# Service integration
if [ -d /run/systemd/system ]; then
    %systemd_post vuio.service
    echo "VuIO Server has been installed successfully via systemd!"
else
    if [ -x /sbin/chkconfig ]; then
        /sbin/chkconfig --add vuio || :
    fi
    echo "VuIO Server has been installed successfully via SysV init!"
fi

%preun
if [ -d /run/systemd/system ]; then
    %systemd_preun vuio.service
else
    if [ \$1 -eq 0 ]; then
        # Package is being uninstalled
        if [ -x %{_initddir}/vuio ]; then
            %{_initddir}/vuio stop >/dev/null 2>&1 || :
        fi
        if [ -x /sbin/chkconfig ]; then
            /sbin/chkconfig --del vuio || :
        fi
    fi
fi

%postun
if [ -d /run/systemd/system ]; then
    %systemd_postun_with_restart vuio.service
fi

# Remove user and group on package removal
if [ \$1 -eq 0 ]; then
    # Package is being removed, not upgraded
    userdel vuio 2>/dev/null || true
    groupdel vuio 2>/dev/null || true
    
    # Remove data directories
    rm -rf %{_localstatedir}/lib/vuio
    rm -rf %{_localstatedir}/log/vuio
    rm -f %{_initddir}/vuio
fi

%files
%{_bindir}/vuio
%config(noreplace) %{_sysconfdir}/vuio/vuio.toml
%{_unitdir}/vuio.service
%{_initddir}/vuio
%attr(755,vuio,vuio) %dir %{_localstatedir}/lib/vuio
%attr(755,vuio,vuio) %dir %{_localstatedir}/log/vuio

%changelog
* $(date '+%a %b %d %Y') VuIO Project <vuio@example.com> - $VERSION-$RELEASE
- Initial release of VuIO Server
- Cross-platform DLNA media server
- Systemd and SysVinit integration
- Real-time file system monitoring
EOF

echo -e "${GREEN}✓ Build environment prepared${NC}"

# Build the package
echo ""
echo -e "${YELLOW}--- Building RPM Package ---${NC}"

echo "Building RPM package..."
rpmbuild --define "_topdir $PWD/$RPM_ROOT" -ba "$RPM_ROOT/SPECS/${PACKAGE_NAME}.spec"

# Find the generated RPM
RPM_FILE=$(find "$RPM_ROOT/RPMS" -name "*.rpm" -type f)
if [[ -z "$RPM_FILE" ]]; then
    echo -e "${RED}✗ RPM file not found after build${NC}"
    exit 1
fi

# Move RPM to output directory
mkdir -p "$OUTPUT_DIR"
FINAL_RPM="$OUTPUT_DIR/$(basename "$RPM_FILE")"
cp "$RPM_FILE" "$FINAL_RPM"

echo -e "${GREEN}✓ RPM package created successfully: $FINAL_RPM${NC}"

# Show file info
if [[ -f "$FINAL_RPM" ]]; then
    FILE_SIZE=$(du -h "$FINAL_RPM" | cut -f1)
    echo ""
    echo -e "${CYAN}Package Details:${NC}"
    echo "  File: $(basename "$FINAL_RPM")"
    echo "  Size: $FILE_SIZE"
    echo "  Path: $FINAL_RPM"
    
    # Show package info
    echo ""
    echo -e "${CYAN}Package Information:${NC}"
    rpm -qip "$FINAL_RPM"
fi

# Cleanup
echo ""
echo -e "${YELLOW}--- Cleaning Up ---${NC}"
rm -rf "$TEMP_DIR"
echo -e "${GREEN}✓ Cleanup completed${NC}"

echo ""
echo -e "${GREEN}--- RPM Build Complete ---${NC}"
echo ""
echo "To install the package:"
echo "  sudo rpm -ivh \"$FINAL_RPM\""
echo "  # or"
echo "  sudo dnf install \"$FINAL_RPM\""
echo "  sudo zypper install \"$FINAL_RPM\""
echo ""
echo "To remove the package:"
echo "  sudo rpm -e $PACKAGE_NAME"
echo "  # or"
echo "  sudo dnf remove $PACKAGE_NAME"
echo "  sudo zypper remove $PACKAGE_NAME"