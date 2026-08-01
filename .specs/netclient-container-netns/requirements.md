# Netclient Container Netns — OVS Internal Port Attachment

> Provide the NetMaker container's netclient process with routable UDP transport
> via an OVS internal port, and later migrate xray from veth to the same pattern.

| Field       | Value                                          |
| ----------- | ---------------------------------------------- |
| Status      | Draft                                          |
| Owner       | —                                              |
| Related     | `crates/op-plugins/src/state_plugins/oci.rs`   |
|             | `crates/op-plugins/src/state_plugins/netmaker.rs` |
|             | `crates/op-network/src/bin/op-ovsbr0-setup.rs` |

---

## 1 · User Stories

### US-1 — NetMaker mesh connectivity (Phase 1)

As the control plane operator, I want `netclient` inside the `NetMaker` container
to have a routable UDP data path to arbitrary WireGuard peer IPs, so that the
mesh network actually forms handshakes and carries traffic.

**Acceptance criteria**

- An OVS internal port (not a veth) is created on `ovsbr0` and moved into the
  `NetMaker` container's network namespace.
- The port is assigned an IP address and has a default route that allows UDP
  egress to public IPs (peer endpoints).
- The `ovsbr0`-connected interface in xray's netns (`eth0`) carries the
  gateway IP (`10.200.1.2/30`) as a secondary address, so the container's
  default route resolves at L2 via xray.
- An OpenFlow egress rule on `ovsbr0` restricts `netmk` to WireGuard UDP
  (port 51822) only — all other egress from `netmk` is denied. This rule
  MUST be verified in place before `netclient join` is attempted.
- `netclient join` succeeds and `wg show` inside the container reports
  handshake activity with at least one remote peer.
- The two existing `proxy` devices (`api-lo`, `broker-mesh`) continue to
  function unchanged.
- The host's own `3tched` WireGuard interface and mesh membership are
  unaffected.
- Provisioning uses the existing D-Bus plugin surface (`rovs_commands`,
  `rtnetlink`, `ovsdb_bridge`) — no raw `ip`/`ovs-vsctl` shell calls.
- `netclient` is supervised inside the container by `op-grpc-adapters`
  (the existing `NetmakerAdapter` tonic service), deployed as a running
  process inside the `NetMaker` container and reachable from the host via
  a loopback `proxy` device. The control plane uses `op-grpc-bridge`'s
  existing `netmaker_join()` / `netmaker_leave()` / `netmaker_restart()`
  client methods — no new client-side supervision code is needed.

### US-2 — xray veth elimination (Phase 2)

As the control plane operator, I want xray's network attachment changed from a
veth-backed `nictype: bridged` Incus NIC to the same OVS-internal-port pattern
used by `svc0`/`grpc`/`pub0` and (after Phase 1) `NetMaker`, so that the
topology is uniform and the one remaining veth (`vethde51090d`) is removed.

**Acceptance criteria**

- xray routes production traffic through an OVS internal port, not a veth.
- Zero observable downtime for `api.3tched.com`, `broker.3tched.com`, and other
  domains currently ingressing through xray.
- A tested rollback plan exists that restores the veth-backed NIC within
  seconds if the cutover fails.
- The change is executed as a scheduled maintenance window, not automatically
  triggered by Phase 1 completion.

---

## 2 · Non-functional Requirements

| ID    | Requirement                                                                                    |
| ----- | ---------------------------------------------------------------------------------------------- |
| NFR-1 | Phase 1 must not disrupt any currently-working service (proxy devices, host mesh, xray).       |
| NFR-2 | Phase 2 cutover target: < 2 s traffic interruption during NIC swap.                            |
| NFR-3 | All network provisioning must use the native D-Bus/OVSDB/rtnetlink surface — no shell wrappers.|
| NFR-4 | OVS port naming must follow existing convention (`svc0`, `grpc`, `pub0` → short, descriptive). |
| NFR-5 | Phase 2 must have an independently reviewable rollback procedure before execution.             |
| NFR-6 | Netclient's WireGuard UDP egress MUST share the same public-facing exit identity as xray's obfuscated outbound (WARP). No distinct raw public IP path. |

---

### US-1 AC addendum — Obfuscated egress (mandatory)

- Netclient's WireGuard UDP egress MUST exit through xray's WARP-obfuscated
  outbound path (`wgcf-egress`, fwmark `0x51821`, table 51820), NOT via a
  separate host-level SNAT through `pub0`.
- The public-facing egress identity for netclient's WireGuard traffic MUST be
  the same Cloudflare WARP IP that xray's own obfuscated outbound uses — not
  the host's raw public IP `188.68.58.237`.
- This means the `netmk` port's default gateway routes into xray's network
  namespace (where xray already has the only NIC), and xray-side policy
  routing applies the WARP fwmark to this forwarded traffic.
- Rationale: this is a privacy-focused product (see
  `pitch/GhostBridge-Netmaker-pitch.md`); all other container egress already
  routes through xray's obfuscation plane. A raw SNAT path for one flow
  creates an attributable fingerprint inconsistent with the product's own
  privacy design. The asymmetry (one flow hidden, one raw) is itself a signal.

**Why not full HTTP-proxy encapsulation**: Xray's `egress-proxy` inbound at
`10.0.0.2:10809` is `protocol: "http"` (verified from live config). HTTP proxy
supports only TCP via CONNECT — it cannot carry WireGuard's UDP data plane.
SOCKS5 UDP-ASSOCIATE is not configured and has known limitations with
always-on bidirectional UDP tunnels. The achievable hiding is at the
**network-identity level** (same WARP exit IP, same obfuscated pipe) rather
than protocol-level encapsulation inside the HTTP proxy.

---

## 3 · Out of Scope

- Replacing or modifying the host's own `3tched` WireGuard membership.
- Changing the Netmaker server configuration or API keys.
- Modifying Incus's `proxy` devices (`api-lo`, `broker-mesh`).
- Implementing dynamic Xray tag routing (that's a separate feature).
- Automating Phase 2 execution from Phase 1's deploy pipeline.
- Wrapping WireGuard UDP inside xray's HTTP proxy protocol (infeasible — see
  §2 AC addendum for rationale).

---

## 4 · Resolved Questions

- ~~What IP subnet should the NetMaker OVS internal port use?~~ **Resolved:**
  `10.200.1.0/30` — container gets `.1`, gateway `.2` on xray-side forwarding
  interface. Confirmed non-colliding with `svc0` (`10.200.0.2/24`), `pub0`
  (`188.68.58.237/22`), and mesh range (`100.69.0.0/16`).
- ~~Does the `NetMaker` container need masquerade/SNAT for outbound UDP?~~
  **Resolved**: Yes, but SNAT happens inside xray's netns (behind WARP), not
  on the host via `pub0`. See design.md §2.3.
- ~~Should netclient egress be obfuscated?~~ **Resolved**: Yes. Full
  HTTP-proxy wrapping is infeasible (xray `10809` is `protocol: http`, cannot
  carry UDP). Network-identity-level hiding is achievable: route through
  xray's netns → xray's WARP-marked outbound → `wgcf-egress` → Cloudflare
  WARP IP. Public-facing identity is unified with all other obfuscated egress.
- For Phase 2, can Incus hot-remove a `nic` device and re-add a `none`-type
  device pointing at the same bridge port without restarting the container, or
  must the container restart?
