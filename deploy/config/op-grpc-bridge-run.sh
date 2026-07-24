#!/bin/sh
exec 2>&1

wait_dep() {
    sv start "$1" >/dev/null 2>&1 || true
    i=0
    until sv check "$1" >/dev/null 2>&1; do
        i=$((i+1))
        [ "$i" -ge 90 ] && { echo "dependency $1 not ready after 90s" >&2; exit 1; }
        sleep 1
    done
}

wait_dep opdbus-rundirs
wait_dep op-session-bus
wait_dep ovsbr0-addr

set -a
[ -r /etc/op-dbus/environment ] && . /etc/op-dbus/environment
set +a

# Hardcode bind address - ignore all env overrides. Single port only:
# 50052 is op-cognitive-mcp's own gRPC port (see CLAUDE.md crate map) —
# binding it here too causes a genuine, permanent port conflict and crash
# loop, not a transient race (confirmed live 2026-07-24).
export ZEROCLAW_BIND_ADDR="0.0.0.0:8090"
export GRPC_BIND="0.0.0.0:8090"
export ZEROCLAW_UNIX_SOCKET="${ZEROCLAW_UNIX_SOCKET:-/run/opdbus/grpc.sock}"
unset ZEROCLAW_TLS_BIND_ADDR
export RUST_LOG="${GRPC_RUST_LOG:-info}"

exec /usr/local/bin/op-grpc-bridge
