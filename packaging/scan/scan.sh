#!/bin/bash
# Get the absolute path of the repository root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Always execute commands from the repository root
cd "$REPO_ROOT"

echo "=== Running SCANOSS Scan ==="
scanoss-py scan --settings ./scanoss.json -o ./scan-results.json .

echo "=== Running Cargo Audit ==="
cargo audit

echo "=== Running Cargo Deny Check ==="
cargo deny check