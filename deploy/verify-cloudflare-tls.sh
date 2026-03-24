#!/bin/bash
# Verify Cloudflare TLS Configuration
# Checks both local certificates and Cloudflare settings

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

DOMAIN="${DOMAIN:-ghostbridge.tech}"
CF_API_TOKEN="${CF_DNS_ZONE_TOKEN:-}"
CF_ZONE_ID="${CF_ZONE_ID:-44e82825999f76048b62999a8c0a1446}"

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           Cloudflare TLS Verification                       ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""

#===============================================================================
# 1. CHECK LOCAL CERTIFICATES
#===============================================================================

echo -e "${CYAN}[1] Local Certificate Status:${NC}"

CERT_LOCATIONS=(
    "/etc/ssl/cloudflare/origin.pem"
    "/etc/nginx/ssl/ghostbridge.crt"
    "/etc/letsencrypt/live/$DOMAIN/fullchain.pem"
    "/etc/pve/nodes/$(hostname)/pve-ssl.pem"
)

for cert in "${CERT_LOCATIONS[@]}"; do
    if [ -f "$cert" ]; then
        echo -e "  ${GREEN}✓${NC} $cert"
        
        # Get details
        subject=$(openssl x509 -in "$cert" -noout -subject 2>/dev/null | sed 's/subject=//')
        issuer=$(openssl x509 -in "$cert" -noout -issuer 2>/dev/null | sed 's/issuer=//')
        expiry=$(openssl x509 -in "$cert" -noout -enddate 2>/dev/null | sed 's/notAfter=//')
        
        echo "    Subject: $subject"
        echo "    Issuer:  $issuer"
        echo "    Expires: $expiry"
        
        # Check if Cloudflare
        if echo "$issuer" | grep -qi "cloudflare"; then
            echo -e "    Type:    ${GREEN}Cloudflare Origin Certificate${NC}"
        elif echo "$issuer" | grep -qi "let.*encrypt"; then
            echo -e "    Type:    ${GREEN}Let's Encrypt Certificate${NC}"
        elif echo "$issuer" | grep -qi "proxmox"; then
            echo -e "    Type:    ${YELLOW}Proxmox Self-Signed${NC}"
        fi
        
        # Check expiry
        if openssl x509 -in "$cert" -noout -checkend 0 2>/dev/null; then
            if openssl x509 -in "$cert" -noout -checkend 2592000 2>/dev/null; then
                echo -e "    Status:  ${GREEN}Valid${NC}"
            else
                echo -e "    Status:  ${YELLOW}Expiring within 30 days${NC}"
            fi
        else
            echo -e "    Status:  ${RED}EXPIRED${NC}"
        fi
        echo ""
    else
        echo -e "  ${YELLOW}○${NC} $cert (not found)"
    fi
done

#===============================================================================
# 2. CHECK CLOUDFLARE API SETTINGS
#===============================================================================

echo -e "${CYAN}[2] Cloudflare Zone Settings:${NC}"

if [ -n "$CF_API_TOKEN" ]; then
    # Get SSL mode
    ssl_mode=$(curl -s -X GET "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/settings/ssl" \
        -H "Authorization: Bearer $CF_API_TOKEN" \
        -H "Content-Type: application/json" | jq -r '.result.value')
    
    echo -n "  SSL Mode: "
    case "$ssl_mode" in
        "strict") echo -e "${GREEN}Full (Strict)${NC}" ;;
        "full") echo -e "${YELLOW}Full${NC}" ;;
        "flexible") echo -e "${RED}Flexible (insecure!)${NC}" ;;
        "off") echo -e "${RED}Off${NC}" ;;
        *) echo -e "${YELLOW}$ssl_mode${NC}" ;;
    esac
    
    # Get Always Use HTTPS
    always_https=$(curl -s -X GET "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/settings/always_use_https" \
        -H "Authorization: Bearer $CF_API_TOKEN" \
        -H "Content-Type: application/json" | jq -r '.result.value')
    
    echo -n "  Always HTTPS: "
    if [ "$always_https" = "on" ]; then
        echo -e "${GREEN}Enabled${NC}"
    else
        echo -e "${YELLOW}Disabled${NC}"
    fi
    
    # Get Min TLS version
    min_tls=$(curl -s -X GET "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/settings/min_tls_version" \
        -H "Authorization: Bearer $CF_API_TOKEN" \
        -H "Content-Type: application/json" | jq -r '.result.value')
    
    echo -e "  Min TLS Version: ${YELLOW}$min_tls${NC}"
    
    # Get TLS 1.3 status
    tls13=$(curl -s -X GET "https://api.cloudflare.com/client/v4/zones/$CF_ZONE_ID/settings/tls_1_3" \
        -H "Authorization: Bearer $CF_API_TOKEN" \
        -H "Content-Type: application/json" | jq -r '.result.value')
    
    echo -n "  TLS 1.3: "
    if [ "$tls13" = "on" ] || [ "$tls13" = "zrt" ]; then
        echo -e "${GREEN}Enabled${NC}"
    else
        echo -e "${YELLOW}$tls13${NC}"
    fi
