# Netclient Container Netns — Design

## 1 · Design Status

Phase 1 is partially live. Native OVS/rtnetlink attachment and basic L3 transit
exist, but the live policy differs from `spec.md` and transport is not yet
production-ready. The design below separates:

1. **observed interim state**, which explains current behavior and counters;
2. **target Phase 1 state**, which restores source attribution, fixes PMTU,
   preserves all required Netmaker functions, and survives restarts.

The live-state checkpoint and evidence in this document are from 2026-08-03.

---

## 2 · Observed Interim Architecture

```text
netmaker container (systemd PID 1)
  netmk 10.200.1.1/30, MTU 1500
  default via 10.200.1.2
      │
      ▼
──────────────────────── ovsbr0 ────────────────────────
      │
      ▼
xray container (systemd PID 1)
  grpc0 10.200.0.1/24 + 10.200.1.2/30, MTU 1500
  default via 10.200.0.2
  ip_forward=1
  ad hoc POSTROUTING MASQUERADE: 10.200.1.1 → 10.200.0.1
      │                    host never sees source 10.200.1.1
      ▼
──────────────────────── ovsbr0 ────────────────────────
      │
      ▼
host svc0 10.200.0.2/24
  existing xray rule matches source 10.200.0.1
  OP_NETMK_MARK → mark 0x51821
  priority 100/10518 → table 51820
      │
      ▼
wgcf-egress, MTU 1280, UP
  table 51820 default
      │
      ▼
Internet
```

Additional observed facts:

- `netmk` and `grpc0` are OVS `internal` interfaces on `ovsbr0` and are live in
  their respective container network namespaces.
- The host route `10.200.1.0/30 via 10.200.0.1 dev svc0` is present.
- Host `OP_NETMK_BYPASS_FWD` and `OP_NETMK_BYPASS_NAT` exist with zero
  references/counters and are not on the carrying path.
- Xray `FORWARD` has two active jumps to `OP_NETMK_BYPASS_FWD`; that chain has
  non-zero counters and currently accepts source `10.200.1.1` plus established
  return traffic. The jumps still name stale input `grpc` while the live OVS
  interface is `grpc0`.
- Xray's active `netmaker-egress-to-wgcf` MASQUERADE then translates the source
  before host ingress.
- The proposed host runit services and readiness stamps are not installed.
  Current behavior is ad hoc state, not durable reconciliation.
- `wgcf-egress` exists now. The older assumption that transit had to use direct
  `svc0 → pub0` bypass is stale for this checkpoint.

### 2.1 What the zero counters actually mean

The zero **host** bypass-chain counters are explained by two concrete
rule-topology facts:

1. the host bypass chains are not referenced by built-in chains; and
2. xray changes the source before the packet enters host `svc0`.

Xray's bypass chain is different: it is actively referenced twice and has
non-zero counters. During convergence, both xray bypass jumps must be replaced
atomically by exactly two jumps to `OP_NETMK_XRAY_FWD`; readiness requires zero
remaining references to `OP_NETMK_BYPASS_FWD`.

They do **not** prove a drop before `FORWARD` or `POSTROUTING`. Live probes from
the Netmaker network namespace reached the Internet and incremented the xray
and host wgcf MASQUERADE counters.

### 2.2 Reverse-path filtering is not causal

The leading `rp_filter` hypothesis was directly refuted:

```text
net.ipv4.conf.all.rp_filter     = 0
net.ipv4.conf.default.rp_filter = 0
net.ipv4.conf.svc0.rp_filter    = 0
net.ipv4.conf.pub0.rp_filter    = 0
xray net.ipv4.conf.all.rp_filter = 0
```

No `rp_filter` mutation is part of the fix. If future hardening enables reverse
path filtering, any exception must be justified and scoped to the minimum
interfaces; global disabling is not a remediation task because it is already
disabled.

### 2.3 Leading remaining hypothesis: PMTU/MSS

Probe progression establishes where the path fails:

