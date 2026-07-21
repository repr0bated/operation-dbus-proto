#!/bin/bash
# Deploy script for op-openvswitch-daemon
# Installs the D-Bus daemon for OpenVSwitch management
#
# Architecture (AGENTS.md §4):
#   Bus name: org.opdbus.v1
#   Object paths: /org/opdbus/rovs/jsonrpc, /org/opdbus/rovs/openflow
#   Interfaces: org.opdbus.rovs.jsonrpc, org.opdbus.rovs.openflow

set -e

DAEMON_NAME="op-openvswitch-daemon"
DAEMON_PATH="/usr/local/bin/${DAEMON_NAME}"
DBUS_SERVICE_FILE="/usr/share/dbus-1/system-services/org.opdbus.v1.service"
DBUS_CONF_FILE="/etc/dbus-1/system.d/org.opdbus.v1.conf"
S6_SERVICE_DIR="/etc/s6/sv/${DAEMON_NAME}"
SYSTEMD_SERVICE_FILE="/etc/systemd/system/${DAEMON_NAME}.service"

echo "=== Deploying ${DAEMON_NAME} ==="

# Check for binary
if [ ! -f "./target/release/${DAEMON_NAME}" ] && [ ! -f "./target/debug/${DAEMON_NAME}" ]; then
    echo "Error: Binary not found. Build first with:"
    echo "  cargo build -p ${DAEMON_NAME} --release"
    exit 1
fi

# Use release binary if available, otherwise debug
if [ -f "./target/release/${DAEMON_NAME}" ]; then
    BINARY="./target/release/${DAEMON_NAME}"
    echo "Using release binary"
else
    BINARY="./target/debug/${DAEMON_NAME}"
    echo "Using debug binary"
fi

# Install binary
echo "Installing binary to ${DAEMON_PATH}..."
sudo cp "${BINARY}" "${DAEMON_PATH}"
sudo chmod +x "${DAEMON_PATH}"

# Create D-Bus service file (for D-Bus activation — optional)
echo "Creating D-Bus service file..."
sudo tee "${DBUS_SERVICE_FILE}" > /dev/null << 'EOF'
[D-Bus Service]
Name=org.opdbus.v1
Exec=/usr/local/bin/op-openvswitch-daemon
User=root
SystemdService=op-openvswitch-daemon.service
EOF

# Create D-Bus policy file
echo "Creating D-Bus policy file..."
sudo tee "${DBUS_CONF_FILE}" > /dev/null << 'EOF'
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
  "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <policy user="root">
    <allow own="org.opdbus.v1"/>
    <allow send_destination="org.opdbus.v1"/>
    <allow receive_sender="org.opdbus.v1"/>
  </policy>
  <policy context="default">
    <allow send_destination="org.opdbus.v1"/>
    <allow receive_sender="org.opdbus.v1"/>
  </policy>
</busconfig>
EOF

# Install s6 service definition (Artix/Chimera Linux)
if [ -d /etc/s6/sv ] || [ -d /run/service ]; then
    echo "Installing s6 service definition..."
    sudo mkdir -p "${S6_SERVICE_DIR}"
    sudo cp -r "./deploy/s6/${DAEMON_NAME}/"* "${S6_SERVICE_DIR}/"
    sudo chmod +x "${S6_SERVICE_DIR}/run"

    # Link into live scan directory if s6 is active
    if [ -d /run/service ]; then
        sudo ln -sf "${S6_SERVICE_DIR}" "/run/service/${DAEMON_NAME}"
        sudo s6-svscanctl -a /run/service 2>/dev/null || true
        echo "Linked s6 service to /run/service/${DAEMON_NAME}"
    fi
fi

# Create systemd service file (for systems with systemd)
if command -v systemctl &> /dev/null; then
    echo "Creating systemd service file..."
    sudo tee "${SYSTEMD_SERVICE_FILE}" > /dev/null << EOF
[Unit]
Description=OpenVSwitch D-Bus Daemon
After=network.target openvswitch-switch.service
Wants=openvswitch-switch.service

[Service]
Type=dbus
BusName=org.opdbus.v1
ExecStart=${DAEMON_PATH} --grpc 127.0.0.1:50051
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
    sudo systemctl daemon-reload
    echo "Systemd service created. Enable with:"
    echo "  sudo systemctl enable ${DAEMON_NAME}"
    echo "  sudo systemctl start ${DAEMON_NAME}"
fi

echo ""
echo "=== Deployment Complete ==="
echo ""
echo "The daemon can be started:"
echo "  - Manually: sudo ${DAEMON_PATH}"
echo "  - With gRPC: sudo ${DAEMON_PATH} --grpc 0.0.0.0:50051"
if [ -d /run/service ]; then
    echo "  - Via s6:    sudo s6-svc -u /run/service/${DAEMON_NAME}"
fi
if command -v systemctl &> /dev/null; then
    echo "  - Via systemd: sudo systemctl start ${DAEMON_NAME}"
fi
echo ""
echo "Test D-Bus JSON-RPC interface:"
echo "  busctl call org.opdbus.v1 /org/opdbus/rovs/jsonrpc org.opdbus.rovs.jsonrpc transact s 'list_dbs' s '[]'"
echo ""
echo "Test gRPC interface:"
echo "  grpcurl -plaintext 127.0.0.1:50051 list ovsdaemon.v1.OvsdbService"
