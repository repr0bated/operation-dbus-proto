# Spec — Netclient Container Netns (interface contract)

---

## 1 · OVS Port Contract

### Port `netmk` (Phase 1)

```
Bridge:          ovsbr0
Port name:       netmk
Interface type:  internal
Residing netns:  NetMaker container (by init PID)
IPv4 address:    10.200.1.1/30  (container-side only)
MAC:             auto-assigned by OVS (no explicit pin)
Link state:      UP
Default route:   via 10.200.1.2 dev netmk
```

**Gateway:** The `10.200.1.2/30` address lives on **xray's `eth0`** as a
secondary address (inside xray's netns) — not on `ovsbr0` bridge interface,
not on `pub0`, not on a dedicated internal port. Since xray's `eth0` is
already on `ovsbr0`, it receives `netmk`'s ARP requests and responds as the
gateway. Xray then forwards the transit packet through its WARP-obfuscated
outbound path.

This is the privacy-preserving alternative to the standard OVS pattern of
putting the gateway on the bridge's own interface (which would route via
`pub0` — raw, unobfuscated). The cost is an explicit boot-order dependency
on xray.

**Invariants:**

- The port MUST be created via OVSDB transact (native JSON-RPC or
  `rovs_commands.add_port` D-Bus), never via `ovs-vsctl` subprocess.
- The port MUST be moved into the container netns via rtnetlink
  `RTM_NEWLINK` + `IFLA_NET_NS_PID`, never via `ip link set netns`.
- If the container restarts (new PID), the port is lost from the netns.
  The reconciliation loop MUST detect this and re-provision (idempotent
  create → move → configure cycle).
- The port name `netmk` is stable across reconciliations — never
  generate unique suffixes.

### Port `xray0` (Phase 2)

```
Bridge:          ovsbr0
Port name:       xray0
Interface type:  internal
Residing netns:  xray container (by init PID)
IPv4 address:    (same IPs currently on eth0, including 10.200.0.1/30)
MAC:             (same MAC currently on eth0 — pin explicitly)
Link state:      UP
Default route:   via 10.200.0.2 dev xray0
```

**Invariants:**

- The MAC address MUST be explicitly set to match xray's prior `eth0` MAC.
  ARP caches on peers (OVS flow tables, connected clients) depend on MAC
  stability. Failure to pin MAC = traffic blackhole until ARP expires.
- Phase 2 port creation MUST happen *before* the veth removal. The port
  exists idle in host netns until the cutover moment.
- The Incus `nic` device removal and OVS port netns-move MUST happen in
  the correct order: move `xray0` into netns and bring UP *first*, then
  remove Incus `eth0`. This minimizes the window where the container has
  no external connectivity.

---

## 2 · Obfuscated Egress Contract (Phase 1)

### Egress path

```
NetMaker netclient UDP → netmk (10.200.1.1/30)
  → OVS ovsbr0 (L2 forward to xray eth0 MAC for 10.200.1.2)
  → xray netns: ip_forward → mangle FORWARD (mark 0x51821)
  → xray netns: nat POSTROUTING (MASQUERADE on wgcf-egress)
  → ip rule fwmark 0x51821 → table 51820 → wgcf-egress
  → Cloudflare WARP tunnel → internet (Cloudflare exit IP)
```

**Invariants:**

