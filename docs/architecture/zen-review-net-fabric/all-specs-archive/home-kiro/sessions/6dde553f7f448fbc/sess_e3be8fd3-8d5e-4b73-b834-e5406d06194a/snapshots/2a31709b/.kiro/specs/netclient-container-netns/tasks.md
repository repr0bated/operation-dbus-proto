# Netclient Container Netns — Implementation Tasks

This task list is reconciled to source and live state as of 2026-08-03.

- `[x]` means independently verified in code and/or live state.
- `[ ]` means required work remains.
- A completed **interim-state** task records what exists; it does not imply the
  interim design satisfies Phase 1.
- Phase 1 is **not complete** until real TLS, Netmaker licensing/control-plane,
  and Netclient peer traffic pass after restart.

Phase 2 remains separately gated and must never be triggered automatically.

---

## A · Verified Implementation and Live-State Baseline

- [x] **A-1 — Native Netmaker OVS/netns attachment exists.**
  - `OciPlugin` uses `OvsdbDbusClient` to create/list/verify OVS ports and force
    `type=internal`.
  - It resolves fresh Incus PIDs, moves links with native rtnetlink, configures
    namespace addresses/link/default route, and independently reads state back.
  - `rovs_commands::AddPortInput` preserves explicit `interface_type` and
    defaults it to `internal`.
  - This verifies the native backend, not capability mediation: the current
    helper instantiates `OciPlugin` directly. Authenticated projected dispatch
    remains open in D-5.
- [x] **A-2 — Read back live OVS membership/type.**
  - `netmk` and `grpc0` are members of `ovsbr0` with `type=internal`.
  - `netmk` is present in the Netmaker network namespace; `grpc0` is present in
    the xray network namespace.
- [x] **A-3 — Read back fixed namespace addressing.**
  - Netmaker: `netmk=10.200.1.1/30`, UP, default
    `via 10.200.1.2 dev netmk`.
  - Xray: `grpc0=10.200.0.1/24` plus `10.200.1.2/30`, UP, default
    `via 10.200.0.2 dev grpc0`.
  - Existing xray `10.200.0.1/24` is preserved.
- [x] **A-4 — Read back host return/forwarding state.**
  - Host route is `10.200.1.0/30 via 10.200.0.1 dev svc0`.
  - Host and xray IPv4 forwarding are enabled.
  - Xray's own Internet egress works after restoration of its default route.
- [x] **A-5 — Verify identity-scoped capability grants exist.**
  - One operator identity hash has rtnetlink move/link-state/address/
    default-route/route-add and OVSDB port-add/list capabilities.
  - No network mutation capability is present in the wildcard grant.
  - This completes grant inventory only; proving the attachment path is
    actually mediated by those grants remains open in D-5.
- [x] **A-6 — Record interim netfilter state without claiming completion.**
  - Host `OP_NETMK_BYPASS_FWD`/`OP_NETMK_BYPASS_NAT` have zero built-in
    references/counters and are not on the carrying path.
  - Xray `FORWARD` has two active references to `OP_NETMK_BYPASS_FWD`; its
    counters are non-zero and the jumps name stale input `grpc` rather than
    live `grpc0`.
  - Xray also has an active feature-subnet MASQUERADE on `grpc0`.
- [x] **A-7 — Verify the actual live upstream.**
  - `wgcf-egress` is present, UP, MTU 1280, and table `51820` contains its
    default route.
  - Marked route lookup selects table `51820`/`wgcf-egress`.
  - The earlier “wgcf absent, use `pub0`” premise is stale for this checkpoint.
- [x] **A-8 — Refute the `rp_filter` hypothesis.**
  - Host `all`, `default`, `svc0`, and `pub0` values are all `0`.
  - Xray `all.rp_filter` is `0`.
  - No `rp_filter` change is required for the observed failure.
- [x] **A-9 — Explain zero host bypass counters.**
  - Xray SNAT changes source `10.200.1.1` to `10.200.0.1` before host ingress.
  - Host bypass rules matching `10.200.1.1` cannot match that packet identity
    and have no built-in references.
  - Xray bypass counters are non-zero because its two jumps precede SNAT.
