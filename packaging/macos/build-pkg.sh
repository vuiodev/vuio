#!/bin/bash

# Build PKG installer for VuIO on macOS
# Creates a signed installer package for distribution

set -e

# Configuration
BINARY_PATH="${1:-../../target/x86_64-apple-darwin/release/vuio}"
OUTPUT_DIR="${2:-../../builds}"
VERSION="${3:-0.1.0}"
PACKAGE_ID="com.vuio.server"
INSTALL_LOCATION="/usr/local/bin"
SCRIPTS_DIR="scripts"
TEMP_DIR="temp"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

function show_help() {
    echo -e "${GREEN}--- PKG Installer Build Script ---${NC}"
    echo ""
    echo "Usage: $0 [BINARY_PATH] [OUTPUT_DIR] [VERSION]"
    echo ""
    echo "Arguments:"
    echo "  BINARY_PATH   Path to the compiled vuio binary (default: ../../target/x86_64-apple-darwin/release/vuio)"
    echo "  OUTPUT_DIR    Output directory for PKG file (default: ../../builds)"
    echo "  VERSION       Version number for the installer (default: 0.1.0)"
    echo ""
    echo "Prerequisites:"
    echo "  - Xcode Command Line Tools"
    echo "  - pkgbuild and productbuild utilities"
    echo "  - Optional: Developer ID certificate for signing"
    echo ""
}

if [[ "$1" == "--help" || "$1" == "-h" ]]; then
    show_help
    exit 0
fi

# Check prerequisites
echo -e "${YELLOW}--- Checking Prerequisites ---${NC}"

if ! command -v pkgbuild &> /dev/null; then
    echo -e "${RED}✗ pkgbuild not found${NC}"
    echo -e "${YELLOW}Please install Xcode Command Line Tools: xcode-select --install${NC}"
    exit 1
fi

if ! command -v productbuild &> /dev/null; then
    echo -e "${RED}✗ productbuild not found${NC}"
    echo -e "${YELLOW}Please install Xcode Command Line Tools: xcode-select --install${NC}"
    exit 1
fi

echo -e "${GREEN}✓ pkgbuild found${NC}"
echo -e "${GREEN}✓ productbuild found${NC}"

if [[ ! -f "$BINARY_PATH" ]]; then
    echo -e "${RED}✗ Binary not found at: $BINARY_PATH${NC}"
    echo -e "${YELLOW}Please build the project first or specify correct path${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Binary found at: $BINARY_PATH${NC}"

# Create build environment
echo ""
echo -e "${YELLOW}--- Preparing Build Environment ---${NC}"

# Clean and create temp directory
if [[ -d "$TEMP_DIR" ]]; then
    rm -rf "$TEMP_DIR"
fi
mkdir -p "$TEMP_DIR"

# Create package root structure
PKG_ROOT="$TEMP_DIR/pkg_root"
mkdir -p "$PKG_ROOT$INSTALL_LOCATION"
mkdir -p "$PKG_ROOT/usr/local/etc/vuio"
mkdir -p "$PKG_ROOT/usr/local/var/log/vuio"
mkdir -p "$PKG_ROOT/Library/LaunchDaemons"

# Copy binary
cp "$BINARY_PATH" "$PKG_ROOT$INSTALL_LOCATION/vuio"
chmod +x "$PKG_ROOT$INSTALL_LOCATION/vuio"

# Create default configuration
cat > "$PKG_ROOT/usr/local/etc/vuio/vuio.toml" << 'EOF'
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
path = "/Users/Shared/Movies"
recursive = true

[[media.directories]]
path = "/Users/Shared/Music"
recursive = true

[[media.directories]]
path = "/Users/Shared/Pictures"
recursive = true

[database]
vacuum_on_startup = false
backup_enabled = true
EOF

