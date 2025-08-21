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
mkdir -p "$SOURCE_DIR"/{bin,etc/vuio,lib/systemd/system}

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
name = "VuIO Server"
uuid = "12345678-1234-1234-1234-123456789012"

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

Requires:       systemd
Requires(pre):  shadow-utils
Requires(post): systemd
Requires(preun): systemd
Requires(postun): systemd

%description
$DESCRIPTION

Features:
- Cross-platform compatibility (Linux, Windows, macOS)
- Automatic media discovery and indexing
- Real-time file system monitoring
- SQLite database for fast media access
- Configurable via TOML configuration files
- Systemd integration for service management

%prep
%setup -q

%build
# Nothing to build, binary is pre-compiled

%install
rm -rf %{buildroot}

# Create directory structure
mkdir -p %{buildroot}%{_bindir}
mkdir -p %{buildroot}%{_sysconfdir}/vuio
mkdir -p %{buildroot}%{_unitdir}
mkdir -p %{buildroot}%{_localstatedir}/lib/vuio
mkdir -p %{buildroot}%{_localstatedir}/log/vuio

# Install files
install -m 755 bin/vuio %{buildroot}%{_bindir}/vuio
install -m 640 etc/vuio/vuio.toml %{buildroot}%{_sysconfdir}/vuio/vuio.toml
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

# Systemd integration
%systemd_post vuio.service

echo "VuIO Server has been installed successfully!"
echo ""
echo "To start the service:"
echo "  sudo systemctl start vuio"
echo ""
echo "To enable automatic startup:"
echo "  sudo systemctl enable vuio"
echo ""
echo "To check service status:"
echo "  sudo systemctl status vuio"
echo ""
echo "Configuration file: %{_sysconfdir}/vuio/vuio.toml"
echo "Log files: %{_localstatedir}/log/vuio/ or 'journalctl -u vuio'"
echo ""

%preun
%systemd_preun vuio.service

%postun
%systemd_postun_with_restart vuio.service

# Remove user and group on package removal
if [ \$1 -eq 0 ]; then
    # Package is being removed, not upgraded
    userdel vuio 2>/dev/null || true
    groupdel vuio 2>/dev/null || true
    
    # Remove data directories
    rm -rf %{_localstatedir}/lib/vuio
    rm -rf %{_localstatedir}/log/vuio
fi

%files
%{_bindir}/vuio
%config(noreplace) %{_sysconfdir}/vuio/vuio.toml
%{_unitdir}/vuio.service
%attr(755,vuio,vuio) %dir %{_localstatedir}/lib/vuio
%attr(755,vuio,vuio) %dir %{_localstatedir}/log/vuio

%changelog
* $(date '+%a %b %d %Y') VuIO Project <vuio@example.com> - $VERSION-$RELEASE
- Initial release of VuIO Server
- Cross-platform DLNA media server
- Systemd integration
- SQLite database support
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