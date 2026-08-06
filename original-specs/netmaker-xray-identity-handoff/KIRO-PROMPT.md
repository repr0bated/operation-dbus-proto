Write a spec for replacing xray's static IP/port ACLs with real WireGuard-identity-based
gating — "the torch pass": WireGuard terminates at netmaker (or wherever the peer's tunnel
actually lands), and that verified peer identity gets handed to xray, which gates purely on
identity rather than source IP/port. This is the zero-trust model CLAUDE.md already claims as
the intended design (`X-Ghostbridge-Footprint`/`X-WireGuard-Pubkey` as "the only real gate";
IP ACLs as "theater") — but investigation this session found that claim is not backed by
working code yet. Cross-reference the actual code against that claim and produce a spec that
closes the gap for real.

Files to review (read these fully, don't skim):
- CLAUDE.md (repo root) — the "Transport & identity (zero-trust)" section is the claimed
  target model.
- crates/op-identity/src/schema_bridge.rs — the `IdentitySled` shm struct (~line 191-224),
  `read_sled()`/`read_sled_at()` (~447-465), footprint derivation `etch_footprint()`
  (~line 1008, Blake3 of wg_pubkey+schema_catalog_hash+mutation_index+source_port),
  `write_sled_from_wg()` (~1062) and `watch_wireguard_handshakes()` (~1132) — this part is
  live and genuinely watches real WireGuard handshake state via `ip monitor route` + `wg show
  <iface> latest-handshakes`. Also read `build_xray_config`/`route_to_outbound`
  (~line 678-950) — these are `#[cfg(test)]`-gated only, never compiled into production, and
  their own doc comment says production uses a static bootstrap config at
  `/etc/xray/xray_config.json` "until a validated control-plane generator owns atomic
  replacement and D-Bus reload." `route_to_outbound` takes `_footprint`/`_trace_id` as unused
  parameters — header-injection was never modeled even in the test-only code.
- crates/op-identity/src/session.rs — `derive_session_id()` (Argon2/Blake3 KDF, ~line 29,63)
  used for the alternate per-identity Cozo verification path.
- crates/op-grpc-bridge/src/interceptor.rs — `load_capability_grants()` (~339-358, reads
  `/dev/shm/opdbus/capability-grants.json`, footprint-keyed, wildcard fallback,
  fails closed), `GhostbridgeInterceptor`/`ghostbridge_interceptor()` (~108-201, requires
  `x-ghostbridge-footprint` + `x-ghostbridge-trace-id`/`x-wireguard-pubkey` headers,
  validates via `verify_per_identity()` (~42-96) or falls back to
  `op_identity::verify_ghostbridge_footprint()`). Note: this validates a *self-presented*
  header against the local sled — nothing here cryptographically ties the header to the
  actual WireGuard peer that terminated the transport connection.
- crates/op-grpc-bridge/src/schema_router.rs — `SchemaBackedInterface::call()` (~710-786),
  the real D-Bus dispatch gate: re-reads the sled per call, computes the footprint hex,
  checks capability grants, rejects with `AccessDenied` before dispatch. This mechanism is
  real, live, and wired end-to-end — it's the *origin* of the footprint (self-asserted vs.
  verified) that's the actual gap, not this gate itself.
- crates/op-cognitive-mcp/src/client_config.rs (~line 239, 568-570) — `ClientConfig
  .wg_pubkey` / `with_wg_pubkey()` — the one place `X-Ghostbridge-Footprint` actually gets
  set on outbound calls today. It's client-self-asserted, not xray/network-verified. This is
  the concrete instance of the "theater" CLAUDE.md warns about, just moved from the IP/port
  layer to the header layer.
- crates/op-xray-daemon/src/dbus.rs, main.rs — confirm this is purely a D-Bus lifecycle
  wrapper (start/stop/reload/status via /proc scanning + SIGTERM/SIGHUP) around an
  externally-managed Xray-core Go binary. It has no header-injection or traffic-inspection
  logic today.
- crates/op-plugins/src/state_plugins/wireguard.rs — `WireGuardPlugin`/`WireGuardPeer`
  (~line 77-99) — plain WG interface/peer CRUD, zero connection today to the identity-sled/
  footprint system above. Confirm whether unifying these two currently-disjoint systems is in
  scope.
- crates/op-network/src/openflow_translate.rs (~189-251) and
  crates/op-plugins/src/state_plugins/openflow.rs — confirm the standard-OF1.3-only match
  field set (`in_port`, `dl_type`, `dl_vlan`, `dl_src`, `tcp_flags`, `tp_src`/`tp_dst`,
  `nw_src`/`nw_dst`, `ct_state`, `tun_id` in actions) has no `reg[N]`/`metadata`/`ct_mark`
  field wired up — meaning per-peer WireGuard-pubkey gating cannot be an OpenFlow/datapath
  match key without new plumbing, and even then only binds to L3/L4 tuples, not application
  identity. The spec should treat this as necessarily an application-layer (gRPC/D-Bus
  capability) gate, not a datapath one, unless it explicitly proposes and justifies new
  datapath work.
- deploy/security/capability-grants.json and /etc/opdbus/capability-grants.json — the durable
  vs. installed vs. SHM-materialized (`/dev/shm/opdbus/capability-grants.json`) grant files;
  note from this session that these three copies silently drifted out of sync after a network
  outage (stale SHM copy missing an operator's granted capabilities until the `opdbus-grants`
  runit service was manually restarted) — the spec should account for materialization
  staleness as a real failure mode, not just correctness of the grants' content.

What "done" must mean (don't accept a self-asserted-header design as complete):
1. A peer's WireGuard identity must be verified at the point it actually terminates (wherever
   that turns out to be architecturally — netmaker, xray, or a dedicated gateway), not
   trusted because a header claims it.
2. xray must gate on that verified identity, replacing today's static IP/port ACL model
   (the `OP_NETMK_*` iptables chains and OVS/OpenFlow rules built during the
   netclient-container-netns work this session are the concrete IP-ACL baseline being
   replaced/superseded — read `.kiro/specs/netclient-container-netns/{design.md,tasks.md}`
   for that baseline).
3. Explicitly address: is the live `IdentitySled`/handshake-watcher genuinely reusable as the
   verification source of truth, or does verified-peer-identity need a different mechanism
   entirely? Justify the choice.
4. Explicitly address the capability-grants materialization staleness failure mode found
   above — a spec that only fixes identity verification but not grant-materialization
   reliability is incomplete.
5. State plainly which existing code is reusable vs. must be built from scratch, matching the
   reusable/from-scratch split found this session (reusable: sled shm layout, handshake
   watcher, Argon2/Blake3 session-id derivation, `GhostbridgeInterceptor`'s
   header-presence/expiry check, `load_capability_grants`'s wiring into
   `SchemaBackedInterface::call`; from-scratch: real xray config generation tying inbound
   traffic to verified peers, cryptographic binding of header-to-transport-peer, and
   integration between `wireguard.rs`'s peer CRUD and the identity-sled system).

Output: design.md, requirements.md, tasks.md following this repo's existing spec conventions
(see `.kiro/specs/netclient-container-netns/` for the format/rigor bar — that spec's
post-outage recovery-gate structure, with explicit `[x]`/`[ ]` evidence-backed checkboxes and
fail-closed behavior on every step, is the quality bar to match, not a UDP-only or
IP-ACL-shaped design that repeats what's already being replaced).