# Create LaunchDaemon plist for system service
cat > "$PKG_ROOT/Library/LaunchDaemons/com.vuio.server.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.vuio.server</string>
    <key>ProgramArguments</key>
    <array>
        <string>$INSTALL_LOCATION/vuio</string>
        <string>--config</string>
        <string>/usr/local/etc/vuio/vuio.toml</string>
    </array>
    <key>RunAtLoad</key>
    <false/>
    <key>KeepAlive</key>
    <false/>
    <key>StandardOutPath</key>
    <string>/usr/local/var/log/vuio/vuio.log</string>
    <key>StandardErrorPath</key>
    <string>/usr/local/var/log/vuio/vuio.error.log</string>
    <key>WorkingDirectory</key>
    <string>/usr/local/etc/vuio</string>
    <key>UserName</key>
    <string>_vuio</string>
    <key>GroupName</key>
    <string>_vuio</string>
</dict>
</plist>
EOF

# Create scripts directory
mkdir -p "$SCRIPTS_DIR"

# Create preinstall script
cat > "$SCRIPTS_DIR/preinstall" << 'EOF'
#!/bin/bash

# Stop the service if it's running
if launchctl list | grep -q "com.vuio.server"; then
    echo "Stopping VuIO service..."
    launchctl unload /Library/LaunchDaemons/com.vuio.server.plist 2>/dev/null || true
fi

# Create user and group for the service
if ! dscl . -read /Groups/_vuio &>/dev/null; then
    echo "Creating _vuio group..."
    dseditgroup -o create -q _vuio
fi

if ! dscl . -read /Users/_vuio &>/dev/null; then
    echo "Creating _vuio user..."
    # Find next available UID starting from 200
    uid=200
    while dscl . -list /Users UniqueID | awk '{print $2}' | grep -q "^$uid$"; do
        ((uid++))
    done
    
    dscl . -create /Users/_vuio
    dscl . -create /Users/_vuio UserShell /usr/bin/false
    dscl . -create /Users/_vuio RealName "VuIO Service"
    dscl . -create /Users/_vuio UniqueID $uid
    dscl . -create /Users/_vuio PrimaryGroupID 20
    dscl . -create /Users/_vuio NFSHomeDirectory /var/empty
    dscl . -create /Users/_vuio Password "*"
fi

exit 0
EOF

# Create postinstall script
cat > "$SCRIPTS_DIR/postinstall" << 'EOF'
#!/bin/bash

# Set proper permissions
chown -R _vuio:_vuio /usr/local/var/log/vuio
chmod 755 /usr/local/bin/vuio
chmod 644 /Library/LaunchDaemons/com.vuio.server.plist
chmod 644 /usr/local/etc/vuio/vuio.toml

# Load the LaunchDaemon (but don't start it automatically)
launchctl load /Library/LaunchDaemons/com.vuio.server.plist

echo "VuIO Server has been installed successfully!"
echo ""
echo "To start the service:"
echo "  sudo launchctl start com.vuio.server"
echo ""
echo "To stop the service:"
echo "  sudo launchctl stop com.vuio.server"
echo ""
echo "Configuration file: /usr/local/etc/vuio/vuio.toml"
echo "Log files: /usr/local/var/log/vuio/"
echo ""

exit 0
EOF

# Create preremove script
cat > "$SCRIPTS_DIR/preremove" << 'EOF'
#!/bin/bash

# Stop and unload the service
if launchctl list | grep -q "com.vuio.server"; then
    echo "Stopping VuIO service..."
    launchctl stop com.vuio.server 2>/dev/null || true
    launchctl unload /Library/LaunchDaemons/com.vuio.server.plist 2>/dev/null || true
fi

exit 0
EOF

