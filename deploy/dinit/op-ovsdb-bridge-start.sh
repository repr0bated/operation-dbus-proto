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
RTNET_PATH="${OP_DBUS_RTNET_PATH:-/org/opdbus/rtnetlink}"
RTNET_IFACE="${OP_DBUS_RTNET_IFACE:-org.opdbus.RtnetlinkV1}"
BRIDGE="${PRIVACY_BRIDGE_NAME:-ovsbr0}"
UPLINK="${PRIVACY_UPLINK_PORT:-ens3}"
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

# Return "UP"/"DOWN" for an interface using brief output.
link_state() {
  ip -br link show "$1" 2>/dev/null | awk '{print $2}'
}

# Ensure the uplink device is administratively up. No-op if already up.
ensure_uplink_up() {
  state="$(link_state "$UPLINK" || true)"
  if [ "$state" != "UP" ] && [ -n "$state" ]; then
    echo "op-ovsdb-bridge: bringing uplink $UPLINK up (state=$state)"
    # Best effort; do not abort the script if this fails.
    ip link set dev "$UPLINK" up || true
  fi
}

# Align the bridge MAC to the uplink's MAC so L2 identity is stable.
# Prefer doing this in OVSDB (Bridge.other-config:hwaddr). If already set
# and equal, do nothing. This step is idempotent and safe to skip.
ensure_bridge_hwaddr() {
  if [ ! -r "/sys/class/net/$UPLINK/address" ]; then
    return 0
  fi
  uplink_mac=$(cat "/sys/class/net/$UPLINK/address" | tr 'A-Z' 'a-z')
  current_hwaddr=$(ovs-vsctl --if-exists get Bridge "$BRIDGE" other-config:hwaddr 2>/dev/null | tr -d '"' | tr 'A-Z' 'a-z' || true)
  if [ "$current_hwaddr" != "$uplink_mac" ]; then
    echo "op-ovsdb-bridge: setting $BRIDGE other-config:hwaddr=$uplink_mac"
    ovs-vsctl set Bridge "$BRIDGE" other-config:hwaddr="$uplink_mac" || true
  fi
}

if ! wait_for_opdbus; then
  echo "op-ovsdb-bridge: D-Bus service $DBUS_DEST unavailable after timeout" >&2
  exit 1
fi

if ! bridge_exists; then
  if ip link show "$BRIDGE" >/dev/null 2>&1; then
    echo "op-ovsdb-bridge: kernel link $BRIDGE exists but OVSDB bridge is missing" >&2
    echo "op-ovsdb-bridge: refusing automatic bridge creation to avoid breaking host connectivity" >&2
    exit 1
  fi

  echo "op-ovsdb-bridge: creating missing OVS bridge $BRIDGE"
  call_dbus "$DBUS_DEST" "$OVS_PATH" "$OVS_IFACE" CreateBridge s "$BRIDGE" >/dev/null
fi

if ! bridge_exists; then
  echo "op-ovsdb-bridge: failed to confirm OVS bridge $BRIDGE after creation" >&2
  exit 1
fi

echo "op-ovsdb-bridge: validating uplink $UPLINK attached to $BRIDGE"
if port_present; then
  echo "op-ovsdb-bridge: uplink $UPLINK is present"
else
  echo "op-ovsdb-bridge: attaching missing uplink $UPLINK to $BRIDGE"
  call_dbus "$DBUS_DEST" "$OVS_PATH" "$OVS_IFACE" AddPort ss "$BRIDGE" "$UPLINK" >/dev/null
fi

if ! port_present; then
  echo "op-ovsdb-bridge: failed to confirm uplink $UPLINK on $BRIDGE after AddPort" >&2
  exit 1
fi

# Bring the uplink up if it isn't already.
ensure_uplink_up

# Align bridge MAC to uplink MAC when missing/mismatched.
ensure_bridge_hwaddr

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
