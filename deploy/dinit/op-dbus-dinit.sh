#!/bin/sh
set -a
if [ -f /etc/op-dbus/environment ]; then
  . /etc/op-dbus/environment
fi
set +a

if [ -x /usr/local/sbin/op-dbus ]; then
  exec /usr/local/sbin/op-dbus
fi

exec /usr/local/bin/op-dbus
