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
add_internal_port grpc-bridge

# --- Bring up internal ports ---
for port in priv_xray priv_warp priv_wg ovsbr0-mgmt ovsbr0-sock grpc-bridge; do
  if wait_for_iface "$port"; then
    ip link set "$port" up
  else
    echo "ovs-attach-ports: WARNING — $port not found in kernel" >&2
  fi
done

# --- Strip all addresses from ports that must have no IP ---
# These are L2-only OVS ports. Flush any v4/v6 addresses that may have
# leaked from races with systemd-networkd or previous runs.
for port in priv_wg priv_warp ovsbr0-sock; do
  ip -4 addr flush dev "$port" 2>/dev/null || true
  ip -6 addr flush dev "$port" 2>/dev/null || true
  # Also disable v6 at the kernel level to prevent auto-assignment
  sysctl -qw "net.ipv6.conf.${port}.disable_ipv6=1" 2>/dev/null || true
  echo "ovs-attach-ports: flushed v4+v6 from $port (no-IP port)"
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

# ovsbr0-mgmt: management IP for op-dbus control-plane (OpenFlow controller etc.)
MGMT_CIDR="${PRIVACY_MGMT_CIDR:-10.200.0.1/24}"
if ip link show ovsbr0-mgmt >/dev/null 2>&1; then
  ip addr flush dev ovsbr0-mgmt 2>/dev/null || true
  ip addr add "$MGMT_CIDR" dev ovsbr0-mgmt
  echo "ovs-attach-ports: ovsbr0-mgmt configured with ${MGMT_CIDR}"
fi

# grpc-bridge: dedicated OVS internal port for the gRPC server to bind on.
# All Incus containers on ovsbr0 reach gRPC via this IP.
GRPC_BRIDGE_CIDR="${PRIVACY_GRPC_BRIDGE_CIDR:-10.200.0.2/24}"
if ip link show grpc-bridge >/dev/null 2>&1; then
  ip addr flush dev grpc-bridge 2>/dev/null || true
  ip addr add "$GRPC_BRIDGE_CIDR" dev grpc-bridge
  echo "ovs-attach-ports: grpc-bridge configured with ${GRPC_BRIDGE_CIDR}"
fi

echo "ovs-attach-ports: done"
