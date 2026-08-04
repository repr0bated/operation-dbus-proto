# Torch-Pass — Implementation Tasks

This task list implements cryptographically verified WireGuard identity-based
gating, replacing the current self-asserted header trust model.

- `[x]` means independently verified with evidence cited.
- `[ ]` means required work remains.
- Each task specifies fail-closed behavior on error.

---

## 0 · Pre-Implementation Analysis and Evidence Baseline

- [x] **T-0.1 — Confirm self-assertion is the actual gap.**
  - `client_config.rs` line 568: `.header("X-Ghostbridge-Footprint", pubkey)`
    is the sole origin of the footprint header.
  - Client sets its own identity; server trusts it.
  - Evidence: code read on 2026-08-04.
- [x] **T-0.2 — Confirm interceptor does not extract source IP.**
  - `GhostbridgeInterceptor` in `interceptor.rs` extracts footprint from
    header metadata only.
  - No `request.extensions().get::<ConnectInfo>()` or equivalent.
  - Validation is format/presence check, not transport binding.
  - Evidence: code read on 2026-08-04.
- [x] **T-0.3 — Confirm capability gate trusts unverified footprint.**
  - `SchemaBackedInterface::call()` at line ~710-786 computes footprint from
    sled data and checks capability grants.
  - The gate is real but the sled was populated from unverified header.
  - Evidence: code read on 2026-08-04.
- [x] **T-0.4 — Confirm WireGuard and identity systems are disjoint.**
  - `WireGuardPlugin` in `wireguard.rs` does peer CRUD.
  - Zero references to `IdentitySled`, `etch_footprint`, or any footprint system.
  - These systems were designed and deployed separately.
  - Evidence: code read on 2026-08-04.
- [x] **T-0.5 — Confirm xray config generation is test-only.**
  - `build_xray_config` and `route_to_outbound` in `schema_bridge.rs` are
    `#[cfg(test)]`-gated.
  - Production uses static `/etc/xray/xray_config.json`.
  - `_footprint` and `_trace_id` params in `route_to_outbound` are unused.
  - Evidence: code read on 2026-08-04.
- [x] **T-0.6 — Confirm OpenFlow cannot support identity gating.**
  - `openflow_translate.rs` supports only standard OF1.3 match fields.
  - No `reg[N]`, `metadata`, or `ct_mark` fields are wired.
  - Per-peer identity cannot be a datapath match key.
  - Identity gating must be application-layer.
  - Evidence: code read on 2026-08-04.
- [x] **T-0.7 — Document capability-grants staleness failure mode.**
  - SHM materialization at `/dev/shm/opdbus/capability-grants.json` drifted
    from source after network outage.
  - Required manual `opdbus-grants` service restart to rematerialize.
  - Solution must include automatic invalidation or bounded staleness.
  - Evidence: operational history documented in netclient-container-netns spec.

---

## 1 · Verified-Peers Registry

### 1.1 Registry data structure

- [ ] **T-1.1 — Define registry SHM layout.**
  - Path: `/dev/shm/opdbus/verified-peers.dat`
  - Header: magic `VPEERS01`, generation counter, entry count, staleness
    threshold, last update timestamp.
  - Entry: tunnel_ip (4 bytes), pubkey (32 bytes), handshake_ts (8 bytes),
    footprint_hash (32 bytes), flags (4 bytes).
  - Total entry size: 80 bytes.
  - Fail-closed: if layout validation fails, reject all verification requests.
- [ ] **T-1.2 — Implement registry operations.**
  - `verify_peer(source_ip, claimed_footprint) -> Result<VerifiedIdentity, RejectReason>`
  - `update_peer(pubkey, tunnel_ip, handshake_ts, footprint_hash)`
  - `remove_peer(pubkey)`
  - `expire_stale()` — mark entries past staleness threshold
  - `get_all_verified_peers()` — for xray config generation
  - Fail-closed: registry lock timeout → reject verification.
