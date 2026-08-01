# Design — Netclient Container Netns

## 1 · Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│  Host (Artix, runit)                                                    │
│                                                                         │
│  ┌─────────────── ovsbr0 (OVS bridge, datapath=system) ──────────────┐  │
│  │                                                                    │  │
│  │  pub0 (internal)       svc0 (internal)    grpc (internal)          │  │
│  │  188.68.58.237/22      10.200.0.2/24      (in grpc netns)          │  │
│  │                                                                    │  │
│  │  netmk (internal)  ← NEW Phase 1                                   │  │
│  │  10.200.1.1/30         moved into NetMaker container netns          │  │
│  │                                                                    │  │
│  │  xray0 (internal)  ← NEW Phase 2 (replaces vethde51090d)           │  │
│  │  10.200.0.1/30         moved into xray container netns              │  │
│  │                                                                    │  │
│  │  eno1 (uplink, enslaved)                                           │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                         │
│  ┌─── NetMaker container (OCI, Incus) ───┐                              │
│  │  lo          (loopback, UP)            │                              │
│  │  netmk       10.200.1.1/30            │ ← OVS internal port          │
│  │  default via 10.200.1.2               │                              │
│  │                                        │                              │
│  │  netclient → WireGuard UDP out via     │                              │
│  │              netmk → ovsbr0 → xray     │                              │
│  │              → WARP → Cloudflare       │                              │
│  │                                        │                              │
│  │  proxy: api-lo   (unchanged)           │                              │
│  │  proxy: broker-mesh (unchanged)        │                              │
│  └────────────────────────────────────────┘                              │
│                                                                         │
│  ┌─── xray container (OCI, Incus) ────────────────────────────────────┐  │
│  │  lo          (loopback, UP)                                        │  │
│  │  eth0        10.200.0.1/24 + 10.0.0.2/24 + 10.200.1.2/30 (GW)    │  │
│  │                                                                    │  │
│  │  ip_forward=1  (transit packets from netmk)                        │  │
│  │  iptables mangle FORWARD: -s 10.200.1.0/30 → mark 0x51821         │  │
│  │  iptables nat POSTROUTING: -s 10.200.1.0/30 -o wgcf-egress → MASQ │  │
│  │  ip rule: fwmark 0x51821 → table 51820 → via wgcf-egress          │  │
│  │                                                                    │  │
│  │  Egress proxy: 0.0.0.0:10809 (HTTP, TCP only — existing)          │  │
│  │  xhttp-in: 10.200.0.1:8444 (VLESS, SNI termination)               │  │
│  │                                                                    │  │
│  │  After Phase 2: xray0 (OVS internal) replaces eth0                 │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                                         │
│  wgcf-egress (host or xray netns — T-NEW-1 determines)                  │
│  → Cloudflare WARP (obfuscated exit, shared IP)                         │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Routing for outbound UDP (NetMaker → WireGuard peers) — Obfuscated Path

**Design principle**: All NetMaker container egress exits through xray's
obfuscated outbound path (WARP), not a raw SNAT via the host's `pub0`. This
is a product requirement: the deployment is a privacy-focused mesh VPN
(`pitch/GhostBridge-Netmaker-pitch.md`); a second, raw, directly-attributable
egress path would be a privacy regression and a fingerprinting signal.

**Why not HTTP-proxy encapsulation**: The existing egress path for TCP traffic
(`127.0.0.1:3128 → 10.0.0.2:10809`) uses xray's `egress-proxy` inbound, which
is `"protocol": "http"` (verified from live `/dev/shm/xray_config.json`). HTTP
proxy carries TCP via CONNECT only — it **cannot** carry WireGuard's UDP data
plane. SOCKS5 UDP-ASSOCIATE is not configured, and even if it were, it has
well-known limitations with bidirectional always-on UDP tunnels (WireGuard
keepalives, rekey handshakes). Full protocol-level encapsulation is infeasible.

**Achievable hiding — network-identity-level**: Route netclient's UDP through
xray's network namespace, where xray's existing WARP policy routing
(fwmark `0x51821` → table 51820 → `wgcf-egress`) applies. The packet exits
bearing the Cloudflare WARP IP, not `188.68.58.237`. Public-facing egress
identity is unified: an observer sees one identity (WARP) for all traffic from
this deployment, not a split between "xray's WARP IP" and "host's raw IP".

