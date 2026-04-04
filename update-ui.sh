#!/usr/bin/env bash
set -e

echo "🚀 Updating embedded UI crate from operation-dashboard-ui submodule..."

# Define directories
SOURCE_DIR="operation-dashboard-ui"
CRATE_UI_DIR="crates/op-web/ui"

# Ensure the submodule exists and is populated
if [ ! -d "$SOURCE_DIR/src" ]; then
    echo "❌ Error: Submodule source $SOURCE_DIR/src not found."
    echo "Please ensure the submodule is initialized: git submodule update --init"
    exit 1
fi

echo "📦 Syncing source files from $SOURCE_DIR to $CRATE_UI_DIR..."
# Sync the src directory, replacing the old crate src completely
rsync -av --delete "$SOURCE_DIR/src/" "$CRATE_UI_DIR/src/"

# Sync configuration files that might have been updated by Lovable
rsync -av "$SOURCE_DIR/components.json" "$CRATE_UI_DIR/components.json" 2>/dev/null || true
rsync -av "$SOURCE_DIR/tailwind.config.ts" "$CRATE_UI_DIR/tailwind.config.ts" 2>/dev/null || true
rsync -av "$SOURCE_DIR/tsconfig.json" "$CRATE_UI_DIR/tsconfig.json" 2>/dev/null || true
rsync -av "$SOURCE_DIR/vite.config.ts" "$CRATE_UI_DIR/vite.config.ts" 2>/dev/null || true
rsync -av "$SOURCE_DIR/package.json" "$CRATE_UI_DIR/package.json" 2>/dev/null || true
rsync -av "$SOURCE_DIR/package-lock.json" "$CRATE_UI_DIR/package-lock.json" 2>/dev/null || true

echo "🔍 Confirming schema and wiring dependencies..."
cd "$CRATE_UI_DIR"

# Ensure required libraries for Generative UI and React Flow are installed
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

# Ensure all standard dependencies are installed
echo "📦 Installing all dependencies..."
npm install --silent --legacy-peer-deps

echo "🏗️ Building the embedded UI..."
npm run build

# Navigate back to root
cd ../../..

echo "============================================================"
echo "✅ UI successfully embedded into $CRATE_UI_DIR/dist!"
echo "Rust's 'rust-embed' will now automatically include these"
echo "compiled assets the next time you run 'cargo build'."
echo "============================================================"
