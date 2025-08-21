#!/bin/bash

# This script automates the cross-compilation of the Rust project for various targets.
# It builds for Linux, Windows, and macOS on both amd64 and arm64 architectures.

# Exit immediately if a command exits with a non-zero status.
set -e

# --- Configuration ---
# Automatically get the package name from Cargo.toml to avoid hardcoding.
PACKAGE_NAME=$(grep '^name =' Cargo.toml | head -n 1 | awk -F '"' '{print $2}')
# Directory where the final binaries will be stored.
BUILD_DIR="builds"

# --- Target Definitions ---
# An array of targets to build for.
# The format is "TARGET_TRIPLE:OUTPUT_FILENAME_SUFFIX".
# The .exe for Windows is handled automatically.
TARGETS=(
    "x86_64-unknown-linux-gnu:linux-amd64"
    "aarch64-unknown-linux-gnu:linux-arm64"
    "x86_64-pc-windows-gnu:windows-amd64"
    "aarch64-pc-windows-gnu:windows-arm64"
    "x86_64-apple-darwin:macos-amd64"
    "aarch64-apple-darwin:macos-arm64"
)

# --- Prerequisite Information ---
echo "--- Build Script for ${PACKAGE_NAME} ---"
echo ""
echo "This script will attempt to cross-compile for multiple targets."
echo "Please ensure you have the necessary linkers and toolchains installed."
echo ""
echo "On macOS with Homebrew, you might need:"
echo "  brew install x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu mingw-w64"
echo ""
echo "On a Debian-based Linux, you might need:"
echo "  sudo apt-get install -y gcc-x86-64-linux-gnu gcc-aarch64-linux-gnu gcc-mingw-w64"
echo ""
echo "You also MUST configure Cargo to use these linkers. Create or edit '.cargo/config.toml' in your project root with content like this:"
echo ""
echo "  # .cargo/config.toml"
echo "  [target.x86_64-unknown-linux-gnu]"
echo "  linker = \"x86_64-linux-gnu-gcc\""
echo ""
echo "  [target.aarch64-unknown-linux-gnu]"
echo "  linker = \"aarch64-linux-gnu-gcc\""
echo ""
echo "  [target.x86_64-pc-windows-gnu]"
echo "  linker = \"x86_64-w64-mingw32-gcc\""
echo ""
echo "  [target.aarch64-pc-windows-gnu]"
echo "  linker = \"aarch64-w64-mingw32-gcc\""
echo ""
echo "For a simpler cross-compilation experience, consider using the 'cross' tool:"
echo "  cargo install cross"
echo "  # Then run: cross build --release --target <target-triple>"
echo "----------------------------------------------------"
echo ""

# --- Build Process ---
echo "Creating build directory: ${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"

for entry in "${TARGETS[@]}"; do
    # Split the entry into the target triple and the output filename suffix.
    IFS=':' read -r TARGET SUFFIX <<< "$entry"

    FILENAME="${PACKAGE_NAME}-${SUFFIX}"
    SOURCE_BINARY_NAME="${PACKAGE_NAME}"

    # Windows targets have a .exe extension.
    if [[ "$TARGET" == *"-windows-"* ]]; then
        FILENAME="${FILENAME}.exe"
        SOURCE_BINARY_NAME="${SOURCE_BINARY_NAME}.exe"
    fi

    SOURCE_PATH="target/${TARGET}/release/${SOURCE_BINARY_NAME}"
    DEST_PATH="${BUILD_DIR}/${FILENAME}"

    echo "--- Building for ${TARGET} ---"

    # Install the Rust target via rustup if it's not already installed.
    echo "Checking for target toolchain ${TARGET}..."
    rustup target add "${TARGET}"

    # Build the project for the specified target in release mode.
    echo "Running cargo build for ${TARGET}..."
    cargo build --release --target "${TARGET}"

    # Copy the compiled binary to the build directory with a clean name.
    echo "Copying binary to ${DEST_PATH}"
    cp "${SOURCE_PATH}" "${DEST_PATH}"

    echo "Build for ${TARGET} complete."
    echo ""
done

echo "--- All builds completed! ---"
echo "Final binaries are located in the '${BUILD_DIR}' directory:"
ls -l "${BUILD_DIR}"