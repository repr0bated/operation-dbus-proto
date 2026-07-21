#!/bin/bash
# Verify TLS configuration for op-dbus-v2

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== TLS Configuration Verification ===${NC}\n"

# Check certificate locations
echo -e "${BLUE}[1] Certificate Locations:${NC}"

CERT_LOCATIONS=(
    "/etc/nginx/ssl/ghostbridge.crt:/etc/nginx/ssl/ghostbridge.key"
    "/etc/letsencrypt/live/proxmox.ghostbridge.tech/fullchain.pem:/etc/letsencrypt/live/proxmox.ghostbridge.tech/privkey.pem"
    "/etc/letsencrypt/live/ghostbridge.tech/fullchain.pem:/etc/letsencrypt/live/ghostbridge.tech/privkey.pem"
    "/etc/pve/nodes/$(hostname)/pve-ssl.pem:/etc/pve/nodes/$(hostname)/pve-ssl.key"
)

for loc in "${CERT_LOCATIONS[@]}"; do
    cert=$(echo "$loc" | cut -d: -f1)
    key=$(echo "$loc" | cut -d: -f2)
    
    if [ -f "$cert" ] && [ -f "$key" ]; then
        echo -e "  ${GREEN}✓${NC} Found: $cert"
        # Show certificate info
        subject=$(openssl x509 -in "$cert" -noout -subject 2>/dev/null | sed 's/subject=//')
        expiry=$(openssl x509 -in "$cert" -noout -enddate 2>/dev/null | sed 's/notAfter=//')
        echo -e "    Subject: $subject"
        echo -e "    Expires: $expiry"
        
        # Check if expired
        if openssl x509 -in "$cert" -noout -checkend 0 2>/dev/null; then
            echo -e "    Status: ${GREEN}Valid${NC}"
        else
            echo -e "    Status: ${RED}EXPIRED${NC}"
        fi
    else
        echo -e "  ${YELLOW}○${NC} Not found: $cert"
    fi
done

# Check /media/ for certificates
echo -e "\n${BLUE}[2] Checking /media/ for certificates:${NC}"
found_media=false
for media_dir in /media/*; do
    if [ -d "$media_dir" ]; then
        certs=$(find "$media_dir" -name "*.crt" -o -name "*.pem" 2>/dev/null | head -5)
        if [ -n "$certs" ]; then
            echo -e "  ${GREEN}✓${NC} Found certificates in $media_dir:"
            echo "$certs" | while read -r cert; do
                echo -e "    - $cert"
            done
            found_media=true
        fi
    fi
done
if [ "$found_media" = false ]; then
    echo -e "  ${YELLOW}○${NC} No certificates found in /media/"
fi

# Check nginx configuration
echo -e "\n${BLUE}[3] Nginx Configuration:${NC}"
if [ -f /etc/nginx/sites-enabled/op-web ]; then
    echo -e "  ${GREEN}✓${NC} op-web site enabled"
    
    # Extract SSL paths from nginx config
    ssl_cert=$(grep -oP 'ssl_certificate\s+\K[^;]+' /etc/nginx/sites-enabled/op-web 2>/dev/null | head -1)
    ssl_key=$(grep -oP 'ssl_certificate_key\s+\K[^;]+' /etc/nginx/sites-enabled/op-web 2>/dev/null | head -1)
    
    if [ -n "$ssl_cert" ]; then
        echo -e "  Configured cert: $ssl_cert"
        echo -e "  Configured key:  $ssl_key"
    fi
else
    echo -e "  ${RED}✗${NC} op-web site not enabled"
fi

if sudo nginx -t 2>&1 | grep -q "successful"; then
    echo -e "  ${GREEN}✓${NC} Nginx config valid"
else
    echo -e "  ${RED}✗${NC} Nginx config invalid"
fi

# Check services
echo -e "\n${BLUE}[4] Service Status:${NC}"
for service in nginx op-web; do
    if systemctl is-active --quiet $service 2>/dev/null; then
        echo -e "  ${GREEN}✓${NC} $service is running"
    else
        echo -e "  ${RED}✗${NC} $service is not running"
    fi
done

# Test HTTPS connectivity
echo -e "\n${BLUE}[5] HTTPS Connectivity:${NC}"

# Test localhost
echo -n "  localhost:443 - "
if curl -sfk --connect-timeout 5 https://localhost/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ OK${NC}"
else
    echo -e "${YELLOW}⚠ Not responding${NC}"
fi

# Test with hostname
HOSTNAME=$(hostname -f 2>/dev/null || hostname)
echo -n "  $HOSTNAME:443 - "
if curl -sfk --connect-timeout 5 "https://$HOSTNAME/health" > /dev/null 2>&1; then
    echo -e "${GREEN}✓ OK${NC}"
else
    echo -e "${YELLOW}⚠ Not responding${NC}"
fi

# Show SSL certificate from server
echo -e "\n${BLUE}[6] Server Certificate (from TLS handshake):${NC}"
echo | openssl s_client -connect localhost:443 -servername localhost 2>/dev/null | openssl x509 -noout -subject -dates -issuer 2>/dev/null | sed 's/^/  /'

# Check ports
echo -e "\n${BLUE}[7] Port Status:${NC}"
for port in 80 443 8081; do
    if ss -tlnp | grep -q ":$port "; then
        process=$(ss -tlnp | grep ":$port " | grep -oP 'users:\(\("\K[^"]+' | head -1)
        echo -e "  ${GREEN}✓${NC} Port $port: $process"
    else
        echo -e "  ${YELLOW}○${NC} Port $port: not listening"
    fi
done

echo -e "\n${BLUE}=== Verification Complete ===${NC}"
