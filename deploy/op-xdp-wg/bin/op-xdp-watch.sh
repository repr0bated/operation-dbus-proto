#!/bin/sh
set -eu

ETH=eth0
VETH=veth-warp
CT=wg-xray
CT_IPV6=2607:5300:205:200::5bc7
INTERVAL=60

echo "=== $(date -u) op-xdp-watch starting ==="

xdp_ok()  { ip -d link show "$ETH"  2>/dev/null | grep -q 'prog/xdp'; }
veth_ok() { ip link show "$VETH"     2>/dev/null | grep -qE '\bUP\b'; }
tc_ok()   { [ "$(tc filter show dev "$ETH" ingress 2>/dev/null | grep -c mirred)" -ge 6 ]; }
ndp_ok()  { ip -6 neigh show proxy   2>/dev/null | grep -q "^${CT_IPV6} "; }
ct_ok()   { incus info "$CT"         2>/dev/null | grep -q '^Status: RUNNING$'; }

while :; do
    sleep $INTERVAL

    failures=""
    xdp_ok  || failures="$failures xdp"
    veth_ok || failures="$failures veth"
    tc_ok   || failures="$failures tc"
    ndp_ok  || failures="$failures ndp"
    ct_ok   || failures="$failures container"

    [ -z "$failures" ] && continue

    echo "=== $(date -u) failures:$failures — restarting $CT ==="
    incus restart $CT 2>/dev/null || incus start $CT || echo "restart failed"
done
