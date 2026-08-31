# Netclient Container Netns — Implementation Tasks

Phase 1 tasks are ordered. Phase 2 is separately gated and must never be
triggered automatically by Phase 1 completion.

---

## Phase 1 — Netmaker Attachment and Forced Egress

### A. Preflight and immutable baseline

- [ ] **T-0** Capture live state without mutation:
  - lowercase Incus names and current init PIDs for netmaker and xray
  - xray OVS-facing interface, host OVS port, `10.200.0.1`, MAC, MTU, routes
  - host `svc0=10.200.0.2` and current route/rule tables
  - current netclient listen port and every peer endpoint destination port
  - existing OVSDB ports and OF1.3 baseline
- [ ] **T-0.1** Verify external `wgcf-egress` prerequisite on the host:
  interface UP, recent handshake, underlay FwMark `0x51820`, and usable table
  `51820`. Record results only; priority 10518 is created later as feature
  integration state.
- [ ] **T-0.2** Assert the feature will not write
  `/etc/wireguard/wgcf-egress.conf`, operate the interface/service, change mark
  `0x51820`, or mutate table `51820`.
- [ ] **T-0.3** Save rollback inventories for host/xray netfilter rules, host
  policy rules, routes, OVSDB port state, and OpenFlow table state.

### B. Native OVS and rtnetlink prerequisites

- [ ] **T-1** Fix `rovs_commands.add_port` runtime dispatch to honor
  `interface_type`; add a test proving `internal` reaches native OVSDB.
- [ ] **T-1.1** Add typed `MoveLinkInput { iface_name, netns_pid }` and native
  `IFLA_NET_NS_PID` implementation.
- [ ] **T-1.2** Add optional `netns_pid` to link-state, IPv4-address, and
  default-route operations; wire schema, dispatch, implementation, and typed
  outputs.
- [ ] **T-1.3** Add typed host `AddRouteInput` sufficient for
  `10.200.1.0/30 via 10.200.0.1 dev svc0`.
- [ ] **T-1.4** Implement target-netns operations on a dedicated OS thread:
  enter namespace, create socket there, perform and verify operations,
  restore/terminate, then return to async code.
- [ ] **T-1.5** Register OSCAL subids and add message unit tests plus
  disposable-netns privileged integration tests. Never use production links in
  tests.

### C. Xray gateway integration

- [ ] **T-2** Resolve fresh xray PID and its current OVS-facing interface.
- [ ] **T-2.1** Through namespace-aware rtnetlink, add
  `10.200.1.2/30` while preserving existing addresses, routes, MAC, MTU, and
  link state.
- [ ] **T-2.2** In runit service `netmk-egress-policy`, persist and verify:
  `net.ipv4.ip_forward=1` and
  `net.ipv4.conf.<inside>.send_redirects=0`.
- [ ] **T-2.3** Create/reconcile xray filter chain
  `OP_NETMK_XRAY_FWD`: approved UDP endpoint ports outbound on the same inside
  interface, established/related return, then source drop. Add exactly two
  built-in FORWARD jumps—one matching source `10.200.1.1/32`, one matching
  destination `10.200.1.1/32`. Do not add xray NAT.

### D. Host mark, return route, and fail-closed policy

- [ ] **T-3** Add/verify host route
  `10.200.1.0/30 via 10.200.0.1 dev svc0` through rtnetlink.
- [ ] **T-3.1** Create/reconcile mangle chain `OP_NETMK_MARK` and one
  PREROUTING jump matching `-i svc0 -s 10.200.1.1/32`. Mark every matching
  packet `0x51821/0xffffffff` before route lookup; do not scope the mark to UDP
  or endpoint ports.
- [ ] **T-3.2** Add/verify feature-owned priority `10518` exactly as
  `fwmark 0x51821/0xffffffff lookup 51820`. Do not add, delete, flush, or
  replace any route in table `51820`.
- [ ] **T-3.3** Add feature-owned priority `10519`:
  `from 10.200.1.1/32 blackhole`.
- [ ] **T-3.4** Create/reconcile filter chain `OP_NETMK_FWD`: require mark
  `0x51821`, source, `svc0 → wgcf-egress`, UDP, and each approved endpoint
  port; allow established/related return; drop remaining source traffic.
- [ ] **T-3.5** Create/reconcile NAT chain `OP_NETMK_NAT`: jump only for
  marked, approved UDP from `10.200.1.1/32` with `-o wgcf-egress`, then
  MASQUERADE. No `pub0` NAT rule is permitted.
