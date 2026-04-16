# Environment Details — Socket Networking Migration

**Date:** 2026-04-15  
**Status:** ✅ Complete

## Overview

Migration of Incus container services from veth/bridged networking to native socket networking with OVS OpenFlow routing. All services use loopback-only networking with unix domain sockets dispatch via the shared `ovsbr0-sock` anchor port.

---

## Network Architecture

### OVS Bridge: ovsbr0

**Ports:**
- `grpc-bridge` — tonic gRPC / OpenFlow control plane
- `wgcf` — WireGuard WARP uplink
- `priv_warp` — privacy WARP chain
- `priv_xray` — xray egress (Freedom)
- `ovsbr0-sock` — **shared anchor** for unix socket dispatch
- `ovsbr0-patch` — L2 uplink to `br-mgmt`

### Linux Bridge: br-mgmt

**Purpose:** Uplink bridge for physical internet access.

**Ports:**
- `ens3` — physical uplink (public IP, default route)
- `br-mgmt-patch` — OVS patch peer to `ovsbr0`

### Traffic Flow

```
services container (lan0-service:53)
    ↓ unix socket
ovsbr0-sock anchor
    ↓ OpenFlow
ovsbr0 (routing via OpenFlow)
    ↓
ovsbr0-patch → br-mgmt-patch (patch port L2)
    ↓
br-mgmt → ens3 → internet
```

---

## Container Naming Convention

### System Containers

Format: `<network>-<7letter_suffix>`

| Container | Loopback Interface | Port Name | Purpose |
|-----------|-------------------|-----------|---------|
| `services` | `lan0-service` | `ovsbr0-sock` | DNS (nextdns), system services |

- `lan` = LAN-side (local network services)
- `wan` = WAN-side (egress services)
- `wlan` = Wireless/ap interface
- `7letter_suffix` = descriptive (e.g., `service`, `xray01`, `wgwarp`)

### User Containers

- Interface name derived from WireGuard public key (first 12 hex chars or truncated base64)
- No separate OVS port — all traffic via shared `ovsbr0-sock` anchor
- Registered via WireGuard key exchange

---

## Implementation Details

### 1. Container Loop Device

Inside each container, the loopback interface is renamed to follow convention:

```sh
ip link set lo name lan0-service
```

`lan0-service` binds nextdns and other services at `127.0.0.1`.

### 2. veth Removal

No veth interfaces — containers are loopback-only:

```sh
incus config device remove <container> eth0
```

### 3. Unix Socket Dispatch

Services bind to `127.0.0.1:<port>` inside container. Host-side dispatcher proxies to unix socket on `ovsbr0-sock` anchor.

### 4. OpenFlow Routing (via busctl D-Bus)

**DNS Ingress (priority 200):**
```
match: udp, tp_dst=53
action: output:ovsbr0-sock
```

**DNS Egress (priority 200):**
```
match: in_port=ovsbr0-sock, udp, tp_src=53
action: output:priv_xray
```

**Security Flows (priority 32000):**
- Drop invalid TCP flags
- Drop fragments
- Drop invalid connection tracking
- TTL normalization to 64
- ARP rate limiting

Applied via:
```sh
busctl --user call org.opdbus.v1 /org/opdbus/v1/state \
  org.opdbus.v1.StateManager ApplyState ssa \
  '{ "plugin": "openflow", "desired": { ... } }'
```

### 5. Patch Port Connectivity

Patch ports connect `ovsbr0` to `br-mgmt` for L2 uplink access:

**OVSDB JSON-RPC (native, no ovs-vsctl):**
```json
// ovsbr0 side
["Open_vSwitch",
  {"op":"insert","table":"Interface",
   "row":{"name":"ovsbr0-patch","type":"patch",
          "options":["map",[["peer","br-mgmt-patch"]]]},
   "uuid-name":"iface_p"},
  {"op":"insert","table":"Port",
   "row":{"name":"ovsbr0-patch",
          "interfaces":["set",[["named-uuid","iface_p"]]]},
   "uuid-name":"port_p"},
  {"op":"mutate","table":"Bridge",
   "where":[["name","==","ovsbr0"]],
   "mutations":[["ports","insert",
                  ["set",[["named-uuid","port_p"]]]]]}
]
```

---

## Verification Commands

```sh
# Container network
incus exec services -- ip a | grep lan0-service
incus exec services -- cat /proc/net/dev

# OVS ports
busctl --system call org.opdbus /org/opdbus/ovsdb \
  org.opdbus.OvsdbV1 Transact s \
  '["Open_vSwitch",{"op":"select","table":"Bridge","where":[["name","==","ovsbr0"]],"columns":["ports"]}]'

# OpenFlow flows
ovs-ofctl -O OpenFlow13 dump-flows ovsbr0 | grep -E 'tp_dst=53|ovsbr0-sock|priv_xray'

# Test connectivity
incus exec services -- ping -c 3 1.1.1.1
```

---

## Future Considerations

- **wan0-*** containers: similar pattern, different anchor/flow rules
- **wlan-xraysrv**: xray Reality server on wireless interface
- **User container registration**: WireGuard pubkey → interface name → socket routing
- **Privacy levels**: ct_zone-based routing for multi-tier privacy

---

## Files

- `crates/op-plugins/src/state_plugins/openflow.rs` — OpenFlow state plugin
- `crates/op-plugins/src/state_plugins/incus.rs` — Incus container management
- `crates/op-network/src/ovsdb.rs` — Native OVSDB JSON-RPC client
- `crates/op-network/src/ovs_netlink.rs` — OVS Netlink client
- `deploy/dinit/op-ovsdb-seed.sh` — Early-boot OVS port seeding
- `deploy/netplan/01-ovsbr0.yaml` — netplan OVS bridge config
- `.kiro/specs/socket-based-container-networking/topology.mmd` — Network topology diagram
