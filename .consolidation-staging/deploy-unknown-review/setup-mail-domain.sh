#!/bin/bash
# Setup Cloudflare DNS and nginx for mail.3tched.com
# Sources credentials from ~/.bash_secrets

set -e

echo "=== Setting up mail.3tched.com ==="

if [ -f ~/.bash_secrets ]; then
    source ~/.bash_secrets
    echo "✅ Loaded credentials from ~/.bash_secrets"
else
    echo "❌ ~/.bash_secrets not found"
    exit 1
fi

DOMAIN="mail.3tched.com"
NGINX_CONF="$(dirname "$0")/nginx/mail-webmail-complete.conf"

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
echo "Server IP: $PROXY_IP"
echo ""

# Determine zone ID for 3tched.com
# Try common variable names from ~/.bash_secrets
ZONE_ID="${CF_ZONEID_3TCHEDCOM:-${CF_ZONE_ID_3TCHED:-${CF_ZONEID_3TCHED:-}}}"

if [ -z "$ZONE_ID" ]; then
    echo "❌ Could not find 3tched.com zone ID in ~/.bash_secrets"
    echo "   Set CF_ZONEID_3TCHEDCOM in ~/.bash_secrets"
    echo ""
    echo "   To find your zone ID:"
    echo "   curl -s https://api.cloudflare.com/client/v4/zones \\"
    echo "     -H 'Authorization: Bearer \$CF_DNS_ZONE_TOKEN' | jq '.result[] | {name,id}'"
    exit 1
fi

echo "☁️  Adding DNS A record: mail.3tched.com -> $PROXY_IP"

# Check if record already exists
EXISTING=$(curl -s "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records?type=A&name=mail.3tched.com" \
  -H "Authorization: Bearer ${CF_DNS_ZONE_TOKEN}" \
  -H "Content-Type: application/json" | jq -r '.result[0].id // empty')

if [ -n "$EXISTING" ]; then
    echo "  Updating existing record (id: $EXISTING)..."
    curl -s -X PUT "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records/${EXISTING}" \
      -H "Authorization: Bearer ${CF_DNS_ZONE_TOKEN}" \
      -H "Content-Type: application/json" \
      --data "{\"type\":\"A\",\"name\":\"mail\",\"content\":\"${PROXY_IP}\",\"ttl\":1,\"proxied\":true}" | jq '{success,errors}'
else
    echo "  Creating new record..."
    curl -s -X POST "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records" \
      -H "Authorization: Bearer ${CF_DNS_ZONE_TOKEN}" \
      -H "Content-Type: application/json" \
      --data "{\"type\":\"A\",\"name\":\"mail\",\"content\":\"${PROXY_IP}\",\"ttl\":1,\"proxied\":true}" | jq '{success,errors}'
fi

echo ""
echo "📋 Installing nginx config..."
sudo cp "$NGINX_CONF" /etc/nginx/sites-available/mail.3tched.com
sudo ln -sf /etc/nginx/sites-available/mail.3tched.com /etc/nginx/sites-enabled/mail.3tched.com

# Remove old duplicate configs if present
sudo rm -f /etc/nginx/sites-enabled/mail-webmail.conf \
           /etc/nginx/sites-enabled/mail-webmail-complete.conf \
           /etc/nginx/http.d/mail-webmail.conf \
           /etc/nginx/http.d/mail-webmail-complete.conf

echo "🔧 Testing nginx config..."
sudo nginx -t

echo "🔄 Reloading nginx..."
sudo nginx -s reload

echo ""
echo "✅ Done! mail.3tched.com is configured."
echo ""
echo "DNS may take a minute to propagate. Test with:"
echo "  dig mail.3tched.com"
echo "  curl -I https://mail.3tched.com"
echo ""
echo "Make sure op-web is running on port 8080:"
echo "  curl -I http://127.0.0.1:8080/health"
