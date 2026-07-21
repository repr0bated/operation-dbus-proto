#!/bin/bash
# cleanup-local-3tched-interfaces.sh
# Removes 3tched hypervisor interfaces accidentally created on the local machine.
# These belong on the 3tched hypervisor, NOT on your laptop/workstation.
# Must be run as root (or with sudo).

set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
    echo "Run as root: sudo bash $0" >&2
    exit 1
fi

echo "=== Step 1: Stop 3tched services that recreate interfaces ==="

# Stop incusd (creates incusbr0, ic-* interfaces)
if systemctl is-active incusd &>/dev/null || systemctl is-active incus &>/dev/null; then
    echo "Stopping incus..."
    systemctl stop incus 2>/dev/null || true
    systemctl stop incusd 2>/dev/null || true
fi

# Stop ovs-vswitchd (creates ovs-system, ovsbr0, ovsbr0-sock, ovsbr0-mgmt)
if systemctl is-active ovs-vswitchd &>/dev/null; then
    echo "Stopping openvswitch..."
    systemctl stop ovs-vswitchd 2>/dev/null || true
    systemctl stop ovsdb-server 2>/dev/null || true
fi

# Stop docker (creates docker0, br-* bridges) — skip if you use docker locally
read -r -p "Stop docker and remove docker bridges? [y/N] " docker_answer
if [[ "$docker_answer" =~ ^[Yy]$ ]]; then
    echo "Stopping docker..."
    systemctl stop docker 2>/dev/null || true
    systemctl stop docker.socket 2>/dev/null || true
    DOCKER_CLEANUP=1
else
    DOCKER_CLEANUP=0
fi

# grpc-bridge is likely a standalone process or OVS port
# priv_* are likely OVS internal ports or standalone

echo ""
echo "=== Step 2: Delete interfaces ==="

# OVS-managed ports need ovs-vsctl, not ip link delete
if command -v ovs-vsctl &>/dev/null; then
    echo "Cleaning OVS bridges..."
    for bridge in $(ovs-vsctl list-br 2>/dev/null); do
        echo "  Deleting OVS bridge: $bridge"
        ovs-vsctl del-br "$bridge" 2>/dev/null || true
    done
fi

# Delete remaining non-OVS interfaces
INTERFACES_TO_REMOVE=(
    "grpc-bridge"
    "priv_xray"
    "priv_warp"
    "priv_wg"
    "incusbr0"
    "ovsbr0"
    "ovs-system"
    "ovsbr0-sock"
    "ovsbr0-mgmt"
)

for iface in "${INTERFACES_TO_REMOVE[@]}"; do
    if ip link show "$iface" &>/dev/null; then
        echo "  Deleting: $iface"
        ip link delete "$iface" 2>/dev/null && echo "    done" || echo "    failed (may already be gone via OVS cleanup)"
    fi
done

# Docker bridges
if [ "$DOCKER_CLEANUP" -eq 1 ]; then
    for iface in docker0 br-a53f8b999319; do
        if ip link show "$iface" &>/dev/null; then
            echo "  Deleting: $iface"
            ip link delete "$iface" 2>/dev/null || true
        fi
    done
fi

# Remove any leftover ic-* interfaces from incus
for iface in $(ip -br link show | awk '/^ic-/{print $1}' | sed 's/:$//'); do
    echo "  Deleting incus interface: $iface"
    ip link delete "$iface" 2>/dev/null || true
done

echo ""
echo "=== Step 3: Disable services so they don't come back on reboot ==="

# Only disable if they shouldn't run on this machine at all
read -r -p "Disable incus/ovs/docker from starting on boot? [y/N] " disable_answer
if [[ "$disable_answer" =~ ^[Yy]$ ]]; then
    systemctl disable incus 2>/dev/null || true
    systemctl disable incusd 2>/dev/null || true
    systemctl disable ovs-vswitchd 2>/dev/null || true
    systemctl disable ovsdb-server 2>/dev/null || true
    if [ "$DOCKER_CLEANUP" -eq 1 ]; then
        systemctl disable docker 2>/dev/null || true
        systemctl disable docker.socket 2>/dev/null || true
    fi
    echo "  Services disabled."
else
    echo "  Services left enabled (interfaces may return after reboot)."
fi

echo ""
echo "=== Remaining interfaces ==="
ip -br a | grep -v "^lo"