- [x] **A-10 — Prove basic transit and isolate the transport failure.**
  - Netmaker-netns ICMP replies, DNS lookup, TCP connect, and HTTP response
    succeeded.
  - Xray and host wgcf MASQUERADE counters incremented.
  - TLS connected and sent ClientHello but timed out.
- [x] **A-11 — Confirm evidence supporting the PMTU/MSS hypothesis.**
  - `netmk` and `grpc0` MTU are 1500; `wgcf-egress` MTU is 1280.
  - No host/xray TCPMSS rule exists.
  - `ss -tin` showed `pmtu:1500`, `advmss:1448`, retransmission/loss, and curl
    timeout. Causality remains open until B-14's one-variable A/B test.
- [x] **A-12 — Verify current supervisors.**
  - Netmaker and xray run systemd as PID 1 and expose system bus sockets.
  - Installed unit files include `netmaker.service`, `netclient.service`, and
    `xray.service`; proposed adapter/network-ready/xray-policy units are absent.
  - Host remains runit; proposed `netmk-*` host services/readiness are absent.

---

## Implementation Preflight — LSP Required

- [ ] **LSP-1 — Start an attached rust-analyzer session before Rust edits.**
  - Installed binary/version:
    `/home/jeremy/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rust-analyzer`
    (`rust-analyzer 1.97.1`).
  - Current Kiro diagnostics fail because `rust-analyzer` is not on its PATH.
  - Launch the implementation client with the toolchain bin on PATH or set its
    LSP command to `rustup run stable rust-analyzer`.
  - Require successful workspace initialization and diagnostics for
    `rtnetlink.rs`, `oci.rs`, `op-netmk-reconcile.rs`, and
    `adapters/netmaker.rs`; keep the client attached throughout implementation.
  - Do not launch an orphan analyzer process as a substitute for LSP health.

---

## B · P0 — Complete Real Netmaker Internet Egress

These tasks form one ordered cutover. Do not enable a catch-all deny or remove
the interim rollback path until all positive probes are ready.

### B.1 Pre-cutover evidence and rollback

- [ ] **B-1 — Capture an exact rollback bundle.**
  - OVSDB ports/types/ofports for `netmk`, `grpc0`, `svc0`, and `pub0`.
  - Netmaker/xray addresses, MTUs, routes, PIDs, and PID-1 supervisors.
  - Host/xray iptables-save output, including the two active xray bypass jumps,
    their interface matches/counters, the exact bypass chain, and the exact
    feature-subnet MASQUERADE; host rules/routes and feature conntrack tuples.
  - `wgcf-egress` identity, MTU, config hash, mark, table routes, handshake,
    and owning-service status.
  - Netmaker API/broker/UI/metrics/license status, xray probes, adapter health,
    and current Netclient/WireGuard state.
- [ ] **B-2 — Define a bounded rollback command sequence.**
  - Restore both captured xray `OP_NETMK_BYPASS_FWD` jumps and the exact
    feature-subnet MASQUERADE.
  - Remove only newly created feature jumps/rules/chains.
  - Restore prior `netmk` MTU if necessary.
  - Re-run Netmaker, xray, and Netclient health probes.
  - Do not flush broad built-in chains or all conntrack state.

### B.2 Native MTU remediation

- [ ] **B-3 — Add native namespace-aware MTU mutation.**
  - Freeze
    `set_link_mtu(SetLinkMtuInput { name: String, mtu: u32, netns_pid: Option<u32> }) -> RtnetlinkMutationOutput`.
  - Use capability `cap.network.rtnetlink.mtu.set@v1` and mutation subid
    `mut.network.rtnetlink.mtu.set@v1`.
  - Reject zero/invalid MTU, stale/nonexistent PID, missing interface, and
    read-back mismatch.
  - Add schema, projected runtime dispatch, native implementation, read-back,
    and disposable-netns coverage; never shell out to `ip` from Rust.
  - Close existing `spec.md` §6 conformance gaps: typed `LinkState` and
    `AddRouteInput.device` with an explicit compatibility alias/deprecation.
