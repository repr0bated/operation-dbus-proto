#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEFAULT_UI_REPO="https://github.com/3tched-com/operation-dashboard-ui.git"
UI_REPO="$DEFAULT_UI_REPO"
UI_REF="${OPERATION_DASHBOARD_UI_REF:-}"
CRATE_UI_DIR="$ROOT_DIR/crates/op-web/ui"
SOURCE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/operation-dashboard-ui.XXXXXX")"

cleanup() {
    rm -rf "$SOURCE_DIR"
}
trap cleanup EXIT

repo_slug_from_url() {
    local url="$1"
    url="${url#https://github.com/}"
    url="${url%.git}"
    echo "$url"
}

echo "🚀 Updating embedded UI crate from a fresh operation-dashboard-ui clone..."
echo "📥 Cloning UI repo from $UI_REPO"

if command -v gh >/dev/null 2>&1; then
    UI_REPO_SLUG="$(repo_slug_from_url "$UI_REPO")"
    echo "🔐 Using GitHub CLI auth flow: gh repo clone $UI_REPO_SLUG"
     gh repo clone "$UI_REPO_SLUG" "$SOURCE_DIR" -- --quiet
else
    echo "ℹ️ GitHub CLI not found; falling back to git clone"
    git clone "$UI_REPO" "$SOURCE_DIR"
fi

if [ -n "$UI_REF" ]; then
    echo "🔀 Checking out UI ref $UI_REF"
    git -C "$SOURCE_DIR" checkout "$UI_REF"
fi

if [ ! -d "$SOURCE_DIR/src" ]; then
    echo "❌ Error: cloned UI source is missing $SOURCE_DIR/src"
    exit 1
fi

echo "📦 Syncing source files from clone to $CRATE_UI_DIR..."
rsync -av --delete "$SOURCE_DIR/src/" "$CRATE_UI_DIR/src/"
rsync -av --delete "$SOURCE_DIR/public/" "$CRATE_UI_DIR/public/" 2>/dev/null || true
rsync -av "$SOURCE_DIR/index.html" "$CRATE_UI_DIR/index.html" 2>/dev/null || true
rsync -av "$SOURCE_DIR/components.json" "$CRATE_UI_DIR/components.json" 2>/dev/null || true
rsync -av "$SOURCE_DIR/tailwind.config.ts" "$CRATE_UI_DIR/tailwind.config.ts" 2>/dev/null || true
rsync -av "$SOURCE_DIR/tsconfig.json" "$CRATE_UI_DIR/tsconfig.json" 2>/dev/null || true
rsync -av "$SOURCE_DIR/tsconfig.app.json" "$CRATE_UI_DIR/tsconfig.app.json" 2>/dev/null || true
rsync -av "$SOURCE_DIR/tsconfig.node.json" "$CRATE_UI_DIR/tsconfig.node.json" 2>/dev/null || true
rsync -av "$SOURCE_DIR/vite.config.ts" "$CRATE_UI_DIR/vite.config.ts" 2>/dev/null || true
rsync -av "$SOURCE_DIR/vitest.config.ts" "$CRATE_UI_DIR/vitest.config.ts" 2>/dev/null || true
rsync -av "$SOURCE_DIR/postcss.config.js" "$CRATE_UI_DIR/postcss.config.js" 2>/dev/null || true
rsync -av "$SOURCE_DIR/eslint.config.js" "$CRATE_UI_DIR/eslint.config.js" 2>/dev/null || true
rsync -av "$SOURCE_DIR/package.json" "$CRATE_UI_DIR/package.json" 2>/dev/null || true
rsync -av "$SOURCE_DIR/package-lock.json" "$CRATE_UI_DIR/package-lock.json" 2>/dev/null || true
rsync -av "$SOURCE_DIR/bun.lockb" "$CRATE_UI_DIR/bun.lockb" 2>/dev/null || true
rsync -av "$SOURCE_DIR/.env" "$CRATE_UI_DIR/.env" 2>/dev/null || true

echo "🔍 Confirming schema and wiring dependencies..."
cd "$CRATE_UI_DIR"

if [ -d dist ] && [ ! -w dist ]; then
    DIST_BACKUP="dist.root-owned.$(date +%Y%m%d%H%M%S)"
    echo "⚠️ Existing dist directory is not writable; moving it to $DIST_BACKUP so the build can continue."
    mv dist "$DIST_BACKUP"
fi

DEPS_TO_ADD=""
if ! grep -q '"@json-render/react"' package.json; then
    DEPS_TO_ADD="$DEPS_TO_ADD @json-render/react @json-render/core"
fi
if ! grep -q '"reactflow"' package.json; then
    DEPS_TO_ADD="$DEPS_TO_ADD reactflow"
fi

if [ -n "$DEPS_TO_ADD" ]; then
    echo "📥 Installing missing dependencies: $DEPS_TO_ADD"
    npm install --save --legacy-peer-deps $DEPS_TO_ADD
else
    echo "✅ UI dependencies already wired."
fi

echo "📦 Installing all dependencies..."
if [ -f package-lock.json ]; then
    npm ci --silent --legacy-peer-deps
else
    npm install --silent --legacy-peer-deps
fi

echo "🏗️ Building the embedded UI..."
npm run build

cd "$ROOT_DIR"

echo "🧹 Invalidating Cargo cache for op-web..."
touch crates/op-web/build.rs
touch crates/op-web/src/embedded_ui.rs

echo "============================================================"
echo "✅ UI successfully synced into $CRATE_UI_DIR and built at $CRATE_UI_DIR/dist"
echo "Rust now embeds the crate-local dist directory on the next cargo build."
echo "============================================================"
