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
- Xray `FORWARD` has two syntactic jumps to `OP_NETMK_BYPASS_FWD` with
  historical non-zero counters, but they name stale input `grpc` while the live
  interface is `grpc0`. A fresh probe did not change those counters; current
  forwarding relies on the xray `FORWARD` ACCEPT policy plus SNAT.
- Xray's active `netmaker-egress-to-wgcf` MASQUERADE translates the source
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

Xray's bypass chain is different: it has two syntactic references and
historical counters, but its stale `-i grpc` match does not see current
`grpc0` traffic. A chain is considered active only when a timestamped probe
produces a counter delta. During convergence, both stale jumps are replaced
atomically by exactly two jumps to `OP_NETMK_XRAY_FWD`; readiness requires zero
remaining references to `OP_NETMK_BYPASS_FWD`. Rollback restores the complete
captured prior `FORWARD` policy plus SNAT rather than relying on stale jumps.

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
3. require nonzero PID, accessible `/proc/<pid>/ns/net`, an existing link, MTU
   within RTM_GETLINK-advertised min/max when present (otherwise kernel
   validation), and exact read-back;
4. keep Incus-PID freshness in the caller: resolve/compare the current init PID
   immediately before and after the method call;
5. add optional `mtu` to `PortAttachConfig` and the OCI schema;
6. apply MTU before bringing `netmk` UP/installing its default route and verify
   it in `namespace_attachment_ready`;
7. add projected-dispatch and disposable-netns coverage for this method and
   close the existing `spec.md` §6 contract gaps (`LinkState` enum and
   `AddRouteInput.device`, with a compatibility alias if needed);
8. grant the exact MTU capability only to the operator identity and reseal/read
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

Freeze the logical type before implementation:

```text
PolicyClass {
  id,
  direction: Direction::{EgressFromNetmaker, IngressToNetmaker},
  protocol: Protocol::{Arp, Tcp, Udp, Icmp},
  src_cidrs,
  dst_cidrs,
  src_ports,
  dst_ports,
  purpose,
  positive_probe: ProbeId
}
```

Direction is always from Netmaker's viewpoint. Validation requires unique
IDs, registered probe IDs, parsed IPv4 CIDRs, ports 1–65535, empty ports for
ARP/ICMP, explicit resolver destinations for DNS, and an operator-approved
HTTPS destination policy. Minimum classes are:

| Class | Direction/minimum allowance | Purpose |
| --- | --- | --- |
| Gateway discovery | Egress ARP on `netmk` | Reach `10.200.1.2` |
| DNS | Egress UDP/TCP 53 to configured resolver set | Name resolution |
| Netmaker control/licensing | Egress TCP 443 to the approved destination policy | Preserve EE/Pro and control-plane behavior |
| Netclient peer endpoints | Egress UDP to discovered peer endpoint destination ports | Join and handshakes |
| Netclient local listener | Separate ingress class only if peer initiation is required | Do not mis-project a listen port as outbound `tp_dst` |
| Baseline additions | Explicit direction and operator approval | Preserve installed Netmaker feature set |

Current `discover_netclient_ports` merges local listen ports and peer endpoint
ports, while `desired_flows` treats all values as outbound destination ports.
Split them: endpoints are `EgressFromNetmaker.dst_ports`; listeners are
`IngressToNetmaker.dst_ports` only when peer initiation is required.

| Direction | OpenFlow projection | Xray projection | Host projection |
| --- | --- | --- | --- |
| `EgressFromNetmaker` | `in_port=netmk` plus protocol/destination match | source `10.200.1.1`, protocol/destination | source `10.200.1.1`, selected egress, filter/NAT |
| `IngressToNetmaker` | not applicable to `in_port=netmk`; return baseline remains | destination `10.200.1.1`, explicit listener/conntrack rule | destination `10.200.1.1`, explicit listener or established return |
| ARP | `in_port=netmk,arp` | not applicable | not applicable |

Each layer compares its normalized projection, including explicit “not
applicable,” rather than raw equality. The feature POSTROUTING jump must be
inserted before the existing broad `-o wgcf-egress -j MASQUERADE`; positive
acceptance requires its counter delta. A catch-all source drop is installed
only after every positive class passes its probe.

### 5.2 Xray-owned state

Xray owns only its namespace-local integration state:

- `net.ipv4.ip_forward=1`;
- `net.ipv4.conf.grpc0.send_redirects=0`;
- `OP_NETMK_XRAY_FWD` with exactly two built-in jumps;
- zero remaining jumps to `OP_NETMK_BYPASS_FWD` before readiness;
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

The current container installs `netmaker.service` and `netclient.service`.
Package two static, host-started units in the immutable deployment:

- `op-netmk-network-ready.service`, a bounded oneshot verifying `netmk`,
  address, MTU, gateway, and default route;
- `op-grpc-adapters.service`, with `Requires=`/`After=` network-ready and the
  runtime mount.

Do not boot-enable the adapter and reintroduce an attachment race. Exact order:

1. host runit completes attachment/policy;
2. host calls `StartUnit("op-grpc-adapters.service", "replace")` over the
   fresh container system bus; systemd pulls network-ready;
