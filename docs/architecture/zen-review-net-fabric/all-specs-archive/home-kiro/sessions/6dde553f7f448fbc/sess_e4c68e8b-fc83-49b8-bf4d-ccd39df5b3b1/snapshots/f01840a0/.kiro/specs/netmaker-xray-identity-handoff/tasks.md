# WireGuard Identity-Based Gating — Implementation Tasks

This task list implements verified WireGuard-identity-based gating to close the
gap between CLAUDE.md's claimed zero-trust model and actual code.

- `[x]` means independently verified with evidence.
- `[ ]` means required work remains.
- Evidence must be concrete: command output, file content, test results.

---

## 0 · Investigation Baseline (Complete)

- [x] **I-0 — Read CLAUDE.md transport & identity section.**
  - Claimed model: `X-Ghostbridge-Footprint`/`X-WireGuard-Pubkey` as "only gate"
  - IP ACLs described as "theater"
  - Identity = WireGuard pubkey → Argon2(PSK, salt=pubkey) session ID

- [x] **I-1 — Analyze op-identity/schema_bridge.rs.**
  - `IdentitySled` at line 191-224: 152-byte SHM struct
  - `etch_footprint()` at line 1008: Blake3(wg_pubkey ‖ catalog_hash ‖ mutation_index ‖ src_port)
  - `watch_wireguard_handshakes()` at line 1132: LIVE, uses `ip monitor` + `wg show`
  - `build_xray_config` at line 678: `#[cfg(test)]` ONLY, unused `_footprint` param

- [x] **I-2 — Analyze op-identity/session.rs.**
  - `derive_session_id()` at line 29: Blake3 KDF for pubkey → session_id
  - `derive_session_id_from_psk()` at line 63: Argon2(PSK, salt=pubkey)

- [x] **I-3 — Analyze op-grpc-bridge/interceptor.rs.**
  - `GhostbridgeInterceptor` at line 108-201: validates self-presented headers
  - `verify_per_identity()` at line 42-96: Cozo-backed sled lookup
  - `load_capability_grants()` at line 339-358: reads `/dev/shm/opdbus/capability-grants.json`
  - **FINDING**: Validates header against sled, NOT cryptographically tied to transport

- [x] **I-4 — Analyze op-grpc-bridge/schema_router.rs.**
  - `SchemaBackedInterface::call()` at line 710-786: dispatch gate
  - Re-reads sled per call, computes footprint, checks grants
  - Gate is REAL and LIVE, but identity source is self-asserted

- [x] **I-5 — Analyze op-cognitive-mcp/client_config.rs.**
  - `ClientConfig.wg_pubkey` at line 239
  - `with_wg_pubkey()` sets `X-Ghostbridge-Footprint` at line 568-570
  - **FINDING**: CLIENT SELF-ASSERTED, not verified

- [x] **I-6 — Analyze op-xray-daemon.**
  - `dbus.rs` + `main.rs`: Pure D-Bus lifecycle wrapper
  - Start/stop/reload/status via /proc + SIGTERM/SIGHUP
  - **FINDING**: NO header injection or traffic inspection

- [x] **I-7 — Analyze op-plugins/wireguard.rs.**
  - `WireGuardPlugin`/`WireGuardPeer` at line 77-99
  - Plain WG interface/peer CRUD
  - **FINDING**: ZERO connection to identity-sled/footprint system

---

## 1 · TransportBindingIndex — SHM Data Structure

Implements design.md §4: new SHM file mapping WG sessions to verified identities.

- [ ] **T-1 — Define the binding entry struct.**
  - Create `op-identity/src/transport_binding.rs`.
  - Struct `BindingEntry`: src_ip ([u8; 4]), src_port (u16), wg_pubkey ([u8; 32]),
    handshake_ts (u64), footprint ([u8; 32]), flags (u32).
  - Entry size: 96 bytes, aligned for atomic operations.
  - Magic: `OPBIND01`, version: 1.
  - Fail-closed: invalid magic/version = reject all lookups.

- [ ] **T-2 — Implement SHM file creation and initialization.**
  - Path: `/dev/shm/opdbus/transport-binding.dat`.
  - Create with proper permissions (0600, owner-only).
  - Initialize header with magic, version, entry_count=0.
  - Fail-closed: creation failure = no bindings available.

