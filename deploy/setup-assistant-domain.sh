#!/bin/bash
# Setup reverse proxy and Cloudflare DNS for assistant.3tched.com
# Serves the axon-trace-ui React SPA over WireGuard zero-trust.
#
# Run from repo root:
#   ./deploy/setup-assistant-domain.sh [PUBLIC_IP]
#
# Requires:
#   - Node.js + npm (for building the UI)
#   - nginx installed and running
#   - ~/.bash_secrets with Cloudflare credentials
#   - SSL certificate at /etc/letsencrypt/live/3tched.com/
#   - WireGuard interface at 10.0.0.1

set -e

echo "=== Setting up assistant.3tched.com ==="

# Load Cloudflare credentials
if [ -f ~/.bash_secrets ]; then
    source ~/.bash_secrets
    echo "✅ Loaded Cloudflare credentials from ~/.bash_secrets"
else
    echo "❌ ~/.bash_secrets not found"
    exit 1
fi

DOMAIN="assistant.3tched.com"
UI_SRC="ui/axon-trace-ui"
WWW_ROOT="/var/www/assistant"
NGINX_CONF="deploy/nginx/assistant-3tched.conf"

detect_public_ipv4() {
    ip -4 route get 1.1.1.1 2>/dev/null | awk '/src/ {for (i = 1; i <= NF; i++) if ($i == "src") {print $(i + 1); exit}}'
}

PROXY_IP="${1:-$(detect_public_ipv4)}"

if [ -z "$PROXY_IP" ]; then
    echo "❌ Could not detect public IPv4 automatically"
    echo "   Pass the desired origin IP as the first argument"
    exit 1
fi

echo "Domain: $DOMAIN"
echo "Proxy IP: $PROXY_IP"
echo "UI source: $UI_SRC"
echo ""

# ── Build axon-trace-ui ──
echo "🔨 Building axon-trace-ui..."
if [ ! -d "$UI_SRC" ]; then
    echo "❌ UI source directory not found: $UI_SRC"
    exit 1
fi

cd "$UI_SRC"
if [ ! -d "node_modules" ]; then
    echo "📦 Installing npm dependencies..."
    npm ci
fi
echo "📦 Running production build..."
npm run build
cd - > /dev/null

if [ ! -d "$UI_SRC/dist" ]; then
    echo "❌ Build failed: $UI_SRC/dist not found"
    exit 1
fi

echo "✅ Build complete"
echo ""

# ── Deploy static files ──
echo "📁 Deploying static files to $WWW_ROOT..."
sudo mkdir -p "$WWW_ROOT"
sudo rm -rf "${WWW_ROOT:?}"/*
sudo cp -r "$UI_SRC/dist/"* "$WWW_ROOT/"
sudo chown -R root:root "$WWW_ROOT"
sudo chmod -R 644 "$WWW_ROOT"
find "$WWW_ROOT" -type d -exec sudo chmod 755 {} \;
echo "✅ Static files deployed"
echo ""

# ── Install nginx config ──
echo "📋 Installing nginx reverse proxy config..."
sudo mkdir -p /etc/nginx/http.d

if [ ! -f "$NGINX_CONF" ]; then
    echo "❌ Nginx config not found: $NGINX_CONF"
    exit 1
fi

sudo cp "$NGINX_CONF" "/etc/nginx/http.d/assistant-3tched.conf"
echo "✅ Nginx config installed"
echo ""

# ── Test nginx ──
echo "🔧 Testing nginx configuration..."
if sudo nginx -t; then
    echo "✅ Nginx config is valid"
else
    echo "❌ Nginx config has errors"
    exit 1
fi
echo ""

# ── Cloudflare DNS ──
echo "☁️  Creating Cloudflare DNS record for $DOMAIN..."

ZONE_ID="${CF_ZONEID_3TCHEDCOM:-${CF_ZONE_ID_3TCHED:-${CF_ZONEID_3TCHED:-}}}"

if [ -z "$ZONE_ID" ]; then
    echo "❌ Could not find 3tched.com zone ID in ~/.bash_secrets"
    echo "   Set CF_ZONEID_3TCHEDCOM in ~/.bash_secrets"
    echo ""
    echo "   To find your zone ID:"
    echo '   curl -s https://api.cloudflare.com/client/v4/zones \'
    echo "     -H 'Authorization: Bearer \$CF_DNS_ZONE_TOKEN' | jq '.result[] | {name,id}'"
    exit 1
fi

EXISTING=$(curl -s "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records?type=A&name=${DOMAIN}" \
  -H "Authorization: Bearer ${CF_DNS_ZONE_TOKEN}" \
  -H "Content-Type: application/json" | jq -r '.result[0].id // empty')

if [ -n "$EXISTING" ]; then
    echo "  Updating existing record (id: $EXISTING)..."
    curl -s -X PUT "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records/${EXISTING}" \
      -H "Authorization: Bearer ${CF_DNS_ZONE_TOKEN}" \
      -H "Content-Type: application/json" \
      --data "{\"type\":\"A\",\"name\":\"assistant\",\"content\":\"${PROXY_IP}\",\"ttl\":1,\"proxied\":true}" | jq '{success,errors}'
else
    echo "  Creating new record..."
    curl -s -X POST "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records" \
      -H "Authorization: Bearer ${CF_DNS_ZONE_TOKEN}" \
      -H "Content-Type: application/json" \
      --data "{\"type\":\"A\",\"name\":\"assistant\",\"content\":\"${PROXY_IP}\",\"ttl\":1,\"proxied\":true}" | jq '{success,errors}'
fi

echo ""
echo "✅ Cloudflare DNS record created (proxied through Cloudflare)"

# ── Reload nginx ──
echo "🔄 Reloading nginx..."
sudo nginx -s reload
echo "✅ Nginx reloaded"

echo ""
echo "🎉 Setup complete!"
echo ""
echo "Your assistant GUI is now available at:"
echo "  https://assistant.3tched.com  (WireGuard tunnel only)"
echo ""
echo "Access requirements:"
echo "  1. Connect to WireGuard VPN"
echo "  2. Open https://assistant.3tched.com in browser"
echo ""
echo "gRPC bridge endpoint (inside wg-xray container):"
echo "  10.200.0.2:50051"
echo ""
echo "Assistant gRPC server (host):"
echo "  127.0.0.1:50052"
echo ""
echo "Static files served from:"
echo "  $WWW_ROOT"
echo ""
echo "Logs:"
echo "  /var/log/nginx/assistant.3tched.com.access.log"
echo "  /var/log/nginx/assistant.3tched.com.error.log"
echo ""
echo "To rebuild and redeploy the UI only:"
echo "  cd $UI_SRC && npm run build"
echo "  sudo rm -rf $WWW_ROOT/* && sudo cp -r $UI_SRC/dist/* $WWW_ROOT/"
echo ""
