#!/bin/bash
set -euo pipefail

# Only run in remote Claude Code environments
if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"

echo "==> Installing frontend dependencies (crates/)..."
cd "$PROJECT_DIR/crates"
npm install

echo "==> Fetching Rust dependencies..."
cd "$PROJECT_DIR"
cargo fetch

echo "==> Session setup complete."
