# Netclient Container Netns — Requirements

> Give `netclient` inside the `netmaker` container a restricted OVS attachment
> whose egress is forced through xray and the host's already-operational
> `wgcf-egress` path. In a separately approved phase, replace xray's veth-backed
> NIC with an OVS internal port.

| Field | Value |
| --- | --- |
| Status | Implementation-ready draft |
| Phase 1 | Netmaker attachment and join |
| Phase 2 | Separately gated xray NIC migration |
| Related crates | `op-network`, `op-plugins`, `op-grpc-adapters`, `op-grpc-bridge` |

---

## 1 · Phase 1 User Story

As the control-plane operator, I want `netclient` in the `netmaker` container
to form WireGuard peer handshakes while every candidate egress packet first
traverses xray and every permitted peer flow exits through the existing
host-side `wgcf-egress` policy path.

### Acceptance criteria

#### Attachment and gateway

- OVS internal port `netmk` exists on `ovsbr0`, is moved into the current
  `netmaker` network namespace, and is not backed by a veth.
- `netmk` is UP with `10.200.1.1/30` and default route
  `via 10.200.1.2 dev netmk`.
- Xray's current OVS-facing interface retains all existing state and gains
  secondary address `10.200.1.2/30`.
- Xray forwards the netclient flow to its existing host gateway
  `10.200.0.2`; ICMP redirects are disabled on the inside interface so the
  container cannot learn a route that bypasses xray.
- The host return route is
  `10.200.1.0/30 via 10.200.0.1 dev svc0`.

#### Mandatory mark and egress policy

- Every IP packet from `10.200.1.1/32` that returns from xray to host `svc0`
  receives fwmark `0x51821/0xffffffff` before host route lookup.
- Mark `0x51821` selects existing policy table `51820`; it is mandatory and is
  not limited to already-approved protocol or port matches.
- Feature-owned priority `10518` maps mark `0x51821` to existing table
  `51820`; priority `10519` blackholes source `10.200.1.1/32` when marking is
  missed or the table lookup is unusable, preventing fallthrough to the host
  main table.
- Host and xray forwarding rules permit only live Netmaker WireGuard UDP peer
  endpoint ports. Marked but unapproved traffic is dropped.
- Source NAT for the permitted flow occurs only on `wgcf-egress`.
- Captures show peer-directed inner UDP on `wgcf-egress`; that inner flow is
  absent from `pub0`, where only the existing tunnel underlay is expected.
- This forced xray-plus-wgcf path supplies the operator-required
  reality/egress signature; the fwmark is the non-bypassable steering control.

#### Existing wgcf boundary

- `wgcf-egress` is an existing, self-contained **host** interface and is only
  observed by this feature.
- This feature does not edit `/etc/wireguard/wgcf-egress.conf`, create or
  remove the interface, start or stop its service, alter its underlay FwMark
  `0x51820`, or rewrite table `51820`.
- If `wgcf-egress` or its table-51820 default path is not ready, Phase 1
  remains unready and reports the external prerequisite failure. The
  netclient-specific priority-10518 lookup rule is feature-owned and does not
  modify the table or wgcf runtime.

#### Isolation and lifecycle

- OpenFlow on `ovsbr0` allows ARP and the live WireGuard UDP peer flows from
  `netmk`, then drops every other flow originating from `netmk`.
- OpenFlow state is verified by a live OF1.3 flow-table query, not only by the
  controller's in-memory list. Kernel datapath flow evidence is collected
  after probes as secondary confirmation.
- Existing Netmaker API/broker devices, xray ingress/proxy paths, `pub0`,
  `svc0`, and host `3tched` state remain healthy.
- Host and in-container feature services use runit and `sv`.
- OVS and link/address/route mutations use native OVSDB/D-Bus/rtnetlink paths.
  Rust plugins do not spawn `ip`, `ovs-vsctl`, or `ovs-ofctl`.
- Netfilter/sysctl commands are confined to named, idempotent runit bootstrap
  services because no native netfilter/sysctl plugin currently exists.

#### Adapter and join

- `op-grpc-adapters` runs inside `netmaker` on
  `/var/lib/opdbus-runtime/netmaker/op-grpc-adapters.sock`.
- Host runit service `netmaker-adapter-loopback` exposes only
  `127.0.0.1:50061` using the existing UDS-to-loopback socat pattern.
- `op-grpc-bridge` uses dedicated `NETMAKER_ADAPTER_ADDR`; unrelated gRPC
  calls retain their existing default endpoint.
- Every Netmaker adapter request carries required Ghostbridge identity
  metadata.
- `netmaker_join` succeeds and `wg show` inside the container reports a recent
  handshake with at least one peer.

---

## 2 · Phase 2 User Story

As the control-plane operator, I want xray's veth-backed OVS NIC replaced by
internal port `xray0` without losing production ingress or Phase 1 netclient
egress.

### Acceptance criteria

- `xray0` is an OVS internal port moved into the current xray namespace.
- Captured MAC, MTU, all addresses (including `10.200.0.1` and
  `10.200.1.2/30`), and all routes are preserved.
- Interface-scoped xray/netclient forwarding rules are atomically retargeted to
  `xray0`.
- Existing wgcf configuration and runtime remain untouched.
- External probes for all xray domains and the netclient handshake remain
  healthy.
- A tested rollback restores the veth-backed NIC and both traffic classes.
- Phase 2 does not run automatically after Phase 1; it requires 48 hours of
  stable observation, explicit owner approval, and a maintenance window.

---

## 3 · Non-functional Requirements

| ID | Requirement |
| --- | --- |
| NFR-1 | All provisioning and reconciliation stages are idempotent. |
| NFR-2 | Container PID changes are handled by re-resolving the current init PID; stale PIDs are never persisted as desired state. |
| NFR-3 | Network-namespace entry occurs only on a dedicated OS thread whose netlink socket is created after entry. |
| NFR-4 | Phase 1 fails closed: no direct main-table or `pub0` fallback for `10.200.1.1/32`. |
| NFR-5 | OVS mutations use native OVSDB; OpenFlow mutations and queries use the native OF1.3 controller; link/route mutations use rtnetlink. |
| NFR-6 | Stable names are `netmk`, `xray0`, `netmk-egress-policy`, `netmk-port-attach`, `netmk-of-restrict`, `netmaker-adapter-loopback`, and `netmk-netclient-start`. |
| NFR-7 | Phase 2 rollback target is under 10 seconds; the cutover interruption target is under 2 seconds. |
| NFR-8 | No feature step mutates wgcf configuration, interface lifecycle, underlay mark, or table contents. |

---

## 4 · Out of Scope

- Managing or repairing `wgcf-egress` itself.
- Replacing WARP credentials or provider configuration.
- Changing the host `3tched` WireGuard membership.
- Changing Netmaker server enrollment keys, API keys, API devices, or broker
  devices.
- Modifying xray application routing or its live JSON configuration.
- General host firewall redesign beyond dedicated `OP_NETMK_*` chains and
  policy rules.
- Automatically executing Phase 2.