**Mechanism**: The `netmk` OVS internal port remains on `ovsbr0` and is moved
into the NetMaker container's netns (unchanged from original design). The
difference is in **gateway placement**: instead of `10.200.1.2` on the
`ovsbr0` bridge interface (host netns, which routes via `pub0`), the gateway
is **inside xray's netns**. Xray's `eth0` is already on `ovsbr0` — it sees
all bridge traffic at L2. A secondary IP (`10.200.1.2/30`) on xray's `eth0`
makes xray the ARP-resolvable next-hop for `netmk`'s default route.

Once the packet arrives in xray's netns:
1. Xray's kernel sees a packet to forward (IP forwarding enabled)
2. The packet is **not** destined for a local xray socket — it's a transit packet
3. Xray's policy routing applies: iptables POSTROUTING marks it with
   `0x51821`, ip rule sends marked traffic to table 51820, which routes
   via `wgcf-egress` → Cloudflare WARP
4. SNAT: masquerade behind the WARP tunnel's assigned IP (Cloudflare address)
5. Packet exits encrypted inside the WARP WireGuard tunnel to Cloudflare edge

Reply path: Cloudflare → `wgcf-egress` → xray netns → kernel routes
`10.200.1.1` via `eth0` (bridge-connected) → OVS delivers to `netmk` port →
NetMaker container netns → netclient socket.

**This mirrors the existing `svc0` ↔ xray relationship** — `svc0`
(`10.200.0.2`) is the host endpoint for xray's `10.200.0.1` subnet, both on
`ovsbr0`. Here, xray (`10.200.1.2`) is the gateway endpoint for NetMaker's
`10.200.1.1`, also on `ovsbr0`. The only addition is IP forwarding + WARP
fwmark application inside xray's netns for transit packets.

**Key advantage over the raw-SNAT design**: No new public-facing egress
identity. No MASQUERADE on `pub0`. No fingerprinting asymmetry. The cost is:
xray must be running before netclient can function (an explicit boot-order
dependency, which is acceptable and already implicitly true for all other
container services).

---

## 2 · Components

### 2.1 — OVS Port Provisioning (Phase 1)

The existing `rovs_commands.add_port` D-Bus method creates an OVS internal port:

```json
{
  "bridge": "ovsbr0",
  "port_name": "netmk",
  "interface_type": "internal"
}
```

After creation, the port exists in the host's default netns as a kernel
network interface named `netmk`. It must then be moved into the NetMaker
container's network namespace.

### 2.2 — Netns Move (Phase 1)

The `rtnetlink` plugin's `AddLinkInput` / namespace operations (or a new
`MoveLinkInput` if not yet exposed) transfer the interface into the container's
netns. The container's netns is identified by the PID of its init process
(`incus info NetMaker` → PID, or `/proc/<pid>/ns/net`).

Steps via D-Bus:
1. `rovs_commands.add_port` → creates `netmk` (internal) on `ovsbr0`
2. `rtnetlink.set_link_netns` (or equivalent) → moves `netmk` into container netns by PID
3. `rtnetlink.add_ipv4_address` (within container netns) → `10.200.1.1/30` on `netmk`
4. `rtnetlink.set_link_state` → bring `netmk` UP
5. `rtnetlink.set_default_route` → `10.200.1.1 via 10.200.1.2 dev netmk`

If the rtnetlink plugin does not currently support cross-netns operations (all
current usage appears host-scoped), a thin extension is needed: accept an
optional `netns_pid` field on inputs, and `setns(CLONE_NEWNET)` before the
netlink call. This is the same pattern used by `ip netns exec`.

### 2.3 — Xray-Side Routing & WARP Egress (Phase 1)

The gateway for `netmk` lives **inside xray's netns**, not on the host's
`ovsbr0` bridge interface. Configuration steps inside xray's network namespace:

- Assign gateway IP: `10.200.1.2/30` as a secondary address on xray's `eth0`
  (which is already on `ovsbr0`). This makes xray the ARP-resolvable
  next-hop for `netmk`'s default route.
- Enable IP forwarding: `sysctl net.ipv4.ip_forward=1` inside xray's netns
  (may already be enabled; verify).
- Add iptables fwmark rule in xray's netns:
  `-t mangle -A FORWARD -s 10.200.1.0/30 -j MARK --set-mark 0x51821`
  This applies the WARP routing mark to transit packets from NetMaker.