- The **public-facing source IP** for netclient's WireGuard UDP MUST be the
  Cloudflare WARP IP — never `188.68.58.237` (host's raw public IP).
- All container egress from this deployment shares a single public-facing
  identity (the WARP IP). No split-identity fingerprint.
- If `wgcf-egress` is DOWN or unreachable, netclient traffic MUST fail
  (blackhole) rather than fall back to raw egress. This is enforced by:
  no MASQUERADE rule on `pub0` for `10.200.1.0/30` (removed from design),
  and no default route in the WARP routing table if the tunnel is down.

### Xray-side forwarding rules

```
Location:   Inside xray container netns
Requires:   net.ipv4.ip_forward = 1

Mangle:     -t mangle -A FORWARD -s 10.200.1.0/30 -j MARK --set-mark 0x51821
NAT:        -t nat -A POSTROUTING -s 10.200.1.0/30 -o wgcf-egress -j MASQUERADE
Address:    10.200.1.2/30 secondary on eth0
```

**Alternative (if `wgcf-egress` is host-only, determined by T-0):**

```
Location:   Inside xray container netns
NAT:        -t nat -A POSTROUTING -s 10.200.1.0/30 -j MASQUERADE
            (MASQs behind xray's own 10.200.0.1 or 10.0.0.2)

Location:   Host netns
Mangle:     -t mangle -A FORWARD -s 10.200.0.1 -p udp --dport 51822 -j MARK --set-mark 0x51821
            (marks netclient-origin traffic after it exits xray back to host)
Policy:     ip rule add fwmark 0x51821 table 51820
Route:      table 51820: default via wgcf-egress dev
```

**Invariants (both paths):**

- The fwmark `0x51821` is the existing WARP routing mark (per
  `host-socket-topology-live.md` §8). Do not invent a new mark.
- The MASQUERADE rule MUST NOT exist on `pub0` for `10.200.1.0/30` — this
  is the removed raw-egress path and must never be reinstated.
- Rules must be idempotent: check with `-C` before inserting with `-A`.
- Rules must persist across container/service restarts (enforced by the
  `xray-egress-ready` boot-order stamp which reapplies them).

### Boot-order dependency contract

```
Ready stamps (all in /run/opdbus/runit-ready/):

xray-egress-ready    gates→  netmk-port-attach
netmk-port-attach    gates→  netmk-of-restrict
netmk-of-restrict    gates→  netclient-start
```

**Invariants:**

- Each stamp is a regular file; presence = gate satisfied.
- Each stage's runit `run` script polls for its predecessor stamp before
  proceeding. Timeout: 30s default, then exit non-zero (runit retries).
- If a predecessor service restarts (e.g., xray), its stamp is removed
  and all downstream stamps are invalidated (cascade re-wait).
- Stamps are in volatile `/run` — they do not survive reboot (correct:
  the chain re-executes on every boot).

---

## 2.5 · OpenFlow Egress Restriction Contract (Phase 1)

```
Bridge:     ovsbr0
Scope:      Traffic originating from OVS port "netmk"
WG Port:    UDP/51822 (from live /etc/netclient/netclient.json "listenport")
```

**Rules (logical, installed via openflow state plugin D-Bus):**

| Priority | Match                                            | Action |
| -------- | ------------------------------------------------ | ------ |
| 100      | in_port=netmk, udp, tp_dst=51822                | NORMAL |
| 100      | in_port=netmk, udp, tp_src=51822                | NORMAL |
| 100      | in_port=netmk, arp                               | NORMAL |
| 50       | in_port=netmk                                    | drop   |
| 0        | (default, all other ports)                       | NORMAL |

**Invariants:**

- The WireGuard port (51822) is sourced from the live container config, not
  assumed. If `netclient.json` changes the listen port, the OpenFlow rules
  MUST be updated to match.
- These rules MUST be installed and verified **before** `netclient join` is
  attempted (T-12.6 before T-16).
- ARP must be explicitly allowed — without it, the container cannot resolve
  the gateway MAC and all traffic fails silently.
- Rules are installed via the `openflow`/`openflow_obfuscation` state
  plugin's D-Bus surface, never raw `ovs-ofctl`.
- Rules MUST be persisted in `op-ovsbr0-setup` or equivalent to survive
  bridge restarts.
- The existing baseline (single priority=0 NORMAL rule) must not be removed —
  it provides default forwarding for all other ports (`pub0`, `svc0`, etc.).

---

## 3 · NetmakerAdapter Deployment Contract

### Binary

```
Crate:       op-grpc-adapters
Binary:      op-grpc-adapters (Cargo [[bin]] target)
Runtime:     Inside the NetMaker container (colocated with netclient)
Listen:      0.0.0.0:<port> (gRPC, tonic)
Service:     op.adapters.v1.NetmakerService
```

### Proxy device (new, third device on NetMaker container)

```
Device name:     grpc-adapter
Type:            proxy
Listen:          tcp:127.0.0.1:<host-port>  (host loopback)
Connect:         tcp:127.0.0.1:<container-port>
Bind:            host
```

This follows the existing pattern of `api-lo` (`127.0.0.1:8081`) and
`broker-mesh` on this container. The proxy provides host-side TCP
reachability for `op-grpc-bridge`'s `netmaker_client()` pool without any
OVS port or netns involvement — the control/supervision path is ordinary
loopback TCP, independent of the `netmk` data-plane port.

### Runtime requirements inside the container

| Requirement | Purpose | Status |
| ----------- | ------- | ------ |
| `op-grpc-adapters` binary installed | gRPC service process | Must deploy |
| `netclient` binary at PATH | CLI passthrough (`execute_command`) | Already present |
| `/etc/netclient/` writable | `patch_netclient_config()` writes JSON here | Already present |
| `sv` (runit) managing `netclient` | `restart_netclient()` calls `sv restart netclient` | **OPEN — verify** |
| Process supervision for `op-grpc-adapters` itself | Keep the adapter running | Must configure |

### Runit dependency (open question)

`NetmakerAdapter::execute_command(Restart)` runs:
```rust
Command::new(op_core::runit::SV_BIN)  // "sv"
    .args(["restart", "netclient"])
```

This requires:
1. `sv` binary present in the container's PATH
2. A runit service directory for `netclient` (e.g., `/etc/service/netclient/run`)
3. `runsvdir` (or equivalent) running as the container's init/supervisor

**Verification task**: Check whether the `NetMaker` container image has runit
available. If not, two options:
- **Option A**: Install minimal runit inside the container. Create
  `/etc/service/netclient/run` → `#!/bin/sh\nexec netclient daemon`. Use
  runit's own `runsvdir` as the container init (or a shim that starts both
  `runsvdir` and `op-grpc-adapters`).
