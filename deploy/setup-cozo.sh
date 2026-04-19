#!/bin/bash
# Setup CozoDB graph database for operation-dbus-proto

set -e

echo "Setting up CozoDB graph database..."

# Install CozoDB server
# CozoDB can be installed via cargo or downloaded as binary

if command -v cargo >/dev/null 2>&1; then
    echo "Installing CozoDB server via cargo..."
    cargo install cozo --version 0.7
else
    echo "Cargo not found, attempting to download CozoDB binary..."
    # Download latest CozoDB binary for Linux
    COZO_VERSION="0.7.2"
    COZO_URL="https://github.com/cozodb/cozo/releases/download/v${COZO_VERSION}/cozo-v${COZO_VERSION}-x86_64-unknown-linux-gnu.tar.gz"

    wget -O cozo.tar.gz "$COZO_URL"
    tar -xzf cozo.tar.gz
    sudo mv cozo /usr/local/bin/
    rm cozo.tar.gz
fi

# Create CozoDB data directory
sudo mkdir -p /var/lib/cozo
sudo chown -R op-dbus:op-dbus /var/lib/cozo

# Create systemd service for CozoDB
cat > /tmp/cozo.service << 'EOF'
[Unit]
Description=CozoDB Graph Database
After=network.target

[Service]
Type=simple
User=op-dbus
ExecStart=/usr/local/bin/cozo run --path /var/lib/cozo
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

sudo mv /tmp/cozo.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable cozo
sudo systemctl start cozo

echo "CozoDB setup complete. Service started on default port 9070."