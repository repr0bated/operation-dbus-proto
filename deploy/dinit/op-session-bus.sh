#!/bin/sh
set -eu

mkdir -p /run/op-dbus
exec /usr/bin/dbus-daemon --session --nofork --address=unix:path=/run/op-dbus/session-bus
