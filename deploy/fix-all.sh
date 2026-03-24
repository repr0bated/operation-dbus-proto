#!/bin/bash
# Fix all services and get op-dbus.ghostbridge.tech running
# Run as: sudo ./deploy/fix-all.sh

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}=== Fixing GhostBridge Services ===${NC}"
echo

# 1. Start op-web
echo -e "${YELLOW}[1/5] Starting op-web...${NC}"
if [ -f /usr/local/sbin/op-web-server ]; then
    systemctl daemon-reload
    systemctl enable op-web.service 2>/dev/null || true
    systemctl restart op-web.service
    sleep 3
    if systemctl is-active --quiet op-web; then
        echo -e "${GREEN}✓ op-web running${NC}"
    else
        echo -e "${RED}✗ op-web failed to start${NC}"
        journalctl -u op-web -n 10 --no-pager
        exit 1
    fi
else
    echo -e "${RED}✗ op-web-server binary not found${NC}"
    exit 1
fi

# 2. Start op-mcp-agents
echo -e "${YELLOW}[2/5] Starting op-mcp-agents...${NC}"
if [ -f /usr/local/sbin/op-mcp-server ]; then
    systemctl enable op-mcp-agents.service 2>/dev/null || true
    systemctl restart op-mcp-agents.service
    sleep 2
    if systemctl is-active --quiet op-mcp-agents; then
        echo -e "${GREEN}✓ op-mcp-agents running${NC}"
    else
        echo -e "${YELLOW}⚠ op-mcp-agents not running (optional)${NC}"
    fi
else
    echo -e "${YELLOW}⚠ op-mcp-server binary not found${NC}"
fi

# 3. Install Caddy config
echo -e "${YELLOW}[3/5] Installing Caddy config...${NC}"
if [ -f deploy/caddy/op-web-ghostbridge.conf ]; then
    mkdir -p /etc/caddy/conf.d
    cp deploy/caddy/op-web-ghostbridge.conf /etc/caddy/conf.d/
    echo -e "${GREEN}✓ Caddy config installed${NC}"
else
    echo -e "${RED}✗ Caddy config not found${NC}"
    exit 1
fi

# 4. Install Hugging Face MCP config
echo -e "${YELLOW}[4/5] Installing Hugging Face MCP config...${NC}"
if [ -f deploy/caddy/huggingface-mcp.conf ]; then
    cp deploy/caddy/huggingface-mcp.conf /etc/caddy/conf.d/
    echo -e "${GREEN}✓ Hugging Face MCP config installed${NC}"
else
    echo -e "${YELLOW}⚠ Hugging Face config not found${NC}"
fi

# 5. Start Caddy
echo -e "${YELLOW}[5/5] Starting Caddy...${NC}"
if command -v caddy >/dev/null; then
    systemctl enable caddy 2>/dev/null || true
    systemctl restart caddy
    sleep 2
    if systemctl is-active --quiet caddy; then
        echo -e "${GREEN}✓ Caddy running${NC}"
    else
        echo -e "${RED}✗ Caddy failed to start${NC}"
        journalctl -u caddy -n 10 --no-pager
    fi
else
    echo -e "${RED}✗ Caddy not installed${NC}"
    echo "Install with: apt install caddy"
    exit 1
fi

echo
echo -e "${GREEN}=== Service Status ===${NC}"
systemctl is-active op-web && echo -e "${GREEN}✓${NC} op-web: RUNNING" || echo -e "${RED}✗${NC} op-web: STOPPED"
systemctl is-active op-mcp-agents 2>/dev/null && echo -e "${GREEN}✓${NC} op-mcp-agents: RUNNING" || echo -e "${YELLOW}⚠${NC} op-mcp-agents: STOPPED"
systemctl is-active caddy && echo -e "${GREEN}✓${NC} caddy: RUNNING" || echo -e "${RED}✗${NC} caddy: STOPPED"

echo
echo -e "${GREEN}=== Testing Endpoints ===${NC}"
echo "Testing localhost:8080..."
if curl -s http://localhost:8080/api/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} http://localhost:8080/api/health - OK"
else
    echo -e "${RED}✗${NC} http://localhost:8080/api/health - FAILED"
fi

echo
echo -e "${GREEN}=== URLs ===${NC}"
echo "Web UI:       https://op-web.ghostbridge.tech"
echo "MCP Agents:   https://mcp.ghostbridge.tech"
echo "MCP Compact:  https://mcp-compact.ghostbridge.tech"
echo
echo -e "${YELLOW}Note: HTTPS certificates will be provisioned automatically by Caddy${NC}"
echo -e "${YELLOW}It may take 30-60 seconds for the first request to succeed${NC}"