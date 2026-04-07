#!/bin/sh
set -eu

wait_for_dinit_dbus() {
  i=0
  while [ "$i" -lt 30 ]; do
    if busctl --system status org.chimera.dinit >/dev/null 2>&1; then
      return 0
    fi
    i=$((i + 1))
    sleep 1
  done
  return 1
}

start_service_via_dbus() {
  service="$1"
  output="$(gdbus call \
    --system \
    --dest org.chimera.dinit \
    --object-path /org/chimera/dinit \
    --method org.chimera.dinit.Manager.StartService \
    "$service" \
    false 2>&1)" && return 0

  printf '%s' "$output" | grep -q 'org.chimera.dinit.Error.ServiceAlready' && return 0
  printf '%s\n' "$output" >&2
  return 1
}

wait_for_ovs_socket() {
  i=0
  while [ "$i" -lt 30 ]; do
    if [ -S /run/openvswitch/db.sock ] || [ -S /var/run/openvswitch/db.sock ]; then
      return 0
    fi
    i=$((i + 1))
    sleep 1
  done
  return 1
}

if ! wait_for_dinit_dbus; then
  echo "op-ovs-services: dinit D-Bus manager unavailable after timeout" >&2
  exit 1
fi

start_service_via_dbus "ovsdb-server"
start_service_via_dbus "ovs-vswitchd"

if ! wait_for_ovs_socket; then
  echo "op-ovs-services: OVSDB socket did not appear after starting services" >&2
  exit 1
fi

echo "op-ovs-services: ovsdb-server + ovs-vswitchd ready"
