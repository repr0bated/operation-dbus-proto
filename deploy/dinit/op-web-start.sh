#!/bin/bash
# op-web-start.sh - Wait for op-dbus to be ready before starting op-web
# 
# This wrapper prevents op-web from crashing when op-dbus isn't fully ready.
# It waits for the op-dbus API to respond on port 8080 before starting op-web.

set -e

echo "[op-web-start] Waiting for op-dbus to be ready..."

# Wait up to 30 seconds for op-dbus port to be open
for i in {1..30}; do
    if curl -s http://localhost:8080/api/tools >/dev/null 2>&1; then
        echo "[op-web-start] op-dbus is ready, starting op-web..."
        exec /usr/local/sbin/op-web-dinit.sh "$@"
    fi
    echo "[op-web-start] Waiting for op-dbus... ($i/30)"
    sleep 1
done

echo "[op-web-start] Timeout waiting for op-dbus, starting anyway..."
exec /usr/local/sbin/op-web-dinit.sh "$@"
