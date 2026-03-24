#!/bin/bash
# Setup DNS records in Cloudflare for ghostbridge.tech
# Uses the Cloudflare API

set -e

# Cloudflare credentials (from your bashrc)
CF_TOKEN="${CF_DNS_ZONE_TOKEN:-your-cloudflare-dns-zone-token-here}"
CF_ZONE_ID="${CF_ZONE_ID:-44e82825999f76048b62999a8c0a1446}"
SERVER_IP="80.209.240.244"

echo "🌐 Setting up Cloudflare DNS for ghostbridge.tech"
echo "Server IP: $SERVER_IP"
echo ""

# Function to create/update DNS record
create_dns_record() {
    local name=$1
    local proxied=${2:-true}
    
    echo "Creating DNS record: $name -> $SERVER_IP (proxied: $proxied)"
    
    # Check if record exists
    existing=$(curl -s -X GET "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/dns_records?type=A&name=$name.ghostbridge.tech" \
        -H "Authorization: Bearer $CF_TOKEN" \
        -H "Content-Type: application/json" | jq -r '.result[0].id // empty')
    
    if [ -n "$existing" ]; then
        echo "  Updating existing record..."
        curl -s -X PUT "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/dns_records/$existing" \
            -H "Authorization: Bearer $CF_TOKEN" \
            -H "Content-Type: application/json" \
            --data '{"type":"A","name":"'$name'","content":"'$SERVER_IP'","ttl":1,"proxied":'$proxied'}' | jq -r '.success'
    else
        echo "  Creating new record..."
        curl -s -X POST "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/dns_records" \
            -H "Authorization: Bearer $CF_TOKEN" \
            -H "Content-Type: application/json" \
            --data '{"type":"A","name":"'$name'","content":"'$SERVER_IP'","ttl":1,"proxied":'$proxied'}' | jq -r '.success'
    fi
}

# Create DNS records
echo "Creating DNS records..."
echo ""

# Root domain
echo "1. Root domain (@)"
create_dns_record "@" true

# WWW subdomain
echo "2. www subdomain"
create_dns_record "www" true

# Chat subdomain
echo "3. chat subdomain"
create_dns_record "chat" true

# op-web subdomain
echo "4. op-web subdomain"
create_dns_record "op-web" true

# Proxmox subdomain (not proxied - direct connection to port 8006)
echo "5. proxmox subdomain (DNS only, not proxied)"
create_dns_record "proxmox" false

# MCP subdomain
echo "6. mcp subdomain"
create_dns_record "mcp" true

# Agents subdomain
echo "7. agents subdomain"
create_dns_record "agents" true

echo ""
echo "✅ DNS setup complete!"
echo ""
echo "Records created:"
echo "  - ghostbridge.tech          -> $SERVER_IP (proxied)"
echo "  - www.ghostbridge.tech      -> $SERVER_IP (proxied)"
echo "  - chat.ghostbridge.tech     -> $SERVER_IP (proxied)"
echo "  - op-web.ghostbridge.tech   -> $SERVER_IP (proxied)"
echo "  - proxmox.ghostbridge.tech  -> $SERVER_IP (DNS only)"
echo "  - mcp.ghostbridge.tech      -> $SERVER_IP (proxied)"
echo "  - agents.ghostbridge.tech   -> $SERVER_IP (proxied)"
echo ""
echo "Note: DNS propagation may take a few minutes."
echo "Test with: dig ghostbridge.tech"
