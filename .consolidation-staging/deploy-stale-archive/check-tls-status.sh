#!/bin/bash
# TLS Certificate Status Check
# Quick diagnostic for TLS configuration

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           TLS Certificate Status Check                      ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""

#===============================================================================
# CHECK NGINX SSL CONFIGURATION
#===============================================================================

echo -e "${BLUE}[1] Nginx SSL Configuration:${NC}"

if [ -f /etc/nginx/sites-enabled/op-web ]; then
    echo -e "  ${GREEN}✓${NC} op-web site enabled"
    
    # Extract SSL paths
    ssl_cert=$(grep -oP 'ssl_certificate\s+\K[^;]+' /etc/nginx/sites-enabled/op-web 2>/dev/null | head -1)
    ssl_key=$(grep -oP 'ssl_certificate_key\s+\K[^;]+' /etc/nginx/sites-enabled/op-web 2>/dev/null | head -1)
    
    if [ -n "$ssl_cert" ]; then
        echo -e "  Certificate path: ${YELLOW}$ssl_cert${NC}"
        
        if [ -f "$ssl_cert" ]; then
            echo -e "  ${GREEN}✓${NC} Certificate file exists"
            
            # Get certificate details
            subject=$(openssl x509 -in "$ssl_cert" -noout -subject 2>/dev/null | sed 's/subject=//')
            issuer=$(openssl x509 -in "$ssl_cert" -noout -issuer 2>/dev/null | sed 's/issuer=//')
            expiry=$(openssl x509 -in "$ssl_cert" -noout -enddate 2>/dev/null | sed 's/notAfter=//')
            
            echo -e "  Subject: ${YELLOW}$subject${NC}"
            echo -e "  Issuer:  ${YELLOW}$issuer${NC}"
            echo -e "  Expires: ${YELLOW}$expiry${NC}"
            
            # Check if expired
            if openssl x509 -in "$ssl_cert" -noout -checkend 0 2>/dev/null; then
                echo -e "  ${GREEN}✓${NC} Certificate is VALID (not expired)"
                
                # Check if expiring soon (30 days)
                if ! openssl x509 -in "$ssl_cert" -noout -checkend 2592000 2>/dev/null; then
                    echo -e "  ${YELLOW}⚠${NC} Certificate expires within 30 days!"
                fi
            else
                echo -e "  ${RED}✗${NC} Certificate is EXPIRED!"
            fi
        else
            echo -e "  ${RED}✗${NC} Certificate file NOT FOUND"
        fi
    else
        echo -e "  ${YELLOW}⚠${NC} No SSL certificate configured"
    fi
    
    if [ -n "$ssl_key" ]; then
        echo -e "  Key path: ${YELLOW}$ssl_key${NC}"
        
        if [ -f "$ssl_key" ]; then
            echo -e "  ${GREEN}✓${NC} Key file exists"
            
            # Verify key matches certificate
            if [ -f "$ssl_cert" ]; then
                cert_mod=$(openssl x509 -in "$ssl_cert" -noout -modulus 2>/dev/null | md5sum | awk '{print $1}')
                key_mod=$(openssl rsa -in "$ssl_key" -noout -modulus 2>/dev/null | md5sum | awk '{print $1}')
                
                if [ "$cert_mod" = "$key_mod" ]; then
                    echo -e "  ${GREEN}✓${NC} Key matches certificate"
                else
                    echo -e "  ${RED}✗${NC} Key does NOT match certificate!"
                fi
            fi
        else
            echo -e "  ${RED}✗${NC} Key file NOT FOUND"
        fi
    fi
else
    echo -e "  ${RED}✗${NC} op-web site not enabled in nginx"
fi

#===============================================================================
# CHECK CERTIFICATE LOCATIONS
#===============================================================================

echo ""
echo -e "${BLUE}[2] Certificate Locations:${NC}"