| Probe | Result |
| --- | --- |
| ICMP to `1.1.1.1` | Replies received; NAT counters incremented |
| DNS for public hostname | Succeeded |
| TCP connect | Succeeded |
| HTTP request | Response headers/body received |
| TLS request | Connected, sent ClientHello, then timed out |

The selected links and TCP telemetry are:

```text
netmk MTU:        1500
grpc0 MTU:        1500
wgcf-egress MTU:  1280
TCP pmtu:          1500
TCP advmss:        1448
TCP result:        repeated loss/retransmission, curl timeout
TCPMSS rule:       absent on host and xray
```

This evidence strongly indicates a path-MTU black hole, but causality is not
final until a one-variable A/B test changes only `netmk` MTU from 1500 to 1280
and repeats the same fixed-endpoint TLS probe. If TLS still fails, packet
tracing precedes any further diagnosis. In all cases, the network is not
considered working until TLS and real Netmaker traffic succeed; a successful
ping is insufficient.

---

## 3 · Target Phase 1 Architecture

```text
netmaker container (systemd-supervised local services)
  netmk 10.200.1.1/30, MTU 1280
  default via 10.200.1.2
  DNS + HTTPS/licensing + approved Netclient UDP
      │ source remains 10.200.1.1
      ▼
──────────────────────── ovsbr0 ────────────────────────
      │
      ▼
xray container
  grpc0 10.200.0.1/24 + 10.200.1.2/30
  ip_forward=1, send_redirects=0
  OP_NETMK_XRAY_FWD generated from approved egress manifest
  NO feature-subnet NAT
      │ source remains 10.200.1.1
      ▼
──────────────────────── ovsbr0 ────────────────────────
      │
      ▼
host svc0 10.200.0.2/24
  mangle PREROUTING: 10.200.1.1 → mark 0x51821
  policy: mark 0x51821 → table 51820
  terminal source blackhole after selected lookup
  filter/NAT generated from same approved manifest
  optional scoped TCPMSS defense-in-depth
      │
      ▼
wgcf-egress, current selected upstream, MTU 1280
      │
      ▼
Internet
```

The target deliberately removes xray SNAT. Source preservation provides:

- exact host attribution and counters for the Netmaker feature;
- host policy that cannot accidentally include all xray traffic;
- compatibility with the fixed source/mark/blackhole contract in `spec.md`;
- one NAT boundary, on the selected host egress only.

`pub0` is not an implicit fallback. If `wgcf-egress` is unavailable, the Phase
1 service reports an external prerequisite failure and remains unready. A
future public-egress mode must explicitly own its route, NAT, policy, and
negative tests; the current bypass chains are not promoted into that role.

---

## 4 · Native Attachment and MTU Reconciliation

### 4.1 Verified attachment implementation

`OciPlugin` already:

1. uses `OvsdbDbusClient` to create/list/verify `type=internal` ports;
2. resolves a fresh Incus PID before and after each sensitive operation;
3. moves the port with native rtnetlink;
4. enters the target namespace on the native namespace worker;
5. adds addresses, brings links UP, installs routes, and reads state back;
6. aborts if the container PID changes during reconciliation.

`op-netmk-reconcile attachment reconcile|verify|teardown` wraps this native
behavior for `netmk`. This proves the backend, but the current helper directly
instantiates `OciPlugin`; it does not prove that projected capability grants
mediate the mutation. Final host orchestration must call authenticated
`rovs_commands` and rtnetlink method surfaces under the operator identity (or
obtain an explicit sandboxed direct-service exception) and record audit events
for each capability.

`op-netmk-reconcile egress-network` discovers xray's OVS-facing interface by
the preserved `10.200.0.1` address, so it does not depend on stale `eth0`; the
current interface is `grpc0`. It does not currently create/move `grpc0` or
reconcile xray's default route. Add a durable `xray-attachment` stage that,
after every xray restart, uses the fresh PID to ensure `grpc0 type=internal`,
preserve/reconcile both addresses, and replace/read back
`default via 10.200.0.2 dev grpc0` before local policy starts. This restores the
current attachment and is not the deferred `xray0` migration.

