#!/usr/bin/env bash
set -euo pipefail

# scripts/publish-crates.sh
# Publishes all VuIO crates to crates.io in strict topological dependency order.

DRY_RUN=""
WAIT_SECS=45

for arg in "$@"; do
    case "$arg" in
        --dry-run)
            DRY_RUN="--dry-run"
            WAIT_SECS=2
            ;;
        --help|-h)
            echo "Usage: $0 [--dry-run]"
            echo "  --dry-run: Test packaging and validation without actually uploading to crates.io."
            exit 0
            ;;
        *)
            echo "Unknown argument: $arg"
            exit 1
            ;;
    esac
done

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

publish_crate() {
    local package="$1"
    local allow_dirty="${2:-}"
    echo "============================================================"
    echo ">> Publishing $package (dry-run: ${DRY_RUN:-false})"
    echo "============================================================"
    if [ -n "$allow_dirty" ]; then
        cargo publish -p "$package" ${DRY_RUN} --allow-dirty
    else
        cargo publish -p "$package" ${DRY_RUN}
    fi
}

wait_step() {
    local msg="$1"
    echo ">> Waiting ${WAIT_SECS}s for crates.io index update: $msg..."
    sleep "$WAIT_SECS"
}

# Ensure git working tree is clean unless dry-run
if [ -z "$DRY_RUN" ]; then
    if [ -n "$(git status --porcelain)" ]; then
        echo "Error: Working directory has uncommitted changes. Please commit or stash before publishing."
        exit 1
    fi
fi

echo "Starting VuIO Crates.io publication sequence..."

# -------------------------------------------------------------
# Layer 1: Foundation crates (no local workspace dependencies)
# -------------------------------------------------------------
publish_crate "vuio-codec-core" ${DRY_RUN:+--allow-dirty}
publish_crate "vuio-cast" ${DRY_RUN:+--allow-dirty}
publish_crate "vuio-web" ${DRY_RUN:+--allow-dirty}

if [ -z "$DRY_RUN" ]; then
    wait_step "vuio-codec-core, vuio-cast, and vuio-web available on crates.io"
fi

# -------------------------------------------------------------
# Layer 2: Codec crates (depend on vuio-codec-core)
# -------------------------------------------------------------
publish_crate "vuio-codec-ac3" ${DRY_RUN:+--allow-dirty}
publish_crate "vuio-codec-dts" ${DRY_RUN:+--allow-dirty}

if [ -z "$DRY_RUN" ]; then
    wait_step "vuio-codec-ac3 and vuio-codec-dts available on crates.io"
fi

# -------------------------------------------------------------
# Layer 3: Core runtime (depends on all layers above)
# -------------------------------------------------------------
publish_crate "vuio-core" ${DRY_RUN:+--allow-dirty}

if [ -z "$DRY_RUN" ]; then
    wait_step "vuio-core available on crates.io"
fi

# -------------------------------------------------------------
# Layer 4: CLI Application
# -------------------------------------------------------------
publish_crate "vuio-cli" ${DRY_RUN:+--allow-dirty}

echo "============================================================"
echo ">> All VuIO crates successfully published to crates.io!"
echo "============================================================"
