#!/bin/bash

# Exit immediately if a command exits with a non-zero status.
set -e

# --- Configuration ---
# Set the target OS and Architecture
TARGET_OS="darwin"
TARGET_ARCH="arm64"

# Set the output directory relative to the project root
OUTPUT_DIR="../build"

# Set the name of the final binary
BINARY_NAME="vuio-go-macos-arm64"
OUTPUT_PATH="$OUTPUT_DIR/$BINARY_NAME"

# --- Build Flags for Maximum Optimization ---
# -s: Omit the symbol table. This makes the binary smaller and harder to reverse-engineer.
# -w: Omit the DWARF debug information. This also significantly reduces binary size.
# -trimpath: Remove all file system paths from the compiled executable. This improves
#            build reproducibility and privacy.
# The Go compiler applies performance optimizations by default, so these flags
# focus on creating the smallest possible release binary.
LDFLAGS="-s -w"

# --- Build Process ---
# Ensure we are in the script's directory so that relative paths work correctly.
cd "$(dirname "$0")"

echo "Creating build directory: $OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"

echo "Starting optimized build for macOS on Apple Silicon ($TARGET_OS/$TARGET_ARCH)..."

# CGO_ENABLED=1 is required for the go-sqlite3 driver.
# On macOS, the necessary C compiler (Clang) is available with Xcode Command Line Tools.
env CGO_ENABLED=1 GOOS="$TARGET_OS" GOARCH="$TARGET_ARCH" go build \
    -ldflags="$LDFLAGS" \
    -trimpath \
    -o "$OUTPUT_PATH" \
    .

echo ""
echo "----------------------------------------"
echo "Build complete!"
echo "Optimized binary created at: $OUTPUT_PATH"
echo "----------------------------------------"