- [ ] **T-1.3 — Add generation counter semantics.**
  - Increment on any mutation (add, update, remove, expire).
  - Consumers can cache and invalidate on generation change.
  - Include in SHM header for atomic read.
- [ ] **T-1.4 — Add unit tests for registry operations.**
  - Add peer, verify present, verify absent, expire, remove.
  - Concurrent access safety.
  - Generation counter increments correctly.

### 1.2 Handshake watcher integration

- [ ] **T-1.5 — Extend `watch_wireguard_handshakes()` to update registry.**
  - On handshake detection: compute footprint, call `update_peer()`.
  - Emit `peer_verified` event for downstream consumers.
  - Fail-closed: watcher crash → existing entries honor TTL, no new verifications.
- [ ] **T-1.6 — Add periodic stale-entry cleanup.**
  - Run `expire_stale()` on configurable interval (default: 30 seconds).
  - Emit `peer_expired` event for expired entries.
  - Do not delete expired entries; mark as unverified for audit trail.
- [ ] **T-1.7 — Add integration test: handshake → registry update.**
  - Mock WireGuard interface with test peer.
  - Trigger handshake, verify registry entry created.
  - Verify footprint computation matches `etch_footprint()`.

---

## 2 · Interceptor Modification

### 2.1 Source IP extraction

- [ ] **T-2.1 — Add source IP extraction to `GhostbridgeInterceptor`.**
  - Extract from gRPC `ConnectInfo` extension.
  - Handle both direct socket and proxied connections.
  - Fail-closed: no source IP extractable → reject request.
- [ ] **T-2.2 — Add IPv4 validation.**
  - Reject IPv6 (not supported in current WireGuard config).
  - Reject localhost/loopback sources for external requests.
  - Allow localhost for internal D-Bus bridge traffic (configurable).

### 2.2 Registry verification

- [ ] **T-2.3 — Add registry lookup in interceptor.**
  - Query registry by extracted source IP.
  - `RejectReason::UnknownPeer` if no entry found.
  - `RejectReason::StaleHandshake` if entry expired.
  - `RejectReason::FootprintMismatch` if claimed != registry.
  - `RejectReason::RegistryUnavailable` if registry inaccessible.
- [ ] **T-2.4 — Replace header trust with registry verification.**
  - Extract claimed footprint from header (existing code).
  - Validate against registry entry for source IP (new code).
  - Proceed only if verification succeeds.
  - Fail-closed: any verification failure → reject before capability check.
- [ ] **T-2.5 — Add audit logging for verification decisions.**
  - Log: source IP, claimed footprint (truncated), result, reason.
  - WARN level for rejections with diagnosis detail.
  - INFO level for successful verifications.
  - No secrets in logs (truncate pubkeys, hash footprints).

### 2.3 Testing

- [ ] **T-2.6 — Add unit tests for interceptor verification.**
  - Valid peer with fresh handshake → proceed.
  - Valid peer with expired handshake → reject.
  - Unknown source IP → reject.
  - Footprint mismatch → reject.
  - Registry unavailable → reject.
- [ ] **T-2.7 — Add integration test: end-to-end verification.**
  - Set up test registry with known peer.
  - Send request from matching source IP with correct footprint → pass.
  - Send request from matching source IP with wrong footprint → reject.
  - Send request from unknown source IP → reject.

---

## 3 · Capability-Grants Freshness

### 3.1 Generation tracking

- [ ] **T-3.1 — Add generation counter to capability-grants SHM.**
  - Include in JSON structure: `{ "generation": N, "grants": {...} }`.
  - Increment on every rematerialization.
  - Consumers reject if generation is stale.
- [ ] **T-3.2 — Add staleness threshold check.**
  - Track `loaded_at` timestamp.
  - If elapsed > threshold (default: 60 seconds), reload from durable source.
  - Fail-closed: stale grants → reject requests until refresh.

### 3.2 Event-driven refresh

- [ ] **T-3.3 — Add inotify watch on durable grants file.**
  - Watch `deploy/security/capability-grants.json`.
  - On modification: reload and rematerialize SHM.
  - Emit refresh signal to consumers.
