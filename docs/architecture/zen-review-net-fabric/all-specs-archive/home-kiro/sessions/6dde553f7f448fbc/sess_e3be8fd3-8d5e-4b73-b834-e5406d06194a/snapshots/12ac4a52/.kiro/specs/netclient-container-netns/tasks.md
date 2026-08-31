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
- [x] **A-5 — Verify identity-scoped capability grants.**
  - One operator identity hash has rtnetlink move/link-state/address/
    default-route/route-add and OVSDB port-add/list capabilities.
  - No network mutation capability is present in the wildcard grant.
- [x] **A-6 — Record interim netfilter state without claiming completion.**
  - Host `OP_NETMK_BYPASS_FWD`/`OP_NETMK_BYPASS_NAT` and xray
    `OP_NETMK_BYPASS_FWD` objects exist.
  - Xray also has an ad hoc feature-subnet MASQUERADE on `grpc0`.
  - The host bypass chains are currently orphaned from built-in chains and are
    not the path carrying observed traffic.
- [x] **A-7 — Verify the actual live upstream.**
  - `wgcf-egress` is present, UP, MTU 1280, and table `51820` contains its
    default route.
  - Marked route lookup selects table `51820`/`wgcf-egress`.
  - The earlier “wgcf absent, use `pub0`” premise is stale for this checkpoint.
- [x] **A-8 — Refute the `rp_filter` hypothesis.**
  - Host `all`, `default`, `svc0`, and `pub0` values are all `0`.
  - Xray `all.rp_filter` is `0`.
  - No `rp_filter` change is required for the observed failure.
- [x] **A-9 — Explain zero bypass counters.**
  - Xray SNAT changes source `10.200.1.1` to `10.200.0.1` before host ingress.
  - Host bypass rules matching `10.200.1.1` cannot match that packet identity.
  - The chains also lacked live built-in jumps at inspection time.
- [x] **A-10 — Prove basic transit and isolate the transport failure.**
  - Netmaker-netns ICMP replies, DNS lookup, TCP connect, and HTTP response
    succeeded.
  - Xray and host wgcf MASQUERADE counters incremented.
  - TLS connected and sent ClientHello but timed out.
- [x] **A-11 — Confirm PMTU/MSS evidence.**
  - `netmk` and `grpc0` MTU are 1500; `wgcf-egress` MTU is 1280.
  - No host/xray TCPMSS rule exists.
  - `ss -tin` showed `pmtu:1500`, `advmss:1448`, retransmission/loss, and curl
    timeout. PMTU/MSS is the leading confirmed remaining transport defect.
- [x] **A-12 — Verify current supervisors.**
  - Netmaker and xray run systemd as PID 1 and expose system bus sockets.
  - Host remains runit.
  - The proposed `netmk-*` host runit services are not installed and no ready
    stamps exist.

---

## B · P0 — Complete Real Netmaker Internet Egress

These tasks form one ordered cutover. Do not enable a catch-all deny or remove
the interim rollback path until all positive probes are ready.

### B.1 Pre-cutover evidence and rollback

- [ ] **B-1 — Capture an exact rollback bundle.**
  - OVSDB ports/types/ofports for `netmk`, `grpc0`, `svc0`, and `pub0`.
  - Netmaker/xray addresses, MTUs, routes, PIDs, and PID-1 supervisors.
  - Host/xray iptables-save output, host rules/routes, and conntrack tuples for
    the feature source.
  - `wgcf-egress` identity, MTU, config hash, mark, table routes, handshake,
    and owning-service status.
  - Netmaker API/broker/UI/metrics/license status, xray probes, adapter health,
    and current Netclient/WireGuard state.
- [ ] **B-2 — Define a bounded rollback command sequence.**
  - Restore only the captured xray feature-subnet MASQUERADE.
  - Remove only newly created feature jumps/rules/chains.
  - Restore prior `netmk` MTU if necessary.
  - Re-run Netmaker, xray, and Netclient health probes.
  - Do not flush broad built-in chains or all conntrack state.

### B.2 Native MTU remediation

