#!/bin/bash
# Update DNS records for 3tched.com on Cloudflare
# Uses credentials from ~/.bash_secrets

set -e

# Load credentials
source ~/.bash_secrets

DOMAIN="3tched.com"
MAIL_IP="10.149.181.121"

# DKIM public key from Maddy
DKIM_VALUE="v=DKIM1; k=rsa; p=MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAxe8X0OkutPyJZVr7kRRU5nf2dMkqgzR/CnGL0OfXi6HkmuddSBPAe3S+Wu+11mE5smh5T52LZOt16jL6jAciZ5p5BNq1kLfxbZpqdWlACmp0Lbvc/x01CQTuy3X59UWWEH2jjC7jTEbBc4WmbFVtSPT8/gGoN/CTtzjHcavC6Eapb7JcSmZS7hVpsGizXuZFLhoRNO6GLbBVjwmpfu2oxIizL2sxyKWI1nwc7WdVIBfV4vAlKPl0Q9KvxvHkkKd62UdP5uJYa6Iwtl2nWkaSyWJjs2GCOHaGoqA9FymoMIGSScgxcpIjPsqaQZs5PbNjJpZnemlsjIA78aTdUE8MXQIDAQAB"

echo "🌐 Updating DNS records for $DOMAIN on Cloudflare..."

# Get zone ID for 3tched.com
echo "📍 Finding zone ID..."
ZONE_RESPONSE=$(curl -s -X GET "https://api.cloudflare.com/client/v4/zones?name=$DOMAIN" \
  -H "X-Auth-Email: $CF_EMAIL" \
  -H "X-Auth-Key: $CF_GLOBAL_API_KEY" \
  -H "Content-Type: application/json")

ZONE_ID=$(echo "$ZONE_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)

if [ -z "$ZONE_ID" ]; then
    echo "❌ Could not find zone ID for $DOMAIN"
    echo "Response: $ZONE_RESPONSE"
    exit 1
fi

echo "✅ Zone ID: $ZONE_ID"

# Function to create or update DNS record
update_dns_record() {
    local type=$1
    local name=$2
    local content=$3
    local priority=${4:-}

    echo ""
    echo "📝 Updating $type record: $name"

    # Check if record exists
    RECORD_RESPONSE=$(curl -s -X GET "https://api.cloudflare.com/client/v4/zones/$ZONE_ID/dns_records?type=$type&name=$name.$DOMAIN" \
      -H "X-Auth-Email: $CF_EMAIL" \
      -H "X-Auth-Key: $CF_GLOBAL_API_KEY" \
      -H "Content-Type: application/json")

    RECORD_ID=$(echo "$RECORD_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)

    # Prepare JSON payload
    if [ -n "$priority" ]; then
        JSON_DATA=$(cat <<EOF
{
  "type": "$type",
  "name": "$name",
  "content": "$content",
  "ttl": 3600,
  "priority": $priority
}
EOF
)
    else
        JSON_DATA=$(cat <<EOF
{
  "type": "$type",
  "name": "$name",
  "content": "$content",
  "ttl": 3600
}
EOF
)
    fi

    if [ -n "$RECORD_ID" ]; then
        # Update existing record
        echo "   Updating existing record..."
        RESULT=$(curl -s -X PUT "https://api.cloudflare.com/client/v4/zones/$ZONE_ID/dns_records/$RECORD_ID" \
          -H "X-Auth-Email: $CF_EMAIL" \
          -H "X-Auth-Key: $CF_GLOBAL_API_KEY" \
          -H "Content-Type: application/json" \
          --data "$JSON_DATA")
    else
        # Create new record
        echo "   Creating new record..."
        RESULT=$(curl -s -X POST "https://api.cloudflare.com/client/v4/zones/$ZONE_ID/dns_records" \
          -H "X-Auth-Email: $CF_EMAIL" \
          -H "X-Auth-Key: $CF_GLOBAL_API_KEY" \
          -H "Content-Type: application/json" \
          --data "$JSON_DATA")
    fi

    SUCCESS=$(echo "$RESULT" | grep -o '"success":[^,]*' | cut -d':' -f2)
    if [ "$SUCCESS" = "true" ]; then
        echo "   ✅ Success"
    else
        echo "   ❌ Failed: $RESULT"
    fi
}

# Add A record for mail server
update_dns_record "A" "mail" "$MAIL_IP"

# Add MX record
update_dns_record "MX" "@" "mail.$DOMAIN" 10

# Add SPF record
update_dns_record "TXT" "@" "v=spf1 mx ~all"

# Add DKIM record
update_dns_record "TXT" "default._domainkey" "$DKIM_VALUE"

# Add DMARC record
update_dns_record "TXT" "_dmarc" "v=DMARC1; p=quarantine; rua=mailto:admin@$DOMAIN"

echo ""
echo "✅ DNS records updated!"
echo ""
echo "📋 Summary:"
echo "   A record: mail.$DOMAIN → $MAIL_IP"
echo "   MX record: $DOMAIN → mail.$DOMAIN"
echo "   SPF: Configured"
echo "   DKIM: Configured"
echo "   DMARC: Configured"
echo ""
echo "⏱  DNS propagation may take a few minutes..."
echo ""
echo "Test with:"
echo "  dig MX $DOMAIN"
echo "  dig A mail.$DOMAIN"
echo "  dig TXT default._domainkey.$DOMAIN"
