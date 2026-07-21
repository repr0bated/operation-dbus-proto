#!/usr/bin/env bash
set -euo pipefail

IFACE="eth0"
PUBLIC_IP="15.235.37.41"
ALIAS_IP="10.200.0.1"
GW="10.200.0.2"

echo "[wg-xray-network] configuring $IFACE"

for _ in $(seq 1 30); do
  if ip link show "$IFACE" >/dev/null 2>&1; then
    echo "[wg-xray-network] interface $IFACE ready"
    break
  fi
  sleep 1
done

if ! ip link show "$IFACE" >/dev/null 2>&1; then
  echo "[wg-xray-network] ERROR: $IFACE did not appear" >&2
  exit 1
fi

if [ -f /run/host-eth0-mac ]; then
  MAC=$(cat /run/host-eth0-mac)
else
  MAC=$(ip -o link show eth0 2>/dev/null | sed -E 's/.*link\/ether ([0-9a-f:]+).*/\1/')
  if [ -z "$MAC" ] || [ "$MAC" = "1500" ]; then
    MAC=$(ip link show eth0 2>/dev/null | grep -oP 'link/ether \K([0-9a-f:]+)' || echo "fa:16:3e:f1:71:d2")
  fi
fi

echo "[wg-xray-network] Using MAC: $MAC"

ip link set "$IFACE" down 2>/dev/null || true
ip link set "$IFACE" address "$MAC" || echo "Warning: Could not set MAC"
ip link set "$IFACE" up

ip addr flush dev "$IFACE" 2>/dev/null || true
ip addr add "$PUBLIC_IP/32" dev "$IFACE"
ip addr add "$ALIAS_IP/30" dev "$IFACE"

ip route replace default via "$GW" dev "$IFACE" src "$PUBLIC_IP" metric 100

/usr/local/sbin/wg-xray-policy-rules.sh

echo "[wg-xray-network] done"
