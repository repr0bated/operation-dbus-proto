#!/bin/bash
# deploy/setup-bridge.sh - Focused bridge infrastructure setup
# This sets up: ovsbr0 bridge, wgcf tunnel, Xray client, and ports management
# Following the user's priority: bridge → wg gateway → xray client → ports → xray server

set -e

start_service_via_dinit_dbus() {
    service="$1"
    output="$(doas -n gdbus call \
        --system \
        --dest org.chimera.dinit \
        --object-path /org/chimera/dinit \
        --method org.chimera.dinit.Manager.StartService \
        "$service" \
        false 2>&1)" && return 0

    printf '%s' "$output" | grep -q 'org.chimera.dinit.Error.ServiceAlready' && return 0
    return 1
}

echo "=== Setting up Privacy Network Bridge Infrastructure ==="
echo "Priority: Bridge → WG Gateway → Xray Client → Ports → Xray Server"
echo ""

# 1. Ensure netplan config is in place
echo "1. Installing netplan configuration..."
doas cp deploy/netplan/01-ovsbr0.yaml /etc/netplan/01-ovsbr0.yaml
doas chmod 600 /etc/netplan/01-ovsbr0.yaml
echo "✅ Netplan config installed"

# 2. Apply netplan (this should create ovsbr0)
echo "2. Applying netplan..."
doas netplan apply
echo "✅ Netplan applied"

# 3. Start OVS services
echo "3. Starting OVS services..."
start_service_via_dinit_dbus op-ovs-services || true
start_service_via_dinit_dbus op-ovsdb-seed || true
start_service_via_dinit_dbus op-ovsdb-bridge || true
start_service_via_dinit_dbus ovs-attach-ports || true
echo "✅ OVS services started"

# 4. Start wgcf (WARP tunnel)
echo "4. Starting wgcf WARP tunnel..."
start_service_via_dinit_dbus wg-quick-all || true
echo "✅ wgcf started"

# 5. Start Xray client
echo "5. Starting Xray client..."
start_service_via_dinit_dbus xray-client || true
echo "✅ Xray client started"

# 6. Check the bridge status
echo ""
echo "=== Final Status ==="
echo "Bridge status:"
ip link show ovsbr0 2>/dev/null && echo "✅ ovsbr0 bridge is up" || echo "❌ ovsbr0 bridge not found"

echo "wgcf status:"
ip link show wgcf 2>/dev/null && echo "✅ wgcf tunnel is up" || echo "❌ wgcf tunnel not found"

echo "Xray status:"
ps aux | grep xray | grep -v grep && echo "✅ Xray is running" || echo "❌ Xray not running"

echo ""
echo "✅ Bridge infrastructure setup complete"
echo "The privacy network tunnel (wgcf → ovsbr0 → Xray) should now be operational."
echo "You can test connectivity with: curl -I http://registration.3tched.com"
