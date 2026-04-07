#!/bin/sh
# Early-boot OVS seed.
# wgcf is brought up first by wg-quick-all. Seed then ensures only the two
# dynamic ports that matter before later services run:
# - wgcf, the WireGuard/WARP uplink attached to ovsbr0
# - grpc-bridge, the internal control-plane port on ovsbr0
#
# This script intentionally avoids ovs-vsctl. It uses native OVSDB JSON-RPC so
# the boot path follows the same command policy as the rest of the repo.
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

wait_for_iface() {
  iface="$1"
  limit="${2:-20}"
  i=0
  while [ "$i" -lt "$limit" ]; do
    if ip link show "$iface" >/dev/null 2>&1; then
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

port_exists() {
  rows="$(query_json "[\"Open_vSwitch\",{\"op\":\"select\",\"table\":\"Port\",\"where\":[[\"name\",\"==\",\"$1\"]]}]")"
  ! rows_empty "$rows"
}

bridge_exists() {
  rows="$(query_json "[\"Open_vSwitch\",{\"op\":\"select\",\"table\":\"Bridge\",\"where\":[[\"name\",\"==\",\"$BRIDGE\"]]}]")"
  ! rows_empty "$rows"
}

add_internal_port() {
  name="$1"
  if port_exists "$name"; then
    echo "op-ovsdb-seed: $name already present in OVSDB"
    return 0
  fi

  tx=$(cat <<EOF
["Open_vSwitch",
 {"op":"insert","table":"Interface","row":{"name":"$name","type":"internal"},"uuid-name":"iface"},
 {"op":"insert","table":"Port","row":{"name":"$name","interfaces":["set",[["named-uuid","iface"]]]},"uuid-name":"port"},
 {"op":"mutate","table":"Bridge","where":[["name","==","$BRIDGE"]],"mutations":[["ports","insert",["set",[["named-uuid","port"]]]]]}
]
EOF
)
  query_json "$tx" >/dev/null
  echo "op-ovsdb-seed: added internal port $name"
}

add_external_port() {
  name="$1"
  if port_exists "$name"; then
    echo "op-ovsdb-seed: $name already present in OVSDB"
    return 0
  fi

  tx=$(cat <<EOF
["Open_vSwitch",
 {"op":"insert","table":"Interface","row":{"name":"$name"},"uuid-name":"iface"},
 {"op":"insert","table":"Port","row":{"name":"$name","interfaces":["set",[["named-uuid","iface"]]]},"uuid-name":"port"},
 {"op":"mutate","table":"Bridge","where":[["name","==","$BRIDGE"]],"mutations":[["ports","insert",["set",[["named-uuid","port"]]]]]}
]
EOF
)
  query_json "$tx" >/dev/null
  echo "op-ovsdb-seed: added external port $name"
}

reconfigure_link_by_name() {
  link_name="$1"

  command -v busctl >/dev/null 2>&1 || return 0

  link_info="$(busctl call \
    org.freedesktop.network1 \
    /org/freedesktop/network1 \
    org.freedesktop.network1.Manager \
    GetLinkByName \
    s \
    "$link_name" 2>/dev/null)" || return 0

  set -- $link_info
  link_index="$2"
  [ -n "$link_index" ] || return 0

  echo "op-ovsdb-seed: reconfiguring $link_name via networkd D-Bus (index $link_index)"
  busctl call \
    org.freedesktop.network1 \
    /org/freedesktop/network1 \
    org.freedesktop.network1.Manager \
    ReconfigureLink \
    i \
    "$link_index" >/dev/null 2>&1 || true
}

if ! wait_for_socket; then
  echo "op-ovsdb-seed: OVSDB socket $SOCKET_PATH unavailable after timeout" >&2
  exit 1
fi

if ! bridge_exists; then
  echo "op-ovsdb-seed: bridge $BRIDGE not present in OVSDB yet" >&2
  exit 1
fi

add_internal_port "grpc-bridge"

if wait_for_iface "wgcf" 30; then
  add_external_port "wgcf"
  ip link set wgcf up >/dev/null 2>&1 || true
else
  echo "op-ovsdb-seed: WARNING - wgcf interface not present yet" >&2
fi

# Ask systemd-networkd to immediately re-evaluate the bridge and helper port
# after the OVSDB rows exist so boot does not wait for a later manual nudge.
reconfigure_link_by_name "$BRIDGE"
reconfigure_link_by_name "grpc-bridge"

exit 0
