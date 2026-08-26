#!/bin/bash
# Vendor the OxideAV codec runtime and DTS decoder into crates/vendor.
#
# VuIO decodes AC-3, E-AC-3 and DTS so a TV without those licences gets sound.
# oxideav-core and oxideav-dts come from github.com/OxideAV, and this repository
# carries a copy rather than a dependency: oxideav-dts is published to crates.io
# only as a yanked 0.0.1, so a registry dependency cannot ship DTS at all, and
# the rest of the family moves fast enough that a floating version would change
# what a release decodes without anyone choosing it.
#
# AC-3 and E-AC-3 are NOT vendored. They were, until VuIO took the encoder
# somewhere upstream had not gone; that crate is now a fork we maintain, at
# crates/vuio-codec-ac3, and this script deliberately leaves it alone.
#
# The copies here are verbatim — same crate names, same file layout, no patches
# — so `diff -r crates/vendor/oxideav-dts/src <upstream>/src` stays meaningful
# and a refresh is a re-run of this script rather than a merge. Never hand-edit
# anything under crates/vendor: change it upstream and re-vendor.
#
# Usage:
#   ./scripts/vendor-oxideav.sh              # re-vendor at the pinned revisions
#   ./scripts/vendor-oxideav.sh --update     # move the pins to upstream HEAD
#
# After --update, review the diff and run the tests: these are decoders, and a
# regression is silent audio rather than a build failure.

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENDOR="$ROOT/crates/vendor"
CACHE="$ROOT/target/vendor-src"
SELF="$ROOT/scripts/vendor-oxideav.sh"

# The pinned revisions. `--update` rewrites these lines in place.
PIN_oxideav_core=defa866dffdd224424d75ac7a38be868723395a5
PIN_oxideav_dts=528203ed608223c5137843009054e05920af5c50

CRATES="oxideav-core oxideav-dts"

UPDATE=0
[ "${1:-}" = "--update" ] && UPDATE=1

command -v git >/dev/null 2>&1 || { echo -e "${RED}[ERROR]${NC} git is required."; exit 1; }
command -v rsync >/dev/null 2>&1 || { echo -e "${RED}[ERROR]${NC} rsync is required."; exit 1; }

mkdir -p "$CACHE" "$VENDOR"

for crate in $CRATES; do
    pin_var="PIN_$(echo "$crate" | tr '-' '_')"
    pin="${!pin_var}"
    src="$CACHE/$crate"
    dst="$VENDOR/$crate"

    echo -e "${BLUE}[INFO]${NC} $crate"

    if [ -d "$src/.git" ]; then
        git -C "$src" fetch --quiet origin
    else
        git clone --quiet "https://github.com/OxideAV/$crate.git" "$src"
    fi

    if [ "$UPDATE" = "1" ]; then
        branch="$(git -C "$src" symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null | sed 's#^origin/##')"
        branch="${branch:-master}"
        pin="$(git -C "$src" rev-parse "origin/$branch")"
        # Rewrite our own pin line so the new revision is committed with the code.
        sed -i.bak "s|^${pin_var}=.*|${pin_var}=${pin}|" "$SELF" && rm -f "$SELF.bak"
        echo -e "${YELLOW}[PIN]${NC}  $crate → ${pin:0:7}"
    fi

    git -C "$src" checkout --quiet "$pin"

    # Only src/ and the legal/provenance files. Upstream's tests/ directory
    # carries whole fixture corpora (350 KB for dts alone) and its benches pull
    # criterion; neither ships, and neither is ours to maintain.
    rm -rf "$dst"
    mkdir -p "$dst"
    rsync -a --delete "$src/src/" "$dst/src/"
    cp "$src/LICENSE" "$dst/LICENSE"
    cp "$src/README.md" "$dst/README.md"

    # The inline #[cfg(test)] modules inside src/ do come along, and some of them
    # `include_bytes!` a fixture out of tests/. Carry exactly those files — a few
    # KB — so `cargo test -p <crate>` still verifies the decoder we ship against
    # real bitstreams. Discovered by grep rather than listed, so a refresh that
    # adds a fixture picks it up instead of failing to build.
    grep -rhoE 'include_bytes!\("\.\./tests/[^"]+"\)' "$dst/src" 2>/dev/null \
      | sed -E 's|include_bytes!\("\.\./||; s|"\)$||' | sort -u \
      | while read -r fixture; do
            [ -f "$src/$fixture" ] || { echo -e "${RED}[ERROR]${NC} missing $crate/$fixture"; exit 1; }
            mkdir -p "$dst/$(dirname "$fixture")"
            cp "$src/$fixture" "$dst/$fixture"
            echo "        fixture: $fixture ($(wc -c < "$src/$fixture" | tr -d ' ') bytes)"
        done

    # The manifest is upstream's, with three mechanical changes: sibling
    # oxideav deps become path deps, publishing is off (we do not own these
    # names on crates.io), and lints are allowed because normalising 6 MB of
    # foreign code to our clippy settings would destroy the diffability that is
    # the whole reason for vendoring verbatim.
    awk '
        /^\[dev-dependencies\]/ { skip = 1; next }
        /^\[\[bench\]\]/        { skip = 1; next }
        /^\[/                   { skip = 0 }
        skip                    { next }
        /^name = / && !done_pub { print; print "publish = false"; done_pub = 1; next }
        { print }
    ' "$src/Cargo.toml" \
      | sed -E 's|^(oxideav-[a-z0-9]+) = "[^"]*"$|\1 = { path = "../\1" }|' \
      | sed -E 's|^(oxideav-[a-z0-9]+) = \{ version = "[^"]*", (.*)\}$|\1 = { path = "../\1", \2}|' \
      > "$dst/Cargo.toml"

    cat >> "$dst/Cargo.toml" <<'LINTS'

# Vendored verbatim — see scripts/vendor-oxideav.sh. Upstream does not build
# under this repository's `-D warnings`, and making it would mean carrying a
# patch set across every refresh.
[lints.rust]
warnings = "allow"

[lints.clippy]
all = "allow"
LINTS

    {
        echo "# Written by scripts/vendor-oxideav.sh. Do not edit, and do not"
        echo "# hand-edit the vendored sources beside it — change them upstream"
        echo "# and re-run the script."
        echo "source = \"https://github.com/OxideAV/$crate\""
        echo "commit = \"$pin\""
        echo "describe = \"$(git -C "$src" describe --tags --always 2>/dev/null || echo unknown)\""
        echo "version = \"$(sed -n 's/^version = "\(.*\)"$/\1/p' "$src/Cargo.toml" | head -1)\""
        echo "vendored_at = \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\""
        echo "patches = []"
    } > "$dst/VENDOR.toml"

    echo -e "        ${GREEN}✓${NC} ${pin:0:7} → crates/vendor/$crate ($(du -sh "$dst/src" | cut -f1))"
done

echo -e "${GREEN}[SUCCESS]${NC} Vendored $(echo $CRATES | wc -w | tr -d ' ') crates."

cd "$ROOT"
if git rev-parse --git-dir >/dev/null 2>&1; then
    if [ -z "$(git status --porcelain -- crates/vendor scripts/vendor-oxideav.sh)" ]; then
        echo -e "${BLUE}[INFO]${NC} Unchanged; nothing to commit."
    else
        echo -e "${BLUE}[INFO]${NC} The vendored tree changed — review and commit it."
    fi
fi
