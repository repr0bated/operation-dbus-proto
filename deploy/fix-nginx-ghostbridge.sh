#!/bin/bash
# Fix nginx to serve Rust op-web-server instead of HuggingFace chat-ui

set -e

echo "🔧 Fixing Nginx configuration for ghostbridge.tech"
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

OP_WEB_DIR="/home/jeremy/op-dbus-v2"

# Step 1: Kill any old HuggingFace/Node processes
echo -e "${YELLOW}Step 1: Stopping old HuggingFace/Node processes...${NC}"
pkill -f 'chat-ui' 2>/dev/null || true
pkill -f 'vite' 2>/dev/null || true
pkill -f 'huggingface' 2>/dev/null || true
echo -e "${GREEN}✓ Old processes stopped${NC}"

# Step 2: Check if op-web-server is running
echo -e "\n${YELLOW}Step 2: Checking op-web-server...${NC}"
if pgrep -f 'op-web-server' > /dev/null; then
    echo -e "${GREEN}✓ op-web-server is running${NC}"
else
    echo -e "${YELLOW}Starting op-web-server...${NC}"
    cd "$OP_WEB_DIR"
    
    # Build if needed
    if [ ! -f "target/release/op-web-server" ]; then
        echo "Building op-web-server..."
        cargo build --release -p op-web
    fi
    
    # Start in background
    nohup ./target/release/op-web-server > /var/log/op-web-server.log 2>&1 &
    sleep 2
    
    if pgrep -f 'op-web-server' > /dev/null; then
        echo -e "${GREEN}✓ op-web-server started${NC}"
    else
        echo -e "${RED}✗ Failed to start op-web-server${NC}"
        echo "Check logs: tail -f /var/log/op-web-server.log"
        exit 1
    fi
fi

# Step 3: Test local backend
echo -e "\n${YELLOW}Step 3: Testing local backend...${NC}"
if curl -s http://localhost:8080/api/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Backend responding on port 8080${NC}"
else
    echo -e "${RED}✗ Backend not responding on port 8080${NC}"
    echo "Checking what's on port 8080:"
    sudo ss -tlnp | grep ':8080' || echo "Nothing on port 8080"
    exit 1
fi

# Step 4: Install clean nginx config
echo -e "\n${YELLOW}Step 4: Installing clean nginx config...${NC}"

# Backup existing config
if [ -f /etc/nginx/sites-available/op-web ]; then
    sudo cp /etc/nginx/sites-available/op-web /etc/nginx/sites-available/op-web.backup.$(date +%Y%m%d%H%M%S)
fi

# Install new config
sudo cp "$OP_WEB_DIR/deploy/nginx/op-web-clean.conf" /etc/nginx/sites-available/op-web

# Remove old configs that might conflict
sudo rm -f /etc/nginx/sites-enabled/default 2>/dev/null || true
sudo rm -f /etc/nginx/sites-enabled/chat-ui 2>/dev/null || true
sudo rm -f /etc/nginx/sites-enabled/huggingface 2>/dev/null || true

# Enable our config
sudo ln -sf /etc/nginx/sites-available/op-web /etc/nginx/sites-enabled/

echo -e "${GREEN}✓ Nginx config installed${NC}"

# Step 5: Test nginx config
echo -e "\n${YELLOW}Step 5: Testing nginx config...${NC}"
if sudo nginx -t; then
    echo -e "${GREEN}✓ Nginx config valid${NC}"
else
    echo -e "${RED}✗ Nginx config invalid${NC}"
    exit 1
fi

# Step 6: Reload nginx
echo -e "\n${YELLOW}Step 6: Reloading nginx...${NC}"
sudo systemctl reload nginx
echo -e "${GREEN}✓ Nginx reloaded${NC}"

# Step 7: Test public access
echo -e "\n${YELLOW}Step 7: Testing public access...${NC}"
sleep 2

echo "Testing HTTPS..."
HTTPS_STATUS=$(curl -sk -o /dev/null -w '%{http_code}' https://ghostbridge.tech/ 2>/dev/null || echo "000")
if [ "$HTTPS_STATUS" = "200" ]; then
    echo -e "${GREEN}✓ HTTPS returning 200 OK${NC}"
else
    echo -e "${YELLOW}⚠ HTTPS returning $HTTPS_STATUS${NC}"
fi

echo "Testing API..."
API_STATUS=$(curl -sk -o /dev/null -w '%{http_code}' https://ghostbridge.tech/api/health 2>/dev/null || echo "000")
if [ "$API_STATUS" = "200" ]; then
    echo -e "${GREEN}✓ API health check passing${NC}"
else
    echo -e "${YELLOW}⚠ API returning $API_STATUS${NC}"
fi

# Final summary
echo ""
echo -e "${GREEN}============================================${NC}"
echo -e "${GREEN}  Fix Complete!${NC}"
echo -e "${GREEN}============================================${NC}"
echo ""
echo "Your Rust chat UI should now be available at:"
echo ""
echo "  https://ghostbridge.tech/"
echo "  https://ghostbridge.tech/chat.html"
echo "  https://ghostbridge.tech/api/health"
echo ""
echo "If you still see the old HuggingFace UI:"
echo "  1. Clear your browser cache (Ctrl+Shift+Delete)"
echo "  2. Try incognito/private window"
echo "  3. Check: curl -I https://ghostbridge.tech/"
echo ""
