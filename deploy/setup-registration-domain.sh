#!/bin/bash
# Setup reverse proxy and Cloudflare DNS for registration.3tched.com
# Uses credentials from ~/.bash_secrets

set -e

echo "=== Setting up registration.3tched.com ==="

# Load Cloudflare credentials
if [ -f ~/.bash_secrets ]; then
    source ~/.bash_secrets
    echo "✅ Loaded Cloudflare credentials from ~/.bash_secrets"
else
    echo "❌ ~/.bash_secrets not found"
    exit 1
fi

DOMAIN="registration.3tched.com"
PROXY_IP="15.235.37.41"  # Your public IP from priv_xray
WEB_PORT="7010"

echo "Domain: $DOMAIN"
echo "Proxy IP: $PROXY_IP"
echo "Web Port: $WEB_PORT"
echo ""

# Create nginx config directory if it doesn't exist
sudo mkdir -p /etc/nginx/http.d

# Install nginx config
echo "📋 Installing nginx reverse proxy config..."
sudo cp deploy/nginx/registration-3tched.conf /etc/nginx/http.d/registration-3tched.conf

# Test nginx config
echo "🔧 Testing nginx configuration..."
if sudo nginx -t; then
    echo "✅ Nginx config is valid"
else
    echo "❌ Nginx config has errors"
    exit 1
fi

# Create Cloudflare DNS record using API
echo "☁️  Creating Cloudflare DNS record for $DOMAIN..."

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

# Check if record already exists
EXISTING=$(curl -s "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records?type=A&name=registration.3tched.com" \
  -H "Authorization: Bearer ${CF_DNS_ZONE_TOKEN}" \
  -H "Content-Type: application/json" | jq -r '.result[0].id // empty')

if [ -n "$EXISTING" ]; then
    echo "  Updating existing record (id: $EXISTING)..."
    curl -s -X PUT "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records/${EXISTING}" \
      -H "Authorization: Bearer ${CF_DNS_ZONE_TOKEN}" \
      -H "Content-Type: application/json" \
      --data "{\"type\":\"A\",\"name\":\"registration\",\"content\":\"${PROXY_IP}\",\"ttl\":1,\"proxied\":true}" | jq '{success,errors}'
else
    echo "  Creating new record..."
    curl -s -X POST "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records" \
      -H "Authorization: Bearer ${CF_DNS_ZONE_TOKEN}" \
      -H "Content-Type: application/json" \
      --data "{\"type\":\"A\",\"name\":\"registration\",\"content\":\"${PROXY_IP}\",\"ttl\":1,\"proxied\":true}" | jq '{success,errors}'
fi

echo ""
echo "✅ Cloudflare DNS record created (proxied through Cloudflare)"

# Reload nginx
echo "🔄 Reloading nginx..."
sudo nginx -s reload

echo ""
echo "🎉 Setup complete!"
echo ""
echo "Your registration service is now available at:"
echo "https://registration.3tched.com"
echo ""
echo "Magic link endpoints:"
echo "• Signup: https://registration.3tched.com/api/privacy/signup"
echo "• Verify: https://registration.3tched.com/privacy/verify?token=XXX"
echo ""
echo "Make sure op-web is running on port 7010:"
echo "curl -I http://127.0.0.1:7010/health"
echo ""

# Make sure op-web listens on all interfaces
echo "⚠️  Make sure op-web is configured to listen on 0.0.0.0, not just 127.0.0.1"
echo "Check your environment: OP_DBUS_WEB_HOST=0.0.0.0"
