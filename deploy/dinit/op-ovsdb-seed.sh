#!/bin/sh
# First-boot safety net: ensure OVSDB has the bridge row before netplan runs.
# netplan apply is the real execution path for the full topology.
# This only creates the bare bridge if OVSDB is completely empty.
set -eu

BRIDGE="ovsbr0"
SOCKET_PATH="/var/run/openvswitch/db.sock"
if [ -S /run/openvswitch/db.sock ]; then
  SOCKET_PATH="/run/openvswitch/db.sock"
fi
SERVER="unix:$SOCKET_PATH"

wait_for_socket() {
  i=0
  while [ "$i" -lt 60 ]; do
    if [ -S "$SOCKET_PATH" ]; then
      return 0
    fi
    i=$((i + 1))
    sleep 1
  done
  return 1
}

query_json() {
  ovsdb-client transact "$SERVER" "$1"
}

rows_empty() {
  printf '%s' "$1" | tr -d '[:space:]' | grep -F '"rows":[]' >/dev/null 2>&1
}

if ! wait_for_socket; then
  echo "op-ovsdb-seed: OVSDB socket $SOCKET_PATH unavailable after timeout" >&2
  exit 1
fi

bridge_rows="$(query_json "[\"Open_vSwitch\",{\"op\":\"select\",\"table\":\"Bridge\",\"where\":[[\"name\",\"==\",\"$BRIDGE\"]]}]")"
if ! rows_empty "$bridge_rows"; then
  echo "op-ovsdb-seed: bridge $BRIDGE already present in OVSDB"
  exit 0
fi

echo "op-ovsdb-seed: bridge $BRIDGE not in OVSDB — netplan apply will create it"
exit 0
