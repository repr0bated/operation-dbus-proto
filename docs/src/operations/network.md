# Network Operations

This page describes **how networking is supposed to work on the current Artix/runit host**, and points at the live inventory when the tree and the machine disagree.

For a dated, command-verified snapshot of interfaces, Incus devices, listeners, and mesh forwarders, see:

**[Host socket & network topology (live)](host-socket-topology-live.md)**

Re-verify that file against the host before topology-changing work.

## Model in one paragraph

One OVS bridge (`ovsbr0`) carries the private fabric. Physical uplink (`eth0`) is enslaved and unaddressed. Host L3 lives on OVS internal ports: **`pub0`** (public), **`svc0`** (`10.0.0.2/24` — tonic entrance), **`grpc0`** (`10.200.0.2/24` — gRPC `:8090`), and bridge LOCAL **`ovsbr0`** (`10.200.0.1/24` — fabric / OpenFlow address). WireGuard mesh is the **`netmaker`** iface (`100.69.0.1/24`) — separate L3, **never** an OVS port. Infra containers are nic-less (UDS + Incus proxy).

**gRPC surface:** `:8090` + `/run/opdbus/grpc.sock` + `/run/ghostbridge/container.sock`. **Not `:50051`.**

**OpenFlow** (`op-of-controller` on `10.200.0.1:6653`) is intentionally NORMAL/standalone today until tag routing — bridge may have no controller set.

## Boot order (runit)

```text
opdbus-rundirs
  → ovsdb-server → ovs-vswitchd
  → ovsbr0-uplink → ovsbr0-addr → ovsbr0-svc-addr → ovsbr0-eth0
  → op-of-controller
  → netclient
  → op-grpc-bridge / op-web / op-cognitive-mcp
  → fwd-nm-* / xsock-netmaker*
```

Config: `/etc/op-dbus/network.conf`  
Helpers: `/usr/local/libexec/3tched/*-up`

## Role map

| Port / net | Address | Use |
| --- | --- | --- |
| pub0 | `188.68.58.237/22` | Public uplink |
| svc0 | `10.0.0.2/24` | Tonic entrance (mail, Netmaker API/broker) |
| grpc0 | `10.200.0.2/24` | gRPC / cognitive MCP `:8090` |
| ovsbr0 LOCAL | `10.200.0.1/24` | Fabric / OF controller IP |
| netmaker (WG) | `100.69.0.1/24` | Mesh |

Netmaker mesh clients reach svc0 via API egress `host-svc0-net` (`10.0.0.0/24`, `direct_nat`).

## Attachment modes (summary)

| Pattern | Used by (live) |
| --- | --- |
| Bridged NIC on `ovsbr0` | none today (all infra CTs nic-less) |
| Shared UDS `/run/ghostbridge/*` | bridge + CT mounts (`NetMaker`, `xray`, `mail-3tched`, …) |
| Incus `proxy` TCP | `mail-3tched`, `NetMaker`, `xray`, `qdrant` (when up) |
| Host `socket-relay` / `fwd-*` | NM on svc0, mail/SNI entrance, mesh side channels |
| Host UDS `/run/opdbus/grpc.sock` | host gRPC clients |

## Do / don't

- **Do** check `sudo incus config device show <name>` before assuming UDS-only or proxy-only (Incus name **`NetMaker`** is case-sensitive).
- **Do** keep mesh on `netmaker`; strip accidental OVS enslavement of that iface.
- **Do** put tonic helpers on **svc0**; gRPC/cognitive on **grpc0 `:8090`**.
- **Do not** bind Netmaker API/broker on `10.200.*`.
- **Do not** revive `:50051` as the fabric gRPC port.
- **Do not** enslave WireGuard into `ovsbr0`.
- **Do not** “fix” Netmaker licensing with host NAT that bypasses the WARP/mark path.

## Stale references

- `docs/network-address-table.md` — old host (ens3 / 10.88.88.x).
- `deploy/runit/ZCALL-INIT-HANDOFF.md` / some `op-grpc-bridge` drafts — still mention `:50051`.
- Jul-22 wording that put `10.200.0.2` on **svc0** and forbade `10.200.0.1` on the host — superseded by the svc0/`10.0.0.2` + grpc0 split.
