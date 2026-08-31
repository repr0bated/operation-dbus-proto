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