- [ ] **B-4 — Make attachment MTU desired state.**
  - Add optional `mtu` to `PortAttachConfig`/OCI schema.
  - Reconcile MTU before link-UP/default-route completion.
  - Verify it in `namespace_attachment_ready`.
  - Set `op-netmk-reconcile` Netmaker attachment MTU to the selected path MTU,
    currently 1280.
  - Do not lower shared xray `grpc0` globally.
- [ ] **B-5 — Add least-privilege MTU authority.**
  - Add exactly `cap.network.rtnetlink.mtu.set@v1` to the existing operator
    identity footprint; verify the wildcard grant is unchanged.
  - Locate/update the durable grant source that regenerates SHM; none was found
    by capability-ID search in the current workspace.
  - Do not hand-edit `/dev/shm/opdbus/capability-grants.json`.
- [ ] **B-6 — Add optional scoped TCPMSS defense in depth.**
  - If retained after MTU validation, match exactly
    `-i svc0 -o wgcf-egress -s 10.200.1.1/32` and SYN packets.
  - Use `--clamp-mss-to-pmtu` or a value derived from the selected egress MTU.
  - Verify the rule cannot affect unrelated xray traffic.

### B.3 Functional policy manifest

- [ ] **B-7 — Capture the required Netmaker egress feature matrix.**
  - DNS resolvers/protocols and HTTPS/licensing destination policy.
  - Discover peer endpoint UDP destination ports separately from local
    WireGuard listen ports; never treat a listen port as outbound `tp_dst`.
  - Model inbound listen-port initiation separately only if required.
  - Capture any additional flow required by the installed EE/Pro feature set.
  - Record purpose, direction, and positive probe for every allowance.
- [ ] **B-8 — Implement one normalized policy manifest.**
  - Freeze `PolicyClass { id, direction, protocol, src_cidrs, dst_cidrs,
    src_ports, dst_ports, purpose, positive_probe }`.
  - Validate unique IDs, IPv4 CIDRs, ports 1–65535, protocol/port
    compatibility, nonempty DNS destinations, and approved HTTPS policy.
  - Generate the directional OpenFlow/xray/host projections from this source.
  - Include ARP, DNS, HTTPS/licensing, endpoint UDP, and approved baseline
    additions; do not break Netmaker functionality with UDP-only catch-all.
- [ ] **B-9 — Add projection equivalence verification.**
  - Normalize and compare each layer's appropriate directional projection, not
    raw equality of unlike rule syntaxes.
  - Prove every manifest class is present at all applicable layers and fail
    readiness on missing/extra feature-owned rules.

### B.4 Source-preserving host/xray convergence

- [ ] **B-10 — Preinstall and verify host source-preserving policy.**
  - Route: `10.200.1.0/30 via 10.200.0.1 dev svc0`.
  - Mangle: mark every `-i svc0 -s 10.200.1.1/32` packet `0x51821` before
    route lookup.
  - Policy: priority `10518` lookup table `51820`; priority `10519` source
    blackhole after it.
  - Filter/NAT: generate approved classes from the manifest and NAT only on
    `wgcf-egress`.
  - Verify no feature route/NAT sends inner traffic to `pub0`.
- [ ] **B-11 — Reconcile xray source-preserving forwarding.**
  - `ip_forward=1`, `grpc0.send_redirects=0`.
  - Build `OP_NETMK_XRAY_FWD` from the manifest and prepare exactly two
    interface-correct `grpc0` jumps.
  - Readiness requires exactly two jumps to `OP_NETMK_XRAY_FWD` and zero jumps
    to `OP_NETMK_BYPASS_FWD`.
  - No final NAT for `10.200.1.0/30`; preserve xray application state and
    `/etc/xray/xray_config.json`.
- [ ] **B-12 — Atomically replace xray bypass/SNAT state.**
  - Confirm B-3 through B-11 are verified first.
  - Replace both active bypass jumps with the two prepared target jumps, then
    remove only the identified Netmaker MASQUERADE.
  - Start fresh probe connections; do not infer success from old conntrack.
  - Require host feature counters to observe source `10.200.1.1`.
- [ ] **B-13 — Retire unreferenced bypass chain objects after stability.**
  - After the rollback window, flush/delete only host/xray
    `OP_NETMK_BYPASS_FWD`/`OP_NETMK_BYPASS_NAT` objects with zero references.
  - Preserve rollback evidence; do not touch unrelated chains.

