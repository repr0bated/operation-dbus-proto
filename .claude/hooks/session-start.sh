#!/bin/bash
set -euo pipefail

# Only run in remote Claude Code environments
if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"

echo "==> Installing frontend dependencies (crates/)..."
cd "$PROJECT_DIR/crates"
if ! npm install; then
  echo "Warning: Failed to install npm dependencies"
fi

echo "==> Fetching Rust dependencies..."
cd "$PROJECT_DIR"
if ! cargo fetch; then
  echo "Warning: Failed to fetch Rust dependencies"
fi

echo "==> Session setup complete."
