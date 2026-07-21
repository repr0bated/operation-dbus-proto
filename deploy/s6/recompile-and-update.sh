#!/bin/sh
# Compatibility shim: Artix hosts now use runit. Forward to the runit helper.
set -eu
SCRIPT_PATH=$(readlink -f "$0" 2>/dev/null || printf '%s' "$0")
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$SCRIPT_PATH")" && pwd)
exec sh "$SCRIPT_DIR/../runit/recompile-and-update.sh" "$@"
