#!/bin/bash
# Start all op-dbus services for GhostBridge deployment
# Usage: sudo ./deploy/start-services.sh

set -e

echo "=== Starting GhostBridge Services ==="
echo

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}Error: Please run as root (use sudo)${NC}"
    exit 1
fi

# 1. Start op-web server (main web UI + MCP)
echo -e "${YELLOW}[1/3] Starting op-web server...${NC}"
if [ -f /usr/local/sbin/op-web-server ]; then
    systemctl daemon-reload
    systemctl enable op-web.service 2>/dev/null || true
    systemctl restart op-web.service
    sleep 2
    if systemctl is-active --quiet op-web; then
        echo -e "${GREEN}✓ op-web running on http://localhost:8080${NC}"
    else
        echo -e "${RED}✗ op-web failed to start${NC}"
        systemctl status op-web --no-pager
        exit 1
    fi
else
    echo -e "${RED}✗ op-web-server binary not found at /usr/local/sbin/op-web-server${NC}"
    echo "Run: cargo build --release -p op-web && sudo cp target/release/op-web-server /usr/local/sbin/"
    exit 1
fi

# 2. Start op-mcp-compact (MCP compact mode server)
echo -e "${YELLOW}[2/3] Starting op-mcp-compact...${NC}"
if [ -f /usr/local/sbin/op-mcp-compact ]; then
    systemctl daemon-reload
    systemctl enable op-mcp-compact.service 2>/dev/null || true
    systemctl restart op-mcp-compact.service
    sleep 1
    if systemctl is-active --quiet op-mcp-compact; then
        echo -e "${GREEN}✓ op-mcp-compact running${NC}"
    else
        echo -e "${YELLOW}⚠ op-mcp-compact not running (optional component)${NC}"
    fi
else
    echo -e "${YELLOW}⚠ op-mcp-compact binary not found (optional)${NC}"
fi

# 3. Start op-mcp-agents (MCP agents server)
echo -e "${YELLOW}[3/3] Starting op-mcp-agents...${NC}"
if [ -f /usr/local/sbin/op-mcp-server ]; then
    systemctl daemon-reload
    systemctl enable op-mcp-agents.service 2>/dev/null || true
    systemctl restart op-mcp-agents.service
    sleep 1
    if systemctl is-active --quiet op-mcp-agents; then
        echo -e "${GREEN}✓ op-mcp-agents running${NC}"
    else
        echo -e "${YELLOW}⚠ op-mcp-agents not running (optional component)${NC}"
    fi
else
    echo -e "${YELLOW}⚠ op-mcp-server binary not found (optional)${NC}"
fi

echo
echo "=== Service Status Summary ==="
echo "op-web:        $(systemctl is-active op-web)"
echo "op-mcp-compact: $(systemctl is-active op-mcp-compact 2>/dev/null || echo 'not installed')"
echo "op-mcp-agents: $(systemctl is-active op-mcp-agents 2>/dev/null || echo 'not installed')"
echo

# Check if Caddy is running
echo "=== Caddy Status ==="
if command_exists caddy; then
    if systemctl is-active --quiet caddy 2>/dev/null; then
        echo -e "${GREEN}✓ Caddy is running${NC}"
        echo "  - https://op-web.ghostbridge.tech should be accessible"
    else
        echo -e "${YELLOW}⚠ Caddy is not running${NC}"
        echo "  Start with: sudo systemctl start caddy"
    fi
else
    echo -e "${YELLOW}⚠ Caddy not installed${NC}"
fi

echo
echo "=== Testing Endpoints ==="
echo "Testing http://localhost:8080/api/health..."
if curl -s http://localhost:8080/api/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ op-web health check passed${NC}"
else
    echo -e "${RED}✗ op-web health check failed${NC}"
fi

echo
echo "=== Access Information ==="
echo "Local:    http://localhost:8080"
echo "External: https://op-web.ghostbridge.tech (via Caddy)"
echo "MCP:      https://op-web.ghostbridge.tech/mcp/compact"
echo "API:      https://op-web.ghostbridge.tech/api"
echo
echo -e "${GREEN}Done!${NC}"