- Add iptables MASQUERADE in xray's netns:
  `-t nat -A POSTROUTING -s 10.200.1.0/30 -o wgcf-egress -j MASQUERADE`
  This SNATs behind the WARP tunnel's Cloudflare-assigned IP.
- Verify: ip rule inside xray already has `fwmark 0x51821 lookup 51820`
  (this is the existing WARP policy route per §8 of the topology doc).
- Verify: table 51820 routes via `wgcf-egress` device.

**Alternative if `wgcf-egress` is not UP in xray's netns** (it runs on the
host, not inside the container): The fwmark + ip-rule approach requires the
WARP interface to be visible in xray's netns. If `wgcf-egress` is host-level
only, the packet must first exit xray's netns back to the host, then get
WARP-routed. In that case, the design becomes:
1. xray's `eth0` receives the forwarded packet
2. xray applies MASQUERADE behind its own `10.200.0.1` (or `10.0.0.2`)
3. Packet exits to `ovsbr0` → host netns
4. Host-side iptables marks packets from xray's known source IPs with `0x51821`
5. Host ip-rule → table 51820 → `wgcf-egress`

Either way, the **public-facing exit** is the WARP IP, not `188.68.58.237`.
The investigation task (T-NEW-1) determines which path applies based on where
`wgcf-egress` actually lives.

**What this replaces**: The prior design had `10.200.1.2/30` on the host's
`ovsbr0` bridge interface with a MASQUERADE rule on `pub0`. That created a
raw SNAT path (`188.68.58.237` as source) — an unobfuscated, directly-
attributable egress identity parallel to xray's hidden one. Removed.

### 2.3.1 — Boot-Order Dependency Chain (Phase 1)

Netclient cannot function until xray's WARP-obfuscated egress path is ready.
This introduces explicit boot-order dependencies, enforced via runit ready-
stamps following the existing `/run/opdbus/runit-ready/` convention.

**New dependency graph (extends §3 of `host-socket-topology-live.md`):**

```text
[existing boot chain]
opdbus-rundirs → ... → ovsbr0-addr → ovsbr0-svc-addr → op-of-controller
                                                              ↓
                                                        wg-3tched
                                                              ↓
                                                    ┌─────────────────┐
                                                    │  xray container  │
                                                    │  (already boots  │
                                                    │   after ovsbr0)  │
                                                    └────────┬────────┘
                                                             ↓
                                                    xray-egress-ready  ← NEW stamp
                                                    (xray netns has:
                                                     - 10.200.1.2/30 on eth0
                                                     - ip_forward=1
                                                     - fwmark/MASQ rules
                                                     - WARP route functional)
                                                             ↓
                                                    netmk-port-attach  ← NEW stamp
                                                    (OVS port created,
                                                     moved into NetMaker netns,
                                                     IP + route configured)
                                                             ↓
                                                    netmk-of-restrict  ← NEW stamp
                                                    (OpenFlow egress rules
                                                     installed on ovsbr0)
                                                             ↓
                                                    netclient-start    ← NEW stamp
                                                    (netclient daemon launched
                                                     inside NetMaker CT)
```

**Ready-stamp definitions:**

| Stamp | Path | Gate condition |
| ----- | ---- | -------------- |
| `xray-egress-ready` | `/run/opdbus/runit-ready/xray-egress-ready` | `10.200.1.2/30` on xray `eth0` + forwarding + fwmark rules + WARP route verified |
| `netmk-port-attach` | `/run/opdbus/runit-ready/netmk-port-attach` | OVS port in NetMaker netns, IP assigned, link UP, default route set |
| `netmk-of-restrict` | `/run/opdbus/runit-ready/netmk-of-restrict` | OpenFlow egress rules verified on `ovsbr0` for `netmk` port |
| `netclient-start` | `/run/opdbus/runit-ready/netclient-start` | `netclient daemon` running (pgrep confirms PID) |

**Enforcement**: Each stage's runit `run` script polls for the predecessor
stamp file before proceeding (same pattern as existing `ovsbr0-uplink` waiting
on `uplink-dhcp`, etc.). If a predecessor stamp is absent after timeout
(configurable, default 30s), the service logs an error and exits non-zero
(runit will retry after `finish` sleep).

**Container restart recovery**: If `NetMaker` restarts (new PID), the `netmk`
port is lost from its netns. The reconciliation loop detects this (port exists
on OVS but not in expected netns), removes the stale stamp
`netmk-port-attach`, and re-runs the attach sequence. Similarly, if xray
restarts, `xray-egress-ready` is invalidated and the chain re-waits.