- [ ] **B-3 — Add native namespace-aware MTU mutation.**
  - Implement typed rtnetlink input/output for setting link MTU in a target
    namespace.
  - Add schema, runtime dispatch, native implementation, read-back
    verification, OSCAL IDs, and targeted disposable-netns coverage.
  - Do not shell out to `ip` from Rust.
- [ ] **B-4 — Make attachment MTU desired state.**
  - Add optional `mtu` to `PortAttachConfig`/OCI schema.
  - Reconcile MTU before link-UP/default-route completion.
  - Verify it in `namespace_attachment_ready`.
  - Set `op-netmk-reconcile` Netmaker attachment MTU to the selected path MTU,
    currently 1280.
  - Do not lower shared xray `grpc0` globally.
- [ ] **B-5 — Add least-privilege MTU authority.**
  - Add the new rtnetlink MTU capability only to the existing operator identity
    footprint.
  - Verify the wildcard grant is unchanged.
  - Persist through the authoritative grant source; do not hand-edit SHM.
- [ ] **B-6 — Add optional scoped TCPMSS defense in depth.**
  - If retained after MTU validation, match exactly
    `-i svc0 -o wgcf-egress -s 10.200.1.1/32` and SYN packets.
  - Use `--clamp-mss-to-pmtu` or a value derived from the selected egress MTU.
  - Verify the rule cannot affect unrelated xray traffic.

### B.3 Functional policy manifest

- [ ] **B-7 — Capture the required Netmaker egress feature matrix.**
  - DNS resolvers/protocols.
  - HTTPS/licensing/control-plane flows.
  - Live Netclient listen and peer endpoint UDP ports.
  - Any additional network flow required by the installed EE/Pro feature set.
  - Record purpose and positive probe for every allowance.
- [ ] **B-8 — Implement one normalized policy manifest.**
  - Generate OpenFlow, xray filter, and host filter/NAT rules from the same
    source.
  - Include ARP, required DNS, HTTPS/licensing, Netclient UDP, and approved
    baseline additions.
  - Do not ship the old WireGuard-UDP-only catch-all policy when it breaks
    Netmaker functionality.
- [ ] **B-9 — Add policy equivalence verification.**
  - Normalize each enforcement layer and prove every manifest class is present
    at all applicable layers.
  - Fail readiness on missing or extra feature-owned rules.

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
  - Exact `OP_NETMK_XRAY_FWD` jumps and manifest-derived rules.
  - No final NAT for `10.200.1.0/30`.
  - Preserve xray's existing address, route, application service, and
    `/etc/xray/xray_config.json`.
- [ ] **B-12 — Atomically remove the ad hoc xray MASQUERADE.**
  - Confirm B-3 through B-11 are verified first.
  - Remove only the rule commented/identified for Netmaker egress.
  - Start fresh probe connections; do not infer success from old conntrack.
  - Require host feature counters to observe source `10.200.1.1`.
- [ ] **B-13 — Retire orphan bypass state after stability.**
  - Remove any built-in bypass jumps if later found.
  - Flush/delete only `OP_NETMK_BYPASS_FWD`/`OP_NETMK_BYPASS_NAT` after the
    source-preserving path and rollback window are accepted.
  - Preserve rollback evidence; do not touch unrelated chains.

### B.5 Verifiable egress acceptance

- [ ] **B-14 — Verify PMTU behavior after remediation.**
  - Read back `netmk` MTU 1280 (or a measured lower selected-path value).
  - Complete a bounded HTTPS request from the Netmaker netns.
  - During a fresh connection, require `ss -tin` `pmtu <= 1280`, IPv4
    `advmss <= 1240`, and no retransmission loop.
  - Exercise UDP with actual Netclient packets; a TCP-only result is not done.
- [ ] **B-15 — Run the positive application matrix.**
  - DNS lookup.
  - Bounded HTTP and completed HTTPS response.
  - Netmaker API, broker, UI, metrics, and EE/Pro licensing health.
  - Adapter health/list/join/restart.
  - Recent `wg show` peer handshake and live peer packets.
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

- [ ] **C-1 — Define Netmaker container-local units.**
  - A bounded network-ready oneshot verifies `netmk`, UP state,
    `10.200.1.1/30`, desired MTU, gateway reachability, and default route.
  - `op-grpc-adapters` starts after network-ready and its runtime mount.
  - Netclient starts after network, adapter, and host-policy readiness.
  - Configure bounded start/restart limits and health failure behavior.
