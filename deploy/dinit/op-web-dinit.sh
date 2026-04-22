#!/bin/sh
set -a
if [ -f /etc/op-dbus/environment ]; then
  . /etc/op-dbus/environment
fi

: "${OP_WEB_TOOL_SOURCE:=op-dbus}"
: "${OP_DBUS_WEB_HOST:=127.0.0.1}"
: "${OP_DBUS_WEB_PORT:=8081}"
: "${OP_WEB_REMOTE_TOOL_URL:=http://${OP_DBUS_WEB_HOST}:${OP_DBUS_WEB_PORT}}"
: "${PRIVACY_CONTAINER_STORAGE_POOL:=registration}"
: "${WG_SERVER_PUBKEY:=+7AlLRCx0cqWwV+cgP7UwA6i//7v0YqA32f8Rbo07zE=}"
: "${WG_SERVER_ENDPOINT:=148.113.204.83:51820}"
: "${WG_INTERFACE:=wg0}"

set +a

# Wait for op-dbus HTTP API to be ready before starting op-web-server.
# op-dbus signals process start to dinit before its HTTP listener binds.
_url="${OP_WEB_REMOTE_TOOL_URL%/}/api/tools"
i=0
while [ "$i" -lt 30 ]; do
  if curl -sf --max-time 2 "$_url" >/dev/null 2>&1; then
    break
  fi
  i=$((i + 1))
  sleep 1
done
unset _url i

if [ -x /usr/local/sbin/op-web-server ]; then
  exec /usr/local/sbin/op-web-server
fi

exec /usr/local/bin/op-web-server