# Make scripts executable
chmod +x "$SCRIPTS_DIR"/*

echo -e "${GREEN}✓ Build environment prepared${NC}"

# Build the package
echo ""
echo -e "${YELLOW}--- Building PKG Installer ---${NC}"

PKG_FILE="vuio-$VERSION-macos.pkg"
COMPONENT_PKG="$TEMP_DIR/vuio-component.pkg"

# Create component package
echo "Creating component package..."
pkgbuild --root "$PKG_ROOT" \
         --identifier "$PACKAGE_ID" \
         --version "$VERSION" \
         --scripts "$SCRIPTS_DIR" \
         --install-location "/" \
         "$COMPONENT_PKG"

# Create distribution XML
cat > "$TEMP_DIR/distribution.xml" << EOF
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="1">
    <title>VuIO Server</title>
    <organization>com.vuio</organization>
    <domains enable_localSystem="true"/>
    <options customize="never" require-scripts="true" rootVolumeOnly="true" />
    
    <welcome file="welcome.html"/>
    <license file="license.txt"/>
    <conclusion file="conclusion.html"/>
    
    <pkg-ref id="$PACKAGE_ID"/>
    
    <options customize="never" require-scripts="false"/>
    <choices-outline>
        <line choice="default">
            <line choice="$PACKAGE_ID"/>
        </line>
    </choices-outline>
    
    <choice id="default"/>
    <choice id="$PACKAGE_ID" visible="false">
        <pkg-ref id="$PACKAGE_ID"/>
    </choice>
    
    <pkg-ref id="$PACKAGE_ID" version="$VERSION" onConclusion="none">vuio-component.pkg</pkg-ref>
</installer-gui-script>
EOF

# Create welcome HTML
cat > "$TEMP_DIR/welcome.html" << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Welcome</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; margin: 20px; }
        h1 { color: #333; }
    </style>
</head>
<body>
    <h1>Welcome to VuIO Server</h1>
    <p>This installer will install VuIO Server, a cross-platform DLNA media server that allows you to share your media files with DLNA-compatible devices on your network.</p>
    <p>VuIO Server will be installed as a system service that can be started and stopped using launchctl commands.</p>
</body>
</html>
EOF

# Create license file
cat > "$TEMP_DIR/license.txt" << 'EOF'
MIT License

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
EOF

# Create conclusion HTML
cat > "$TEMP_DIR/conclusion.html" << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Installation Complete</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, sans-serif; margin: 20px; }
        h1 { color: #333; }
        code { background-color: #f5f5f5; padding: 2px 4px; border-radius: 3px; }
    </style>
</head>
<body>
    <h1>Installation Complete</h1>
    <p>VuIO Server has been successfully installed!</p>
    
    <h2>Getting Started</h2>
    <p>To start the VuIO service, open Terminal and run:</p>
    <p><code>sudo launchctl start com.vuio.server</code></p>
    
    <p>To stop the service:</p>
    <p><code>sudo launchctl stop com.vuio.server</code></p>
    
    <h2>Configuration</h2>
    <p>The configuration file is located at:</p>
    <p><code>/usr/local/etc/vuio/vuio.toml</code></p>
    
    <p>Log files can be found at:</p>
    <p><code>/usr/local/var/log/vuio/</code></p>
    
    <p>For more information, visit the VuIO project page.</p>
</body>
</html>
EOF

# Create final product package
echo "Creating product package..."
mkdir -p "$OUTPUT_DIR"
FINAL_PKG="$OUTPUT_DIR/$PKG_FILE"

productbuild --distribution "$TEMP_DIR/distribution.xml" \
             --package-path "$TEMP_DIR" \
             --resources "$TEMP_DIR" \
             "$FINAL_PKG"

echo -e "${GREEN}✓ PKG installer created successfully: $FINAL_PKG${NC}"

# Show file info
if [[ -f "$FINAL_PKG" ]]; then
    FILE_SIZE=$(du -h "$FINAL_PKG" | cut -f1)
    echo ""
    echo -e "${CYAN}Installer Details:${NC}"
    echo "  File: $(basename "$FINAL_PKG")"
    echo "  Size: $FILE_SIZE"
    echo "  Path: $FINAL_PKG"
fi

# Cleanup
echo ""
echo -e "${YELLOW}--- Cleaning Up ---${NC}"
rm -rf "$TEMP_DIR"
rm -rf "$SCRIPTS_DIR"
echo -e "${GREEN}✓ Cleanup completed${NC}"

echo ""
echo -e "${GREEN}--- PKG Build Complete ---${NC}"
echo ""
echo "To install the package:"
echo "  sudo installer -pkg \"$FINAL_PKG\" -target /"
echo ""
echo "To sign the package (optional):"
echo "  productsign --sign \"Developer ID Installer: Your Name\" \"$FINAL_PKG\" \"$FINAL_PKG.signed\""