### 4.2 MTU as desired state

The primary PMTU correction is to make the Netmaker attachment MTU no larger
than the selected egress MTU. For the current path:

```text
netmk desired MTU = min(OVS/internal default, wgcf-egress MTU) = 1280
```

Implementation work:

1. freeze
   `set_link_mtu(SetLinkMtuInput { name: String, mtu: u32, netns_pid: Option<u32> }) -> RtnetlinkMutationOutput`;
2. use capability `cap.network.rtnetlink.mtu.set@v1` and mutation subid
   `mut.network.rtnetlink.mtu.set@v1`;
3. reject zero/invalid MTU, stale/nonexistent PID, missing interface, and
   read-back mismatch;
4. add optional `mtu` to `PortAttachConfig` and the OCI schema;
5. apply MTU before bringing `netmk` UP/installing its default route and verify
   it in `namespace_attachment_ready`;
6. add projected-dispatch and disposable-netns coverage for this method and
   close the existing `spec.md` §6 contract gaps (`LinkState` enum and
   `AddRouteInput.device`, with a compatibility alias if needed);
7. grant the exact MTU capability only to the operator identity and reseal/read
   back affected plugin blobs.

Do not lower `grpc0` globally: that interface also carries existing xray
traffic. Endpoint MTU on `netmk` scopes the behavior to Netmaker.

As defense in depth, the host may install a feature-owned mangle/FORWARD
TCPMSS rule matching exactly:

```text
-i svc0 -o wgcf-egress -s 10.200.1.1/32
-p tcp --tcp-flags SYN,RST SYN -j TCPMSS --clamp-mss-to-pmtu
```

The clamp is secondary. Correct `netmk` MTU is also needed for UDP and for
applications that do not use TCP.

---

## 5 · Unified Egress Policy

The old design independently generated a UDP-only OpenFlow policy and
netfilter rules. That can break Netmaker licensing/control-plane traffic and
violates the no-downgrade requirement. Replace it with one policy model used by
all enforcement layers.

### 5.1 Policy model

Each approved class contains protocol, destination constraints where known,
port set, purpose, and validation probe. Minimum classes are:

| Class | Minimum allowance | Purpose |
| --- | --- | --- |
| Gateway discovery | ARP on `netmk` | Reach `10.200.1.2` |
| DNS | UDP/TCP 53 to configured resolver set | Name resolution |
| Netmaker control/licensing | TCP 443, with tighter destinations when a durable source of truth exists | Preserve EE/Pro and control-plane behavior |
| Netclient peers | Discovered WireGuard UDP endpoint/listen ports | Join and handshakes |
| Baseline additions | Explicitly captured and operator-approved | Preserve installed Netmaker feature set |

OpenFlow, xray filter, and host filter/NAT derive from the same normalized
manifest. A catch-all source drop is installed only after all positive classes
have passed their probes.

### 5.2 Xray-owned state

Xray owns only its namespace-local integration state:

- `net.ipv4.ip_forward=1`;
- `net.ipv4.conf.grpc0.send_redirects=0`;
- `OP_NETMK_XRAY_FWD` and exact built-in jumps;
- no NAT for `10.200.1.0/30` after cutover.

The ad hoc `netmaker-egress-to-wgcf` MASQUERADE is captured for rollback and
then removed. No xray application-routing or JSON configuration changes are
needed; `/etc/xray/xray_config.json` remains the only live config path.

### 5.3 Host-owned state

The host owns:

- `10.200.1.0/30 via 10.200.0.1 dev svc0`;
- `OP_NETMK_MARK`, `OP_NETMK_FWD`, `OP_NETMK_NAT`, and an optional scoped MSS
  chain/rule;
- priority `10518` mark lookup and priority `10519` source blackhole;
- feature readiness and rollback inventory.

Marking remains protocol-independent and occurs before route lookup. Filtering
and NAT then apply the policy manifest. Table `51820`, mark `0x51820`,
`wgcf-egress`, and its service remain read-only external state.

### 5.4 OpenFlow

