#!/bin/sh
# Add all ports to ovsbr0 via native OVSDB JSON-RPC.
# netplan creates the bare bridge; this script adds wgcf + all internal ports.
# IP assignment via ip(8) after ports are up.
set -eu

BRIDGE="ovsbr0"
SOCKET_PATH="/var/run/openvswitch/db.sock"
if [ -S /run/openvswitch/db.sock ]; then
  SOCKET_PATH="/run/openvswitch/db.sock"
fi
SERVER="unix:$SOCKET_PATH"

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

wait_for_iface() {
  iface="$1"
  i=0
  while [ "$i" -lt 20 ]; do
    if ip link show "$iface" >/dev/null 2>&1; then
      return 0
    fi
    i=$((i + 1))
    sleep 1
  done
  return 1
}

add_internal_port() {
  name="$1"
  if port_exists "$name"; then
    echo "ovs-attach-ports: $name already in OVSDB"
    return
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
  echo "ovs-attach-ports: added internal port $name"
}

add_external_port() {
  name="$1"
  if port_exists "$name"; then
    echo "ovs-attach-ports: $name already in OVSDB"
    return
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
  echo "ovs-attach-ports: added external port $name"
}

# --- wgcf (WireGuard/WARP tunnel — must exist before this runs) ---
if wait_for_iface wgcf; then
  add_external_port wgcf
  ip link set wgcf up
else
  echo "ovs-attach-ports: WARNING — wgcf not found" >&2
fi

# --- Internal ports ---
add_internal_port priv_xray
add_internal_port priv_warp
add_internal_port priv_wg
add_internal_port ovsbr0-mgmt
add_internal_port ovsbr0-sock
add_internal_port ovsbr0-uplink

# --- Bring up internal ports and assign IPs ---
for port in priv_xray priv_warp priv_wg ovsbr0-mgmt ovsbr0-sock; do
  if wait_for_iface "$port"; then
    ip link set "$port" up
  else
    echo "ovs-attach-ports: WARNING — $port not found in kernel" >&2
  fi
done

# priv_xray: Xray client public identity
if ip link show priv_xray >/dev/null 2>&1; then
  ip addr flush dev priv_xray 2>/dev/null || true
  ip addr add 15.235.37.41/32 dev priv_xray
  ip route add 148.113.204.1 dev priv_xray onlink 2>/dev/null || true
  ip route replace default via 148.113.204.1 dev priv_xray metric 4096 onlink 2>/dev/null || true
  ip rule add from 15.235.37.41/32 table 200 priority 100 2>/dev/null || true
  ip route replace default via 148.113.204.1 dev priv_xray table 200 metric 100 onlink 2>/dev/null || true
  echo "ovs-attach-ports: priv_xray configured with 15.235.37.41"
fi

echo "ovs-attach-ports: done"
