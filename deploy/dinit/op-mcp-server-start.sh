#!/bin/sh
# Source environment variables if needed
source /etc/dinit.d/environment.op-dbus 2>/dev/null || true

exec /home/jeremy/git/operation-dbus-proto/target/debug/op-mcp-server