The cookie namespace and live OF1.3 query design in `spec.md` remain valid, but
the allowed matches must come from the expanded policy manifest rather than a
WireGuard-UDP-only list. Switch state remains authoritative; controller memory
alone is not sufficient verification.

---

## 6 · Split Supervision and Race Removal

Both current containers run systemd as PID 1 and expose their system bus.
Using those supervisors for container-local processes removes races without
changing host PID 1 or violating host runit policy.

### 6.1 Host runit responsibilities

Host runit services, managed only with `sudo sv ...`, retain ownership of:

- OVSDB port reconciliation and namespace attachment;
- host route/rule/netfilter reconciliation;
- host adapter UDS-to-loopback bridge;
- cross-namespace verification and readiness aggregation.

The host never invokes `systemctl`.

### 6.2 Netmaker systemd responsibilities

Install container-local units in the immutable container deployment for:

- a bounded network-ready oneshot that verifies `netmk`, address, MTU, gateway,
  and default route;
- `op-grpc-adapters`, ordered after network-ready and the runtime mount;
- Netclient, ordered after network-ready, adapter readiness, and host policy
  readiness as exposed through an agreed read-only signal.

Use restart limits and health checks. A late OVS attachment or new Incus PID
must cause waiting/retry, not a start-before-link race.

### 6.3 Xray systemd responsibilities

A container-local oneshot can own xray namespace sysctls and
`OP_NETMK_XRAY_FWD`, ordered after `grpc0` exists and has both required
addresses. It must not modify or reload xray application configuration.

### 6.4 Lifecycle control

Host orchestration resolves the current container PID/system-bus socket and
calls `org.freedesktop.systemd1.Manager` with `busctl`. Unit status is read back
from D-Bus. No lifecycle step uses `systemctl`; no host service is converted to
systemd.

```text
host runit: attach netmk + host policy
       │
       ├── verify Netmaker network state ──► container systemd starts adapter/netclient
       │
       └── verify xray grpc0 state ────────► container systemd applies local policy

both container readiness results ─────────► host Phase 1 ready stamp
```

The existing `migrate-netmaker-to-runit.sh` remains separate and blocked on the
missing golden tree. Because systemd is the current working container
supervisor, that migration must not run until an explicit supervisor decision
and equivalent unit-to-runit ordering plan exist.

---

## 7 · Capability and Catalog Design

The live grant proves the correct model: network capabilities are attached to
one identity hash, while the wildcard entry contains unrelated read/chat
capabilities only. Preserve that shape.

The MTU mutation and any new service-control capability require:

1. typed schema and dispatch;
2. exact OSCAL mutation IDs;
3. operator-identity grant only;
4. `cargo check -p op-plugins` and targeted tests;
5. blob reseal by the canonical `op-blob` writer;
6. SHM manifest generation/catalog-hash verification;
7. read-back through the consuming bridge/catalog path.

Do not hand-edit `/dev/shm/opdbus/capability-grants.json` or plugin blobs.

---

## 8 · Atomic Convergence and Rollback

The source-preserving cutover is ordered to avoid an egress outage:

1. Capture xray/host rules, routes, policy rules, MTUs, OVSDB state, active
   connections, Netmaker feature health, xray health, and wgcf state.
2. Reconcile `netmk` MTU 1280 and verify fresh TCP sockets advertise an
   appropriate MSS.
3. Install and verify host source-preserving mark/filter/NAT rules plus the
   return route while xray SNAT still provides the old path.
4. Install and verify xray source-preserving forwarding rules generated from
   the shared manifest.
5. Remove only the ad hoc xray feature-subnet MASQUERADE. Do not broadly flush
   conntrack; allow old translated connections to drain or remove only exact
   feature tuples if operationally required.
6. Generate fresh DNS, HTTPS, licensing, and Netclient probes. Require host
   feature-chain counters for source `10.200.1.1` and wgcf egress evidence.
7. On any failure, restore the captured xray SNAT rule, remove only newly owned
   jumps/rules, and re-run health checks.