### B.5 Verifiable egress acceptance

- [ ] **B-14 — Prove or refute PMTU causality with one-variable A/B.**
  - Against one fixed TLS endpoint, record a fresh MTU-1500 failure and socket
    telemetry.
  - Change only `netmk` to MTU 1280 through the new native method; repeat the
    same request.
  - Require completed TLS, `ss -tin` `pmtu <= 1280`, IPv4 `advmss <= 1240`,
    and no retransmission loop.
  - If it still fails, run B-16 tracing before assigning another cause.
  - Exercise UDP separately with a named Netclient peer.
- [ ] **B-15 — Run the positive application matrix.**
  - DNS lookup, bounded HTTP, and completed HTTPS response.
  - Netmaker API, broker, UI, metrics, and EE/Pro licensing health.
  - Adapter health/list/join/restart.
  - Record test start and one named peer's RX/TX counters, generate a bounded
    peer probe, require handshake newer than start/no older than 180 seconds,
    and require RX/TX growth.
  - Existing xray domains/proxy health.
- [ ] **B-16 — Prove the packet path.**
  - Capture or use equivalent native tracing at `netmk`, `grpc0`, `svc0`,
    `wgcf-egress`, and `pub0`.
  - Show source `10.200.1.1` through xray/`svc0`, selected egress on wgcf, and
    only tunnel underlay—not inner Netmaker traffic—on `pub0`.
  - Show expected feature-chain and policy-rule counters increment.
  - Packet-capture tooling was absent during diagnosis; make it available by
    the approved host image/deployment path rather than an untracked install.
- [ ] **B-17 — Run negative/fail-closed probes.**
  - Unapproved protocol/port/destination classes are denied.
  - A missed mark or unusable selected lookup reaches the source blackhole,
    not the main table.
  - Test failure behavior in a disposable namespace or maintenance-approved
    window; do not deliberately break live wgcf service/table state.
- [ ] **B-18 — Prove restart recovery.**
  - Restart containers through the approved D-Bus control plane.
  - Re-resolve PIDs and reattach/reverify native OVS/rtnetlink state.
  - Restart host feature services with `sudo sv restart ...`.
  - Require the full positive matrix and no restart storm/stale readiness.

---

## C · Container Systemd and Host Runit Race Removal

Container systemd work is allowed only inside the existing containers. Host
services remain runit.

- [ ] **C-0 — Add durable Phase 1 xray-attachment reconciliation.**
  - Resolve fresh xray PID after every restart.
  - Ensure `grpc0` is the OVS `internal` port in that namespace.
  - Preserve/reconcile `10.200.0.1/24` and `10.200.1.2/30`.
  - Replace and read back exact `default via 10.200.0.2 dev grpc0`; do not
    treat an arbitrary `EEXIST` route as success.
  - Complete before `op-netmk-xray-policy.service`; this is not Phase 2.
- [ ] **C-1 — Define Netmaker container-local units.**
  - Add `op-netmk-network-ready.service` to verify `netmk`, address, MTU,
    gateway reachability, and default route with a bounded wait.
  - Add `op-grpc-adapters.service`, ordered after network-ready and runtime
    mount.
  - Make existing `netclient.service` `Requires=`/`After=` both units.
  - Configure bounded start/restart limits and health failure behavior.
- [ ] **C-2 — Define xray container-local policy readiness.**
  - Add `op-netmk-xray-policy.service`, ordered after C-0 has restored
    `grpc0`, both addresses, and default route.
  - Apply/verify only xray namespace sysctls and feature-owned filter state.
  - Do not modify/reload xray application JSON or use another live config path.
- [ ] **C-3 — Package units through the deployment artifact.**
  - Place unit definitions/helpers in the immutable container image/subvolume
    workflow.
  - Do not hand-copy binaries or units onto the running host/container as the
    deployment mechanism.
- [ ] **C-4 — Implement container lifecycle over D-Bus.**
  - Resolve each fresh container system-bus socket.
  - Use `busctl` calls to `org.freedesktop.systemd1.Manager` and read unit
    status back; never invoke `systemctl`.
  - Replace adapter `sv restart netclient` in restart/join/leave with
    `StartUnit("netclient.service", "replace")` on the Netmaker system bus.
  - Change OCI supervision metadata/example from `runit` to `systemd` for this
    deployment.
