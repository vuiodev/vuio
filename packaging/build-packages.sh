#!/bin/bash

# Master packaging script for VuIO
# Builds all supported package formats based on the current platform

set -e

# Configuration
VERSION="${1:-0.1.0}"
OUTPUT_DIR="${2:-../builds}"
BINARY_DIR="${3:-../target}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

function show_help() {
    echo -e "${GREEN}--- VuIO Master Packaging Script ---${NC}"
    echo ""
    echo "Usage: $0 [VERSION] [OUTPUT_DIR] [BINARY_DIR]"
    echo ""
    echo "Arguments:"
    echo "  VERSION       Version number for packages (default: 0.1.0)"
    echo "  OUTPUT_DIR    Output directory for packages (default: ../builds)"
    echo "  BINARY_DIR    Directory containing compiled binaries (default: ../target)"
    echo ""
    echo "Supported package formats:"
    echo "  - Windows: MSI installer (requires WiX Toolset)"
    echo "  - macOS: PKG installer (requires Xcode Command Line Tools)"
    echo "  - Linux: DEB and RPM packages (requires dpkg-deb and rpmbuild)"
    echo ""
    echo "Options:"
    echo "  --help, -h    Show this help message"
    echo "  --list        List available binaries"
    echo "  --windows     Build Windows packages only"
    echo "  --macos       Build macOS packages only"
    echo "  --linux       Build Linux packages only"
    echo ""
}

function list_binaries() {
    echo -e "${CYAN}Available binaries in $BINARY_DIR:${NC}"
    if [[ -d "$BINARY_DIR" ]]; then
        find "$BINARY_DIR" -name "vuio*" -type f -executable 2>/dev/null | sort || echo "No binaries found"
    else
        echo "Binary directory not found: $BINARY_DIR"
    fi
}

function detect_platform() {
    case "$(uname -s)" in
        Darwin*)    echo "macos" ;;
        Linux*)     echo "linux" ;;
        CYGWIN*|MINGW*|MSYS*) echo "windows" ;;
        *)          echo "unknown" ;;
    esac
}

function build_windows_packages() {
    echo -e "${YELLOW}--- Building Windows Packages ---${NC}"
    
    local msvc_binary="$BINARY_DIR/x86_64-pc-windows-msvc/release/vuio.exe"
    local gnu_binary="$BINARY_DIR/x86_64-pc-windows-gnu/release/vuio.exe"
    
    if [[ -f "$msvc_binary" ]]; then
        echo "Building MSI installer (MSVC)..."
        cd windows
        if command -v pwsh &> /dev/null; then
            pwsh -File build-msi.ps1 -BinaryPath "$msvc_binary" -OutputDir "$OUTPUT_DIR" -Version "$VERSION"
        elif command -v powershell &> /dev/null; then
            powershell -File build-msi.ps1 -BinaryPath "$msvc_binary" -OutputDir "$OUTPUT_DIR" -Version "$VERSION"
        else
            echo -e "${RED}✗ PowerShell not found. Cannot build MSI installer.${NC}"
        fi
        cd ..
    elif [[ -f "$gnu_binary" ]]; then
        echo "Building MSI installer (GNU)..."
        cd windows
        if command -v pwsh &> /dev/null; then
            pwsh -File build-msi.ps1 -BinaryPath "$gnu_binary" -OutputDir "$OUTPUT_DIR" -Version "$VERSION"
        elif command -v powershell &> /dev/null; then
            powershell -File build-msi.ps1 -BinaryPath "$gnu_binary" -OutputDir "$OUTPUT_DIR" -Version "$VERSION"
        else
            echo -e "${RED}✗ PowerShell not found. Cannot build MSI installer.${NC}"
        fi
        cd ..
    else
        echo -e "${RED}✗ No Windows binaries found${NC}"
        echo "Expected: $msvc_binary or $gnu_binary"
    fi
}

function build_macos_packages() {
    echo -e "${YELLOW}--- Building macOS Packages ---${NC}"
    
    local x64_binary="$BINARY_DIR/x86_64-apple-darwin/release/vuio"
    local arm64_binary="$BINARY_DIR/aarch64-apple-darwin/release/vuio"
    
    if [[ -f "$x64_binary" ]]; then
        echo "Building PKG installer (x86_64)..."
        cd macos
        ./build-pkg.sh "$x64_binary" "$OUTPUT_DIR" "$VERSION"
        cd ..
    fi
    
    if [[ -f "$arm64_binary" ]]; then
        echo "Building PKG installer (ARM64)..."
        cd macos
        # Create ARM64 specific package
        ./build-pkg.sh "$arm64_binary" "$OUTPUT_DIR" "$VERSION-arm64"
        cd ..
    fi
    
    if [[ ! -f "$x64_binary" && ! -f "$arm64_binary" ]]; then
        echo -e "${RED}✗ No macOS binaries found${NC}"
        echo "Expected: $x64_binary or $arm64_binary"
    fi
}