8. After stability, delete orphan `OP_NETMK_BYPASS_*` chains and their rollback
   artifacts if they are no longer selected by an approved mode.

No step restarts OVS, wgcf, uplink, DHCP, or the host session bus
automatically.

---

## 9 · Verification Model

Mutation success is never proof by itself. Completion requires these
independent observations:

- **OVSDB:** `netmk`/`grpc0` membership and `type=internal`.
- **Namespace rtnetlink:** addresses, MTUs, link state, defaults, and xray
  address preservation.
- **Host routing:** source/mark route decisions and terminal blackhole order.
- **Netfilter:** exact jumps/rules plus positive counters using the packet
  source visible at that hop.
- **Transport:** DNS, HTTP, completed HTTPS, `ss -tin` PMTU/MSS/retransmission
  state, and actual Netclient UDP.
- **Application:** Netmaker API/broker/UI/metrics/license state and xray proxy
  probes.
- **Adapter:** dedicated endpoint, Ghostbridge metadata, health, and controls.
- **WireGuard:** recent handshake and expected endpoint ports.
- **Supervision:** host `sudo sv status ...`; container unit state through
  `busctl` on each container system bus.
- **External boundary:** wgcf config hash, interface identity, underlay mark,
  table routes, and owning-service state unchanged.

A packet capture tool was not installed during diagnosis. Final validation must
include an approved capture or equivalent native trace at `netmk`, `grpc0`,
`svc0`, `wgcf-egress`, and `pub0` to prove source preservation and absence of
inner traffic on `pub0`.

---

## 10 · Failure Modes

| Failure | Required behavior |
| --- | --- |
| `rp_filter` suspected | Read exact values first; do not change them when already disabled. |
| Bypass counters remain zero | Verify jump reachability and packet source after each NAT boundary; do not infer pre-netfilter drop. |
| TLS stalls with small probes working | Check MTU/MSS/socket retransmissions; reconcile `netmk` MTU and scoped MSS defense. |
| Xray SNAT still active after cutover | Readiness fails because host cannot attribute source `10.200.1.1`. |
| `wgcf-egress` absent/unhealthy | Fail closed and report external prerequisite; do not silently use `pub0`. |
| Required Netmaker feature flow denied | Keep catch-all enforcement unready; update only the reviewed policy manifest. Never downgrade licensing/features. |
| Container restarts | Re-resolve PID; systemd units wait for live interfaces/routes; host runit re-verifies. |
| Container unit fails | Read D-Bus unit status, respect start limits, and invalidate downstream readiness. |
| Capability/schema not resealed | Treat the mutation as unavailable even if Rust source compiles. |
| Source-preserving probe fails | Restore exact interim xray SNAT and remove only newly owned state. |
| Phase 2 probe fails | Restore captured NIC and interface-scoped rules immediately. |

---

## 11 · Cross-reference Corrections to `spec.md`

| `spec.md` area | Current interpretation |
| --- | --- |
| §2 expected xray `eth0` | Interface discovery is normative; live name is `grpc0`. |
| §4 “No NAT occurs in xray” | Correct target invariant; currently violated by the interim MASQUERADE and therefore an open convergence task. |
| §4 UDP-only examples | Incomplete for real Netmaker functionality; rules must be generated from the expanded manifest in `requirements.md` FR-4. |
| §4 source `10.200.1.1` host chains | Correct target; zero counters today are explained by xray SNAT/orphan jumps. |
| §9 runit graph | Host stages remain runit; container-local process ordering uses the containers' existing systemd via D-Bus. |
| §10 Phase 2 | Remains separately gated and unchanged by this Phase 1 remediation. |

---

## 12 · Phase 2

Phase 2 remains deferred. After Phase 1 has stable application-level egress
and the approval gate passes, `xray0` may replace the current xray attachment
using captured MAC, MTU, addresses, routes, and rollback state. Phase 2 must
preserve `/etc/xray/xray_config.json`, all xray ingress, the Phase 1 source and
MTU policy, and the selected external egress boundary.
