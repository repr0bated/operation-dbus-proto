#!/bin/bash
# Install Rust OpenFlow controller natively on the hypervisor (Artix Linux/s6)

set -euo pipefail

echo "=== Installing OpenFlow Controller on Hypervisor ==="

REPO_DIR=$(dirname "$(dirname "$(readlink -f "$0")")")

# 1. Compile op-of-controller
echo "Building op-of-controller..."
cd "$REPO_DIR"
cargo build --release -p op-network --bin op-of-controller

# 2. Install binary
echo "Installing binary to /usr/local/sbin/..."
install -m 755 target/release/op-of-controller /usr/local/sbin/op-of-controller

# 3. Setup s6 service
echo "Configuring s6 service for op-of-controller..."
mkdir -p /etc/s6/sv/op-of-controller

cat > /etc/s6/sv/op-of-controller/run << 'EOF'
#!/bin/execlineb -P
fdmove -c 2 1
export OF_CONTROLLER_LISTEN "127.0.0.1:6653"
export OF_FLOW_PAIRS "grpc-uplink0:ovsbr0"
export OVSDB_SOCKET "/run/openvswitch/db.sock"
/usr/local/sbin/op-of-controller
EOF
chmod +x /etc/s6/sv/op-of-controller/run

# Link to scan directory if s6 is active
if [ -d /run/service ]; then
    ln -sf /etc/s6/sv/op-of-controller /run/service/op-of-controller
    s6-svscanctl -a /run/service
    echo "Started op-of-controller daemon via s6."
fi

# 4. Configure OVS to use local controller
echo "Setting ovsbr0 controller to tcp:127.0.0.1:6653..."
ovs-vsctl set-controller ovsbr0 tcp:127.0.0.1:6653 || true

echo "=== Controller Hypervisor Installation Complete ==="
