#!/bin/bash
# Installation script for op-xray-daemon D-Bus service
# Sets up D-Bus policy and service files

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

echo "Installing op-xray-daemon..."

# Check if running as root (required for system bus operations)
if [ "$EUID" -ne 0 ]; then
    echo "Warning: Not running as root. May need sudo for some operations."
fi

# Detect init system
if [ -d /run/systemd/system ] || [ -d /var/run/systemd/system ]; then
    INIT_SYSTEM="systemd"
elif [ -x /sbin/s6-rc ] || [ -x /usr/bin/s6-rc ]; then
    INIT_SYSTEM="s6"
else
    INIT_SYSTEM="unknown"
fi

echo "Detected init system: ${INIT_SYSTEM}"

# Create D-Bus policy file
DBUS_POLICY_DIR="/etc/dbus-1/system.d"
if [ ! -d "$DBUS_POLICY_DIR" ]; then
    echo "Creating D-Bus policy directory..."
    mkdir -p "$DBUS_POLICY_DIR"
fi

cat > "${DBUS_POLICY_DIR}/org.opdbus.v1.Xray.conf" << 'EOF'
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <!-- Policy for org.opdbus.v1.Xray service -->

  <!-- Allow root to own the service -->
  <policy user="root">
    <allow own="org.opdbus.v1"/>
    <allow send_destination="org.opdbus.v1"/>
    <allow receive_sender="org.opdbus.v1"/>
  </policy>

  <!-- Allow anyone to call the Xray interface methods -->
  <policy context="default">
    <allow send_destination="org.opdbus.v1"/>
    <allow send_destination="org.opdbus.v1"
           send_interface="org.opdbus.v1.Xray"/>
    <allow receive_sender="org.opdbus.v1"/>
  </policy>

  <!-- Allow specific group members full access -->
  <policy group="opdbus">
    <allow own="org.opdbus.v1"/>
    <allow send_destination="org.opdbus.v1"/>
    <allow send_interface="org.opdbus.v1.Xray"/>
    <allow receive_sender="org.opdbus.v1"/>
  </policy>
</busconfig>
EOF

echo "Created D-Bus policy: ${DBUS_POLICY_DIR}/org.opdbus.v1.Xray.conf"

# Install the binary
BINARY_PATH="${PROJECT_ROOT}/target/release/op-xray-daemon"
if [ ! -f "$BINARY_PATH" ]; then
    echo "Release binary not found, building..."
    cd "$PROJECT_ROOT"
    cargo build --release -p op-xray-daemon
fi

# Copy binary to system location
INSTALL_DIR="/usr/local/bin"
if [ -d /usr/local/sbin ]; then
    INSTALL_DIR="/usr/local/sbin"
fi

install -m 755 "$BINARY_PATH" "${INSTALL_DIR}/op-xray-daemon"
echo "Installed binary to: ${INSTALL_DIR}/op-xray-daemon"

# Create service based on init system
if [ "$INIT_SYSTEM" = "systemd" ]; then
    cat > /etc/systemd/system/op-xray-daemon.service << EOF
[Unit]
Description=Xray D-Bus Daemon
After=dbus.service network.target

[Service]
Type=dbus
BusName=org.opdbus.v1
ExecStart=${INSTALL_DIR}/op-xray-daemon
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
    systemctl daemon-reload
    echo "Created systemd service: /etc/systemd/system/op-xray-daemon.service"
    echo "Enable with: systemctl enable op-xray-daemon.service"

elif [ "$INIT_SYSTEM" = "s6" ]; then
    # s6/s6-rc service setup (Artix/Chimera Linux)
    S6_SERVICE_DIR="/etc/s6/sv/op-xray-daemon"
    if [ -d /etc/s6/sv ]; then
        mkdir -p "$S6_SERVICE_DIR"
        cat > "${S6_SERVICE_DIR}/run" << EOF
#!/bin/execlineb -P
fdmove -c 2 1
${INSTALL_DIR}/op-xray-daemon
EOF
        chmod +x "${S6_SERVICE_DIR}/run"
        echo "Created s6 service: ${S6_SERVICE_DIR}"
    elif [ -d /etc/sv ]; then
        # runit-style directories
        mkdir -p /etc/sv/op-xray-daemon
        cat > /etc/sv/op-xray-daemon/run << EOF
#!/bin/sh
exec 2>&1
exec ${INSTALL_DIR}/op-xray-daemon
EOF
        chmod +x /etc/sv/op-xray-daemon/run
        echo "Created runit service: /etc/sv/op-xray-daemon"
    fi
fi

# Reload D-Bus configuration
echo "Reloading D-Bus configuration..."
if [ -x /bin/dbus-send ] || [ -x /usr/bin/dbus-send ]; then
    dbus-send --system --type=method_call --dest=org.freedesktop.DBus \
        /org/freedesktop/DBus org.freedesktop.DBus.ReloadConfig 2>/dev/null || true
fi

echo ""
echo "Installation complete!"
echo ""
echo "Start the daemon with:"
if [ "$INIT_SYSTEM" = "systemd" ]; then
    echo "  systemctl start op-xray-daemon.service"
elif [ "$INIT_SYSTEM" = "s6" ]; then
    echo "  s6-rc -u change op-xray-daemon"
else
    echo "  ${INSTALL_DIR}/op-xray-daemon"
fi
echo ""
echo "Test with:"
echo "  dbus-send --system --dest=org.opdbus.v1 --type=method_call /org/opdbus/v1/xray org.opdbus.v1.Xray.Status"
echo ""