locations=(
    "/etc/nginx/ssl/ghostbridge.crt"
    "/etc/nginx/ssl/ghostbridge.key"
    "/media/home/jeremy/certs/chat_cert.pem"
    "/media/home/jeremy/certs/chat_key.pem"
    "/etc/letsencrypt/live/proxmox.ghostbridge.tech/fullchain.pem"
    "/etc/letsencrypt/live/ghostbridge.tech/fullchain.pem"
    "/etc/pve/nodes/$(hostname)/pve-ssl.pem"
    "/home/jeremy/certs/ghostbridge.crt"
)

for loc in "${locations[@]}"; do
    if [ -f "$loc" ]; then
        echo -e "  ${GREEN}✓${NC} $loc"
        
        # If it's a certificate, show expiry
        if [[ "$loc" == *.crt ]] || [[ "$loc" == *.pem ]] && [[ "$loc" != *.key ]]; then
            if openssl x509 -in "$loc" -noout 2>/dev/null; then
                exp=$(openssl x509 -in "$loc" -noout -enddate 2>/dev/null | sed 's/notAfter=//')
                echo -e "    Expires: $exp"
            fi
        fi
    else
        echo -e "  ${YELLOW}○${NC} $loc (not found)"
    fi
done

#===============================================================================
# CHECK /media/ FOR CERTIFICATES
#===============================================================================

echo ""
echo -e "${BLUE}[3] Scanning /media/ for certificates:${NC}"

found_certs=0
for media_dir in /media/*; do
    if [ -d "$media_dir" ]; then
        certs=$(find "$media_dir" -type f \( -name "*.crt" -o -name "*.pem" -o -name "*cert*" \) 2>/dev/null | grep -v ".key" | head -5)
        if [ -n "$certs" ]; then
            echo -e "  ${GREEN}Found in $media_dir:${NC}"
            echo "$certs" | while read -r cert; do
                echo -e "    ${YELLOW}$cert${NC}"
                if openssl x509 -in "$cert" -noout 2>/dev/null; then
                    exp=$(openssl x509 -in "$cert" -noout -enddate 2>/dev/null | sed 's/notAfter=//')
                    echo -e "      Expires: $exp"
                fi
            done
            found_certs=1
        fi
    fi
done

if [ $found_certs -eq 0 ]; then
    echo -e "  ${YELLOW}○${NC} No certificates found in /media/"
fi

#===============================================================================
# CHECK SERVICES
#===============================================================================

echo ""
echo -e "${BLUE}[4] Service Status:${NC}"

for service in nginx op-web; do
    echo -n "  $service: "
    if systemctl is-active --quiet $service 2>/dev/null; then
        echo -e "${GREEN}running${NC}"
    elif systemctl is-enabled --quiet $service 2>/dev/null; then
        echo -e "${YELLOW}enabled but stopped${NC}"
    else
        echo -e "${RED}not running${NC}"
    fi
done

#===============================================================================
# CHECK HTTPS CONNECTIVITY
#===============================================================================

echo ""
echo -e "${BLUE}[5] HTTPS Connectivity:${NC}"

# Test localhost
echo -n "  localhost:443 - "
if curl -sfk --connect-timeout 5 https://localhost/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ OK${NC}"
else
    echo -e "${RED}✗ FAILED${NC}"
fi

# Test domain
echo -n "  proxmox.ghostbridge.tech - "
if curl -sfk --connect-timeout 5 https://proxmox.ghostbridge.tech/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ OK${NC}"
else
    echo -e "${YELLOW}⚠ Not accessible${NC}"
fi

#===============================================================================
# SHOW SSL HANDSHAKE
#===============================================================================

echo ""
echo -e "${BLUE}[6] SSL Handshake (localhost):${NC}"
echo | openssl s_client -connect localhost:443 -servername localhost 2>/dev/null | openssl x509 -noout -subject -issuer -dates 2>/dev/null | sed 's/^/  /'

echo ""
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
