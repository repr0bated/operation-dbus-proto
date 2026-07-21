#!/bin/bash
# Setup script for ghostbridge.tech public access
# Run this on the Proxmox server (80.209.240.244)

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${GREEN}================================================${NC}"
echo -e "${GREEN}  ghostbridge.tech Public Access Setup${NC}"
echo -e "${GREEN}================================================${NC}"
echo ""

# Check if running as root
if [ "$EUID" -eq 0 ]; then 
    echo -e "${RED}Don't run as root! Use sudo when prompted.${NC}"
    exit 1
fi

SERVER_IP="80.209.240.244"
DOMAIN="ghostbridge.tech"
OP_WEB_DIR="/home/jeremy/op-dbus-v2"

echo -e "${BLUE}Server IP:${NC} $SERVER_IP"
echo -e "${BLUE}Domain:${NC} $DOMAIN"
echo ""

# Step 1: Ensure nginx is installed
echo -e "${YELLOW}Step 1: Checking Nginx...${NC}"
if ! command -v nginx &> /dev/null; then
    echo "Installing nginx..."
    sudo apt update
    sudo apt install -y nginx
fi
echo -e "${GREEN}✓ Nginx installed${NC}"

# Step 2: Create SSL directory
echo -e "\n${YELLOW}Step 2: Setting up SSL certificates...${NC}"
sudo mkdir -p /etc/nginx/ssl

# Check for existing certs
if [ -f "/etc/nginx/ssl/ghostbridge.crt" ]; then
    echo -e "${GREEN}✓ SSL certificates already exist${NC}"
else
    # Try to copy from Proxmox
    if [ -f "/etc/pve/nodes/proxmox/pve-ssl.pem" ]; then
        echo "Copying Proxmox SSL certificates..."
        sudo cp /etc/pve/nodes/proxmox/pve-ssl.pem /etc/nginx/ssl/ghostbridge.crt
        sudo cp /etc/pve/nodes/proxmox/pve-ssl.key /etc/nginx/ssl/ghostbridge.key 2>/dev/null || \
            sudo find /etc/pve -name "*.key" -exec cp {} /etc/nginx/ssl/ghostbridge.key \; 2>/dev/null
        echo -e "${GREEN}✓ Copied Proxmox certificates${NC}"
    else
        echo -e "${YELLOW}⚠ No SSL certificates found${NC}"
        echo "Creating self-signed certificate (recommend getting Let's Encrypt later)..."
        sudo openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
            -keyout /etc/nginx/ssl/ghostbridge.key \
            -out /etc/nginx/ssl/ghostbridge.crt \
            -subj "/CN=$DOMAIN/O=GhostBridge/C=US"
        echo -e "${GREEN}✓ Self-signed certificate created${NC}"
    fi
fi

sudo chmod 600 /etc/nginx/ssl/ghostbridge.key
sudo chmod 644 /etc/nginx/ssl/ghostbridge.crt

# Step 3: Install nginx configuration
echo -e "\n${YELLOW}Step 3: Installing Nginx configuration...${NC}"
sudo cp "$OP_WEB_DIR/deploy/nginx/ghostbridge-public.conf" /etc/nginx/sites-available/op-web
sudo ln -sf /etc/nginx/sites-available/op-web /etc/nginx/sites-enabled/
sudo rm -f /etc/nginx/sites-enabled/default
echo -e "${GREEN}✓ Nginx configuration installed${NC}"

# Step 4: Test nginx configuration
echo -e "\n${YELLOW}Step 4: Testing Nginx configuration...${NC}"
if sudo nginx -t; then
    echo -e "${GREEN}✓ Nginx configuration valid${NC}"
else
    echo -e "${RED}✗ Nginx configuration error${NC}"
    exit 1
fi

# Step 5: Create certbot webroot
echo -e "\n${YELLOW}Step 5: Creating certbot webroot...${NC}"
sudo mkdir -p /var/www/certbot
echo -e "${GREEN}✓ Certbot webroot created${NC}"

# Step 6: Ensure op-web service is set up
echo -e "\n${YELLOW}Step 6: Setting up op-web service...${NC}"
if [ -f "/etc/systemd/system/op-web.service" ]; then
    echo -e "${GREEN}✓ op-web service already exists${NC}"
