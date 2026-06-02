#!/usr/bin/env bash
set -uo pipefail

# Boot-safety guard: a hung `incus stop` inside `op-xdp-wg prepare` previously
# blocked `s6-rc -up change default`, wedging boot and leaving only one tty.
# Bound the whole prepare so the s6 oneshot's `up` always returns; on timeout we
# exit 0 so the rest of the default bringup proceeds regardless.
timeout -s KILL 120 /usr/local/sbin/op-xdp-wg prepare || \
    echo "op-xdp-final: prepare timed out or failed (rc=$?), continuing boot" >&2
exit 0
