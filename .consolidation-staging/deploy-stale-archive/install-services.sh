#!/bin/bash
# Install and configure all op-dbus systemd services
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SYSTEMD_DIR="$SCRIPT_DIR/systemd"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== Installing Op-DBus Systemd Services ==="

# Check if running as root or with sudo
if [[ $EUID -ne 0 ]]; then
    echo "This script requires root privileges. Using sudo..."
    exec sudo "$0" "$@"
fi

# Copy all service files
echo "Copying service files to /etc/systemd/system/..."
cp "$SYSTEMD_DIR/op-web.service" /etc/systemd/system/
cp "$SYSTEMD_DIR/op-mcp.service" /etc/systemd/system/
cp "$SYSTEMD_DIR/op-mcp-server.service" /etc/systemd/system/ 2>/dev/null || true
cp "$SYSTEMD_DIR/op-web-hf.service" /etc/systemd/system/
cp "$SYSTEMD_DIR/op-agents.service" /etc/systemd/system/
cp "$SYSTEMD_DIR/op-agent@.service" /etc/systemd/system/
cp "$SYSTEMD_DIR/op-dbus.target" /etc/systemd/system/
cp "$SYSTEMD_DIR/op-dbus-all.service" /etc/systemd/system/

# Reload systemd
echo "Reloading systemd daemon..."
systemctl daemon-reload

# Enable core services
echo "Enabling core services..."
systemctl enable op-web.service
systemctl enable op-mcp.service
systemctl enable op-dbus-all.service

echo ""
echo "=== Installation Complete ==="
echo ""
echo "Available commands:"
echo "  Start all:     sudo systemctl start op-dbus-all"
echo "  Stop all:      sudo systemctl stop op-dbus-all"
echo "  Status:        sudo systemctl status op-dbus-all op-web op-mcp"
echo "  Logs:          journalctl -u op-web -u op-mcp -f"
echo ""
echo "Individual services:"
echo "  op-web         - Main web server (port 8081)"
echo "  op-mcp         - MCP HTTP server (port 8082)"
echo "  op-web-hf      - HuggingFace chat UI (port 8080)"
echo "  op-agents      - Agent services"
echo ""
echo "To start everything now: sudo systemctl start op-dbus-all"
