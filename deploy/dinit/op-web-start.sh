#!/bin/bash
# op-web-start.sh - Wait for op-dbus to be ready before starting op-web
# 
# This wrapper prevents op-web from crashing when op-dbus isn't fully ready.
# It waits for the op-dbus API to respond on port 8080 before starting op-web.

set -e

if [ -f /etc/op-dbus/environment ]; then
    # shellcheck disable=SC1091
    . /etc/op-dbus/environment
fi

: "${OP_DBUS_WEB_HOST:=127.0.0.1}"
: "${OP_DBUS_WEB_PORT:=8081}"
: "${OP_WEB_REMOTE_TOOL_URL:=http://${OP_DBUS_WEB_HOST}:${OP_DBUS_WEB_PORT}}"

tool_url="${OP_WEB_REMOTE_TOOL_URL%/}/api/tools"

echo "[op-web-start] Waiting for op-dbus to be ready..."

# Wait up to 30 seconds for op-dbus HTTP API to be reachable.
for i in {1..30}; do
    if curl -sf --max-time 2 "$tool_url" >/dev/null 2>&1; then
        echo "[op-web-start] op-dbus is ready, starting op-web..."
        exec /usr/local/sbin/op-web-dinit.sh "$@"
    fi
    echo "[op-web-start] Waiting for op-dbus at $tool_url... ($i/30)"
    sleep 1
done

echo "[op-web-start] Timeout waiting for op-dbus, starting anyway..."
exec /usr/local/sbin/op-web-dinit.sh "$@"