- [ ] **T-3 — Implement read primitives.**
  - `lookup_binding(src_ip: [u8; 4]) -> Option<BindingEntry>`.
  - Return `None` if no valid entry, entry expired, or SHM unreadable.
  - Expiry check: `now - handshake_ts > 180s` = expired.
  - Fail-closed: any error returns `None`, not panic.

- [ ] **T-4 — Implement write primitives.**
  - `upsert_binding(entry: BindingEntry) -> Result<()>`.
  - Atomic update: mark old entry invalid, write new, then mark valid.
  - Handle slot reuse for expired entries.
  - Fail-closed: write failure logs WARN, does not create binding.

- [ ] **T-5 — Implement expiry purge.**
  - `purge_expired() -> usize` returns count of purged entries.
  - Mark entries with `now - handshake_ts > 180s` as invalid.
  - Called periodically by handshake watcher.

- [ ] **T-6 — Add unit tests for binding index.**
  - Test creation, lookup, upsert, expiry, and purge.
  - Test concurrent read/write safety.
  - Test fail-closed behavior on corrupted SHM.

---

## 2 · Handshake Watcher Extension

Extends `watch_wireguard_handshakes()` to update TransportBindingIndex per design.md §7.2.

- [ ] **H-1 — Parse allowed-IPs from wg show output.**
  - Extract allowed-IPs for each peer from `wg show <iface> allowed-ips`.
  - Handle single /32 IPs and broader ranges.
  - Fail-closed: parse failure = skip binding update for that peer.

- [ ] **H-2 — Integrate binding update into handshake watcher.**
  - After existing `write_sled_from_wg()`, call binding index upsert.
  - For each IP in allowed-IPs, create binding entry with:
    - src_ip: the allowed IP
    - src_port: 0 (any port from this IP)
    - wg_pubkey: verified pubkey from handshake
    - handshake_ts: current timestamp
    - footprint: pre-computed via `etch_footprint()`
  - Fail-closed: binding write failure does not block sled update.

- [ ] **H-3 — Add periodic expiry purge to watcher loop.**
  - Call `purge_expired()` on each watcher iteration.
  - Log purged count at DEBUG level.

- [ ] **H-4 — Add binding index verification to watcher health check.**
  - Health check includes: SHM readable, magic valid, entry count reasonable.
  - Fail-closed: unhealthy binding index = report degraded, continue operation.

- [ ] **H-5 — Add integration tests for handshake → binding flow.**
  - Simulate handshake event, verify binding created.
  - Verify binding expires after 180s without re-handshake.
  - Verify re-handshake updates timestamp.

---

## 3 · Grants Materialization Reliability

Implements design.md §6: staleness detection and auto-recovery.

- [ ] **G-1 — Add Blake3 hash comparison utility.**
  - `blake3_file(path: &Path) -> Result<[u8; 32]>`.
  - Used to compare installed vs. SHM grants files.

- [ ] **G-2 — Implement staleness detection in opdbus-grants service.**
  - Compare `/etc/opdbus/capability-grants.json` hash vs.
    `/dev/shm/opdbus/capability-grants.json` hash.
  - Log WARN on mismatch: `"Grants SHM stale: installed={} shm={}"`.
  - Fail-closed: hash comparison failure = assume stale, rematerialize.

- [ ] **G-3 — Implement auto-recovery materialization.**
  - On staleness detection, re-read installed file and atomic-write to SHM.
  - Log INFO: `"Grants materialized: {} bytes"`.
  - Fail-closed: materialization failure = WARN and retry on next check.

- [ ] **G-4 — Add D-Bus method for forced rematerialization.**
  - Method: `org.opdbus.Grants.Rematerialize()`.
  - Callable by operator identity for manual recovery.
  - Returns success/failure with reason.

- [ ] **G-5 — Integrate staleness check into load_capability_grants().**
  - Before loading from SHM, verify freshness.
  - On stale, call rematerialize via D-Bus, then retry load.
  - Fail-closed: if rematerialization fails, reject capability check.

- [ ] **G-6 — Add periodic staleness check to opdbus-grants service.**
  - Check every 60 seconds while running.
  - Auto-recover on drift.

- [ ] **G-7 — Add tests for staleness detection and recovery.**
  - Simulate SHM/installed mismatch, verify auto-recovery.
  - Verify D-Bus rematerialize method works.

---

## 4 · xray Header Injection

Implements design.md §5: strip client headers, inject verified identity.

