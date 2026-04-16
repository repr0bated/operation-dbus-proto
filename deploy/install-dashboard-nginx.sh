#!/bin/bash
# deploy/install-dashboard-nginx.sh
# Install the WG-bound dashboard gateway nginx config with token substitution.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NGINX_TEMPLATE="${SCRIPT_DIR}/nginx/dashboard-3tched-socket.conf.template"
ENV_FILE="/etc/op-dbus/environment"
TARGET="/etc/nginx/http.d/dashboard-3tched.conf"

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

# Read OpenClaw gateway token from environment file. OPENCLAW_TOKEN is accepted
# for compatibility with older local env files, but the rendered nginx config
# only injects it as an upstream bearer token.
OPENCLAW_GATEWAY_TOKEN="${OPENCLAW_GATEWAY_TOKEN:-}"
if [[ -f "$ENV_FILE" ]]; then
    set -a
    # shellcheck disable=SC1090
    . "$ENV_FILE"
    set +a
    OPENCLAW_GATEWAY_TOKEN="${OPENCLAW_GATEWAY_TOKEN:-${OPENCLAW_TOKEN:-}}"
fi

if [[ -z "$OPENCLAW_GATEWAY_TOKEN" || "$OPENCLAW_GATEWAY_TOKEN" == "your-openclaw-api-token-here" ]]; then
    echo "[WARN] OPENCLAW_GATEWAY_TOKEN not set in ${ENV_FILE}"
    echo "[WARN] Please set it first, then re-run this script"
    echo ""
    echo "To set the token:"
    echo "  install -m 0600 /dev/null ${ENV_FILE}"
    echo "  printf '%s\n' 'OPENCLAW_GATEWAY_TOKEN=<token>' >> ${ENV_FILE}"
    exit 1
fi

echo "[INFO] Found OpenClaw gateway token in environment"

# Install nginx config with token substitution
mkdir -p /etc/nginx/http.d

# Replace token placeholder and install. Use Python instead of sed so token
# characters are not interpreted as replacement syntax.
OPENCLAW_GATEWAY_TOKEN="$OPENCLAW_GATEWAY_TOKEN" python3 - "$NGINX_TEMPLATE" "$TARGET" <<'PY'
import os
import pathlib
import sys

template = pathlib.Path(sys.argv[1])
target = pathlib.Path(sys.argv[2])
target.write_text(
    template.read_text().replace(
        "<OPENCLAW_GATEWAY_TOKEN>",
        os.environ["OPENCLAW_GATEWAY_TOKEN"],
    )
)
PY

echo "[INSTALL] Installed ${TARGET}"

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
echo "dashboard.3tched.com configured"
echo "=========================================="
echo ""
echo "Routes:"
echo "  10.0.0.1 /gateway/       -> /run/services0/gateway.sock"
echo "  148.113.204.83 /gateway/ -> 404 public sink"
echo ""
echo "Make sure:"
echo "  1. OpenClaw gateway socket exists at /run/services0/gateway.sock"
echo "  2. WireGuard wg0 is active on 10.0.0.1/24"
echo "  3. dashboard.3tched.com resolves to 10.0.0.1 for WG clients"
echo ""