- [ ] **T-3.6** Implement deterministic teardown: remove owned jumps, flush and
  delete only `OP_NETMK_*` chains, delete only feature-owned priorities, and
  remove only the feature host route. Do not touch external wgcf state.

### E. Netmaker OVS attachment

- [ ] **T-4** Create `netmk` on `ovsbr0` through native OVSDB with
  `interface_type="internal"`; independently verify type and membership.
- [ ] **T-4.1** Resolve fresh netmaker PID and move `netmk` with rtnetlink.
- [ ] **T-4.2** In that namespace assign `10.200.1.1/30`, bring the link UP,
  and install `default via 10.200.1.2 dev netmk`.
- [ ] **T-4.3** Verify namespace-local state and ARP resolution for
  `10.200.1.2`. Do not start netclient yet.

### F. OpenFlow restriction and authoritative query

- [ ] **T-5** Reserve cookie prefix
  `0x4e4d4b0000000000/0xffffff0000000000` and add cookie-scoped normalize,
  compare, replace, and delete behavior.
- [ ] **T-5.1** Add OF1.3 `OFPMP_FLOW` request/reply encoding and parsing to
  `op-network::controller` for the active switch connection.
- [ ] **T-5.2** Expose D-Bus `DumpSwitchFlows` with table, priority, cookie,
  matches, actions, packet count, and byte count. Keep the old in-memory method
  clearly labeled non-authoritative.
- [ ] **T-5.3** Resolve current `netmk` ofport and install cookie-tagged ARP
  allow, one UDP destination-port allow per live peer, and catch-all drop.
  Preserve other ports' baseline. Do not add a source-port-only allow because
  it would permit arbitrary destinations.
- [ ] **T-5.4** Verify exact desired flows through `DumpSwitchFlows`.
- [ ] **T-5.5** After probes, call
  `OvsNetlinkClient::dump_flows("ovsbr0")` for secondary datapath/counter
  evidence; do not treat kernel megaflows as logical table truth.
- [ ] **T-5.6** Re-resolve ofport and reconcile after OVS restart or port
  recreation.

### G. Adapter deployment and dedicated endpoint

- [ ] **T-6** Package `op-grpc-adapters` into the netmaker image/deployment and
  add its runit service with
  `ADAPTERS_SOCKET=/var/lib/opdbus-runtime/netmaker/op-grpc-adapters.sock`.
- [ ] **T-6.1** Bind-mount the runtime directory to the host with permissions
  limited to the service identities that need it.
- [ ] **T-6.2** Add `deploy/runit/netmaker-adapter-loopback/run`, mirroring
  `qdrant-grpc-loopback`: wait for the UDS, then exec socat from
  `127.0.0.1:50061` to the socket. Port `50061` is fixed; `50054` is occupied.
- [ ] **T-6.3** Extend `RemoteOperationClient` with `netmaker_address` and parse
  `NETMAKER_ADAPTER_ADDR`, default `http://127.0.0.1:50061`. Preserve
  `default_address` for every non-Netmaker client.
- [ ] **T-6.4** Change every `netmaker_*` and `netclient_*` method to use
  `netmaker_address` and attach Ghostbridge metadata to every request.
- [ ] **T-6.5** Add unit tests proving endpoint separation and metadata, plus
  an integration test for health/list/join/leave/restart through the socat
  endpoint.

### H. OCI reconciliation

- [ ] **T-7** Normalize `port_attach` to `port_name` and set `netmk`,
  `10.200.1.1/30`, gateway `10.200.1.2`, socket path, and host endpoint
  `http://127.0.0.1:50061`.
- [ ] **T-7.1** Implement actual `calculate_diff`, `apply_state`, and
  `verify_state` for create/move/address/route reconciliation.
- [ ] **T-7.2** Reconcile after a controlled netmaker restart using the fresh
  PID; verify existing API and broker devices are byte-for-byte unchanged.

### I. Runit graph

- [ ] **T-8** Add `deploy/runit/netmk-egress-policy/run`. It verifies external
  wgcf read-only, reconciles T-2/T-3, then writes
  `/run/opdbus/runit-ready/netmk-egress-policy`.
- [ ] **T-8.1** Add `deploy/runit/netmk-port-attach/run`; wait for egress
  policy, reconcile T-4, write `netmk-port-attach` readiness.
- [ ] **T-8.2** Add `deploy/runit/netmk-of-restrict/run`; wait for attachment,
  reconcile and live-query T-5, write `netmk-of-restrict` readiness.
