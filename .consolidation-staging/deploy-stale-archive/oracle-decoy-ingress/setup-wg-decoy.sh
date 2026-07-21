#!/usr/bin/env bash
set -euo pipefail
# setup-wg-decoy.sh — Oracle Always Free ARM VM decoy WG ingress
#
# Architecture:
#   Oracle VPS = WG decoy ingress only (handshake + identity injection)
#   3tched     = Xray + data plane (WG protocol dropped after identity)
#
# This script sets up:
#   1. WireGuard server listener on port 51821
#   2. Identity injection via PostUp (writes WG pubkey to /dev/shm/decoy_identity)
#   3. Forwarding rule: after identity check, route to 3tched Xray via WG tunnel
#   4. Anti-reclamation keepalive (Oracle reclaims idle <20% for 7 days)
#   5. s6 service supervision
#
# Prerequisites:
#   - Oracle Always Free VM.Standard.A1.Flex (Oracle Linux ARM)
#   - Root access
#   - 3tched's WG pubkey and IP set below

# ── Detect distro ─────────────────────────────────────────────
if [ -f /etc/oracle-release ]; then
    DISTRO="oracle"
    PKG_MGR="dnf"
elif [ -f /etc/debian_version ]; then
    DISTRO="debian"
    PKG_MGR="apt"
else
    DISTRO="unknown"
    PKG_MGR="apt"
fi

# ── Configuration ──────────────────────────────────────────────
WG_PORT=51821
WG_IFACE=wg0
WG_CONF="/etc/wireguard/${WG_IFACE}.conf"
WG_NETWORK="10.189.0.0/24"         # decoy WG subnet (separate from 10.200.0.0/24 gRPC)
WG_VPS_IP="10.189.0.1/24"

# 3tched hypervisor — where Xray lives
THREETCHED_WG_PUBKEY=""             # SET: 3tched's WG public key
THREETCHED_ENDPOINT=""              # SET: 3tched's public IP:51821 or hostname
THREETCHED_WG_IP="10.189.0.2/32"    # 3tched's IP within decoy WG subnet

# Identity sled output (consumed by ghostbridge on 3tched)
IDENTITY_FILE="/dev/shm/decoy_identity"

# Anti-reclamation settings
CPU_BURN_SECONDS=5                  # seconds of CPU work per cycle
CYCLE_SECONDS=60                    # seconds per keepalive cycle

# ── Install packages ─────────────────────────────────────────
echo "[decoy] Installing packages on ${DISTRO}..."

if [ "$DISTRO" = "oracle" ]; then
    # Oracle Linux 8+: WireGuard module is built into kernel (kernel-modules-extra)
    dnf install -y wireguard-tools curl jq iptables-services
    # Ensure kernel module loaded
    modprobe wireguard 2>/dev/null || true
    echo "wireguard" > /etc/modules-load.d/wireguard.conf
elif [ "$DISTRO" = "debian" ]; then
    apt-get update -qq
    apt-get install -y -qq wireguard-tools curl jq
fi

# Generate server keypair if not exists
if [ ! -f /etc/wireguard/server_private.key ]; then
    wg genkey | tee /etc/wireguard/server_private.key | wg pubkey > /etc/wireguard/server_public.key
    chmod 600 /etc/wireguard/server_private.key
fi

SERVER_PRIVKEY=$(cat /etc/wireguard/server_private.key)
SERVER_PUBKEY=$(cat /etc/wireguard/server_public.key)

echo "[decoy] Server WG pubkey: ${SERVER_PUBKEY}"

# ── Generate WireGuard config ─────────────────────────────────
if [ -n "${THREETCHED_WG_PUBKEY}" ] && [ -n "${THREETCHED_ENDPOINT}" ]; then
    cat > "${WG_CONF}" <<EOF
[Interface]
PrivateKey = ${SERVER_PRIVKEY}
Address = ${WG_VPS_IP}
ListenPort = ${WG_PORT}

# Identity injection: log peer pubkey on handshake
PostUp = echo "[decoy] WG interface up" && mkdir -p /dev/shm
PostUp = wg show ${WG_IFACE} latest-handshakes 2>/dev/null || true

# Peer: 3tched hypervisor (Xray data plane)
[Peer]
PublicKey = ${THREETCHED_WG_PUBKEY}
AllowedIPs = ${THREETCHED_WG_IP}
Endpoint = ${THREETCHED_ENDPOINT}
PersistentKeepalive = 25
EOF
else
    echo "[decoy] WARNING: THREETCHED_WG_PUBKEY/THREETCHED_ENDPOINT not set."
    echo "[decoy] Generating server-only config. Add peer manually later."
    cat > "${WG_CONF}" <<EOF
