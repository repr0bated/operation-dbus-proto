#!/bin/sh
set -a
if [ -f /etc/op-dbus/environment ]; then
  . /etc/op-dbus/environment
fi

: "${OP_WEB_TOOL_SOURCE:=op-dbus}"
: "${OP_WEB_PORT:=8081}"
: "${PORT:=$OP_WEB_PORT}"

if [ -n "${OP_DBUS_GRPC_ADDR:-}" ] && \
   [ "${OP_DBUS_GRPC_ADDR#*://}" = "$OP_DBUS_GRPC_ADDR" ]; then
  OP_DBUS_GRPC_ADDR="http://${OP_DBUS_GRPC_ADDR}"
fi

if [ "$OP_WEB_TOOL_SOURCE" = "op-dbus" ] && [ -z "${OP_WEB_REMOTE_TOOL_URL:-}" ]; then
  if [ -n "${OP_DBUS_WEB_CLIENT_URL:-}" ]; then
    OP_WEB_REMOTE_TOOL_URL="$OP_DBUS_WEB_CLIENT_URL"
  elif [ -n "${OP_DBUS_WEB_CLIENT_HOST:-}" ] && [ -n "${OP_DBUS_WEB_PORT:-}" ]; then
    OP_WEB_REMOTE_TOOL_URL="http://${OP_DBUS_WEB_CLIENT_HOST}:${OP_DBUS_WEB_PORT}"
  else
    echo "op-web: OP_WEB_REMOTE_TOOL_URL or OP_DBUS_WEB_CLIENT_URL is required" >&2
    exit 1
  fi
fi

: "${PRIVACY_CONTAINER_STORAGE_POOL:=registration}"
: "${WG_SERVER_PUBKEY:?WG_SERVER_PUBKEY is required}"
: "${WG_SERVER_ENDPOINT:?WG_SERVER_ENDPOINT is required}"
: "${WG_INTERFACE:=wg0}"

set +a

exec /usr/local/sbin/op-web-server
