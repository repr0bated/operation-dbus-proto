#!/bin/sh
# fix-steering.sh — fix IPv4 routing and iptables so 15.235.37.41 is reachable from internet
#
# Run as root in novnc.  Safe to re-run.
# Changes take effect immediately (no reboot needed for the networking part).
set -eu

CONTAINER_IP="15.235.37.41"
HOST_VETH_IP="10.200.0.2"
VETH="grpc-uplink"
UPLINK="eth0"
HOST_PUBLIC="148.113.204.83"

log() { printf '\033[1;32m[fix]\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m[ERROR]\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(id -u)" = 0 ] || die "run as root"

# ── 1. Add missing host route for container public IP ────────────────────────
# Without this route the kernel drops inbound packets for 15.235.37.41.
log "adding route: ${CONTAINER_IP}/32 dev ${VETH}"
ip route replace "${CONTAINER_IP}/32" dev "$VETH" src "$HOST_VETH_IP"
log "route added"

# ── 2. Fix FORWARD chain — allow NEW inbound connections to container IP ──────
# Current rules only allow ESTABLISHED/RELATED from eth0→grpc-uplink.
# Policy is ACCEPT so NEW passes anyway, but make it explicit.
iptables -C FORWARD -i "$UPLINK" -o "$VETH" -d "${CONTAINER_IP}/32" -j ACCEPT 2>/dev/null || \
    iptables -I FORWARD 1 -i "$UPLINK" -o "$VETH" -d "${CONTAINER_IP}/32" -j ACCEPT
log "FORWARD: inbound NEW to ${CONTAINER_IP} explicitly allowed"

# ── 3. Fix SNAT — remove blanket SNAT for container public IP ────────────────
# The current rule rewrites xray's TCP response source from 15.235.37.41 to
# 148.113.204.83, causing the client to RST (wrong source IP vs what it connected to).
# 15.235.37.41 is a static OVH failover IP routed to this host — no SNAT needed for server traffic.
# Replace with a NEW-only SNAT so only container-originated outbound connections are masqueraded.

# Remove the old blanket rules
iptables -t nat -D POSTROUTING -s "${CONTAINER_IP}/32" -o "$UPLINK" -j SNAT --to-source "$HOST_PUBLIC" 2>/dev/null || true
iptables -t nat -D POSTROUTING -s "${CONTAINER_IP}/32" -o "$UPLINK" -j MASQUERADE 2>/dev/null || true
log "removed blanket SNAT/MASQUERADE for ${CONTAINER_IP}"

# Add back SNAT restricted to NEW (container-initiated outbound only, not server responses)
iptables -t nat -C POSTROUTING -s "${CONTAINER_IP}/32" -o "$UPLINK" -m conntrack --ctstate NEW -j SNAT --to-source "$HOST_PUBLIC" 2>/dev/null || \
    iptables -t nat -A POSTROUTING -s "${CONTAINER_IP}/32" -o "$UPLINK" -m conntrack --ctstate NEW -j SNAT --to-source "$HOST_PUBLIC"
log "added NEW-only SNAT for ${CONTAINER_IP} (outbound from container only)"

# ── 4. Persist: update ovs-vethpair/shell_up so these survive reboot ─────────
log "patching /etc/s6/sv/ovs-vethpair/shell_up to fix SNAT and ensure route"
# Check if the patched version is already there
if grep -q 'ctstate NEW' /etc/s6/sv/ovs-vethpair/shell_up 2>/dev/null; then
    log "shell_up already patched — skipping"
else
    # Patch in place: replace the SNAT/MASQUERADE lines for CONTAINER_IP
    # Use a temp file to rewrite the script
    tmp=$(mktemp)
    awk '
    /iptables -t nat.*POSTROUTING.*15\.235\.37\.41.*SNAT.*148\.113/ { next }
    /iptables -t nat.*POSTROUTING.*15\.235\.37\.41.*MASQUERADE/ { next }
    /POSTROUTING.*-s 15\.235\.37\.41.*-o eth0.*-j SNAT/ { next }
    /POSTROUTING.*-s 15\.235\.37\.41.*-o eth0.*-j MASQUERADE/ { next }
    { print }
    ' /etc/s6/sv/ovs-vethpair/shell_up > "$tmp"

    # Insert the corrected SNAT rule (NEW-only) before the FORWARD section
    awk '
    /iptables -C FORWARD -i grpc-uplink/ && !inserted {
        print "    iptables -t nat -C POSTROUTING -s 15.235.37.41/32 -o eth0 -m conntrack --ctstate NEW -j SNAT --to-source 148.113.204.83 2>/dev/null ||"
        print "        iptables -t nat -I POSTROUTING 1 -s 15.235.37.41/32 -o eth0 -m conntrack --ctstate NEW -j SNAT --to-source 148.113.204.83"
        inserted = 1
    }
    { print }
    ' "$tmp" > /etc/s6/sv/ovs-vethpair/shell_up
    rm -f "$tmp"
    log "ovs-vethpair/shell_up patched"
fi

# ── 5. Verify ─────────────────────────────────────────────────────────────────
log ""
log "=== Verification ==="
log "Route to container:"
ip route show "${CONTAINER_IP}" 2>/dev/null | grep -v '^$' || echo "  WARNING: still missing"
log "FORWARD rules for ${CONTAINER_IP}:"
iptables -L FORWARD -n 2>/dev/null | grep "$CONTAINER_IP" | sed 's/^/  /'
log "NAT POSTROUTING rules for ${CONTAINER_IP}:"
iptables -t nat -L POSTROUTING -n 2>/dev/null | grep "$CONTAINER_IP" | sed 's/^/  /'
log ""
log "=== Test from another host ==="
log "  curl -v --resolve '${CONTAINER_IP}:443:${CONTAINER_IP}' https://${CONTAINER_IP}/"
log "  OR: nc -zv ${CONTAINER_IP} 443"
log ""
log "=== Container xray status ==="
incus exec wg-xray -- systemctl status xray --no-pager -l 2>/dev/null | head -10 || true