[Interface]
PrivateKey = ${SERVER_PRIVKEY}
Address = ${WG_VPS_IP}
ListenPort = ${WG_PORT}

PostUp = echo "[decoy] WG interface up" && mkdir -p /dev/shm
EOF
fi

chmod 600 "${WG_CONF}"

# ── Identity injection watcher ────────────────────────────────
mkdir -p /usr/local/sbin

cat > /usr/local/sbin/decoy-identity-watcher.sh <<'WATCHER'
#!/bin/sh
# Watches for WG handshakes and injects identity into /dev/shm/decoy_identity
IFACE="${1:-wg0}"
IDENTITY_FILE="/dev/shm/decoy_identity"
LAST_HANDSHAKE=0

while true; do
    HANDSHAKE=$(wg show "$IFACE" latest-handshakes 2>/dev/null | head -1)
    PUBKEY=$(wg show "$IFACE" peers 2>/dev/null | head -1)

    if [ -n "$HANDSHAKE" ] && [ "$HANDSHAKE" != "$LAST_HANDSHAKE" ] && [ -n "$PUBKEY" ]; then
        LAST_HANDSHAKE="$HANDSHAKE"
        echo "{\"pubkey\":\"${PUBKEY}\",\"timestamp\":\"$(date -Iseconds)\",\"handshake\":\"${HANDSHAKE}\"}" > "$IDENTITY_FILE"
        echo "[decoy-identity] Injected identity for peer ${PUBKEY}"
    fi
    sleep 5
done
WATCHER
chmod +x /usr/local/sbin/decoy-identity-watcher.sh

# ── Anti-reclamation keepalive ────────────────────────────────
# Oracle reclaims Always Free VMs idle >7 days at <20% CPU/net/mem
cat > /usr/local/sbin/decoy-keepalive.sh <<'KEEPALIVE'
#!/bin/sh
# Anti-reclamation keepalive for Oracle Always Free

# Light CPU burn: compress /dev/urandom for a few seconds
burn_cpu() {
    timeout ${CPU_BURN_SECONDS:-5} sh -c 'cat /dev/urandom | gzip -1 > /dev/null' 2>/dev/null || true
}

# Light network activity: curl a small endpoint
burn_net() {
    curl -sf -o /dev/null https://www.microsoft.com/ 2>/dev/null || true
    curl -sf -o /dev/null https://www.apple.com/ 2>/dev/null || true
}

# WG keepalive: send traffic through tunnel if peer exists
burn_wg() {
    IFACE="${1:-wg0}"
    PEER_IP=$(wg show "$IFACE" allowed-ips 2>/dev/null | awk '{print $2}' | head -1)
    if [ -n "$PEER_IP" ] && [ "$PEER_IP" != "(none)" ]; then
        ping -c 1 -W 2 "${PEER_IP%%/*}" 2>/dev/null || true
    fi
}

CYCLE="${CYCLE_SECONDS:-60}"
echo "[keepalive] Starting anti-reclamation loop (cycle=${CYCLE}s)"

while true; do
    burn_cpu
    burn_net
    burn_wg "${1:-wg0}"
    sleep "$CYCLE"
done
KEEPALIVE
chmod +x /usr/local/sbin/decoy-keepalive.sh

# ── s6 service supervision ───────────────────────────────────
if ! command -v s6-svc >/dev/null 2>&1; then
    echo "[decoy] Installing s6..."
    if [ "$DISTRO" = "oracle" ]; then
        # Build s6 from source on Oracle Linux
        S6_VER="2.13.1.0"
        SKALIBS_VER="2.14.1.1"
        EXECLINE_VER="2.9.6.1"
        TMP="/tmp/s6-build"
        mkdir -p "$TMP"
        for pkg in skalibs execline s6; do
            ver=""
            [ "$pkg" = "skalibs" ] && ver="$SKALIBS_VER"
            [ "$pkg" = "execline" ] && ver="$EXECLINE_VER"
            [ "$pkg" = "s6" ] && ver="$S6_VER"
            curl -sL "https://skarnet.org/software/${pkg}/${pkg}-${ver}.tar.gz" | tar xz -C "$TMP"
            cd "${TMP}/${pkg}-${ver}"
            ./configure --prefix=/usr --disable-shared --libdir=/usr/lib64
            make -j$(nproc) && make install
        done
        cd -
    elif [ "$DISTRO" = "debian" ]; then
        apt-get install -y -qq s6 || {
            # Same source build fallback
            S6_VER="2.13.1.0"
            SKALIBS_VER="2.14.1.1"
            EXECLINE_VER="2.9.6.1"
            TMP="/tmp/s6-build"
            mkdir -p "$TMP"
            for pkg in skalibs execline s6; do
                ver=""
                [ "$pkg" = "skalibs" ] && ver="$SKALIBS_VER"
                [ "$pkg" = "execline" ] && ver="$EXECLINE_VER"
                [ "$pkg" = "s6" ] && ver="$S6_VER"
                curl -sL "https://skarnet.org/software/${pkg}/${pkg}-${ver}.tar.gz" | tar xz -C "$TMP"
                cd "${TMP}/${pkg}-${ver}"
                ./configure --prefix=/usr --disable-shared
                make -j$(nproc) && make install
            done
            cd -
        }
    fi
