#!/bin/bash
# Setup op-web Chat Server on Public Domain

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}================================================${NC}"
echo -e "${GREEN}  op-web Chat Server - Public Domain Setup${NC}"
echo -e "${GREEN}================================================${NC}"
echo ""

# Check if running as root for system setup
if [ "$EUID" -eq 0 ]; then 
    echo -e "${RED}Don't run as root! Use sudo when prompted.${NC}"
    exit 1
fi

# Get domain name
read -p "Enter your domain name (e.g., chat.example.com): " DOMAIN
if [ -z "$DOMAIN" ]; then
    echo -e "${RED}Domain name required!${NC}"
    exit 1
fi

echo ""
echo -e "${YELLOW}Domain:${NC} $DOMAIN"
echo ""

# Ask for setup type
echo "Choose reverse proxy:"
echo "1) Caddy (recommended - automatic HTTPS)"
echo "2) Nginx (manual HTTPS with certbot)"
echo "3) Skip (I'll configure manually)"
read -p "Choice [1-3]: " PROXY_CHOICE

# Create environment file
echo -e "\n${YELLOW}Creating environment file...${NC}"
cat > ~/.op-web.env << EOF
# op-web Environment Variables
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
echo -e "\n${YELLOW}Installing systemd service...${NC}"
sudo cp /home/jeremy/op-dbus-v2/deploy/systemd/op-web.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable op-web.service
echo -e "${GREEN}✓ Systemd service installed${NC}"

# Setup based on choice
case $PROXY_CHOICE in
    1)
        echo -e "\n${YELLOW}Setting up Caddy...${NC}"
        
        # Install Caddy if not present
        if ! command -v caddy &> /dev/null; then
            echo "Installing Caddy..."
            sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https curl
            curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
            curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | sudo tee /etc/apt/sources.list.d/caddy-stable.list
            sudo apt update
            sudo apt install -y caddy
        fi
        
        # Configure Caddy
        sudo mkdir -p /etc/caddy
        sed "s/\${DOMAIN:localhost}/$DOMAIN/g" /home/jeremy/op-dbus-v2/deploy/caddy/Caddyfile | sudo tee /etc/caddy/Caddyfile > /dev/null
        
        # Create log directory
        sudo mkdir -p /var/log/caddy
        sudo chown caddy:caddy /var/log/caddy
        
        # Restart Caddy
        sudo systemctl restart caddy
        sudo systemctl enable caddy
        
        echo -e "${GREEN}✓ Caddy configured and started${NC}"
        echo -e "${GREEN}✓ HTTPS will be automatically configured!${NC}"
        ;;
        
    2)
        echo -e "\n${YELLOW}Setting up Nginx...${NC}"
        
        # Install nginx and certbot
        if ! command -v nginx &> /dev/null; then
            echo "Installing Nginx..."
            sudo apt update
            sudo apt install -y nginx
        fi
        
        if ! command -v certbot &> /dev/null; then
            echo "Installing Certbot..."
            sudo apt install -y certbot python3-certbot-nginx
        fi
        
        # Configure nginx
        sudo mkdir -p /var/www/certbot
        sed "s/your-domain.com/$DOMAIN/g" /home/jeremy/op-dbus-v2/deploy/nginx/op-web.conf | sudo tee /etc/nginx/sites-available/op-web.conf > /dev/null
        
        # Enable site (without SSL first)
        sudo ln -sf /etc/nginx/sites-available/op-web.conf /etc/nginx/sites-enabled/
        
        # Test nginx config
        sudo nginx -t
        
        echo -e "\n${YELLOW}Getting SSL certificate...${NC}"
        echo "Make sure your DNS points to this server!"
        read -p "Press Enter when ready..."
        
        # Get certificate
        sudo certbot --nginx -d "$DOMAIN" --non-interactive --agree-tos --email "admin@$DOMAIN" || {
            echo -e "${RED}SSL certificate failed. Run manually:${NC}"
            echo "sudo certbot --nginx -d $DOMAIN"
        }
        
        # Reload nginx
        sudo systemctl reload nginx
        sudo systemctl enable nginx
        
        echo -e "${GREEN}✓ Nginx configured${NC}"
        ;;
        
    3)
        echo -e "${YELLOW}Skipping reverse proxy setup${NC}"
        echo "Configuration files are in: /home/jeremy/op-dbus-v2/deploy/"
        ;;
        
    *)
        echo -e "${RED}Invalid choice${NC}"
        exit 1
        ;;
esac

# Start op-web service
echo -e "\n${YELLOW}Starting op-web service...${NC}"
sudo systemctl start op-web.service

# Wait for service to start
sleep 3

# Check status
if sudo systemctl is-active --quiet op-web.service; then
    echo -e "${GREEN}✓ op-web service is running!${NC}"
else
    echo -e "${RED}✗ Service failed to start. Check logs:${NC}"
    echo "sudo journalctl -u op-web.service -n 50"
    exit 1
fi

# Firewall setup
echo -e "\n${YELLOW}Configuring firewall...${NC}"
if command -v ufw &> /dev/null; then
    sudo ufw allow 80/tcp
    sudo ufw allow 443/tcp
    sudo ufw allow 22/tcp  # Don't lock yourself out!
    echo -e "${GREEN}✓ UFW rules added${NC}"
elif command -v firewall-cmd &> /dev/null; then
    sudo firewall-cmd --permanent --add-service=http
    sudo firewall-cmd --permanent --add-service=https
    sudo firewall-cmd --reload
    echo -e "${GREEN}✓ Firewall rules added${NC}"
else
    echo -e "${YELLOW}No firewall detected. Make sure ports 80 and 443 are open!${NC}"
fi

# Final instructions
echo ""
echo -e "${GREEN}================================================${NC}"
echo -e "${GREEN}  Setup Complete!${NC}"
echo -e "${GREEN}================================================${NC}"
echo ""
echo -e "${GREEN}Your chat server is now available at:${NC}"
echo -e "  ${YELLOW}https://$DOMAIN${NC}"
echo ""
echo -e "${GREEN}Next steps:${NC}"
echo "1. Make sure DNS points to this server's IP"
if [ "$PROXY_CHOICE" = "1" ]; then
    echo "2. Caddy will auto-configure HTTPS (wait a few minutes)"
fi
echo "3. Open https://$DOMAIN/chat.html in your browser"
echo ""
echo -e "${GREEN}Useful commands:${NC}"
echo "  Status:  sudo systemctl status op-web"
echo "  Logs:    sudo journalctl -u op-web -f"
echo "  Restart: sudo systemctl restart op-web"
if [ "$PROXY_CHOICE" = "1" ]; then
    echo "  Caddy:   sudo systemctl status caddy"
    echo "           sudo journalctl -u caddy -f"
elif [ "$PROXY_CHOICE" = "2" ]; then
    echo "  Nginx:   sudo systemctl status nginx"
    echo "           sudo tail -f /var/log/nginx/op-web-access.log"
fi
echo ""
echo -e "${GREEN}Server IP:${NC}"
ip -4 addr show | grep inet | grep -v 127.0.0.1 | awk '{print "  " $2}'
echo ""
