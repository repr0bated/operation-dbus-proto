#!/bin/bash
# Setup DNS records in Cloudflare for 3tched.com
# Uses credentials from ~/.bash_secrets

set -e

if [ -f ~/.bash_secrets ]; then
    source ~/.bash_secrets
else
    echo "❌ ~/.bash_secrets not found"
    exit 1
fi

CF_TOKEN="${CF_DNS_ZONE_TOKEN}"
ZONE_ID="${CF_ZONEID_3TCHEDCOM:-${CF_ZONE_ID_3TCHED:-${CF_ZONEID_3TCHED:-}}}"
SERVER_IP="${1:-15.235.37.41}"

if [ -z "$ZONE_ID" ]; then
    echo "❌ Could not find 3tched.com zone ID in ~/.bash_secrets"
    echo "   Set CF_ZONEID_3TCHEDCOM in ~/.bash_secrets"
    echo ""
    echo "   To find your zone ID:"
    echo "   curl -s https://api.cloudflare.com/client/v4/zones \\"
    echo "     -H 'Authorization: Bearer \$CF_DNS_ZONE_TOKEN' | jq '.result[] | {name,id}'"
    exit 1
fi

echo "Setting up Cloudflare DNS for 3tched.com"
echo "Server IP: $SERVER_IP"
echo ""

# Function to create/update an A record under 3tched.com
create_dns_record() {
    local subdomain=$1
    local proxied=${2:-true}
    local fqdn="${subdomain}.3tched.com"

    echo "Upserting: $fqdn -> $SERVER_IP (proxied: $proxied)"

    existing=$(curl -s -X GET "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records?type=A&name=${fqdn}" \
        -H "Authorization: Bearer ${CF_TOKEN}" \
        -H "Content-Type: application/json" | jq -r '.result[0].id // empty')

    if [ -n "$existing" ]; then
        curl -s -X PUT "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records/${existing}" \
            -H "Authorization: Bearer ${CF_TOKEN}" \
            -H "Content-Type: application/json" \
            --data "{\"type\":\"A\",\"name\":\"${subdomain}\",\"content\":\"${SERVER_IP}\",\"ttl\":1,\"proxied\":${proxied}}" \
            | jq '{success,errors}'
    else
        curl -s -X POST "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/dns_records" \
            -H "Authorization: Bearer ${CF_TOKEN}" \
            -H "Content-Type: application/json" \
            --data "{\"type\":\"A\",\"name\":\"${subdomain}\",\"content\":\"${SERVER_IP}\",\"ttl\":1,\"proxied\":${proxied}}" \
            | jq '{success,errors}'
    fi
}

create_dns_record "mail" true
create_dns_record "registration" true

echo ""
echo "DNS setup complete!"
echo ""
echo "Records created/updated:"
echo "  - mail.3tched.com         -> $SERVER_IP (proxied)"
echo "  - registration.3tched.com -> $SERVER_IP (proxied)"
echo ""
echo "Test with:"
echo "  dig mail.3tched.com"
echo "  dig registration.3tched.com"