- [ ] **T-8.3** Add readiness to `netmaker-adapter-loopback` only after a TCP
  connection reaches adapter health; write `netmaker-adapter-loopback`.
- [ ] **T-8.4** Add `deploy/runit/netmk-netclient-start/run`; wait for both
  OpenFlow and adapter readiness, invoke adapter restart/start, verify process
  health, then write `netmk-netclient-start`.
- [ ] **T-8.5** Every stage removes its stale stamp before work, uses bounded
  waits, exits non-zero for runit retry, and invalidates downstream stamps when
  its owned state changes.

### J. Phase 1 validation

- [ ] **T-9** Verify attachment: OVSDB internal type, current namespace PID,
  addresses, link state, default route, xray gateway ARP.
- [ ] **T-9.1** Verify xray: secondary address, forwarding, redirect
  suppression, exact chain/jump/rules, and no NAT for the feature subnet.
- [ ] **T-9.2** Verify host: return route, all source packets marked, priority
  rules, dedicated chains, approved-port restriction, and wgcf-only NAT.
- [ ] **T-9.3** Verify immutable wgcf boundary by comparing pre/post config
  hash, interface identity, underlay mark, table routes, and owning service
  state. Expected result: no feature-caused change.
- [ ] **T-9.4** Positive probe: approved peer UDP crosses xray, is marked,
  increments host rule/NAT counters, appears as inner traffic on
  `wgcf-egress`, and produces only existing underlay traffic on `pub0`.
- [ ] **T-9.5** Negative probes: ICMP, TCP, and unapproved UDP from `netmk` are
  denied; no direct host-main-table or `pub0` path is observed.
- [ ] **T-9.6** Controlled fail-closed test in a disposable namespace that
  mirrors priorities 10518/10519 and an unavailable table lookup: prove the
  source is blackholed. Do not perturb live `wgcf-egress`, table `51820`, or
  its owning service.
- [ ] **T-9.7** Verify live OF multipart state and kernel datapath evidence.
- [ ] **T-9.8** Invoke join through `NETMAKER_ADAPTER_ADDR`; require a recent
  `wg show` peer handshake.
- [ ] **T-9.9** Re-test Netmaker API/broker, all current xray domains, host
  `3tched` peers/routes, and `pub0`/`svc0` state.

---

## Phase Gate

- [ ] **G-1** Observe 48 continuous hours of recent netclient and wgcf
  handshakes, stable OVS ports/ofports, no namespace leaks, no direct inner
  peer flow on `pub0`, and healthy existing services.
- [ ] **G-2** Obtain infrastructure-owner approval and schedule a maintenance
  window.
- [ ] **G-3** Prove Phase 2 cutover and rollback on a non-production container.

No Phase 1 service or completion hook may invoke Phase 2.

---

## Phase 2 — Replace Xray Veth with `xray0`

- [ ] **T-10** Capture fresh xray PID, Incus NIC definition, host veth mapping,
  MAC, MTU, addresses including `10.200.0.1` and `10.200.1.2/30`, routes,
  sysctls, and interface-scoped `OP_NETMK_XRAY_FWD` jump.
- [ ] **T-10.1** Create idle OVS internal port `xray0` through native OVSDB.
- [ ] **T-10.2** Implement idempotent cutover: move `xray0`, restore captured
  state, retarget only xray interface-scoped rules, verify both traffic classes,
  then remove the veth-backed Incus NIC.
- [ ] **T-10.3** Implement rollback: restore captured Incus NIC and rule
  interface, verify proxy plus netclient traffic, then remove `xray0`.
- [ ] **T-10.4** Execute only in the approved window; trigger rollback on any
  xray domain failure, netclient handshake failure, route mismatch, or
  interruption exceeding the approved threshold.
- [ ] **T-10.5** On success, update OCI desired state to `xray0` and remove
  stale veth references from active scripts/docs.
- [ ] **T-10.6** Verify old veth absence, `xray0 type=internal`, exact captured
  state, healthy proxy/netclient flows, and unchanged host wgcf config/runtime.

---

## Definition of Done

- All Phase 1 tests and the 48-hour gate pass.
- No placeholders or unresolved implementation choices remain for Phase 1.
- All mutations have independent read-back verification.
- Rollback artifacts exist for every owned route, rule, chain, port, and
  interface change.
- The repository contains no feature code that configures or controls
  `wgcf-egress`.
- Phase 2 remains separately approved and manually initiated.