### 2.4 — Netclient Supervision (Phase 1) — Resolution

**Problem**: `netmaker.rs`'s `ServiceController::S6` variant calls
`org.opdbus.v1.S6.Systemctl` on the host's system D-Bus. This manages
host-level runit services. But `netclient` must run *inside* the container's
namespace (it needs the `netmk` interface and its routing table).

**Resolution**: The problem is already solved in code. `crates/op-grpc-adapters`
contains `NetmakerAdapter`, a full tonic gRPC service (`op.adapters.v1.NetmakerService`)
that is designed to run **colocated with netclient** inside the container:

- `join_network` / `leave_network` call `patch_netclient_config()` which does
  `tokio::fs::write("/etc/netclient/netclient.json", ...)` directly, then
  `restart_netclient()` which runs `sv restart netclient` (runit) locally.
- `execute_command` provides full netclient CLI passthrough (connect, disconnect,
  list, peers, ping, pull, push, register, install, use, version) by shelling to
  the local `netclient` binary.
- `get_server_health`, `list_hosts`, `list_nodes`, `stream_events` provide
  health and observability.

The host-side consumer already exists: `crates/op-grpc-bridge/src/grpc_client.rs`
has a working `netmaker_client()` pool accessor and helper methods
`netmaker_join()`, `netmaker_leave()`, `netmaker_restart()`, and `netclient_peers()`.

**What is needed (deployment, not new code):**

1. **Deploy `op-grpc-adapters` inside the `NetMaker` container.** The binary
   (`[[bin]] name = "op-grpc-adapters"`) is already buildable. It needs to run
   as a long-lived process inside the container where netclient lives.

2. **Add a third Incus `proxy` device** to expose the gRPC endpoint to the
   host. This follows the existing `api-lo` (`127.0.0.1:8081`) and
   `broker-mesh` pattern already on this container. Only ordinary loopback TCP —
   no OVS port or netns work needed for the control path.

3. **Configure `op-grpc-bridge`'s `netmaker_client()` endpoint** to point at
   the new proxy device's host-side address. Once the proxy exists and
   `op-grpc-adapters` is listening behind it, the existing client methods
   work against the real thing — no new client-side code is needed.

**Open question (must verify before deploying):**
`NetmakerAdapter::restart_netclient()` delegates to `execute_command(Restart)`,
which runs `Command::new(op_core::runit::SV_BIN).args(["restart", "netclient"])` —
i.e., it assumes `sv` (runit) is present and managing `netclient` inside the
container. Earlier live checks confirmed the `netclient` binary and
`/etc/netclient/` config exist in the container, but runit/`sv` presence inside
this specific container was never verified. If runit is not available:
- Option A: Install and configure a minimal runit setup inside the container
  (a single `run` script for netclient under `/etc/service/netclient/`).
- Option B: Replace the `restart_netclient` code path to use a direct
  `pkill netclient && netclient daemon` sequence instead of `sv restart`.
  This is a one-line change in the `Restart` arm of `execute_command`.

**Note (out of scope — do not fix in this spec):**
`crates/op-plugins/src/state_plugins/netmaker.rs`'s `dispatch_netmaker_method()`
does its own separate `reqwest` REST calls to the Netmaker server API directly,
rather than going through the `op.adapters.v1.NetmakerService` described here.
This is a third parallel path (schema-driven plugin → raw REST) alongside the
tonic adapter. It should eventually be reconciled (the state_plugins dispatch
could call the same `netmaker_client()` helpers instead of hand-rolled REST),
but this spec does not take that on — just be aware it exists and the design
does not imply these paths are already unified.

### 2.5 — OCI Plugin Schema Update (Phase 1)

Update the `NetMaker` entry in the OCI plugin schema to declare `port_attach`
and the native gRPC adapter:

```json
{
  "container": "NetMaker",
  "loopback_required": true,
  "port_attach": {
    "bridge": "ovsbr0",
    "iface_name": "netmk",
    "ip_addrs": ["10.200.1.1/30"],
    "gateway": "10.200.1.2",
    "routes": []
  },
  "grpc_adapter": {
    "binary": "op-grpc-adapters",
    "proxy_device": "grpc-adapter",
    "host_endpoint": "http://127.0.0.1:<port>"
  },
  "supervision": "native_grpc_adapter"
}
```

