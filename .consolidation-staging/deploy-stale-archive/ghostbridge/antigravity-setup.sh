#!/bin/bash
# One-time setup for Antigravity server (ghostbridge user)
# Run this once to install extensions before starting antigravity.service

set -e

GHOSTBRIDGE_HOME="/var/lib/ghostbridge"
VSIX_PATH="$GHOSTBRIDGE_HOME/extensions/op-dbus-bridge.vsix"

echo "=== Installing op-dbus extension for ghostbridge ==="

# Install extension as ghostbridge user
sudo -u ghostbridge HOME="$GHOSTBRIDGE_HOME" /usr/bin/antigravity \
    --user-data-dir="$GHOSTBRIDGE_HOME/.antigravity-data" \
    --install-extension "$VSIX_PATH"

echo ""
echo "=== Extension installed ==="
echo "Now you can start the service:"
echo "  sudo -u ghostbridge HOME=$GHOSTBRIDGE_HOME XDG_RUNTIME_DIR=/run/user/999 systemctl --user start antigravity"