else
    echo -e "  ${YELLOW}⚠ CF_DNS_ZONE_TOKEN not set - skipping API checks${NC}"
fi

#===============================================================================
# 3. CHECK HTTPS CONNECTIVITY
#===============================================================================

echo ""
echo -e "${CYAN}[3] HTTPS Connectivity Tests:${NC}"

# Test localhost
echo -n "  localhost:443 - "
if curl -sfk --connect-timeout 5 https://localhost/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ OK${NC}"
else
    echo -e "${YELLOW}⚠ Not responding${NC}"
fi

# Test via Cloudflare
SUBDOMAINS="proxmox op-web chat"
for sub in $SUBDOMAINS; do
    echo -n "  $sub.$DOMAIN - "
    if curl -sf --connect-timeout 10 "https://$sub.$DOMAIN/health" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ OK${NC}"
    elif curl -sf --connect-timeout 10 "https://$sub.$DOMAIN/" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ OK (no /health)${NC}"
    else
        echo -e "${YELLOW}⚠ Not responding${NC}"
    fi
done

#===============================================================================
# 4. CHECK SSL CERTIFICATE FROM SERVER
#===============================================================================

echo ""
echo -e "${CYAN}[4] SSL Handshake Test (via Cloudflare):${NC}"

echo -e "  Connecting to proxmox.$DOMAIN:443..."
echo | openssl s_client -connect "proxmox.$DOMAIN:443" -servername "proxmox.$DOMAIN" 2>/dev/null | \
    openssl x509 -noout -subject -issuer -dates 2>/dev/null | sed 's/^/    /'

#===============================================================================
# 5. CHECK SERVICES
#===============================================================================

echo ""
echo -e "${CYAN}[5] Service Status:${NC}"

for service in nginx op-web; do
    echo -n "  $service: "
    if systemctl is-active --quiet $service 2>/dev/null; then
        echo -e "${GREEN}running${NC}"
    else
        echo -e "${RED}not running${NC}"
    fi
done

#===============================================================================
# SUMMARY
#===============================================================================

echo ""
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
echo ""
echo -e "${CYAN}Recommendations:${NC}"

# Check SSL mode
if [ "$ssl_mode" != "strict" ] && [ -n "$ssl_mode" ]; then
    echo -e "  ${YELLOW}⚠${NC} Set Cloudflare SSL mode to 'Full (Strict)' for best security"
fi

# Check for Cloudflare Origin cert
if [ ! -f "/etc/ssl/cloudflare/origin.pem" ]; then
    echo -e "  ${YELLOW}⚠${NC} Consider using Cloudflare Origin Certificate for 15-year validity"
    echo -e "     Run: ${YELLOW}sudo ./deploy/setup-cloudflare-tls.sh${NC}"
fi

# Check for expiring certs
for cert in "${CERT_LOCATIONS[@]}"; do
    if [ -f "$cert" ]; then
        if ! openssl x509 -in "$cert" -noout -checkend 2592000 2>/dev/null; then
            echo -e "  ${YELLOW}⚠${NC} Certificate expiring soon: $cert"
        fi
    fi
done

echo ""
