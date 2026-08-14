# Network Operations

This page describes **how networking is supposed to work on the current Artix/runit host**, and points at the live inventory when the tree and the machine disagree.

For a dated, command-verified snapshot of interfaces, Incus devices, listeners, and mesh forwarders, see:

**[Host socket & network topology (live)](host-socket-topology-live.md)**

Re-verify that file against the host before topology-changing work.

## Model in one paragraph

One OVS bridge (`ovsbr0`) carries the private fabric. Physical uplink (`eth0`) is enslaved and unaddressed. Host L3 lives on OVS internal ports: **`pub0`** (public), **`3tched`** (`10.0.0.2/24` + `10.200.0.2/24` incoming identities), **`svc0`** (`10.0.0.3/24`, Tonic `:8090`), and bridge LOCAL **`ovsbr0`** (`10.200.0.1/24`, fabric/OpenFlow). The WireGuard **`netmaker`** iface (`100.69.0.1/24`) is deliberately added to `ovsbr0` through typed `rovs_commands add_port`; because it is MAC-less L3, an OpenFlow 1.5 `packet_type` + `encap(ethernet)` flow delivers its IPv4 packets to the host stack. Infra containers are NIC-less and use ghostbridge UDS relays.

**gRPC surface:** `:8090` + `/run/opdbus/grpc.sock` + `/run/ghostbridge/container.sock`. **Not `:50051`.**

**OpenFlow** (`op-of-controller` on `10.200.0.1:6653`) negotiates 1.5. `fail_mode=standalone` and the cookied NORMAL fallback preserve host safety; controller-managed static flows add the NetMaker L3 delivery rule and the existing narrow NORMAL classifiers.

## Boot order (runit)

```text
opdbus-rundirs
  → ovsdb-server → ovs-vswitchd
  → ovsbr0-uplink → ovsbr0-svc-addr → ovsbr0-eth0
  → incus-ct-netmaker → op-grpc-bridge → netclient
  → netmaker-ovs-attach → op-of-controller
  → op-web / op-cognitive-mcp
  → fwd-nm-* / xsock-netmaker*
```

Config: `/etc/op-dbus/network.conf`  
Helpers: `/usr/local/libexec/3tched/*-up`

## Role map

| Port / net | Address | Use |
| --- | --- | --- |
| pub0 | `188.68.58.237/22` | Public uplink |
| 3tched | `10.0.0.2/24`, `10.200.0.2/24` | Incoming identities |
| svc0 | `10.0.0.3/24` | Tonic `:8090` |
| ovsbr0 LOCAL | `10.200.0.1/24` | Fabric / OF controller IP |
| netmaker (WG) | `100.69.0.1/24` | Mesh L3 port on `ovsbr0` |

NetMaker API egress resources currently advertise the two host identities (`10.0.0.2/32`, `10.200.0.2/32`, `direct_nat`) from the origin and the decoy route (`10.0.0.1/32`, NAT disabled) from the decoy node. The API route and the OpenFlow L3-delivery flow solve different halves of the path; both are required.

## Attachment modes (summary)

| Pattern | Used by (live) |
| --- | --- |
| Bridged NIC on `ovsbr0` | none today (all infra CTs nic-less) |
| Shared UDS `/run/ghostbridge/*` | bridge + CT mounts (`NetMaker`, `xray`, `mail-3tched`, …) |
| Incus `proxy` TCP | none; retired in favor of UDS relays |
| Host `socket-relay` / `fwd-*` | NM on svc0, mail/SNI entrance, mesh side channels |
| Host UDS `/run/opdbus/grpc.sock` | host gRPC clients |

## Do / don't

- **Do** check `sudo incus config device show <name>` before assuming UDS-only or proxy-only (Incus name **`NetMaker`** is case-sensitive).
- **Do** keep `netmaker` on `ovsbr0` and preserve its OF1.5 L3 `encap` flow.
- **Do** keep incoming identities on **3tched** and Tonic on **svc0 `10.0.0.3:8090`**.
- **Do not** bind Netmaker API/broker on `10.200.*`.
- **Do not** revive `:50051` as the fabric gRPC port.
- **Do not** hard-code NetMaker's ofport; netclient recreation changes it.
- **Do not** “fix” Netmaker licensing with host NAT that bypasses the WARP/mark path.

## Stale references

- `docs/network-address-table.md` — old host (ens3 / 10.88.88.x).
- `deploy/runit/ZCALL-INIT-HANDOFF.md` / some `op-grpc-bridge` drafts — still mention `:50051`.
- Jul-22 wording that split the two incoming identities across `svc0`/`grpc0` — superseded by `3tched` + Tonic-only `svc0`; `grpc0` is retired.