- [ ] **C-5 — Install the host runit graph.**
  - `xray-attachment`: perform C-0 before xray local policy.
  - `netmk-egress-policy`: external-upstream checks, host route/rule/chains,
    then `busctl` start/verification of `op-netmk-xray-policy.service`.
  - `netmk-port-attach`: authenticated native OVS/rtnetlink attachment with MTU.
  - `netmk-of-restrict`: manifest-derived flows and authoritative live query.
  - `netmaker-adapter-loopback`: UDS to `127.0.0.1:50061`.
  - `netmk-netclient-start`: wait for host stamps, resolve fresh PID, then
    `busctl` StartUnit for `netclient.service` and verify it.
  - Enable definitions through `/etc/runit/runsvdir/default`; manage with
    `sudo sv ...`, never edit `/run/runit/service`.
- [ ] **C-6 — Make readiness generation-safe.**
  - Include container PID, OVS ofport, manifest hash, selected upstream/MTU,
    and catalog generation in readiness evidence.
  - Remove stale stamps before work and invalidate downstream stamps on change.
  - Use bounded waits; no open-ended poll/restart loops.

---

## D · Adapter, OpenFlow, and OCI Completion

- [ ] **D-1 — Complete adapter deployment/readiness.**
  - Supervise `op-grpc-adapters` inside Netmaker with systemd.
  - Bind the runtime directory with least privilege.
  - Install host runit loopback on `127.0.0.1:50061`.
  - Verify health through the loopback before readiness.
- [ ] **D-2 — Verify endpoint separation and Ghostbridge metadata live.**
  - All `netmaker_*`/`netclient_*` calls use `NETMAKER_ADAPTER_ADDR`.
  - Unrelated methods retain `default_address`.
  - Every Netmaker request includes required metadata.
- [ ] **D-3 — Complete authoritative OpenFlow read-back.**
  - Implement/verify OF1.3 `OFPMP_FLOW` multipart request/reply handling on the
    active switch connection.
  - Expose parsed live switch flows over D-Bus.
  - Compare only the feature cookie namespace against the manifest.
  - Use kernel OVS datapath dumps after probes only as corroboration.
- [ ] **D-4 — Reconcile after OVS/container recreation.**
  - Re-resolve ofport and container PID.
  - Reinstall only feature-cookie flows.
  - Verify unrelated OVS flows/ports are unchanged.
- [ ] **D-5 — Put attachment mutations behind authenticated capability gates.**
  - Call projected `rovs_commands.add_port/list_ports` and rtnetlink methods
    under the named operator footprint; native OVSDB/rtnetlink remain backend.
  - In a disposable target, prove explicit `interface_type="internal"` reaches
    OVSDB and every exact capability emits audit/event evidence.
  - Prove wildcard grants lack those capabilities and identify the durable
    grant source used to regenerate SHM.
  - Remove the disposable port through the authorized native path.
  - If direct-root reconciliation is retained instead, stop and obtain an
    explicit sandboxed service exception before claiming FR-5 compliance.

---

## E · Non-blocking Plugin Catalog Work

These tasks do not block B/C design work but must complete before declaring the
runtime schema current.

- [x] **E-1 — Verify `netmaker.rs` schema/dispatch compiles.**
  - Ran `CXXFLAGS="-include cstdint" cargo check -p op-plugins` successfully;
    warnings only.
- [ ] **E-2 — Reseal affected plugin blobs into the SHM catalog.**
  - Record current manifest baseline: generation `20299`, catalog hash
    `1f5c40566f23fee1ccfacd5cc2e60a1fb4c8b7c94b641b25abff3a99c783225a`
    (re-read immediately before execution in case it changes).
  - Run the canonical sole writer: `/usr/local/bin/opblob seal-shm` after all
    Netmaker/rtnetlink/OCI schema changes compile.
  - Do not create/edit `/dev/shm/opdbus/plugin-blobs` by hand.
- [ ] **E-3 — Verify catalog publication.**
  - Read the SHM manifest and prove generation/catalog hash changed.
  - Read back affected method schemas through the bridge/catalog consumer.
  - Smoke-call corrected Netmaker methods and the new MTU surface using the
    scoped operator identity.

