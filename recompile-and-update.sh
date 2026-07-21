#!/bin/sh
# Compatibility entrypoint at repo root — delegates to deploy/runit/.
set -eu
SCRIPT_PATH=$(readlink -f "$0" 2>/dev/null || printf '%s' "$0")
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$SCRIPT_PATH")" && pwd)
exec sh "$SCRIPT_DIR/deploy/runit/recompile-and-update.sh" "$@"