else
    sudo cp "$OP_WEB_DIR/deploy/systemd/op-web.service" /etc/systemd/system/
    sudo systemctl daemon-reload
    sudo systemctl enable op-web
    echo -e "${GREEN}✓ op-web service installed${NC}"
fi

# Step 7: Start/restart services
echo -e "\n${YELLOW}Step 7: Starting services...${NC}"

# Start op-web
if sudo systemctl is-active --quiet op-web; then
    sudo systemctl restart op-web
    echo -e "${GREEN}✓ op-web restarted${NC}"
else
    sudo systemctl start op-web
    echo -e "${GREEN}✓ op-web started${NC}"
fi

sleep 2

# Check if backend is responding
if curl -s http://localhost:8080/api/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ op-web backend responding${NC}"
else
    echo -e "${YELLOW}⚠ op-web backend not responding yet${NC}"
fi

# Restart nginx
sudo systemctl restart nginx
sudo systemctl enable nginx
echo -e "${GREEN}✓ Nginx restarted${NC}"

# Step 8: Configure firewall
echo -e "\n${YELLOW}Step 8: Configuring firewall...${NC}"
if command -v ufw &> /dev/null; then
    sudo ufw allow 80/tcp
    sudo ufw allow 443/tcp
    sudo ufw allow 8006/tcp  # Proxmox
    echo -e "${GREEN}✓ UFW rules added${NC}"
elif command -v firewall-cmd &> /dev/null; then
    sudo firewall-cmd --permanent --add-service=http
    sudo firewall-cmd --permanent --add-service=https
    sudo firewall-cmd --permanent --add-port=8006/tcp
    sudo firewall-cmd --reload
    echo -e "${GREEN}✓ Firewall rules added${NC}"
else
    echo -e "${YELLOW}⚠ No firewall detected${NC}"
fi

# Step 9: Test connectivity
echo -e "\n${YELLOW}Step 9: Testing connectivity...${NC}"
sleep 2

echo "Testing local backend..."
if curl -s http://localhost:8080/api/health | grep -q "healthy\|ok\|status"; then
    echo -e "${GREEN}✓ Backend health check passed${NC}"
else
    echo -e "${YELLOW}⚠ Backend health check inconclusive${NC}"
fi

echo "Testing HTTPS..."
if curl -sk https://localhost/api/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ HTTPS working${NC}"
else
    echo -e "${YELLOW}⚠ HTTPS not responding yet${NC}"
fi

# Final summary
echo ""
echo -e "${GREEN}================================================${NC}"
echo -e "${GREEN}  Setup Complete!${NC}"
echo -e "${GREEN}================================================${NC}"
echo ""
echo -e "${GREEN}Your services are now available at:${NC}"
echo ""
echo -e "  ${BLUE}Chat Interface:${NC}"
echo -e "    https://$DOMAIN/chat/"
echo -e "    https://chat.$DOMAIN/"
echo -e "    https://op-web.$DOMAIN/"
echo ""
echo -e "  ${BLUE}API:${NC}"
echo -e "    https://$DOMAIN/api/health"
echo -e "    https://$DOMAIN/api/tools"
echo -e "    https://$DOMAIN/api/status"
echo ""
echo -e "  ${BLUE}WebSocket:${NC}"
echo -e "    wss://$DOMAIN/ws"
echo ""
echo -e "  ${BLUE}MCP Protocol:${NC}"
echo -e "    https://$DOMAIN/mcp"
echo ""
echo -e "  ${BLUE}Proxmox:${NC}"
echo -e "    https://proxmox.$DOMAIN:8006"
echo ""
echo -e "${GREEN}Service Management:${NC}"
echo -e "  Status:  ${YELLOW}sudo systemctl status op-web nginx${NC}"
echo -e "  Logs:    ${YELLOW}sudo journalctl -u op-web -f${NC}"
echo -e "  Restart: ${YELLOW}sudo systemctl restart op-web nginx${NC}"
echo ""
echo -e "${BLUE}For proper SSL certificates, run:${NC}"
echo -e "  ${YELLOW}sudo apt install certbot python3-certbot-nginx${NC}"
echo -e "  ${YELLOW}sudo certbot --nginx -d $DOMAIN -d chat.$DOMAIN -d op-web.$DOMAIN${NC}"
echo ""
