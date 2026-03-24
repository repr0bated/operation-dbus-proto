#!/bin/bash
# Diagnostic script for op-dbus.ghostbridge.tech connection issues
# Run this on the Proxmox host to diagnose the connection problem

echo "=== Connection Diagnostics for op-dbus.ghostbridge.tech ==="
echo

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

check_service() {
    local service=$1
    if systemctl is-active --quiet "$service" 2>/dev/null; then
        echo -e "${GREEN}✓${NC} $service is running"
        return 0
    else
        echo -e "${RED}✗${NC} $service is NOT running"
        return 1
    fi
}

echo "1. Checking systemd services..."
check_service "op-web"
check_service "op-dbus"
check_service "op-mcp-agents"
check_service "caddy"

echo
echo "2. Checking local endpoint (localhost:8080)..."
if curl -s http://localhost:8080/api/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} op-web responding on localhost:8080"
    curl -s http://localhost:8080/api/health | head -5
else
    echo -e "${RED}✗${NC} op-web NOT responding on localhost:8080"
fi

echo
echo "3. Checking Caddy configuration..."
if [ -f /etc/caddy/conf.d/op-web-ghostbridge.conf ]; then
    echo -e "${GREEN}✓${NC} Caddy config file exists"
    grep -E "op-web.ghostbridge.tech|reverse_proxy" /etc/caddy/conf.d/op-web-ghostbridge.conf | head -3
elif grep -q "op-web.ghostbridge.tech" /etc/caddy/Caddyfile 2>/dev/null; then
    echo -e "${GREEN}✓${NC} Found in main Caddyfile"
else
    echo -e "${RED}✗${NC} Caddy config NOT found"
    echo "   Run: sudo cp deploy/caddy/op-web-ghostbridge.conf /etc/caddy/conf.d/"
fi

echo
echo "4. Checking DNS resolution..."
if command -v nslookup >/dev/null; then
    nslookup op-dbus.ghostbridge.tech 2>/dev/null | grep -E "Name:|Address:" | head -3
elif command -v dig >/dev/null; then
    dig +short op-dbus.ghostbridge.tech | head -3
else
    echo "   (nslookup/dig not available)"
fi

echo
echo "5. Checking firewall (port 80/443)..."
if command -v ufw >/dev/null; then
    ufw status | grep -E "80|443" || echo "   No specific rules for 80/443"
elif command -v iptables >/dev/null; then
    iptables -L -n | grep -E "dpt:80|dpt:443" | head -3 || echo "   Check iptables manually"
else
    echo "   (ufw/iptables not available)"
fi

echo
echo "6. Recent logs..."
echo "--- op-web (last 5 lines) ---"
journalctl -u op-web --no-pager -n 5 2>/dev/null || echo "   No logs available"

echo
echo "--- Caddy (last 5 lines) ---"
journalctl -u caddy --no-pager -n 5 2>/dev/null || echo "   No logs available"

echo
echo "=== Quick Fixes ==="
echo "If op-web is not running:"
echo "  sudo systemctl start op-web"
echo "  sudo systemctl enable op-web"
echo

echo "If Caddy config is missing:"
echo "  sudo cp deploy/caddy/op-web-ghostbridge.conf /etc/caddy/conf.d/"
echo "  sudo systemctl reload caddy"
echo

echo "To restart everything:"
echo "  sudo systemctl restart op-web caddy"
echo

echo "=== End Diagnostics ==="