#!/bin/sh
set -eu

if [ -f /etc/op-dbus/environment ]; then
  # shellcheck disable=SC1091
  . /etc/op-dbus/environment
fi

DBUS_DEST="${OP_DBUS_DEST:-org.opdbus}"
OVS_PATH="${OP_DBUS_OVS_PATH:-/org/opdbus/ovsdb}"
OVS_IFACE="${OP_DBUS_OVS_IFACE:-org.opdbus.OvsdbV1}"
MIRROR_DEST="${OP_DBUS_MIRROR_DEST:-org.opdbus.v1}"
MIRROR_PATH="${OP_DBUS_MIRROR_PATH:-/org/opdbus/v1}"
MIRROR_IFACE="${OP_DBUS_MIRROR_IFACE:-org.opdbus.MirrorV1}"
BRIDGE="ovsbr0"
UPLINK="ens3"
BUSCTL_TIMEOUT_SECS="${OP_DBUS_BUSCTL_TIMEOUT_SECS:-3}"

wait_for_opdbus() {
  i=0
  while [ "$i" -lt 60 ]; do
    if busctl --system status "$DBUS_DEST" >/dev/null 2>&1; then
      return 0
    fi
    i=$((i + 1))
    sleep 1
  done
  return 1
}

call_dbus() {
  busctl --system --timeout="$BUSCTL_TIMEOUT_SECS" call "$@"
}

wait_for_kernel_link() {
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

bridge_exists() {
  call_dbus "$DBUS_DEST" "$OVS_PATH" "$OVS_IFACE" BridgeExists s "$BRIDGE" 2>/dev/null | grep -q "true"
}

port_present() {
  call_dbus "$DBUS_DEST" "$OVS_PATH" "$OVS_IFACE" ListPorts s "$BRIDGE" 2>/dev/null | grep -F "\"$UPLINK\"" >/dev/null 2>&1
}

if ! wait_for_opdbus; then
  echo "op-ovsdb-bridge: D-Bus service $DBUS_DEST unavailable after timeout" >&2
  exit 1
fi

if ! bridge_exists; then
  echo "op-ovsdb-bridge: expected persisted OVS bridge $BRIDGE is missing from OVSDB" >&2
  echo "op-ovsdb-bridge: seed OVSDB once via JSON-RPC before booting this profile" >&2
  exit 1
fi

if ! port_present; then
  echo "op-ovsdb-bridge: expected persisted uplink $UPLINK is missing from bridge $BRIDGE" >&2
  echo "op-ovsdb-bridge: seed OVSDB once via JSON-RPC before booting this profile" >&2
  exit 1
fi

if wait_for_kernel_link "$BRIDGE"; then
  echo "op-ovsdb-bridge: kernel link $BRIDGE is present"
else
  echo "op-ovsdb-bridge: kernel link $BRIDGE did not appear after OVS restore" >&2
  exit 1
fi

# MirrorV1 does not always expose Introspectable reliably, so probe by method call.
if call_dbus "$MIRROR_DEST" "$MIRROR_PATH" "$MIRROR_IFACE" GetStats >/dev/null 2>&1; then
  call_dbus "$MIRROR_DEST" "$MIRROR_PATH" "$MIRROR_IFACE" Reconcile >/dev/null 2>&1 || true
elif call_dbus "$DBUS_DEST" "/org/opdbus" "$MIRROR_IFACE" GetStats >/dev/null 2>&1; then
  # Legacy fallback for older deployments where MirrorV1 lived on org.opdbus.
  call_dbus "$DBUS_DEST" "/org/opdbus" "$MIRROR_IFACE" Reconcile >/dev/null 2>&1 || true
else
  echo "op-ovsdb-bridge: mirror interface $MIRROR_IFACE unavailable on $MIRROR_DEST$MIRROR_PATH, skipping reconcile"
fi

echo "op-ovsdb-bridge: reconciliation complete"
