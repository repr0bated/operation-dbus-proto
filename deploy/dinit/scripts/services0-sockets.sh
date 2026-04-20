#!/bin/sh
set -eu

SOCKET_DIR="${SERVICES0_SOCKET_DIR:-/run/services0}"
SOCKET_GROUP="${SERVICES0_SOCKET_GROUP:-_nginx}"
SOCKET_GROUP_ID="${SERVICES0_SOCKET_GID:-980}"

if getent group "$SOCKET_GROUP" >/dev/null 2>&1; then
  group="$SOCKET_GROUP"
else
  group="$SOCKET_GROUP_ID"
fi

install -d -o root -g "$group" -m 0770 "$SOCKET_DIR"

if [ -S "$SOCKET_DIR/gateway.sock" ]; then
  chgrp "$group" "$SOCKET_DIR/gateway.sock"
  chmod 0660 "$SOCKET_DIR/gateway.sock"
fi
