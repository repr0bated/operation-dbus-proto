#!/bin/bash
#
# WireGuard Server Configuration Helper
# Run this on your WireGuard server to add the ICUS client
#

set -euo pipefail

CLIENT_PUBLIC_KEY="${1:-}"
CLIENT_NAME="${2:-icus-client}"
SERVER_WG_IF="${SERVER_WG_IF:-wg0}"
SERVER_WG_CONF="${SERVER_WG_CONF:-/etc/wireguard/wg0.conf}"

if [[ -z "$CLIENT_PUBLIC_KEY" ]]; then
    echo "Usage: $0 <client-public-key> [client-name]"
    echo ""
    echo "Example:"
    echo "  $0 abc123def456... chimera-icus"
    exit 1
fi

# Find next available IP
get_next_ip() {
    local subnet="$1"
    local existing_ips=$(wg show "$SERVER_WG_IF" allowed-ips 2>/dev/null | grep -oE '10\.[0-9]+\.[0-9]+\.[0-9]+' | sort -t. -k4 -n | tail -1)
    
    if [[ -z "$existing_ips" ]]; then
        echo "10.200.200.2"
    else
        local last_octet=$(echo "$existing_ips" | cut -d. -f4)
        echo "10.200.200.$((last_octet + 1))"
    fi
}

CLIENT_IP=$(get_next_ip)

echo "=========================================="
echo "WireGuard Client Configuration"
echo "=========================================="
echo ""
echo "Client Name: $CLIENT_NAME"
echo "Client IP:   $CLIENT_IP/32"
echo "Public Key:  $CLIENT_PUBLIC_KEY"
echo ""
echo "Add this to your $SERVER_WG_CONF:"
echo ""
echo "# $CLIENT_NAME"
echo "[Peer]"
echo "PublicKey = $CLIENT_PUBLIC_KEY"
echo "AllowedIPs = $CLIENT_IP/32"
echo ""
echo "Then restart WireGuard:"
echo "  sudo wg-quick down $SERVER_WG_IF && sudo wg-quick up $SERVER_WG_IF"
echo ""
echo "Or use: sudo wg set $SERVER_WG_IF peer $CLIENT_PUBLIC_KEY allowed-ips $CLIENT_IP/32"
