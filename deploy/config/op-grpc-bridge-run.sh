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
[ -r /etc/op-dbus/netmaker-broker.env ] && . /etc/op-dbus/netmaker-broker.env
set +a

# One TLS TCP door: :8090 demuxes MQTT/WebSocket `/mqtt`, gRPC-Web, and native
# gRPC. Mesh/svc0 publishers relay back to this loopback listener.
# Do not bind :50051 — same routes already live on :8090 and the sockets.
export ZEROCLAW_BIND_ADDR="127.0.0.1:${NETMAKER_BROKER_PORT:-8090}"
export GRPC_BIND="$ZEROCLAW_BIND_ADDR"
export EMQX_BROKER_SOCKET="${NETMAKER_BROKER_SOCKET:-/run/ghostbridge/NetMaker/broker.sock}"
export ZEROCLAW_UNIX_SOCKET="${ZEROCLAW_UNIX_SOCKET:-/run/opdbus/grpc.sock}"
export ZEROCLAW_TLS_CERT_FILE="${ZEROCLAW_TLS_CERT_FILE:-/etc/op-dbus/tls/tonic-svc0.crt}"
export ZEROCLAW_TLS_KEY_FILE="${ZEROCLAW_TLS_KEY_FILE:-/etc/op-dbus/tls/tonic-svc0.key}"
export RUST_LOG="${GRPC_RUST_LOG:-info}"
export COGNITIVE_MCP_MCP_URL="${COGNITIVE_MCP_MCP_URL:-http://10.200.0.2:8090/mcp}"

exec /usr/local/bin/op-grpc-bridge
