#!/bin/bash
# Generate individual crate specifications

CRATES_DIR="/home/jeremy/git/operation-dbus/crates"
OUTPUT_DIR="$CRATES_DIR/SPECS"
mkdir -p "$OUTPUT_DIR"

# Array of all crates
CRATES=(
  "op-agents" "op-blockchain" "op-cache" "op-chat" "op-cognitive-mcp"
  "op-core" "op-dbus-mirror" "op-dbus-model" "op-deployment" "op-dynamic-loader"
  "op-execution-tracker" "op-gateway" "op-grpc-bridge" "op-http" "op-identity"
  "op-inspector" "op-introspection" "op-jsonrpc" "op-llm" "op-mcp-aggregator"
  "op-mcp-proxy" "op-mcp" "op-ml" "op-network" "op-plugins"
  "op-services" "op-state-store" "op-state" "op-tools" "op-web" "op-workflows"
)

# Generate spec for each crate
for i in "${!CRATES[@]}"; do
  CRATE="${CRATES[$i]}"
  NUM=$(printf "%02d" $((i+1)))
  SPEC_FILE="$OUTPUT_DIR/${NUM}-${CRATE}.md"
  
  echo "Generating spec for $CRATE..."
  
  cat > "$SPEC_FILE" << EOF
# $CRATE - Specification

## Overview
**Crate**: \`$CRATE\`  
**Location**: \`crates/$CRATE\`

## Quick Reference

### From Cargo.toml
\`\`\`toml
$(grep -A 5 '^\[package\]' "$CRATES_DIR/$CRATE/Cargo.toml" 2>/dev/null || echo "# Cargo.toml not found")
\`\`\`

### Source Structure
\`\`\`
$(find "$CRATES_DIR/$CRATE/src" -name "*.rs" 2>/dev/null | head -20 | sed 's|.*/crates/||' || echo "No source files found")
\`\`\`

### Key Dependencies
\`\`\`toml
$(grep -A 50 '^\[dependencies\]' "$CRATES_DIR/$CRATE/Cargo.toml" 2>/dev/null | grep -v '^\[' | head -20 || echo "# No dependencies section")
\`\`\`

### Binaries
\`\`\`toml
$(grep -A 3 '^\[\[bin\]\]' "$CRATES_DIR/$CRATE/Cargo.toml" 2>/dev/null || echo "# No binaries")
\`\`\`

### Features
\`\`\`toml
$(grep -A 10 '^\[features\]' "$CRATES_DIR/$CRATE/Cargo.toml" 2>/dev/null || echo "# No features")
\`\`\`

## Documentation Files
$(find "$CRATES_DIR/$CRATE" -maxdepth 1 -name "*.md" 2>/dev/null | xargs -I {} basename {} || echo "None")

## Module Structure
$(find "$CRATES_DIR/$CRATE/src" -name "*.rs" -type f 2>/dev/null | wc -l) Rust source files

### Main Modules
$(find "$CRATES_DIR/$CRATE/src" -maxdepth 1 -name "*.rs" -type f 2>/dev/null | xargs -I {} basename {} .rs | grep -v '^lib$\|^main$' | head -10 || echo "See source tree above")

## Purpose
$(grep '^description' "$CRATES_DIR/$CRATE/Cargo.toml" 2>/dev/null | cut -d'"' -f2 || echo "See Cargo.toml")

## Build Information
- **Edition**: $(grep '^edition' "$CRATES_DIR/$CRATE/Cargo.toml" 2>/dev/null | cut -d'"' -f2 || echo "workspace")
- **Version**: $(grep '^version' "$CRATES_DIR/$CRATE/Cargo.toml" 2>/dev/null | cut -d'"' -f2 || echo "workspace")
- **License**: $(grep '^license' "$CRATES_DIR/$CRATE/Cargo.toml" 2>/dev/null | cut -d'"' -f2 || echo "workspace")

## Related Crates
Internal dependencies:
$(grep 'path = "\.\.' "$CRATES_DIR/$CRATE/Cargo.toml" 2>/dev/null | sed 's/.*= { path = "\.\.\/\([^"]*\)".*/- \1/' || echo "None")

---
*Generated from crate analysis*
EOF

done

echo "Generated ${#CRATES[@]} specification files in $OUTPUT_DIR"
