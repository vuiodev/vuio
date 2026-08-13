#!/bin/bash
# Build the vuio-web browser interface into the crate that embeds it.
#
# The interface is developed in its own repository — github.com/vuiodev/vuio-web —
# so this repository carries only the built bundle, committed at
# crates/vuio-web/dist. That is what lets `cargo build` and the Docker builder
# work with no Node installed, and what lets frontend work happen without a Rust
# toolchain or this checkout.
#
# Usage:
#   ./scripts/build-web.sh                # expects ../vuio-web beside this repo
#   ./scripts/build-web.sh ../some/path   # or say where the checkout is
#   VUIO_WEB_SRC=/path/to/vuio-web ./scripts/build-web.sh
#
# Run it after a UI change lands in vuio-web, and commit the rebuilt dist/ here
# with a message naming the vuio-web commit it came from. Nothing about a stale
# bundle looks wrong at runtime — the server starts and serves the old UI —
# so BUILD_INFO beside dist/ records where it came from.

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CRATE="$ROOT/crates/vuio-web"
DIST="$CRATE/dist"

SRC="${1:-${VUIO_WEB_SRC:-$ROOT/../vuio-web}}"
if [ ! -f "$SRC/package.json" ]; then
    echo -e "${RED}[ERROR]${NC} No vuio-web checkout at: $SRC"
    echo "        The browser interface lives in its own repository. Clone it beside"
    echo "        this one, or point this script at it:"
    echo ""
    echo "          git clone git@github.com:vuiodev/vuio-web.git $ROOT/../vuio-web"
    echo "          ./scripts/build-web.sh [path-to-vuio-web]"
    echo ""
    echo "        Building vuio itself does not need any of this — the committed"
    echo "        dist/ covers that."
    exit 1
fi
SRC="$(cd "$SRC" && pwd)"

if ! command -v npm >/dev/null 2>&1; then
    echo -e "${RED}[ERROR]${NC} npm is not installed. It is needed to build the interface,"
    echo "        but not to build vuio — the committed dist/ covers that."
    exit 1
fi

echo -e "${BLUE}[INFO]${NC} Source: $SRC"
cd "$SRC"
# `npm ci` needs a lockfile in sync with package.json; fall back to install so a
# fresh dependency does not turn into a confusing lockfile error.
if [ -f package-lock.json ]; then
    npm ci
else
    npm install
fi

echo -e "${BLUE}[INFO]${NC} Building into $DIST"
rm -rf "$DIST"
VUIO_WEB_DIST="$DIST" npm run build

if [ ! -f "$DIST/index.html" ]; then
    echo -e "${RED}[ERROR]${NC} The build produced no $DIST/index.html."
    exit 1
fi

# Provenance, so the shipped UI can be traced back to the commit that made it.
# Kept outside dist/ because build.rs embeds every file it finds in there, and
# this is not something to serve.
{
    echo "# Written by scripts/build-web.sh. Do not edit."
    echo "source = \"$(git -C "$SRC" config --get remote.origin.url 2>/dev/null || echo unknown)\""
    echo "commit = \"$(git -C "$SRC" rev-parse HEAD 2>/dev/null || echo unknown)\""
    echo "describe = \"$(git -C "$SRC" describe --tags --always --dirty 2>/dev/null || echo unknown)\""
    echo "version = \"$(node -p "require('$SRC/package.json').version" 2>/dev/null || echo unknown)\""
} > "$CRATE/BUILD_INFO.toml"

if ! git -C "$SRC" diff --quiet 2>/dev/null || [ -n "$(git -C "$SRC" status --porcelain 2>/dev/null)" ]; then
    echo -e "${YELLOW}[WARNING]${NC} The vuio-web checkout has uncommitted changes."
    echo "          Commit them there first, so the bundle here maps to a real commit."
fi

echo -e "${GREEN}[SUCCESS]${NC} Built $(find "$DIST" -type f | wc -l | tr -d ' ') files ($(du -sh "$DIST" | cut -f1))"

cd "$ROOT"
if git rev-parse --git-dir >/dev/null 2>&1; then
    # `git status --porcelain` rather than `git diff`, so a bundle that is not
    # tracked yet reads as changed instead of silently as clean.
    changes=$(git status --porcelain -- crates/vuio-web/dist crates/vuio-web/BUILD_INFO.toml)
    if [ -z "$changes" ]; then
        echo -e "${BLUE}[INFO]${NC} The bundle is unchanged; nothing to commit."
    else
        echo -e "${BLUE}[INFO]${NC} The bundle changed — commit it:"
        echo "$changes" | head -20
    fi
fi
