#!/usr/bin/env bash
set -euo pipefail

PUBLIC_IP="${PUBLIC_IP:-15.235.37.41}"
ALIAS_IP="${ALIAS_IP:-10.200.0.1}"

for pref in 20 21 30 31 40 41 50; do
  ip rule del pref "$pref" 2>/dev/null || true
done

while ip rule show | grep -q 'lookup 51820'; do
  pref=$(ip rule show | awk '/lookup 51820/ { sub(":", "", $1); print $1; exit }')
  ip rule del pref "$pref" 2>/dev/null || break
done

while ip rule show | grep -q 'suppress_prefixlength 0'; do
  pref=$(ip rule show | awk '/suppress_prefixlength 0/ { sub(":", "", $1); print $1; exit }')
  ip rule del pref "$pref" 2>/dev/null || break
done

ip rule add pref 20 from "$PUBLIC_IP/32" lookup main
ip rule add pref 21 from "$ALIAS_IP/32" lookup main
ip route flush cache