fi

S6_SVDIR="/etc/s6/sv"
S6_DIR="${S6_SVDIR}/decoy-wg"

mkdir -p "${S6_DIR}"
cat > "${S6_DIR}/run" <<'S6RUN'
#!/bin/sh
exec 2>&1
IFACE="${IFACE:-wg0}"
CONF="${WG_CONF:-/etc/wireguard/wg0.conf}"

if [ ! -f "$CONF" ]; then
    echo "[decoy-wg] No config at $CONF, exiting"
    exit 0
fi

wg-quick up "$IFACE" 2>/dev/null || true
echo "[decoy-wg] WireGuard up on ${IFACE}"

for _ in $(seq 1 10); do
    ip link show "$IFACE" >/dev/null 2>&1 && break
    sleep 1
done

/usr/local/sbin/decoy-identity-watcher.sh "$IFACE" &
IDENTITY_PID=$!

/usr/local/sbin/decoy-keepalive.sh "$IFACE" &
KEEPALIVE_PID=$!

wait -n 2>/dev/null || wait

kill "$IDENTITY_PID" "$KEEPALIVE_PID" 2>/dev/null || true
wg-quick down "$IFACE" 2>/dev/null || true
S6RUN
chmod +x "${S6_DIR}/run"

cat > "${S6_DIR}/finish" <<'S6FIN'
#!/bin/sh
exec 2>&1
IFACE="${IFACE:-wg0}"
wg-quick down "$IFACE" 2>/dev/null || true
echo "[decoy-wg] WireGuard down"
S6FIN
chmod +x "${S6_DIR}/finish"

echo "longrun" > "${S6_DIR}/type"

# ── Firewall ─────────────────────────────────────────────────
if [ "$DISTRO" = "oracle" ]; then
    # Oracle Linux uses iptables/firewalld
    if systemctl is-active firewalld >/dev/null 2>&1; then
        firewall-cmd --permanent --add-port="${WG_PORT}/udp"
        firewall-cmd --permanent --add-port=22/tcp
        firewall-cmd --reload
    else
        iptables -A INPUT -p udp --dport "${WG_PORT}" -j ACCEPT
        iptables -A INPUT -p tcp --dport 22 -j ACCEPT
        iptables -A INPUT -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT
        iptables -P INPUT DROP
        iptables-save > /etc/sysconfig/iptables 2>/dev/null || true
        systemctl enable iptables 2>/dev/null || true
    fi
elif command -v ufw >/dev/null 2>&1; then
    ufw allow "${WG_PORT}/udp" comment "WG decoy ingress"
    ufw allow 22/tcp comment "SSH"
    ufw --force enable
fi

# ── Start ─────────────────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════════════════"
echo "  Oracle Decoy WG Ingress — Setup Complete"
echo "═══════════════════════════════════════════════════════"
echo ""
echo "  Server WG Pubkey: ${SERVER_PUBKEY}"
echo "  Listen Port:      ${WG_PORT}"
echo "  WG Subnet:        ${WG_NETWORK}"
echo "  Identity File:    ${IDENTITY_FILE}"
echo "  Distro:           ${DISTRO}"
echo ""
echo "  NEXT STEPS:"
echo "  1. Set THREETCHED_WG_PUBKEY and THREETCHED_ENDPOINT above"
echo "  2. Re-run this script or edit ${WG_CONF} manually"
echo "  3. On 3tched, add this server as a WG peer:"
echo "     PublicKey = ${SERVER_PUBKEY}"
echo "     Endpoint = <ORACLE_VPS_IP>:${WG_PORT}"
echo "     AllowedIPs = 10.189.0.1/32"
echo "  4. Start: wg-quick up ${WG_IFACE}"
echo "     Or via s6: s6-svc -u ${S6_DIR}"
echo ""