This integrates with the existing lifecycle: boot → loopback UP →
`rovs_commands.AttachPort` → rtnetlink configure. Netclient supervision is
handled by the in-container `op-grpc-adapters` process, reachable from the
host via the `grpc-adapter` proxy device and consumed through
`op-grpc-bridge`'s existing `netmaker_client()` methods.

### 2.6 — xray Port Replacement (Phase 2)

Replace xray's `eth0` (veth-backed, Incus `nictype: bridged`) with an OVS
internal port `xray0`:

```json
{
  "bridge": "ovsbr0",
  "port_name": "xray0",
  "interface_type": "internal"
}
```

The OCI plugin entry already has `port_attach` referencing `gbr_xray`
(`10.200.0.1/30`). Phase 2 replaces the Incus-managed veth with a
control-plane-managed OVS internal port using the same IP/gateway.

---

## 3 · Data Flow

### Phase 1 — netclient outbound WireGuard handshake (obfuscated path)

1. `netclient` inside NetMaker opens UDP socket, calls `sendto(peer_ip:51822, ...)`
2. Kernel routes via default route → `netmk` interface
3. Frame exits into OVS bridge `ovsbr0` (port `netmk`, destination MAC = gateway `10.200.1.2`)
4. OVS L2 learning delivers frame to xray's `eth0` (which holds `10.200.1.2/30`)
5. **Inside xray's netns**: kernel receives packet for forwarding (ip_forward=1)
6. iptables mangle FORWARD: marks packet with `0x51821` (WARP fwmark)
7. iptables nat POSTROUTING: MASQUERADE behind WARP tunnel source IP
8. ip rule: fwmark `0x51821` → lookup table 51820 → route via `wgcf-egress`
9. Packet enters WARP WireGuard tunnel → encrypted → exits to Cloudflare edge
10. Cloudflare forwards to destination WireGuard peer

**Reply path:**
1. Peer sends UDP reply → Cloudflare edge → WARP tunnel → `wgcf-egress` decrypts
2. Packet arrives in xray's netns (or host netns, depending on where `wgcf-egress` lives — see §2.3 alternative)
3. Conntrack DNAT restores original destination (`10.200.1.1`)
4. Kernel routes `10.200.1.1/30` via xray's `eth0` → OVS bridge
5. OVS delivers to port `netmk` → NetMaker container netns
6. Netclient socket receives reply

**What an external observer sees**: UDP traffic from a Cloudflare WARP IP
(shared with millions of other WARP users) to the WireGuard peer. The same
IP identity used by xray's own obfuscated outbound. No `188.68.58.237`
attribution. No split-identity fingerprint.

### Phase 2 — xray cutover sequence

1. Pre-stage: create OVS internal port `xray0` on `ovsbr0` (does not disrupt anything)
2. Assign xray's current IP (`10.200.0.1/30`) to `xray0` inside container netns
3. Atomically: remove Incus `eth0` NIC device (`incus config device remove xray eth0`)
4. Immediately: move `xray0` into xray's netns, bring UP, set routes
5. Verify: `curl -x '' http://10.200.0.1:443` from host confirms xray responding
6. If fail within 10 s: rollback (re-add Incus `eth0` device, remove `xray0`)

---

## 4 · Failure Modes

| Failure                                           | Behavior                                                       |
| ------------------------------------------------- | -------------------------------------------------------------- |
| `rovs_commands.add_port` fails (port exists)      | Idempotent — check existence first, skip if present            |
| Netns move fails (wrong PID, container restarted) | Retry with fresh PID lookup; alert if 3 consecutive failures   |
| Xray-side rules missing after restart             | `xray-egress-ready` stamp removed; boot-chain re-executes      |
| `wgcf-egress` DOWN (WARP tunnel offline)          | Netclient traffic blackholes (by design — no fallback to raw); alert fires |
| xray container restarted                          | `xray-egress-ready` invalidated; `netmk-port-attach` re-waits; netclient stops until chain re-converges |
| Phase 2: `xray0` move fails mid-cutover          | Rollback: re-add Incus `eth0` device (< 5 s restore)          |
| Phase 2: xray unresponsive after swap             | Health check (10 s timeout) triggers automatic rollback        |
| netclient fails to join after port attached       | `netmaker.rs` health loop retries join; port remains for manual debug |

---

## 5 · Security

