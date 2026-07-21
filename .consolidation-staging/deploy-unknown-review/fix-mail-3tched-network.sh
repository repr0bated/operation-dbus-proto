#!/bin/bash
# Fix mail-3tched container: remove all NICs, seal network, add Unix socket
# mounts for Xray / OpenFlow routing only.
#
# mail-3tched has NO IP address. All traffic flows via Unix sockets proxied
# through Xray's ds outbound (domain socket transport).
#
# Usage: sudo bash deploy/fix-mail-3tched-network.sh
set -euo pipefail

CONTAINER="mail-3tched"

log() { echo "[mail-fix] $*"; }
die() { echo "[mail-fix] ERROR: $*" >&2; exit 1; }

[[ $EUID -eq 0 ]] || die "Run as root: sudo bash $0"
command -v incus &>/dev/null || die "incus not found"

# ─── ensure container exists ──────────────────────────────────────────────────

if ! incus info "$CONTAINER" &>/dev/null; then
    die "Container '$CONTAINER' does not exist"
fi

# ─── stop container ───────────────────────────────────────────────────────────

log "Stopping $CONTAINER"
incus stop "$CONTAINER" --force 2>/dev/null || true

# ─── remove ALL NIC devices ───────────────────────────────────────────────────

log "Removing all network interfaces from $CONTAINER"
for dev in $(incus config device list "$CONTAINER" 2>/dev/null | grep -E "^eth|^enp" || true); do
    log "  Removing NIC device: $dev"
    incus config device remove "$CONTAINER" "$dev" 2>/dev/null || true
done

# Also remove any bridged/physical NICs that might have different names
for dev in $(incus config device list "$CONTAINER" 2>/dev/null); do
    type=$(incus config device get "$CONTAINER" "$dev" type 2>/dev/null || echo "")
    if [[ "$type" == "nic" ]]; then
        log "  Removing NIC device: $dev (type=nic)"
        incus config device remove "$CONTAINER" "$dev" 2>/dev/null || true
    fi
done

# Remove from default profile if it adds a NIC (it shouldn't, but be safe)
if incus profile show default | grep -q "type: nic" 2>/dev/null; then
    log "WARNING: default profile contains a NIC — mail-3tched should use a profile without NIC"
fi

# ─── verify no IPs ────────────────────────────────────────────────────────────

log "Verifying container has no network interfaces with IP addresses"
incus start "$CONTAINER" 2>/dev/null || true
sleep 2

ip_count=$(incus exec "$CONTAINER" -- ip -4 addr show 2>/dev/null | grep -c "inet " || echo "0")
if [[ "$ip_count" -gt 1 ]]; then
    # >1 because lo always has 127.0.0.1
    log "WARNING: container still has $((ip_count - 1)) non-loopback IP(s):"
    incus exec "$CONTAINER" -- ip -4 addr show | grep -v "127.0.0.1" || true
else
    log "Container sealed — only loopback present"
fi

# ─── add D-Bus session bus mount (for control plane) ──────────────────────────

log "Adding D-Bus session bus mount"
incus config device remove "$CONTAINER" host-op-dbus-session 2>/dev/null || true
incus config device add "$CONTAINER" host-op-dbus-session disk \
    source=/run/op-dbus/session-bus.sock \
    path=/run/host-op-dbus/session-bus.sock

# ─── add Unix socket proxies for mail services ──────────────────────────────
#
# Xray ds outbound routes host Unix sockets → container TCP ports.
# OpenFlow rules on the host steer traffic to these sockets.

# SMTP submission socket (Postfix/Maddy inside container listens on 127.0.0.1:587)
SMTP_SOCKET="/run/mail-3tched/smtp.sock"
mkdir -p "$(dirname "$SMTP_SOCKET")"
incus config device remove "$CONTAINER" mail-smtp 2>/dev/null || true
incus config device add "$CONTAINER" mail-smtp proxy \
    listen=unix:"$SMTP_SOCKET" \
    connect=tcp:127.0.0.1:587 \
    bind=host \
    mode=0660 \
    gid="$(id -g)"
log "SMTP Unix socket: $SMTP_SOCKET → container:587"

# IMAP socket (Dovecot/Maddy inside container listens on 127.0.0.1:143)
IMAP_SOCKET="/run/mail-3tched/imap.sock"
incus config device remove "$CONTAINER" mail-imap 2>/dev/null || true
incus config device add "$CONTAINER" mail-imap proxy \
    listen=unix:"$IMAP_SOCKET" \
    connect=tcp:127.0.0.1:143 \
    bind=host \
    mode=0660 \
    gid="$(id -g)"
log "IMAP Unix socket: $IMAP_SOCKET → container:143"

# IMAPS socket (TLS wrapper inside container listens on 127.0.0.1:993)
IMAPS_SOCKET="/run/mail-3tched/imaps.sock"
incus config device remove "$CONTAINER" mail-imaps 2>/dev/null || true
incus config device add "$CONTAINER" mail-imaps proxy \
    listen=unix:"$IMAPS_SOCKET" \
    connect=tcp:127.0.0.1:993 \
    bind=host \
    mode=0660 \
    gid="$(id -g)"
log "IMAPS Unix socket: $IMAPS_SOCKET → container:993"

# ─── restart container ────────────────────────────────────────────────────────

log "Restarting $CONTAINER"
incus restart "$CONTAINER" --force 2>/dev/null || incus start "$CONTAINER"

# ─── summary ─────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════════════════════════"
echo " mail-3tched network sealed"
echo "══════════════════════════════════════════════════════════════════"
echo " Container:      $CONTAINER"
echo " NICs:           NONE (no IP addresses)"
echo " Routing:        Xray ds outbound + OpenFlow via Unix sockets"
echo ""
echo " Unix sockets (host → container TCP):"
echo "   $SMTP_SOCKET  → 127.0.0.1:587  (SMTP submission)"
echo "   $IMAP_SOCKET  → 127.0.0.1:143  (IMAP)"
echo "   $IMAPS_SOCKET → 127.0.0.1:993  (IMAPS)"
echo ""
echo " Xray routing (add to xray config):"
echo "   UNIX_SOCKET_ENDPOINTS=mail-smtp:$SMTP_SOCKET:587"
echo "   UNIX_SOCKET_ENDPOINTS+=,mail-imap:$IMAP_SOCKET:143"
echo "   UNIX_SOCKET_ENDPOINTS+=,mail-imaps:$IMAPS_SOCKET:993"
echo ""
echo " Verify:"
echo "   sudo incus exec $CONTAINER -- ip addr show"
echo "   sudo incus exec $CONTAINER -- ss -tlnp"
echo "══════════════════════════════════════════════════════════════════"