- **Option B**: Modify the `Restart` arm in `execute_command` to use
  `pkill -x netclient && sleep 1 && netclient daemon &` instead of `sv restart`.
  Minimal code change, but less clean process lifecycle management.

Option A is preferred (matches the host's own runit convention and gives
proper supervision of both processes inside the container).

### `op-grpc-bridge` client configuration

`GrpcClientPool::netmaker_client()` connects to a configured endpoint.
Once the proxy device exists, configure this endpoint to
`http://127.0.0.1:<host-port>` (the proxy device's listen address).

**Invariants:**

- The proxy device MUST NOT interfere with the existing `api-lo` and
  `broker-mesh` proxy devices.
- The proxy device uses loopback-only TCP — it never traverses the OVS
  `netmk` port or any network namespace boundary other than the Incus
  proxy mechanism itself.
- If `op-grpc-adapters` is not running inside the container, the gRPC
  channel returns connection-refused; `op-grpc-bridge` handles this as
  a standard gRPC unavailable error (no crash, no retry storm).
- The adapter binary's own process supervision is independent of
  netclient's supervision — both need long-lived process management
  inside the container.

---

## 4 · OCI Plugin Schema Contract

### NetMaker entry (Phase 1 addition)

```json
{
  "name": "NetMaker",
  "loopback_required": true,
  "port_attach": {
    "bridge": "ovsbr0",
    "iface_name": "netmk",
    "ip_addrs": ["10.200.1.1/30"],
    "gateway": "10.200.1.2",
    "routes": []
  },
  "socket_proxies": [
    { "host": "unix:/run/netmaker/api.sock", "container": "tcp:127.0.0.1:8081" },
    { "host": "unix:/run/netmaker/broker.sock", "container": "tcp:127.0.0.1:8083" }
  ],
  "grpc_adapter": {
    "binary": "op-grpc-adapters",
    "proxy_device": "grpc-adapter",
    "host_endpoint": "http://127.0.0.1:<port>"
  },
  "supervision": "native_grpc_adapter"
}
```

**Invariants:**

- `port_attach` and the existing Incus `proxy` devices are independent —
  they MUST NOT interfere. The proxy devices use loopback-only TCP paths
  that never traverse the OVS port.
- The reconciliation loop MUST handle the case where the container
  restarts and the port must be re-attached (see §1 invariants).
- `supervision: "native_grpc_adapter"` instructs the netmaker plugin to use
  `op-grpc-bridge`'s `netmaker_client()` methods (routed through the proxy
  device to the in-container `op-grpc-adapters` process) for netclient
  lifecycle management — no `incus exec` or `ServiceController::IncusExec`
  needed.

---

## 5 · Phase Gate Contract

**Phase 1 → Phase 2 gate conditions (ALL must be true):**

1. `netclient` has maintained continuous mesh connectivity for ≥ 48 hours
   (no handshake timeouts reported by `wg show` inside NetMaker).
2. No OVS port flaps detected in `ovs-vsctl show` or OVSDB monitor events.
3. All Phase 1 validation tasks (T-13 through T-18) pass on re-run.
4. Explicit written approval from infrastructure owner.
5. Maintenance window scheduled and communicated.

**Phase 2 rollback trigger conditions (ANY triggers rollback):**

1. `curl --max-time 5` to xray's IP fails after cutover.
2. External probe to `api.3tched.com` fails within 30 s of cutover.
3. `incus exec xray -- ip link show xray0` reports link DOWN or missing.
4. Any unhandled error in the cutover script.

**Rollback time budget:** Target 10 seconds from trigger detection to traffic
restoration — this is a goal to validate, not a guaranteed SLA. The actual
budget will be established by T-23's non-production rollback test; the real
measured value replaces this placeholder once available. The rollback script
MUST be pre-loaded in memory (not fetched from disk) during execution.

---

## 6 · Observability

### Phase 1 metrics to emit

- `netmk_port_attached{container="NetMaker"}` — gauge, 1 when port is in
  container netns with correct IP, 0 otherwise.
- `netclient_active{container="NetMaker"}` — gauge, 1 when `pgrep` succeeds.
- `netclient_peers{container="NetMaker"}` — gauge, count of peers with
  recent handshake (< 180 s) from `wg show`.

### Phase 2 events to log

- `xray_cutover_started` — timestamp
- `xray_cutover_completed` / `xray_cutover_rolled_back` — timestamp + reason
- `xray_health_check_result` — pass/fail + latency
