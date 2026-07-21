#!/bin/sh
# Compatibility wrapper.  The AF_XDP cutover must use the dispatcher-aware
# script with rollback, not the older netdev-only flow.
set -eu

DIR="$(cd "$(dirname "$0")" && pwd)"
exec "$DIR/cutover-afxdp.sh" "$@"