- [ ] **C-2 — Define xray container-local policy readiness.**
  - Wait for `grpc0` and both `10.200.0.1/24` and `10.200.1.2/30`.
  - Apply/verify only xray namespace sysctls and feature-owned filter state.
  - Do not modify/reload xray application JSON or use another live config path.
- [ ] **C-3 — Package units through the deployment artifact.**
  - Place unit definitions/helpers in the immutable container image/subvolume
    workflow.
  - Do not hand-copy binaries or units onto the running host/container as the
    deployment mechanism.
- [ ] **C-4 — Implement container lifecycle over D-Bus.**
  - Resolve each current container system-bus socket.
  - Use `busctl` calls to `org.freedesktop.systemd1.Manager` for start/restart
    and read unit status back over D-Bus.
  - Do not invoke `systemctl`.
- [ ] **C-5 — Install the host runit graph.**
  - `netmk-egress-policy`: external-upstream checks, host route/rule/chains,
    xray local-policy D-Bus readiness.
  - `netmk-port-attach`: native OVS/rtnetlink attachment including MTU.
  - `netmk-of-restrict`: manifest-derived flows and authoritative live query.
  - `netmaker-adapter-loopback`: UDS to `127.0.0.1:50061`.
  - readiness aggregator/Netclient gate coordinated with container systemd.
  - Enable definitions through `/etc/runit/runsvdir/default`; manage with
    `sudo sv ...`, never `/run/runit/service` edits.
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
- [ ] **D-5 — Complete direct `rovs_commands` dispatch smoke checks.**
  - Call `add_port`/`list_ports` through the projected method surface in a
    disposable target and prove explicit `interface_type="internal"` reaches
    OVSDB.
  - Remove the disposable port through the authorized native path.

---

## E · Non-blocking Plugin Catalog Work

These tasks do not block B/C design work but must complete before declaring the
runtime schema current.

- [x] **E-1 — Verify `netmaker.rs` schema/dispatch compiles.**
  - Ran `CXXFLAGS="-include cstdint" cargo check -p op-plugins` successfully;
    warnings only.
- [ ] **E-2 — Reseal affected plugin blobs into the SHM catalog.**
  - Use the canonical `op-blob` sealer (the sole catalog writer) for Netmaker
    and every plugin whose schema changes for MTU/OCI work.
  - The `op-blob` CLI was not installed in the current PATH at inspection time;
    build/install it through the normal artifact workflow first.
  - Do not create/edit `/dev/shm/opdbus/plugin-blobs` by hand.
- [ ] **E-3 — Verify catalog publication.**
  - Read the SHM manifest and prove generation/catalog hash changed.
  - Read back affected method schemas through the bridge/catalog consumer.
  - Smoke-call corrected Netmaker methods and the new MTU surface using the
    scoped operator identity.

---

## F · Separate Golden Tree / Netmaker Supervisor Migration

This work is tracked but does not block the egress repair.

- [x] **F-1 — Confirm current blocker.**
  - `/opt/op-dbus/golden` required artifacts are absent.
  - `deploy/runit/build-golden.sh` has not produced the golden tree.
  - `deploy/runit/migrate-netmaker-to-runit.sh` must not run yet.
- [ ] **F-2 — Make the container-supervisor decision explicit.**
  - Current Netmaker PID 1 is systemd and this design uses it for race-free
    container-local ordering.
  - Decide whether the runit-PID1 migration is still desired after Phase 1.
  - If retained, map every systemd dependency/readiness guarantee to runit and
    schedule it as a separate migration; never run both supervisors for the
    same process.
- [ ] **F-3 — Build/review the golden artifacts if migration remains approved.**
  - Build once:
    `CXXFLAGS="-include cstdint" cargo build --workspace --release`.
  - Review:
    `sudo deploy/runit/build-golden.sh --dry-run`.
  - Publish through the prescribed btrfs golden/live workflow:
    `sudo deploy/runit/build-golden.sh`.
  - Do not hand-copy binaries as deployment.
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
  - Recent Netclient and wgcf handshakes.
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
