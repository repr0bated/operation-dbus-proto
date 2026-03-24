#!/bin/bash
# GhostBridge + Antigravity Session Integration
# Deploy script for BunsenLabs/Openbox
set -e

GHOSTBRIDGE_HOME="/var/lib/ghostbridge"
SOURCE_DIR="/home/jeremy/git/operation-dbus"

echo "=== GhostBridge Session Deploy ==="

# 1. Install op-dbus binary
echo "Installing op-dbus binary..."
sudo cp "$SOURCE_DIR/target/release/op-dbus" /usr/local/bin/
sudo chmod +x /usr/local/bin/op-dbus

# 2. Create systemd user directory for ghostbridge
sudo mkdir -p "$GHOSTBRIDGE_HOME/.config/systemd/user"
sudo chown -R ghostbridge:ghostbridge "$GHOSTBRIDGE_HOME/.config"

# 3. Copy service files
echo "Copying service files..."
sudo cp "$SOURCE_DIR/deploy/ghostbridge/openbox.target" "$GHOSTBRIDGE_HOME/.config/systemd/user/"
sudo cp "$SOURCE_DIR/deploy/ghostbridge/op-dbus.service" "$GHOSTBRIDGE_HOME/.config/systemd/user/"
sudo cp "$SOURCE_DIR/deploy/ghostbridge/op-dbus.socket" "$GHOSTBRIDGE_HOME/.config/systemd/user/"
sudo cp "$SOURCE_DIR/deploy/ghostbridge/antigravity.service" "$GHOSTBRIDGE_HOME/.config/systemd/user/"
sudo chown -R ghostbridge:ghostbridge "$GHOSTBRIDGE_HOME/.config/systemd"

# 4. Create symlink for antigravity-server version  
echo "Setting up antigravity-server symlink..."
# Note: we use jeremy's installation for the binary, but we can symlink it if needed
# For now the service points directly to jeremy's home which is readable

# 4. Copy stable bridge extension
echo "Copying bridge extension..."
sudo mkdir -p "$GHOSTBRIDGE_HOME/extensions"
sudo cp "$SOURCE_DIR/extensions/antigravity-bridge/op-dbus-antigravity-bridge-fresh.vsix" "$GHOSTBRIDGE_HOME/extensions/op-dbus-bridge.vsix"
sudo chown -R ghostbridge:ghostbridge "$GHOSTBRIDGE_HOME/extensions"

# 5. Setup Antigravity user settings and scripts
echo "Setting up Antigravity settings and scripts..."
sudo mkdir -p "$GHOSTBRIDGE_HOME/.antigravity-data/User"
sudo cp "$SOURCE_DIR/deploy/ghostbridge/antigravity-settings.json" "$GHOSTBRIDGE_HOME/.antigravity-data/User/settings.json"
sudo cp "$SOURCE_DIR/deploy/ghostbridge/antigravity-setup.sh" "$GHOSTBRIDGE_HOME/"
sudo chmod +x "$GHOSTBRIDGE_HOME/antigravity-setup.sh"
sudo chown -R ghostbridge:ghostbridge "$GHOSTBRIDGE_HOME"

# 6. Enable services as ghostbridge user
echo "Enabling services for ghostbridge..."
sudo -u ghostbridge HOME="$GHOSTBRIDGE_HOME" XDG_RUNTIME_DIR=/run/user/999 systemctl --user daemon-reload || true
sudo -u ghostbridge HOME="$GHOSTBRIDGE_HOME" XDG_RUNTIME_DIR=/run/user/999 systemctl --user enable openbox.target op-dbus.service antigravity.service op-dbus.socket || true

echo ""
echo "=== Deploy Complete ==="
echo ""
echo "To start services as ghostbridge:"
echo "  sudo -u ghostbridge HOME=$GHOSTBRIDGE_HOME XDG_RUNTIME_DIR=/run/user/999 systemctl --user start openbox.target"
echo ""
echo "Verify with:"
echo "  curl http://localhost:8088/health"
echo "  curl http://localhost:8080"

