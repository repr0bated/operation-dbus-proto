# WireGuard Identity-Based Gating — Implementation Tasks

This task list implements verified WireGuard-identity-based gating using `wg-lan`
and per-registration identity containers, closing the gap between CLAUDE.md's
claimed zero-trust model and actual code.

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
  - `watch_wireguard_handshakes(iface: &str)` at line 1132: LIVE, uses `ip monitor` + `wg show`
  - `run_schema_shuttle()` at line 1244: reads `WG_INTERFACE` env var, defaults to `wg0`
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

- [x] **I-8 — Verify wg-lan infrastructure exists.**
  - `/etc/wireguard/wg-lan.conf`: standalone identity WG server config
  - `/etc/runit/sv/wg-lan/`: runit service for wg-lan interface
  - **FINDING**: wg-lan is deliberately decoupled from netmaker mesh

- [x] **I-9 — Verify op-identity-shuttle runit service status.**
  - `/etc/runit/sv/op-identity-shuttle/`: DOES NOT EXIST
  - Binary code exists (`run_schema_shuttle()`) but service not deployed
  - **FINDING**: Shuttle is not running on the live host today

- [x] **I-10 — Verify OpenFlow crate capabilities (CORRECTED).**
  - Inspected `rovs-openflow-0.2.0` in `/home/admin/.cargo/registry/...`
  - `match_fields.rs`: `metadata`/`metadata_mask`, `ct_mark`/`ct_mark_mask` SUPPORTED
  - `oxm.rs`: NXM REG0–REG15, XXREG0–3 encoding SUPPORTED
  - `op-plugins/src/state_plugins/openflow.rs`: `JsonFlowAction::LoadRegister` EXISTS
  - **CORRECTED FINDING**: OpenFlow CAN carry per-peer identity. Previous "OF1.3
    limitation" conclusion was WRONG. Actual gap is missing plumbing in
    `openflow_translate.rs` `parse_match()` (lines ~145-230), not protocol.



---

## 1 · op-identity-shuttle Runit Service

Creates the missing runit service to run the identity shuttle with `WG_INTERFACE=wg-lan`.

- [ ] **S-1 — Create runit service directory.**
  - Create `/etc/runit/sv/op-identity-shuttle/`.
  - Fail-closed: directory creation failure = abort.

- [ ] **S-2 — Create run script.**
  - Path: `/etc/runit/sv/op-identity-shuttle/run`
  - Content:
    ```bash
    #!/bin/sh
    exec 2>&1
    export WG_INTERFACE=wg-lan
    exec /usr/local/bin/op-identity-shuttle
    ```
  - Permissions: 755.
  - Fail-closed: script must be executable.

- [ ] **S-3 — Create log directory and log run script.**
  - Path: `/etc/runit/sv/op-identity-shuttle/log/run`
  - Content: standard svlogd wrapper to `/var/log/op-identity-shuttle/`.
  - Create `/var/log/op-identity-shuttle/` with proper permissions.

- [ ] **S-4 — Add wg-lan dependency check.**
  - Create `/etc/runit/sv/op-identity-shuttle/check` script.
  - Verify `wg-lan` interface is up: `ip link show wg-lan | grep -q 'state UP'`.
  - Fail-closed: exit non-zero if wg-lan not up.

- [ ] **S-5 — Enable the service.**
  - Symlink: `ln -s /etc/runit/sv/op-identity-shuttle /etc/runit/runsvdir/default/`
  - Verify: `sudo sv status op-identity-shuttle` shows running.
  - Evidence: `sv status` output showing `run:` state.

- [ ] **S-6 — Verify shuttle is watching wg-lan.**
  - Check logs: `tail /var/log/op-identity-shuttle/current`
  - Expected: log entries mentioning `wg-lan` interface.
  - Evidence: log output showing handshake watcher active on wg-lan.

---

## 2 · TransportBindingIndex — SHM Data Structure

Implements design.md §4: new SHM file mapping wg-lan sessions to verified identities.

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

## 3 · Handshake Watcher Extension (wg-lan Scoped)

Extends `watch_wireguard_handshakes()` to update TransportBindingIndex per design.md §8.2.

- [ ] **H-1 — Parse allowed-IPs from wg show output.**
  - Extract allowed-IPs for each peer from `wg show wg-lan allowed-ips`.
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
  - Simulate handshake event on wg-lan, verify binding created.
  - Verify binding expires after 180s without re-handshake.
  - Verify re-handshake updates timestamp.
