#!/bin/bash
# deploy/install-dashboard-nginx.sh
# Install nginx config for dashboard.3tched.com with token substitution

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NGINX_TEMPLATE="${SCRIPT_DIR}/nginx/dashboard-3tched.conf.template"
ENV_FILE="/etc/op-dbus/environment"

echo "[INSTALL] Setting up dashboard.3tched.com nginx config..."

# Check if running as root
if [[ "$EUID" -ne 0 ]]; then
    echo "[ERROR] Must run as root (sudo)"
    exit 1
fi

# Check nginx is installed
if ! command -v nginx >/dev/null 2>&1; then
    echo "[ERROR] nginx not installed"
    exit 1
fi

# Read OpenClaw token from environment file
OPENCLAW_TOKEN=""
if [[ -f "$ENV_FILE" ]]; then
    OPENCLAW_TOKEN=$(grep "^OPENCLAW_TOKEN=" "$ENV_FILE" 2>/dev/null | cut -d'=' -f2- || true)
fi

if [[ -z "$OPENCLAW_TOKEN" || "$OPENCLAW_TOKEN" == "your-openclaw-api-token-here" ]]; then
    echo "[WARN] OPENCLAW_TOKEN not set in ${ENV_FILE}"
    echo "[WARN] Please set it first, then re-run this script"
    echo ""
    echo "To set the token:"
    echo "  echo 'OPENCLAW_TOKEN=your-actual-token' >> ${ENV_FILE}"
    exit 1
fi

echo "[INFO] Found OPENCLAW_TOKEN in environment file"

# Install nginx config with token substitution
mkdir -p /etc/nginx/http.d

# Replace token placeholder and install
sed "s/OPENCLAW_TOKEN_PLACEHOLDER/${OPENCLAW_TOKEN}/g" "$NGINX_TEMPLATE" \
    > /etc/nginx/http.d/dashboard-3tched.conf

echo "[INSTALL] Installed /etc/nginx/http.d/dashboard-3tched.conf"

# Test nginx config
echo "[TEST] Testing nginx configuration..."
if nginx -t; then
    echo "[TEST] Nginx config OK"
else
    echo "[ERROR] Nginx config test failed!"
    exit 1
fi

# Reload nginx
echo "[RELOAD] Reloading nginx..."
nginx -s reload

echo ""
echo "=========================================="
echo "✅ dashboard.3tched.com configured!"
echo "=========================================="
echo ""
echo "Routes:"
echo "  /gateway/  -> OpenClaw gateway (with auth token)"
echo "  /          -> op-web UI"
echo ""
echo "Make sure:"
echo "  1. OpenClaw gateway socket exists at /run/services0/gateway.sock"
echo "  2. op-dbus is running on port 8080"
echo "  3. op-web is running on port 8081"
echo "  4. WireGuard is active and peer is connected"
echo ""
