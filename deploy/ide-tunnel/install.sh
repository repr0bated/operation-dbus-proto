#!/bin/bash
set -e

if [ "$EUID" -ne 0 ]; then
  echo "Please run as root"
  exit 1
fi

echo "Installing Antigravity Wayland IDE Base..."

# Create user/group if not exists
if ! id "antigravity" &>/dev/null; then
    echo "Creating 'antigravity' user..."
    useradd -r -s /bin/false antigravity
fi

# Copy script
echo "Installing session launcher..."
cp antigravity-session /usr/local/bin/
chmod +x /usr/local/bin/antigravity-session

# Copy services
echo "Installing systemd services..."
cp antigravity-session.service /etc/systemd/system/
cp antigravity-iap.service /etc/systemd/system/

# Reload systemd
echo "Reloading systemd..."
systemctl daemon-reload

echo "Installation complete."
echo ""
echo "IMPORTANT: Please edit /etc/systemd/system/antigravity-iap.service and set your Google Cloud Project ID."
echo "Then enable and start the services:"
echo "  systemctl enable --now antigravity-session"
echo "  systemctl enable --now antigravity-iap"
