#!/bin/bash
# Setup op-web Chat Server with Existing Proxmox Server

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

install_packages() {
    if command -v op-packagekit-install &> /dev/null; then
        op-packagekit-install "$@"
        return
    fi

    sudo apt update
    sudo apt install -y "$@"
}

echo -e "${GREEN}================================================${NC}"
echo -e "${GREEN}  op-web Chat - Proxmox Integration Setup${NC}"
echo -e "${GREEN}================================================${NC}"
echo ""

# Check if running as root
if [ "$EUID" -eq 0 ]; then 
    echo -e "${RED}Don't run as root! Use sudo when prompted.${NC}"
    exit 1
fi

echo -e "${BLUE}Current setup:${NC}"
echo "  - Proxmox Web UI: https://proxmox.ghostbridge.tech:8006"
echo "  - Server IP: 80.209.240.244"
echo "  - Chat will be: https://proxmox.ghostbridge.tech/chat"
echo ""

# Install Nginx
echo -e "${YELLOW}Step 1: Installing Nginx...${NC}"
if ! command -v nginx &> /dev/null; then
    install_packages nginx
    echo -e "${GREEN}✓ Nginx installed${NC}"
else
    echo -e "${GREEN}✓ Nginx already installed${NC}"
fi

# Check for existing SSL certificates
echo -e "\n${YELLOW}Step 2: Checking SSL certificates...${NC}"
CERT_PATH="/etc/pve/nodes/proxmox/pve-ssl.pem"
KEY_PATH="/etc/pve/nodes/proxmox/pve-ssl.key"

if [ -f "$CERT_PATH" ]; then
    echo -e "${GREEN}✓ Found Proxmox SSL certificate${NC}"
    
    # Copy certs to standard location for nginx
    sudo mkdir -p /etc/nginx/ssl
    sudo cp "$CERT_PATH" /etc/nginx/ssl/ghostbridge.crt
    sudo cp "$KEY_PATH" /etc/nginx/ssl/ghostbridge.key 2>/dev/null || {
        echo -e "${YELLOW}⚠ Private key not found at expected location${NC}"
        echo "Looking for key in /etc/pve/nodes/proxmox/"
        sudo find /etc/pve/nodes/proxmox/ -name "*.key" -exec cp {} /etc/nginx/ssl/ghostbridge.key \;
    }
    
    # Check if we got the files
    if [ -f "/etc/nginx/ssl/ghostbridge.crt" ] && [ -f "/etc/nginx/ssl/ghostbridge.key" ]; then
        echo -e "${GREEN}✓ SSL certificates copied to nginx${NC}"
        USE_EXISTING_CERT=true
    else
        echo -e "${YELLOW}⚠ Could not copy SSL certificates${NC}"
        USE_EXISTING_CERT=false
    fi
else
    echo -e "${YELLOW}⚠ No existing SSL certificate found${NC}"
    USE_EXISTING_CERT=false
fi

# Configure Nginx
echo -e "\n${YELLOW}Step 3: Configuring Nginx...${NC}"
sudo tee /etc/nginx/sites-available/op-web << 'NGINX_EOF'
# op-web Chat Server
# Runs alongside Proxmox (which uses port 8006)

# Rate limiting
limit_req_zone $binary_remote_addr zone=chat_limit:10m rate=10r/s;

upstream op_web {
    server 127.0.0.1:8081;
    keepalive 32;
}

# HTTP to HTTPS redirect
server {
    listen 80;
    listen [::]:80;
    server_name proxmox.ghostbridge.tech;
    
    # Redirect to HTTPS
    return 301 https://$server_name$request_uri;
}

# HTTPS server
server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name proxmox.ghostbridge.tech;
    
    # SSL certificates
    ssl_certificate /etc/nginx/ssl/ghostbridge.crt;
    ssl_certificate_key /etc/nginx/ssl/ghostbridge.key;
    
    # SSL configuration
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 10m;
    
    # Security headers
    add_header Strict-Transport-Security "max-age=31536000" always;
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;
    
    # Logging
    access_log /var/log/nginx/op-web-access.log;
    error_log /var/log/nginx/op-web-error.log;
    
    # Root location - redirect to /chat/
    location = / {
        return 301 /chat/;
    }
    
    # Chat interface (prefix to avoid Proxmox conflicts)
    location /chat/ {
        limit_req zone=chat_limit burst=20 nodelay;
        
        proxy_pass http://op_web/;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        proxy_buffering off;
        proxy_read_timeout 300s;
        proxy_connect_timeout 75s;
    }
    
    # API endpoints
    location /api/ {
        proxy_pass http://op_web/api/;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
    
    # WebSocket
    location /ws {
        proxy_pass http://op_web/ws;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        
        proxy_read_timeout 7d;
        proxy_send_timeout 7d;
    }
    
    # Static files with caching
    location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg|woff|woff2|ttf|eot)$ {
        proxy_pass http://op_web;
        expires 1y;
        add_header Cache-Control "public, immutable";
    }
}
NGINX_EOF

