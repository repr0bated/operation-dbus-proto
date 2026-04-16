#!/bin/sh
# Tear down all wg-quick-managed WireGuard interfaces.
set -eu

wg-quick down wg0  2>/dev/null || true
wg-quick down wgcf 2>/dev/null || true