- [ ] **T-3.4 — Add D-Bus signal for grants refresh.**
  - `software.zeroclaw.CapabilityGrants.Refreshed` signal.
  - Include new generation counter.
  - Consumers subscribe and invalidate caches.
- [ ] **T-3.5 — Add startup reconciliation.**
  - On bridge startup: compare durable source hash with SHM hash.
  - If divergent: rematerialize SHM from durable source.
  - Log reconciliation action.

### 3.3 Testing

- [ ] **T-3.6 — Add unit tests for grants freshness.**
  - Fresh grants → pass.
  - Stale grants → fail-closed.
  - File modification → refresh triggered.
  - SHM divergence → rematerialization.

---

## 4 · WireGuard Integration

### 4.1 Peer lifecycle events

- [ ] **T-4.1 — Add registry seeding on peer addition.**
  - `WireGuardPlugin::add_peer()` calls `registry.seed_peer()`.
  - Seeded entry is unverified until handshake completes.
  - Fail-closed: seed failure → peer addition fails.
- [ ] **T-4.2 — Add registry cleanup on peer removal.**
  - `WireGuardPlugin::remove_peer()` calls `registry.remove_peer()`.
  - Emit `peer_removed` event.
  - Subsequent requests from that tunnel IP → reject.
- [ ] **T-4.3 — Add event bridge between WireGuard and registry.**
  - D-Bus signals: `PeerAdded`, `PeerRemoved`, `HandshakeCompleted`.
  - Registry subscribes to WireGuard signals.
  - Xray config generator subscribes to registry events.

### 4.2 Testing

- [ ] **T-4.4 — Add integration test: peer lifecycle.**
  - Add peer → seeded in registry.
  - Handshake → verified in registry.
  - Request → passes verification.
  - Remove peer → removed from registry.
  - Request → fails verification.

---

## 5 · Xray Config Generator

### 5.1 Generator daemon

- [ ] **T-5.1 — Create xray config generator service.**
  - New crate: `op-xray-config-gen` or module in `op-xray-daemon`.
  - Subscribe to verified-peers registry events.
  - Generate routing config on peer changes.
  - Atomic write to `/etc/xray/xray_config.json`.
- [ ] **T-5.2 — Implement config template system.**
  - Base template with static routes (existing ingress).
  - Dynamic section for identity-based peer routes.
  - Validation before write (JSON syntax, required fields).
- [ ] **T-5.3 — Implement atomic config update.**
  - Write to temp file, rename to target.
  - Verify file contents match expected after rename.
  - Fail-closed: write failure → retain existing config, log error.
- [ ] **T-5.4 — Implement xray reload trigger.**
  - Call `op-xray-daemon` D-Bus method `reload_config()`.
  - D-Bus method sends SIGHUP to xray process.
  - Verify xray reloaded successfully (check process status).

### 5.2 op-xray-daemon extension

- [ ] **T-5.5 — Add `reload_config()` D-Bus method.**
  - Find xray PID via `/proc` scan (existing pattern).
  - Send SIGHUP signal.
  - Return success/failure.
  - Fail-closed: PID not found → return error, do not spawn.
- [ ] **T-5.6 — Add config validation before reload.**
  - Parse JSON syntax.
  - Validate required xray config fields.
  - Reject invalid config before SIGHUP.

### 5.3 Phased rollout

- [ ] **T-5.7 — Implement shadow mode for generator.**
  - Generate config but do not write to live path.
  - Compare against static config.
  - Log differences for validation.
  - Duration: 1 week minimum.
- [ ] **T-5.8 — Implement cutover with fallback.**
  - Rename static config to `.static-backup`.
  - Generator writes to live path.
  - On generator failure: restore from backup.
  - Rollback command: `sv stop op-xray-config-gen && cp .static-backup config.json`.

### 5.4 Testing

- [ ] **T-5.9 — Add unit tests for config generation.**
  - Empty peer set → base config only.
  - Single peer → base + one route.
  - Multiple peers → base + all routes.
  - Peer removal → route removed.