- [ ] **X-1 — Decide injection approach: plugin vs. sidecar.**
  - Option A (preferred): xray plugin in Go.
  - Option B (fallback): Rust sidecar proxy.
  - Decision criteria: deployment complexity, performance, maintenance burden.
  - Document decision with rationale.

- [ ] **X-2 — Implement binding lookup service.**
  - Unix socket: `/run/opdbus/transport-binding.sock`.
  - Protocol: simple request/response with src_ip, returns binding or rejection.
  - Fail-closed: socket unavailable = reject all lookups.

- [ ] **X-3 — Implement header stripping logic.**
  - Strip headers before injection:
    - `X-Ghostbridge-Footprint`
    - `X-WireGuard-Pubkey`
    - `X-Ghostbridge-Trace-Id`
  - Fail-closed: if stripping fails, reject request.

- [ ] **X-4 — Implement header injection logic.**
  - Query binding by source IP.
  - If valid binding: inject headers from binding entry.
  - If no binding: reject request (fail-closed).
  - Headers injected:
    - `X-Ghostbridge-Footprint`: hex(footprint)
    - `X-WireGuard-Pubkey`: base64(wg_pubkey)
    - `X-Ghostbridge-Trace-Id`: generated trace ID

- [ ] **X-5 — Implement injection point for HTTP traffic.**
  - Intercept HTTP requests arriving via WG tunnels.
  - Apply strip → lookup → inject pipeline.
  - Forward to destination only if injection succeeds.

- [ ] **X-6 — Add health endpoint for injection service.**
  - Reports: binding index health, lookup latency, injection success rate.
  - Fail-closed: unhealthy = reject all requests.

- [ ] **X-7 — Add tests for header injection.**
  - Test stripping of client-provided headers.
  - Test injection with valid binding.
  - Test rejection with no binding (fail-closed).
  - Test rejection with expired binding.

---

## 5 · xray Config Generator

Implements design.md §8: atomic config generation and reload.

- [ ] **C-1 — Create config generator module.**
  - `op-xray-daemon/src/config_generator.rs`.
  - Struct `XrayConfigGenerator` with template_path, output_path.

- [ ] **C-2 — Implement template loading.**
  - Load existing `/etc/xray/xray_config.json` as template.
  - Preserve existing routing/proxy rules.
  - Fail-closed: template load failure = abort generation.

- [ ] **C-3 — Implement identity-aware inbound handler generation.**
  - Add `wg-inbound` handler for WG-terminated traffic.
  - Configure sniffing, sockopt mark.
  - Generate routing rule to identity-inject outbound.

- [ ] **C-4 — Implement config validation.**
  - Validate JSON structure before write.
  - Validate required fields present.
  - Fail-closed: invalid config = abort, keep existing.

- [ ] **C-5 — Implement atomic write.**
  - Write to temp file, then rename to `/etc/xray/xray_config.json`.
  - Verify write succeeded by re-reading.
  - Fail-closed: write failure = abort, keep existing.

- [ ] **C-6 — Implement D-Bus reload with verification.**
  - Call existing `op-xray-daemon` reload method.
  - Wait for xray health check to pass.
  - Fail-closed: reload failure = rollback config, alert.

- [ ] **C-7 — Implement config rollback on failure.**
  - Keep backup of previous config.
  - On reload failure, restore backup and reload.
  - Log ERROR with failure details.

- [ ] **C-8 — Add tests for config generation.**
  - Test template loading and preservation.
  - Test identity-aware component addition.
  - Test atomic write and rollback.

---

## 6 · WireGuard Plugin Integration

Implements design.md §7.3: D-Bus signals for peer events.

- [ ] **W-1 — Define D-Bus signal schemas.**
  - Signal `PeerAdded`: interface, public_key, allowed_ips.
  - Signal `PeerRemoved`: interface, public_key.
  - Register on `org.opdbus.WireGuard` interface.

- [ ] **W-2 — Implement signal emission in WireGuardPlugin.add_peer().**
  - After peer insertion, emit `PeerAdded` signal.
  - Include all peer metadata.
  - Fail-closed: signal emission failure = log WARN, peer still added.

- [ ] **W-3 — Implement signal emission in WireGuardPlugin.remove_peer().**
  - Before peer removal, emit `PeerRemoved` signal.
  - Fail-closed: signal emission failure = log WARN, peer still removed.

- [ ] **W-4 — Implement signal subscription in handshake watcher.**
  - Subscribe to `PeerAdded`/`PeerRemoved` signals.
  - Update binding index on peer events.
  - Supplement polling-based `wg show` with event-driven updates.