- The `10.200.1.0/30` subnet is not exposed externally; MASQUERADE inside
  xray's netns ensures only the WARP-assigned Cloudflare IP appears as
  source on the wire — the same IP identity used by all other obfuscated
  traffic from this deployment.
- **No raw public-IP attribution**: Unlike the prior design (SNAT behind
  `pub0`/`188.68.58.237`), netclient's WireGuard UDP never bears the host's
  directly-attributable public IP. An observer cannot correlate this
  WireGuard traffic to the host's other public services.
- **Unified egress identity**: All container egress (TCP via xray HTTP proxy,
  UDP via xray WARP forwarding) exits through the same Cloudflare WARP
  endpoint. No asymmetry to fingerprint against.
- **OpenFlow egress restriction on `netmk` (Phase 1, required):** The
  `NetMaker` container was deliberately made NIC-less for isolation — its
  only current network paths are two narrowly-scoped `proxy` devices
  (`api-lo`, `broker-mesh`) on fixed single ports. Adding `netmk` must not
  undo that isolation posture. Before `netclient join` is attempted, an
  OpenFlow rule MUST be installed on `ovsbr0` restricting `netmk` to:
  - Allow: UDP egress to any destination on port 51822 (WireGuard listen
    port — read from `/etc/netclient/netclient.json` `"listenport"` in the
    live container; 51822 as of this writing, not the generic default 51820).
  - Allow: return traffic (established UDP sessions via conntrack or
    symmetric allow rule).
  - Deny: all other egress from `netmk`.
  This is installed via the existing `openflow`/`openflow_obfuscation` state
  plugin's D-Bus surface, not raw `ovs-ofctl` shell commands.
- `incus exec` calls from the plugin run as root inside the container; this
  matches the container's existing `CapEff` (full capabilities).
- Phase 2 cutover window is the only moment when xray lacks connectivity; the
  health-check rollback bounds this to < 10 s worst case.
- **Boot-order security**: netclient cannot start until the obfuscation path
  is verified ready (see §2.3.1 dependency chain). This prevents accidental
  raw egress during a race where `netmk` has a route but WARP isn't ready yet.

---

## 6 · Trade-offs Accepted

**Native gRPC adapter inside the container instead of `incus exec` reach-in** —
The existing `op-grpc-adapters` binary runs inside the `NetMaker` container and
supervises `netclient` through local filesystem and process operations. This is
better than an `incus exec`-based approach because: (a) it already exists and
is tested, (b) it's a native gRPC service the host-side bridge already knows
how to consume, (c) it eliminates the Incus API as a runtime dependency for
supervision, and (d) it reuses proven `patch_netclient_config` + `sv restart`
patterns instead of inventing a new reach-in mechanism. The cost is one
additional long-lived process inside the container and one additional proxy
device on the host.

**Point-to-point /30 subnet per container** — Uses two IPs per container port
(one for the container, one implicit for the bridge/host gateway). This is
slightly wasteful but avoids shared-subnet ARP complexity and matches the
existing `svc0` (`10.200.0.2/24`) pattern's intent of isolation.

**Xray as forwarding gateway instead of raw host SNAT** — Adds a dependency
on xray being UP for netclient to function. Acceptable because: (a) xray
is already the single point of egress for all other container traffic, (b)
if xray is down, the deployment's privacy posture is already degraded, and
(c) the boot-order chain makes this explicit rather than a hidden assumption.

**WARP-level hiding, not protocol-level encapsulation** — WireGuard UDP is
not wrapped inside xray's HTTP proxy or a SOCKS5 tunnel. An observer who can
see inside the WARP tunnel (i.e., Cloudflare themselves) would see raw
WireGuard UDP. This is acceptable because: (a) the threat model is
external network observers and hosting provider logging, not Cloudflare
adversarial analysis, (b) protocol-level wrapping of UDP inside TCP
(udp2raw, etc.) adds latency and fragility for minimal additional privacy
against the relevant threat model, (c) the primary goal — unified public IP
identity — is fully achieved.

**Additional complexity inside xray's netns** — IP forwarding + iptables
mangle/nat rules inside a container add configuration surface. Mitigated by:
the rules are declarative, enforced at boot via ready-stamps, and testable
in isolation.

**Phase 2 requires container restart if hot-remove fails** — Incus may not
support hot-removing a `nic` device from a running container. If it doesn't,
Phase 2 falls back to a brief container restart. The spec accepts this as a
worst-case bounded outage (< 5 s) covered by the rollback plan.
