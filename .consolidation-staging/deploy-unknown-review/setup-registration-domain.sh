#!/bin/bash
# Setup Cloudflare DNS for registration.3tched.com
# The backend transport is socket-projected through the bridge; this script
# only publishes the public DNS record for the already-provisioned service.

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
echo "Public transport: bridge-owned unix socket"
echo ""

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

echo ""
echo "🎉 Setup complete!"
echo ""
echo "Your registration service is now available at:"
echo "https://registration.3tched.com"
echo ""
echo "Magic link endpoints are expected to be served through the bridge-owned socket."
echo "Ensure the registration plugin is registered via the unix_socket plugin path."
