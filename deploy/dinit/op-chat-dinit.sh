#!/bin/sh
set -a
if [ -f /etc/op-dbus/environment ]; then
  . /etc/op-dbus/environment
fi

if [ -n "${OP_DBUS_GRPC_ADDR:-}" ] && \
   [ "${OP_DBUS_GRPC_ADDR#*://}" = "$OP_DBUS_GRPC_ADDR" ]; then
  OP_DBUS_GRPC_ADDR="http://${OP_DBUS_GRPC_ADDR}"
fi
set +a

exec /usr/local/sbin/op-chat
