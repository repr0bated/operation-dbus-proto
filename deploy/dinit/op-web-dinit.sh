#!/bin/sh
set -a
if [ -f /etc/op-dbus/environment ]; then
  . /etc/op-dbus/environment
fi

: "${OP_WEB_TOOL_SOURCE:=op-dbus}"
: "${OP_WEB_REMOTE_TOOL_URL:=http://127.0.0.1:8081}"
: "${PRIVACY_CONTAINER_STORAGE_POOL:=registration}"
: "${WG_SERVER_PUBKEY:=+7AlLRCx0cqWwV+cgP7UwA6i//7v0YqA32f8Rbo07zE=}"
: "${WG_SERVER_ENDPOINT:=148.113.204.83:51820}"
: "${WG_INTERFACE:=wg0}"

set +a

exec /usr/local/sbin/op-web-server