3. host waits for the returned job, `ActiveState=active`, and UDS creation;
4. host starts/verifies `netmaker-adapter-loopback` and writes its stamp;
5. host calls initial `StartUnit("netclient.service", "replace")` and verifies
   active state.

Adapter join/leave/restart calls use
`RestartUnit("netclient.service", "replace")`, wait for the returned job, and
require `ActiveState=active`; `StartUnit` is not a restart. Remove both the
adapter's current `sv restart netclient` and the runit-backed
`NetmakerPlugin::ServiceController` path. OCI supervision metadata changes to
`systemd`. Unit-file installation is followed by systemd `Manager.Reload` over
D-Bus.

### 6.3 Xray systemd responsibilities and restart trigger

Add `op-netmk-xray-policy.service` for xray namespace sysctls and
`OP_NETMK_XRAY_FWD`. A long-running host runit xray-attachment owner performs
initial reconcile and subscribes to Incus lifecycle events. For every new xray
PID it restores `grpc0`, addresses, and default route, calls systemd
`StartUnit` for the policy unit, verifies it, and only then starts
`xray.service`.

Package a dependency/drop-in or disable independent boot enablement so
`xray.service` cannot start before policy. Event-stream reconnect uses bounded
backoff and readiness keyed by PID; it is not a polling loop. Neither unit may
modify/reload xray application configuration.

### 6.4 Lifecycle control

Host orchestration resolves each current container system-bus socket and calls
`org.freedesktop.systemd1.Manager` with `busctl`; it waits for returned jobs and
reads unit state. No lifecycle step uses `systemctl`; no host service is
converted to systemd. This split explicitly supersedes `spec.md` §§7–9 where
they assign container-local supervision to runit; host runit remains normative.

```text
host runit attachment/policy
       │
       ├──► StartUnit(op-grpc-adapters.service)
       │       └── Requires/After op-netmk-network-ready.service
       ├──► wait UDS ─► host adapter-loopback stamp
       └──► StartUnit(netclient.service)

Incus xray lifecycle event
       └──► restore grpc0/default ─► StartUnit(xray policy) ─► StartUnit(xray)

container unit states + host stamps ──────► host Phase 1 ready stamp
```

The golden tree now exists and contains every path required by
`migrate-netmaker-to-runit.sh`. That migration remains separate because
systemd is the current working container supervisor; it must not run until an
explicit supervisor decision, baseline capture, owner approval, maintenance
window, and equivalent unit-to-runit ordering plan exist.

---

## 7 · Capability and Catalog Design

The live grant has the desired least-privilege shape: network capabilities are
attached to one identity hash, while the wildcard entry contains unrelated
read/chat capabilities only. However, the current direct `OciPlugin` helper
path is not proof of mediation. Freeze these projected contracts:

- `ensure_internal_port(EnsureInternalPortInput { bridge_name, port_name }) -> PortStateOutput`, capability `cap.network.ovsdb.port.ensure-internal@v1`, subid `mut.network.ovsdb.port.ensure-internal@v1`; it idempotently creates or corrects type and returns type/ofport;
- existing projected move/address/state plus the exact MTU method;
- `replace_default_route(ReplaceDefaultRouteInput { name, gateway,
  netns_pid }) -> RtnetlinkMutationOutput`, capability
  `cap.network.rtnetlink.default-route.replace@v1`, with exact replace/readback;
- existing `remove_port` with scoped
  `cap.network.ovsdb.port.delete@v1` for teardown.

Every call produces audit/event evidence under the operator identity. Otherwise
an explicit sandboxed direct-service exception is required. The durable grant
source that regenerates SHM must be located; only method declarations were
found in the workspace.

All new methods require typed schema/dispatch, exact OSCAL IDs, projected and
disposable-netns tests, operator-only grants, `cargo check -p op-plugins`,
`/usr/local/bin/opblob seal-shm`, before/after SHM manifest evidence, and
consumer read-back. Do not hand-edit live grant or blob files.

---

## 8 · Atomic Convergence and Rollback

The source-preserving cutover is ordered to avoid an egress outage:

1. Capture xray/host rules, routes, policy rules, MTUs, OVSDB state, active
   connections, Netmaker feature health, xray health, and wgcf state.
2. Reconcile `netmk` MTU 1280 and verify fresh TCP sockets advertise an
   appropriate MSS.
3. Install and verify host source-preserving mark/filter/NAT rules plus the
   return route while xray SNAT still provides the old path.
4. Install and verify xray source-preserving `OP_NETMK_XRAY_FWD` rules from
   the shared manifest while capturing the complete prior `FORWARD` policy,
   including the two stale bypass references.
5. Atomically replace the two xray `OP_NETMK_BYPASS_FWD` references with
   exactly two interface-correct jumps to `OP_NETMK_XRAY_FWD`, then remove only
   the ad hoc feature-subnet MASQUERADE. Readiness requires zero bypass
   references. Do not broadly flush conntrack; allow old translated connections
   to drain or remove only exact feature tuples if operationally required.