function build_linux_packages() {
    echo -e "${YELLOW}--- Building Linux Packages ---${NC}"
    
    local x64_binary="$BINARY_DIR/x86_64-unknown-linux-gnu/release/vuio"
    local arm64_binary="$BINARY_DIR/aarch64-unknown-linux-gnu/release/vuio"
    local musl_binary="$BINARY_DIR/x86_64-unknown-linux-musl/release/vuio"
    
    cd linux
    
    # Build DEB packages
    if [[ -f "$x64_binary" ]]; then
        echo "Building DEB package (x86_64)..."
        ./build-deb.sh "$x64_binary" "$OUTPUT_DIR" "$VERSION" "amd64"
    fi
    
    if [[ -f "$arm64_binary" ]]; then
        echo "Building DEB package (ARM64)..."
        ./build-deb.sh "$arm64_binary" "$OUTPUT_DIR" "$VERSION" "arm64"
    fi
    
    if [[ -f "$musl_binary" ]]; then
        echo "Building DEB package (musl)..."
        ./build-deb.sh "$musl_binary" "$OUTPUT_DIR" "$VERSION-musl" "amd64"
    fi
    
    # Build RPM packages
    if [[ -f "$x64_binary" ]]; then
        echo "Building RPM package (x86_64)..."
        ./build-rpm.sh "$x64_binary" "$OUTPUT_DIR" "$VERSION" "1" "x86_64"
    fi
    
    if [[ -f "$arm64_binary" ]]; then
        echo "Building RPM package (ARM64)..."
        ./build-rpm.sh "$arm64_binary" "$OUTPUT_DIR" "$VERSION" "1" "aarch64"
    fi
    
    if [[ -f "$musl_binary" ]]; then
        echo "Building RPM package (musl)..."
        ./build-rpm.sh "$musl_binary" "$OUTPUT_DIR" "$VERSION" "1" "x86_64"
    fi
    
    cd ..
    
    if [[ ! -f "$x64_binary" && ! -f "$arm64_binary" && ! -f "$musl_binary" ]]; then
        echo -e "${RED}✗ No Linux binaries found${NC}"
        echo "Expected: $x64_binary, $arm64_binary, or $musl_binary"
    fi
}

# Parse command line arguments
BUILD_WINDOWS=false
BUILD_MACOS=false
BUILD_LINUX=false
BUILD_ALL=true

while [[ $# -gt 0 ]]; do
    case $1 in
        --help|-h)
            show_help
            exit 0
            ;;
        --list)
            list_binaries
            exit 0
            ;;
        --windows)
            BUILD_WINDOWS=true
            BUILD_ALL=false
            shift
            ;;
        --macos)
            BUILD_MACOS=true
            BUILD_ALL=false
            shift
            ;;
        --linux)
            BUILD_LINUX=true
            BUILD_ALL=false
            shift
            ;;
        *)
            # Positional arguments are handled at the top
            break
            ;;
    esac
done

# Change to packaging directory
cd "$(dirname "$0")"

echo -e "${GREEN}--- VuIO Package Builder ---${NC}"
echo "Version: $VERSION"
echo "Output Directory: $OUTPUT_DIR"
echo "Binary Directory: $BINARY_DIR"
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Detect platform and build appropriate packages
PLATFORM=$(detect_platform)
echo "Detected platform: $PLATFORM"
echo ""

if [[ "$BUILD_ALL" == true ]]; then
    case $PLATFORM in
        windows)
            build_windows_packages
            ;;
        macos)
            build_macos_packages
            ;;
        linux)
            build_linux_packages
            ;;
        *)
            echo -e "${YELLOW}Unknown platform. Building all available packages...${NC}"
            build_windows_packages || true
            build_macos_packages || true
            build_linux_packages || true
            ;;
    esac
else
    [[ "$BUILD_WINDOWS" == true ]] && build_windows_packages
    [[ "$BUILD_MACOS" == true ]] && build_macos_packages
    [[ "$BUILD_LINUX" == true ]] && build_linux_packages
fi

# Show summary
echo ""
echo -e "${GREEN}--- Packaging Complete ---${NC}"
echo "Generated packages:"
if [[ -d "$OUTPUT_DIR" ]]; then
    ls -la "$OUTPUT_DIR"/*.{msi,pkg,deb,rpm} 2>/dev/null || echo "No packages found in output directory"
else
    echo "Output directory not found: $OUTPUT_DIR"
fi

echo ""
echo -e "${CYAN}Package installation commands:${NC}"
echo "Windows MSI: msiexec /i package.msi"
echo "macOS PKG:   sudo installer -pkg package.pkg -target /"
echo "DEB package: sudo dpkg -i package.deb"
echo "RPM package: sudo rpm -ivh package.rpm"