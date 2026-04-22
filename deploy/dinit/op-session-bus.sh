#!/bin/sh
set -eu

mkdir -p /run/op-dbus
rm -f /run/op-dbus/session-bus
exec /usr/bin/dbus-daemon --session --nofork --print-address=4 --address=unix:path=/run/op-dbus/session-bus
