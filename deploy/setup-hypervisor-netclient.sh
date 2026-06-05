#!/bin/bash
# Install Netmaker netclient natively on the hypervisor (Artix Linux/s6)
# Binds to eth0 and establishes Layer 3 VPN interfaces without MAC constraints

set -euo pipefail

echo "=== Installing Netclient on Hypervisor ==="

# 1. Download and install netclient binary
echo "Downloading netclient-linux-amd64..."
curl -L "https://github.com/gravitl/netmaker/releases/latest/download/netclient-linux-amd64" -o /usr/local/bin/netclient
chmod +x /usr/local/bin/netclient

# 2. Setup s6 service for the daemon
# Netclient usually installs a systemd service via `netclient install`
# For Artix/s6, we supervise `netclient daemon` natively.
echo "Configuring s6 service for netclient..."
mkdir -p /etc/s6/sv/netclient
cat > /etc/s6/sv/netclient/run << 'EOF'
#!/bin/execlineb -P
fdmove -c 2 1
/usr/local/bin/netclient daemon
EOF
chmod +x /etc/s6/sv/netclient/run

# Link to scan directory if s6 is active
if [ -d /run/service ]; then
    ln -sf /etc/s6/sv/netclient /run/service/netclient
    s6-svscanctl -a /run/service
    echo "Started netclient daemon via s6."
fi

# 2.5 Setup s6 OVS attach daemon for Netmaker
echo "Configuring native OVS auto-attach hook for Netmaker..."
mkdir -p /etc/s6/sv/netmaker-ovs-attach
cat > /etc/s6/sv/netmaker-ovs-attach/run << 'EOF'
#!/bin/sh
exec 2>&1
echo "Waiting for netmaker interface to appear..."
while true; do
    if ip link show netmaker >/dev/null 2>&1; then
        # Use native rovs-ovsdb setup tool instead of ovs-vsctl
        VETH_HOST=netmaker /usr/local/bin/op-ovsbr0-setup
    fi
    sleep 5
done
EOF
chmod +x /etc/s6/sv/netmaker-ovs-attach/run

if [ -d /run/service ]; then
    ln -sf /etc/s6/sv/netmaker-ovs-attach /run/service/netmaker-ovs-attach
    s6-svscanctl -a /run/service
    echo "Started netmaker OVS attach daemon."
fi

# 3. Enable IP Forwarding on Hypervisor
echo "Enabling IP forwarding..."
cat > /etc/sysctl.d/99-netmaker-forwarding.conf << 'EOF'
net.ipv4.ip_forward=1
net.ipv6.conf.all.forwarding=1
EOF
sysctl -p /etc/sysctl.d/99-netmaker-forwarding.conf || true

echo "=== Netclient Hypervisor Installation Complete ==="
echo ""
echo "To join your hypervisor to a Netmaker network, run:"
echo "  netclient join -t <ENROLLMENT_TOKEN>"
echo ""
echo "Your Incus containers can then access the resulting nm-* interface"
echo "via standard Linux IP forwarding through incusbr0."