- [ ] **T-5.10 — Add integration test: config → xray reload.**
  - Generate config with test peer.
  - Write to test path.
  - Verify JSON valid.
  - Verify xray accepts config (dry-run mode if available).

---

## 6 · Migration and Rollback

### 6.1 Phase 1: Registry and shadow verification

- [ ] **T-6.1 — Deploy registry alongside existing system.**
  - Registry operational.
  - Handshake watcher updating registry.
  - No enforcement yet.
- [ ] **T-6.2 — Deploy interceptor in log-only mode.**
  - Modified interceptor deployed.
  - Verification runs but does not reject.
  - Log mismatches between header trust and registry verification.
- [ ] **T-6.3 — Monitor Phase 1 for 1 week.**
  - Track mismatch rate.
  - Investigate any legitimate traffic that would be rejected.
  - Tune staleness threshold if needed.

### 6.2 Phase 2: Verification enforcement

- [ ] **T-6.4 — Enable verification rejection.**
  - Flip enforcement flag.
  - Requests failing verification are rejected.
  - Static xray config still in use.
- [ ] **T-6.5 — Monitor Phase 2 for 1 week.**
  - Track rejection rate.
  - Investigate false positives.
  - Tune if needed.
- [ ] **T-6.6 — Document rollback procedure.**
  - Revert interceptor to log-only.
  - Or revert to original interceptor.
  - No data migration needed.

### 6.3 Phase 3: Dynamic xray config

- [ ] **T-6.7 — Deploy config generator in shadow mode.**
  - Generator running, not writing to live path.
  - Compare output against static config.
  - Log and review differences.
- [ ] **T-6.8 — Cut over to generated config.**
  - Backup static config.
  - Enable generator writes.
  - Monitor xray health.
- [ ] **T-6.9 — Document Phase 3 rollback.**
  - Stop generator.
  - Restore static config from backup.
  - Restart xray if needed.

---

## 7 · Definition of Done

- [ ] Verified-peers registry operational with handshake watcher integration.
- [ ] `GhostbridgeInterceptor` extracts source IP and validates against registry.
- [ ] Requests from unverified sources are rejected (fail-closed).
- [ ] Capability-grants have generation tracking and event-driven refresh.
- [ ] `WireGuardPlugin` emits events to registry on peer add/remove.
- [ ] Xray config generator produces identity-based routing rules.
- [ ] Config generator has shadow mode and rollback procedure.
- [ ] All unit and integration tests pass.
- [ ] Phase 1 shadow mode shows <1% mismatch rate for 1 week.
- [ ] Phase 2 enforcement shows <0.1% false positive rate for 1 week.
- [ ] Phase 3 generated config matches expected output for all peers.
- [ ] Audit logging captures all verification decisions.
- [ ] Documentation updated: CLAUDE.md zero-trust section reflects new reality.

---

## 8 · Post-Implementation Verification

- [ ] **V-1 — End-to-end test: legitimate client.**
  - Client with valid WG tunnel connects.
  - Handshake completes, registry updated.
  - Request with correct footprint succeeds.
  - Capability grants applied correctly.
- [ ] **V-2 — End-to-end test: spoofed header from non-WG source.**
  - Request from non-WG IP with spoofed header.
  - Rejected at registry lookup (unknown peer).
- [ ] **V-3 — End-to-end test: spoofed header claiming different peer.**
  - Request from WG IP A with footprint of peer B.
  - Rejected at footprint mismatch.
- [ ] **V-4 — End-to-end test: expired handshake.**
  - Peer with handshake older than threshold.
  - Request rejected (stale handshake).
  - Peer performs new handshake.
  - Subsequent request succeeds.
- [ ] **V-5 — Grants staleness test.**
  - Modify grants file.
  - Verify SHM refreshed within 5 seconds.
  - Verify new grants applied to subsequent requests.
- [ ] **V-6 — Xray routing test.**
  - Add new peer.
  - Verify xray config regenerated.
  - Verify traffic routed by identity.
  - Remove peer.
  - Verify route removed.