---

## F · Separate Golden Tree / Netmaker Supervisor Migration

This work is tracked but does not block the egress repair.

- [x] **F-1 — Recheck and clear the old missing-golden blocker.**
  - `/opt/op-dbus/golden/MANIFEST` exists: build `20260802T170227Z`, commit
    `8afd632f`, matching the current repository commit at inspection time.
  - All five paths required by `migrate-netmaker-to-runit.sh` are present,
    including container runit/sv/init, `op-grpc-adapters`, and the host runit
    definition.
  - Migration is no longer blocked by tree absence; it remains blocked by F-2,
    baseline/approval, and any required rebuild for newer implementation.
- [ ] **F-2 — Make the container-supervisor decision explicit.**
  - Current Netmaker PID 1 is systemd and this design uses it for race-free
    container-local ordering.
  - Decide whether the runit-PID1 migration is still desired after Phase 1.
  - If retained, map every systemd dependency/readiness guarantee to runit and
    schedule it separately; never run both supervisors for the same process.
- [ ] **F-3 — Refresh/review golden artifacts if migration remains approved.**
  - After implementation, build once:
    `CXXFLAGS="-include cstdint" cargo build --workspace --release`.
  - Review: `sudo deploy/runit/build-golden.sh --dry-run`.
  - Publish through btrfs golden/live workflow only if the manifest does not
    already represent the intended binaries: `sudo deploy/runit/build-golden.sh`.
  - Verify the new manifest commit/hashes; do not hand-copy binaries.
- [ ] **F-4 — Run the migration only after prerequisites and approval.**
  - Provide the captured `NETMAKER_BASELINE_DIR`.
  - Confirm all expected golden paths exist.
  - Execute through its D-Bus-based Incus lifecycle path in a maintenance
    window with rollback snapshot available.
  - Re-run the entire Phase 1 application and network matrix afterward.

---

## G · Phase Gate and Phase 2 (Deferred)

- [ ] **G-1 — Observe 48 continuous hours after Phase 1 acceptance.**
  - Successful Netmaker licensing/control-plane health.
  - Named-peer handshake newer than each test start/no older than 180 seconds,
    increasing peer RX/TX counters, and healthy wgcf handshakes.
  - Stable OVS ports/ofports, container units, host runit stages, and catalog
    generation.
  - No PMTU retransmission pattern, namespace leak, or direct inner flow on
    `pub0`.
- [ ] **G-2 — Obtain infrastructure-owner approval and maintenance window.**
- [ ] **G-3 — Prove xray NIC cutover and rollback on non-production state.**
- [ ] **G-4 — Execute Phase 2 `xray0` migration only after G-1 through G-3.**
  - Capture fresh xray NIC/MAC/MTU/addresses/routes/rules.
  - Create/move/configure `xray0` through native OVSDB/rtnetlink.
  - Preserve `/etc/xray/xray_config.json`, xray ingress, Phase 1 egress, and
    host wgcf boundary.
  - Roll back on any proxy, licensing, Netclient, route, or timing failure.

No Phase 1 service or completion hook may invoke Phase 2.

---

## Definition of Done

- [ ] Native attachment includes verified path-compatible MTU.
- [ ] Xray forwards without feature-subnet NAT; host sees/counts
  `10.200.1.1`.
- [ ] DNS, HTTP, completed HTTPS, Netmaker EE/Pro licensing/control-plane, and
  Netclient handshake/peer traffic all pass.
- [ ] OpenFlow, xray, and host rules derive from one approved policy manifest.
- [ ] Inner Netmaker traffic uses the selected upstream and never silently
  falls through to `pub0`.
- [ ] Host runit and container systemd readiness recover correctly after
  restart using fresh PIDs; lifecycle operations follow `sv`/`busctl` policy.
- [ ] Scoped grants and sealed plugin catalog reflect all new methods.
- [ ] Existing xray, Netmaker, wgcf, `svc0`, `pub0`, and host networking remain
  healthy.
- [ ] No task or workaround disables/downgrades Netmaker features or licensing.
- [ ] Phase 2 remains separately gated.