6. Generate fresh DNS, HTTPS, licensing, and named-peer Netclient probes.
   Require host feature-chain counters for source `10.200.1.1`, a handshake
   newer than test start/no older than 180 seconds, peer RX/TX growth, and wgcf
   egress evidence.
7. On any failure, restore the complete captured prior xray `FORWARD` policy
   and SNAT rule, remove only newly owned state, and re-run health checks.
8. After stability, delete the now-unreferenced host/xray
   `OP_NETMK_BYPASS_*` chain objects and their rollback artifacts.

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
- **Transport:** one-variable fixed-endpoint MTU A/B, DNS, HTTP, completed
  HTTPS, `ss -tin` PMTU/MSS/retransmission state, and actual Netclient UDP.
- **Application:** Netmaker API/broker/UI/metrics/license state and xray proxy
  probes.
- **Adapter:** dedicated endpoint, Ghostbridge metadata, health, and controls.
- **WireGuard:** frozen
  `PeerProbe { interface, peer_public_key, tunnel_target, timeout_seconds }`;
  same-key handshake newer than start/no older than 180 seconds and same-key
  RX/TX byte growth before timeout. Replace the current any-peer helper.
- **Supervision:** host `sudo sv status ...`; container unit state through
  `busctl` on each container system bus.
- **External boundary:** wgcf config hash, interface identity, underlay mark,
  table routes, and owning-service state unchanged.

A packet capture tool was not installed during diagnosis. Final validation must
include an approved capture or equivalent native trace at `netmk`, `grpc0`,
`svc0`, `wgcf-egress`, and `pub0` to prove source preservation and absence of
inner traffic on `pub0`.

---

## 10 · Implementation LSP Preflight

Rust implementation must not begin without an attached rust-analyzer client.
The binary exists at
`/home/jeremy/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rust-analyzer`
(version 1.97.1), but `rust-analyzer` is not on the current Kiro process PATH,
so the code diagnostics call currently fails. Before B-3 or any Rust edit:

1. launch the implementation client/Kiro session with the rustup toolchain bin
   on PATH, or configure its LSP command as
   `rustup run stable rust-analyzer`;
2. initialize `/srv/git/odbus` successfully;
3. require diagnostics to succeed for `rtnetlink.rs`, `oci.rs`,
   `op-netmk-reconcile.rs`, and the Netmaker adapter;
4. keep that client attached while editing; do not start an orphan analyzer
   process merely to satisfy a process check.

---

## 11 · Failure Modes

| Failure | Required behavior |
| --- | --- |
| `rp_filter` suspected | Read exact values first; do not change them when already disabled. |
| Bypass counters remain zero | Verify jump reachability and packet source after each NAT boundary; do not infer pre-netfilter drop. |
| TLS stalls with small probes working | Treat PMTU as the leading hypothesis; run the fixed-endpoint one-variable MTU A/B. If it does not resolve TLS, trace before assigning cause. |
| Xray SNAT still active after cutover | Readiness fails because host cannot attribute source `10.200.1.1`. |
| `wgcf-egress` absent/unhealthy | Fail closed and report external prerequisite; do not silently use `pub0`. |
| Required Netmaker feature flow denied | Keep catch-all enforcement unready; update only the reviewed policy manifest. Never downgrade licensing/features. |
| Container restarts | Re-resolve PID; systemd units wait for live interfaces/routes; host runit re-verifies. |
| Container unit fails | Read D-Bus unit status, respect start limits, and invalidate downstream readiness. |
| Capability/schema not resealed | Treat the mutation as unavailable even if Rust source compiles. |
| Source-preserving probe fails | Restore the two captured xray bypass jumps and exact interim SNAT; remove only newly owned state. |
| Phase 2 probe fails | Restore captured NIC and interface-scoped rules immediately. |

---

## 12 · Cross-reference Corrections to `spec.md`

| `spec.md` area | Current interpretation |
| --- | --- |
| §2 expected xray `eth0` | Interface discovery is normative; live name is `grpc0`. |
| §4 “No NAT occurs in xray” | Correct target invariant; currently violated by the interim MASQUERADE and therefore an open convergence task. |
| §4 UDP-only examples | Incomplete for real Netmaker functionality; rules must be generated from the expanded manifest in `requirements.md` FR-4. |
| §4 source `10.200.1.1` host chains | Correct target; zero host counters today are explained by xray SNAT plus orphaned host jumps, while xray bypass jumps are active and must be replaced. |
| §9 runit graph | Host stages remain runit; container-local supervision is explicitly superseded by named systemd units controlled through D-Bus. |
| §10 Phase 2 | Remains separately gated and unchanged by this Phase 1 remediation. |

---

## 13 · Phase 2

Phase 2 remains deferred. After Phase 1 has stable application-level egress
and the approval gate passes, `xray0` may replace the current xray attachment
using captured MAC, MTU, addresses, routes, and rollback state. Phase 2 must
preserve `/etc/xray/xray_config.json`, all xray ingress, the Phase 1 source and
MTU policy, and the selected external egress boundary.
