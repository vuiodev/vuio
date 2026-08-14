#!/usr/bin/env bash
# Package the stdio bridge as an MCP Bundle for Claude Desktop.
#
# The bundle carries the `vuio` binary and runs it as `vuio mcp`, which proxies
# to a VuIO server that is already running. It does not open the library
# database itself, so the binary here needs no media, no config and no state —
# it is a client.
#
#   ./claude/mcpb/build.sh                  # uses target/release/vuio
#   ./claude/mcpb/build.sh path/to/vuio     # or an explicit binary
#
# Produces claude/mcpb/dist/vuio.mcpb, which installs into Claude Desktop by
# double-clicking it.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
binary="${1:-$repo/target/release/vuio}"

if [[ ! -x "$binary" ]]; then
    echo "No binary at $binary" >&2
    echo "Build one first: cargo build --release" >&2
    exit 1
fi

# The manifest is the bundle's contract; a typo in it fails at install time on a
# user's machine rather than here, so check it while we can.
if command -v python3 >/dev/null 2>&1; then
    python3 -c "import json,sys; json.load(open(sys.argv[1]))" "$here/manifest.json"
fi

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT

mkdir -p "$staging/bin"
cp "$here/manifest.json" "$staging/manifest.json"
cp "$repo/LICENSE-MIT" "$staging/LICENSE" 2>/dev/null || true

# Windows expects the .exe name the manifest's platform override names.
case "$binary" in
    *.exe) cp "$binary" "$staging/bin/vuio.exe" ;;
    *)     cp "$binary" "$staging/bin/vuio" ;;
esac
chmod +x "$staging/bin/"*

out="$here/dist"
mkdir -p "$out"
rm -f "$out/vuio.mcpb"

# An .mcpb is a zip. Built from inside the staging directory so paths in the
# archive are relative to the bundle root, which is where the manifest looks.
( cd "$staging" && zip -qr "$out/vuio.mcpb" . )

echo "Wrote $out/vuio.mcpb ($(du -h "$out/vuio.mcpb" | cut -f1))"
echo "Install it by double-clicking, or drag it onto Claude Desktop's Extensions pane."
