#!/bin/sh
set -eu

# deploy/bridge-uplink-patch.sh — ens3 → br-mgmt → ovsbr0-patch
# Run with doas ./bridge-uplink-patch.sh

BR_MGMT='br-mgmt'
OVS_PATCH='ovsbr0-patch'
MGMT_PATCH='br-mgmt-patch'

echo '1. Create br-mgmt Linux bridge with ens3'
ip link add $BR_MGMT type bridge || true
ip link set ens3 master $BR_MGMT
ip addr flush dev ens3
ip addr add 148.113.204.83/24 dev $BR_MGMT  # adjust /24 mask
ip link set $BR_MGMT up
ip link set ens3 up

GATEWAY=$(ip route show default | awk '{print $3}' | head -1)
ip route replace default via $GATEWAY dev $BR_MGMT

echo '2. OVSDB: ovsbr0-patch on ovsbr0 (peer br-mgmt-patch)'
busctl --system call org.opdbus /org/opdbus/ovsdb org.opdbus.OvsdbV1 Transact sa 1 s '["Open_vSwitch", {\"op\":\"insert\",\"table\":\"Interface\",\"row\":{\"name\":\"'$OVS_PATCH'\",\"type\":\"patch\",\"options\":[\"map\",[[\"peer\",\"'$MGMT_PATCH'\"]]]},\"uuid-name\":\"iface_p\"}, {\"op\":\"insert\",\"table\":\"Port\",\"row\":{\"name\":\"'$OVS_PATCH'\",\"interfaces\":[\"set\",[[\"named-uuid\",\"iface_p\"]]]},\"uuid-name\":\"port_p\"}, {\"op\":\"mutate\",\"table\":\"Bridge\",\"where\":[[\"name\",\"==\",\"ovsbr0\"]],\"mutations\":[[\"ports\",\"insert\",[\"set\",[[\"named-uuid\",\"port_p\"]]]]]} ]'

echo '3. OVSDB: br-mgmt-patch on br-mgmt (peer ovsbr0-patch)'
busctl --system call org.opdbus /org/opdbus/ovsdb org.opdbus.OvsdbV1 Transact sa 1 s '["Open_vSwitch", {\"op\":\"insert\",\"table\":\"Interface\",\"row\":{\"name\":\"'$MGMT_PATCH'\",\"type\":\"patch\",\"options\":[\"map\",[[\"peer\",\"'$OVS_PATCH'\"]]]},\"uuid-name\":\"iface_m\"}, {\"op\":\"insert\",\"table\":\"Port\",\"row\":{\"name\":\"'$MGMT_PATCH'\",\"interfaces\":[\"set\",[[\"named-uuid\",\"iface_m\"]]]},\"uuid-name\":\"port_m\"}, {\"op\":\"mutate\",\"table\":\"Bridge\",\"where\":[[\"name\",\"==\",\"'$BR_MGMT'\"]],\"mutations\":[[\"ports\",\"insert\",[\"set\",[[\"named-uuid\",\"port_m\"]]]]]} ]'

echo '4. Bring patch ports up'
ip link set $OVS_PATCH up
ip link set $MGMT_PATCH up

echo '5. Verify'
ovs-vsctl show | grep -E '(ovsbr0|br-mgmt)'
ip a show $BR_MGMT
ip route | grep default
incus exec services -- ping -c 3 1.1.1.1

echo '✅ Uplink patch complete'