- [ ] **W-5 — Add tests for signal emission and subscription.**
  - Test signal emission on peer add/remove.
  - Test subscription receives signals.
  - Test binding index updates on signal.

---

## 7 · Interceptor Trust Model

Implements design.md §7.4: verify injection point, not just headers.

- [ ] **I-1 — Define expected injection point configuration.**
  - Config: `expected_injector_addr: SocketAddr`.
  - Loaded from `/etc/opdbus/identity-config.json`.
  - Fail-closed: missing config = reject all requests.

- [ ] **I-2 — Implement injection point verification.**
  - In `GhostbridgeInterceptor`, check `req.peer_addr() == expected_injector`.
  - If mismatch: reject with `Error::UnexpectedSource`.
  - Log WARN on rejection with actual vs. expected addresses.

- [ ] **I-3 — Update verify_identity() to require injection point check.**
  - Add `expected_injector` parameter.
  - Call injection point verification before header validation.
  - Fail-closed: injection point mismatch = immediate rejection.

- [ ] **I-4 — Update SchemaBackedInterface::call() trust model.**
  - Document that headers are now trusted because:
    1. Request came through verified injection point (xray).
    2. xray performed binding lookup and header injection.
    3. Client-provided headers were stripped.

- [ ] **I-5 — Add tests for trust model.**
  - Test rejection of requests from unexpected source.
  - Test acceptance of requests from expected injector.
  - Test that header validation still occurs after injection point check.

---

## 8 · End-to-End Verification

Validates the complete identity-based gating system.

- [ ] **V-1 — Verify binding creation on WG handshake.**
  - Establish WG connection from test peer.
  - Query binding index, verify entry exists with correct pubkey.
  - Evidence: `opctl binding lookup --ip <peer_ip>` output.

- [ ] **V-2 — Verify header injection on request.**
  - From WG peer, make HTTP request to protected endpoint.
  - Capture headers at destination.
  - Evidence: headers present, matching binding pubkey/footprint.

- [ ] **V-3 — Verify header stripping prevents forgery.**
  - From WG peer, include fake identity headers in request.
  - Verify headers stripped and replaced with binding-derived values.
  - Evidence: destination sees binding values, not client-provided.

- [ ] **V-4 — Verify fail-closed on no binding.**
  - Attempt request from IP without binding.
  - Verify request rejected.
  - Evidence: error response, no headers injected.

- [ ] **V-5 — Verify fail-closed on expired binding.**
  - Let binding expire (wait >180s without re-handshake).
  - Attempt request.
  - Verify request rejected until re-handshake.
  - Evidence: rejection, then acceptance after handshake.

- [ ] **V-6 — Verify replay prevention.**
  - Capture valid headers from one request.
  - Attempt replay from different source IP.
  - Verify rejection (headers stripped, no binding for new source).
  - Evidence: rejection with "no binding" or "unexpected source".

- [ ] **V-7 — Verify grants gate uses verified footprint.**
  - Make request with capability requiring specific grant.
  - Verify grant check uses footprint from binding, not self-asserted.
  - Evidence: audit log shows binding-derived footprint.

- [ ] **V-8 — Verify grants staleness recovery.**
  - Simulate SHM/installed grants mismatch.
  - Make request requiring capability.
  - Verify auto-rematerialization occurs.
  - Evidence: WARN log, then successful capability check.

- [ ] **V-9 — Run 48-hour stability observation.**
  - Monitor: binding creation/expiry, header injection success rate,
    grants freshness, xray health, interceptor rejections.
  - No unexpected rejections for valid peers.
  - No successful requests from unbound sources.
  - Evidence: metrics/logs showing stable operation.

---

## Definition of Done

- [ ] TransportBindingIndex SHM file created and operational.
- [ ] Handshake watcher updates binding on every verified handshake.
- [ ] Grants materialization detects staleness and auto-recovers.
- [ ] xray strips client identity headers before processing.
- [ ] xray injects verified identity headers from binding.
- [ ] xray rejects requests without valid binding (fail-closed).
- [ ] xray config generator produces valid identity-aware config.
- [ ] WireGuard plugin emits D-Bus signals for peer events.
- [ ] GhostbridgeInterceptor verifies injection point before trusting headers.
- [ ] All VR-* verification tests pass.
- [ ] 48-hour stability observation complete with no regressions.
- [ ] Existing xray, Netmaker, WireGuard functionality preserved.