echo -e "${GREEN}✓ Nginx configuration created${NC}"

# Enable site
sudo ln -sf /etc/nginx/sites-available/op-web /etc/nginx/sites-enabled/
sudo rm -f /etc/nginx/sites-enabled/default  # Remove default site

# Test nginx config
echo -e "\n${YELLOW}Step 4: Testing Nginx configuration...${NC}"
if sudo nginx -t; then
    echo -e "${GREEN}✓ Nginx configuration valid${NC}"
else
    echo -e "${RED}✗ Nginx configuration error${NC}"
    exit 1
fi

# Create environment file
echo -e "\n${YELLOW}Step 5: Creating environment file...${NC}"
cat > ~/.op-web.env << EOF
HF_TOKEN=${HF_TOKEN}
GITHUB_PERSONAL_ACCESS_TOKEN=${GH_TOKEN}
MCP_CONFIG_FILE=/home/jeremy/op-dbus-v2/crates/op-mcp/mcp-config.json
HUGGINGFACE_API_KEY=${HF_TOKEN}
CLOUDFLARE_API_TOKEN=${CF_DNS_ZONE_TOKEN}
CLOUDFLARE_ACCOUNT_ID=${CF_ACCOUNT_ID}
PINECONE_API_KEY=${PINECONE_API_KEY}
EOF
chmod 600 ~/.op-web.env
echo -e "${GREEN}✓ Environment file created${NC}"

# Install systemd service
echo -e "\n${YELLOW}Step 6: Installing systemd service...${NC}"
sudo cp /home/jeremy/op-dbus-v2/deploy/systemd/op-web.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable op-web.service
echo -e "${GREEN}✓ Systemd service installed${NC}"

# Start services
echo -e "\n${YELLOW}Step 7: Starting services...${NC}"

# Start op-web
sudo systemctl start op-web.service
sleep 2

if sudo systemctl is-active --quiet op-web.service; then
    echo -e "${GREEN}✓ op-web service started${NC}"
else
    echo -e "${RED}✗ op-web service failed to start${NC}"
    echo "Check logs: sudo journalctl -u op-web -n 50"
    exit 1
fi

# Start/restart nginx
sudo systemctl restart nginx
sudo systemctl enable nginx

if sudo systemctl is-active --quiet nginx; then
    echo -e "${GREEN}✓ Nginx started${NC}"
else
    echo -e "${RED}✗ Nginx failed to start${NC}"
    exit 1
fi

# Configure firewall
echo -e "\n${YELLOW}Step 8: Configuring firewall...${NC}"
if command -v ufw &> /dev/null; then
    sudo ufw allow 80/tcp
    sudo ufw allow 443/tcp
    sudo ufw allow 8006/tcp  # Keep Proxmox accessible
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

# Test connection
echo -e "\n${YELLOW}Step 9: Testing connections...${NC}"
sleep 2

if curl -s http://localhost:8080/api/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ op-web backend responding${NC}"
else
    echo -e "${YELLOW}⚠ Backend not responding yet (may need a moment)${NC}"
fi

# Final status
echo ""
echo -e "${GREEN}================================================${NC}"
echo -e "${GREEN}  Setup Complete!${NC}"
echo -e "${GREEN}================================================${NC}"
echo ""
echo -e "${GREEN}Your services are now available:${NC}"
echo ""
echo -e "  ${BLUE}Chat Server:${NC}"
echo -e "    https://proxmox.ghostbridge.tech/chat/"
echo -e "    https://proxmox.ghostbridge.tech/chat/chat.html"
echo ""
echo -e "  ${BLUE}Proxmox Web UI (unchanged):${NC}"
echo -e "    https://proxmox.ghostbridge.tech:8006"
echo ""
echo -e "${GREEN}Service Management:${NC}"
echo -e "  Status:  ${YELLOW}sudo systemctl status op-web${NC}"
echo -e "  Logs:    ${YELLOW}sudo journalctl -u op-web -f${NC}"
echo -e "  Restart: ${YELLOW}sudo systemctl restart op-web${NC}"
echo ""
echo -e "  Nginx:   ${YELLOW}sudo systemctl status nginx${NC}"
echo -e "  Logs:    ${YELLOW}sudo tail -f /var/log/nginx/op-web-access.log${NC}"
echo ""
echo -e "${GREEN}Test it:${NC}"
echo -e "  ${YELLOW}curl https://proxmox.ghostbridge.tech/chat/api/health${NC}"
echo ""

if [ "$USE_EXISTING_CERT" = true ]; then
    echo -e "${BLUE}Note: Using existing Proxmox SSL certificate${NC}"
    echo -e "${YELLOW}For a proper certificate, consider:${NC}"
    echo -e "  sudo certbot --nginx -d proxmox.ghostbridge.tech"
    echo ""
fi
