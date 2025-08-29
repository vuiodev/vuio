#!/bin/bash

# Exit immediately if a command exits with a non-zero status.
set -e

# The main package directory
MAIN_DIR="."

# The output directory, relative to the project root
OUTPUT_DIR="../build"

# Build flags for release:
# -s: Omit the symbol table
# -w: Omit the DWARF debug information
# This combination significantly reduces the binary size.
LDFLAGS="-s -w"

# List of target platforms and architectures
# Format: "os/arch"
TARGETS=(
    "linux/amd64"
    "linux/arm64"
    "windows/amd64"
    "windows/arm64"
    "darwin/amd64"
    "darwin/arm64"
    "freebsd/amd64"
    "freebsd/arm64"
)

# Ensure we are in the script's directory so paths are correct
cd "$(dirname "$0")"

echo "Creating build directory: $OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

# requires a C cross-compiler toolchain for each target platform.
#
# Before running this script, you MUST have these toolchains installed.
# For example, on a Debian/Ubuntu system, you might need:
#   - sudo apt install gcc-aarch64-linux-gnu
#   - sudo apt install gcc-mingw-w64
#
# A modern alternative is to install Zig and use 'zig cc' as the compiler,
# which simplifies cross-compilation significantly.

echo "Starting build process..."

for target in "${TARGETS[@]}"; do
    # Split the target into OS and ARCH
    IFS='/' read -r GOOS GOARCH <<< "$target"

    echo "Building for $GOOS/$GOARCH..."

    # Set the output binary name
    BINARY_NAME="vuio-go-$GOOS-$GOARCH"
    if [ "$GOOS" = "windows" ]; then
        BINARY_NAME="$BINARY_NAME.exe"
    fi

    OUTPUT_PATH="$OUTPUT_DIR/$BINARY_NAME"

    # Set environment variables for the go build command
    # and execute the build.
    env CGO_ENABLED=1 GOOS="$GOOS" GOARCH="$GOARCH" go build \
        -ldflags="$LDFLAGS" \
        -trimpath \
        -o "$OUTPUT_PATH" \
        "$MAIN_DIR"

    echo "Successfully built $OUTPUT_PATH"
    echo "-------------------------"
done

echo "All builds completed successfully!"