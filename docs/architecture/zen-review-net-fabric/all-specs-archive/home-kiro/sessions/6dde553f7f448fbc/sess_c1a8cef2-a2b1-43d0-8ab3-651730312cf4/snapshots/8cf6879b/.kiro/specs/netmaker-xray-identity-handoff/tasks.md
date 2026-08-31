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



---

## 4 · Grants Materialization Reliability

Implements design.md §7: staleness detection and auto-recovery.

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

## 5 · Per-Registration Identity Container Provisioning

Implements design.md §6: container provisioned at netmaker registration.

- [ ] **P-1 — Define container provisioning event trigger.**
  - Hook into netmaker registration flow (enrollment-key/join-token).
  - Event source: `enrollment_keys_v1`/`tenants_v1` table changes.
  - Trigger: new registration creates provisioning request.
  - Fail-closed: provisioning failure = registration fails.

- [ ] **P-2 — Design container template.**
  - Base image: minimal container with Rust runtime.
  - Contents: verification binary, grants reader, egress config.
  - Bind mounts: `/dev/shm/opdbus/transport-binding.dat` (read-only).
  - Network: container-specific egress (**consumer** direction, isolated).

- [ ] **P-3 — Implement container provisioning logic (Rust).**
  - Create `op-identity/src/container_provisioner.rs`.
  - Function: `provision_identity_container(tenant_id: &str, config: &ProvisionConfig)`.
  - Uses incus API (or CLI) to create container.
  - Assigns dedicated OVS port for this container.
  - Computes identity tag via `derive_identity_tag(tenant_id)`.
  - Fail-closed: any provisioning step failure = abort and clean up partial state.

- [ ] **P-4 — Implement per-container grants materialization.**
  - Materialize identity-specific grants into container.
  - Path: `/etc/opdbus/capability-grants.json` inside container.
  - Source: filtered from host grants based on identity.

- [ ] **P-5 — Implement container network configuration.**
  - Each container gets isolated egress network (**consumer** direction).
  - No cross-container traffic allowed.
  - Egress rules based on identity grants.

- [ ] **P-6 — Add D-Bus interface for provisioning.**
  - Method: `org.opdbus.Identity.ProvisionContainer(tenant_id)`.
  - Returns: container name, alias, OVS port, identity tag, status, error if any.
  - Callable from netmaker registration hook.

- [ ] **P-7 — Add tests for container provisioning.**
  - Test successful provisioning creates container with correct config.
  - Test provisioning failure cleans up partial state.
  - Test container has read-only access to binding index.
  - Test OVS port and identity tag are correctly assigned.

- [ ] **P-8 — Implement human-friendly alias generation.**
  - Function: `generate_petname_alias(tenant_id: &str, wg_pubkey: &[u8; 32]) -> String`.
  - Derivation: `Blake3(tenant_id ‖ wg_pubkey)`, map to adjective-noun pair.
  - Use deterministic wordlists (e.g., 256 adjectives × 256 nouns = 65536 combinations).
  - Same identity always produces same alias.
  - Store alias in container labels: `incus config set <container> user.alias <alias>`.
  - **CONSTRAINT**: Alias is for display only. NEVER accept as input to verification
    or capability-grants logic. Only Blake3-derived footprint is authoritative.
  - Fail-closed: alias generation failure = log WARN, proceed without alias.

- [ ] **P-9 — Add alias to audit logs and container metadata.**
  - Include alias in all lifecycle log entries.
  - Add alias to D-Bus provisioning response.
  - Display alias in `incus list` output (via container labels).
  - Fail-closed: alias display failure does not block operations.

- [ ] **P-10 — Add tests for alias generation.**
  - Test deterministic alias: same input always produces same alias.
  - Test alias uniqueness: different inputs produce different aliases (probabilistically).
  - Test alias never used in verification logic (negative test).
  - Test alias appears in logs and container metadata.



---

## 6 · In-Container Verification Logic

Implements design.md §6.3: Rust verification binary running inside identity container.

- [ ] **V-1 — Create verification binary crate.**
  - New crate: `op-identity-verifier` (or module in `op-identity`).
  - Binary: `/usr/local/bin/op-identity-verifier` inside container.
  - Rust-only (per CLAUDE.md: "Rust-first: no new Python").

- [ ] **V-2 — Implement binding index reader.**
  - Read-only access to `/dev/shm/opdbus/transport-binding.dat`.
  - Reuse `lookup_binding()` from transport_binding.rs.
  - Fail-closed: unreadable binding index = reject all requests.

- [ ] **V-3 — Implement source IP extraction.**
  - Extract source IP from incoming TCP connection.
  - Handle both IPv4 and mapped IPv6 addresses.
  - Fail-closed: cannot determine source IP = reject.

- [ ] **V-4 — Implement verification pipeline.**
  - Pipeline steps:
    1. Extract source IP from connection
    2. Lookup binding by source IP
    3. Verify binding not expired (180s threshold)
    4. Optionally verify OpenFlow register/ct_mark matches expected value
    5. Compute footprint from verified pubkey
    6. Check capability grants for requested operation
    7. Reject if any step fails (fail-closed)
  - Return verified identity on success.

- [ ] **V-5 — Implement request forwarding.**
  - On successful verification, forward request via container egress.
  - Attach verified identity context to forwarded request.
  - Fail-closed: forwarding failure = reject request.

- [ ] **V-6 — Add health endpoint.**
  - Endpoint: `/health` returns verification service status.
  - Checks: binding index readable, grants loaded, egress reachable.
  - Fail-closed: unhealthy = reject all requests.

- [ ] **V-7 — Add tests for verification logic.**
  - Test verification with valid binding.
  - Test rejection with no binding (fail-closed).
  - Test rejection with expired binding.
  - Test grants enforcement.

---

## 7 · Container Lifecycle Management

Implements design.md §6.4–6.6: deprovisioning, expiry, and deactivation handling.

- [ ] **L-1 — Implement deprovisioning on enrollment-key revocation.**
  - Hook into netmaker enrollment-key revocation flow.
  - On revocation: stop container, remove storage, remove OVS port, clean up.
  - This is **permanent removal** — distinct from deactivation.
  - Fail-closed: deprovisioning failure = log ERROR, retry.

- [ ] **L-2 — Implement deprovisioning on tenant removal.**
  - Hook into netmaker tenant removal flow.
  - Same cleanup steps as L-1.

- [ ] **L-3 — Implement TTL-based expiry.**
  - Configurable TTL per enrollment key (default: none).
  - Background job checks container ages against TTL.
  - Warning notification N days before expiry (configurable).
  - Auto-deprovision on expiry.

- [ ] **L-4 — Add D-Bus method for manual deprovisioning.**
  - Method: `org.opdbus.Identity.DeprovisionContainer(container_name)`.
  - Callable by operator identity.
  - Returns: success/failure with reason.

- [ ] **L-5 — Implement audit logging for lifecycle events.**
  - Log: provisioning, deprovisioning, expiry warnings, deactivation, reactivation.
  - Include: tenant_id, container_name, alias, timestamp, reason.
  - Retention: per audit policy.

- [ ] **L-6 — Add tests for lifecycle management.**
  - Test deprovisioning on revocation.
  - Test TTL expiry flow.
  - Test manual deprovisioning via D-Bus.

- [ ] **L-7 — Implement account deactivation (suspend without deprovisioning).**
  - Set `deactivated` flag in container metadata (label or host-side state file).
  - Verification binary checks flag before processing requests.
  - If deactivated: reject with `VerifyError::AccountDeactivated`.
  - Optionally modify OpenFlow rules to drop traffic at datapath (defense in depth).
  - **Distinction from revocation (L-1)**: Deactivation is reversible, preserves state.
  - Fail-closed: deactivation flag check failure = treat as deactivated.

- [ ] **L-8 — Implement account reactivation.**
  - Clear `deactivated` flag via D-Bus method.
  - Method: `org.opdbus.Identity.ReactivateContainer(container_name)`.
  - Restore OpenFlow rules if they were removed.
  - Verification resumes immediately without reprovisioning.
  - Fail-closed: reactivation failure = log ERROR, container stays deactivated.

- [ ] **L-9 — Add D-Bus method for deactivation.**
  - Method: `org.opdbus.Identity.DeactivateContainer(container_name)`.
  - Callable by operator identity.
  - Returns: success/failure with reason.

- [ ] **L-10 — Add tests for deactivation/reactivation.**
  - Test deactivation blocks requests.
  - Test reactivation restores access.
  - Test deactivated container is NOT deprovisioned (state preserved).
  - Test audit logging for deactivation events.



---

## 8 · WireGuard Plugin Integration

Implements design.md §8.3: D-Bus signals for peer events.

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

## 9 · OpenFlow Identity Tagging

Implements design.md §9: datapath-level identity binding via OVS registers.

- [ ] **OF-1 — Extend parse_match() for metadata field.**
  - Location: `op-network/src/openflow_translate.rs`, function `parse_match()`.
  - Add parsing for `"metadata"` key in JSON match fields.
  - Map to `match.metadata` and `match.metadata_mask` in `rovs_openflow::Match`.
  - Fail-closed: invalid metadata value = skip that match criterion with WARN.

- [ ] **OF-2 — Extend parse_match() for ct_mark field.**
  - Add parsing for `"ct_mark"` key in JSON match fields.
  - Map to `match.ct_mark` and `match.ct_mark_mask`.
  - Fail-closed: invalid ct_mark value = skip with WARN.

- [ ] **OF-3 — Extend parse_match() for register matching (reg0–reg15).**
  - Add parsing for `"reg0"` through `"reg15"` keys.
  - Map to NXM register match fields via `oxm.rs` encoding.
  - Fail-closed: invalid register value = skip with WARN.

- [ ] **OF-4 — Wire LoadRegister action in build_actions().**
  - `JsonFlowAction::LoadRegister { register, value }` already exists in enum.
  - Emit actual OXM `NXM_NX_REG` load instruction in generated flow.
  - Verify output: `ovs-ofctl dump-flows` shows `load:<value>->NXM_NX_REG<n>[]`.
  - Fail-closed: invalid register number = skip action with WARN.

- [ ] **OF-5 — Implement identity tag derivation.**
  - Function: `derive_identity_tag(tenant_id: &str) -> u32`.
  - Derivation: lower 32 bits of `Blake3(tenant_id.as_bytes())`.
  - Deterministic: same tenant always gets same tag.
  - Store in container metadata: `incus config set <container> user.identity_tag <hex>`.

- [ ] **OF-6 — Integrate identity flow installation into provisioning.**
  - On container provision (P-3):
    1. Create OVS port for container.
    2. Derive identity tag.
    3. Install flow: `in_port=<container_port> actions=load:<tag>->NXM_NX_REG0[],NORMAL`.
  - Fail-closed: flow install failure = log ERROR, provisioning continues (defense in depth).

- [ ] **OF-7 — Integrate identity flow removal into deprovisioning.**
  - On container deprovision (L-1, L-2):
    1. Remove OpenFlow rules matching container's port.
    2. Delete OVS port.
  - Fail-closed: flow removal failure = log WARN, continue deprovision.

- [ ] **OF-8 — Add tests for OpenFlow identity tagging.**
  - Test `parse_match()` correctly parses metadata, ct_mark, reg[N].
  - Test `build_actions()` emits correct LoadRegister instruction.
  - Test identity tag derivation is deterministic.
  - Test flow installation on provision, removal on deprovision.
  - Test `ovs-ofctl dump-flows` shows expected rules.

- [ ] **OF-9 — Add end-to-end verification for identity tagging.**
  - Provision container, verify flow installed with correct tag.
  - Send traffic through container port, verify register value set.
  - Deprovision container, verify flow removed.
  - Evidence: `ovs-ofctl dump-flows` before/after.



---

## 10 · End-to-End Verification

Validates the complete identity-based gating system.

- [ ] **E-1 — Verify op-identity-shuttle running with wg-lan.**
  - Check: `sudo sv status op-identity-shuttle` shows running.
  - Check: logs show `WG_INTERFACE=wg-lan`.
  - Evidence: service status and log output.

- [ ] **E-2 — Verify binding creation on wg-lan handshake.**
  - Establish WG connection from test peer via wg-lan.
  - Query binding index, verify entry exists with correct pubkey.
  - Evidence: `opctl binding lookup --ip <peer_ip>` output.

- [ ] **E-3 — Verify identity container provisioned on registration.**
  - Register test peer via netmaker enrollment key.
  - Verify container created with correct name and alias.
  - Verify OVS port assigned with identity tag flow.
  - Evidence: `incus list` showing identity container, `ovs-ofctl dump-flows`.

- [ ] **E-4 — Verify in-container verification with valid binding.**
  - From wg-lan peer, make request through identity container.
  - Verify request succeeds (binding valid, grants checked).
  - Evidence: 200 OK response, audit log entry.

- [ ] **E-5 — Verify fail-closed on no binding.**
  - Attempt request from IP without binding.
  - Verify request rejected.
  - Evidence: 403 response, rejection log entry.

- [ ] **E-6 — Verify fail-closed on expired binding.**
  - Let binding expire (wait >180s without re-handshake).
  - Attempt request.
  - Verify request rejected until re-handshake.
  - Evidence: rejection, then acceptance after handshake.

- [ ] **E-7 — Verify grants gate uses verified footprint.**
  - Make request with capability requiring specific grant.
  - Verify grant check uses footprint from binding, not self-asserted.
  - Evidence: audit log shows binding-derived footprint.

- [ ] **E-8 — Verify grants staleness recovery.**
  - Simulate SHM/installed grants mismatch.
  - Make request requiring capability.
  - Verify auto-rematerialization occurs.
  - Evidence: WARN log, then successful capability check.

- [ ] **E-9 — Verify container deprovisioning on revocation.**
  - Revoke enrollment key for test peer.
  - Verify container removed, OVS port deleted, flows removed.
  - Evidence: `incus list` no longer shows container, `ovs-ofctl dump-flows` clean.

- [ ] **E-10 — Verify account deactivation blocks access.**
  - Deactivate identity via D-Bus.
  - Attempt request from that identity.
  - Verify request rejected with AccountDeactivated.
  - Evidence: 403 response, deactivation audit log.

- [ ] **E-11 — Verify account reactivation restores access.**
  - Reactivate previously deactivated identity via D-Bus.
  - Attempt request from that identity.
  - Verify request succeeds.
  - Evidence: 200 OK response, reactivation audit log.

- [ ] **E-12 — Verify alias appears in logs and metadata.**
  - Check audit logs for provisioning/deprovisioning events.
  - Verify alias is included in log entries.
  - Check `incus list` or `incus config get <container> user.alias`.
  - Evidence: alias present in logs and container metadata.

- [ ] **E-13 — Run 48-hour stability observation.**
  - Monitor: binding creation/expiry, container health, grants freshness,
    verification success rate, lifecycle events, OpenFlow rule stability.
  - No unexpected rejections for valid peers.
  - No successful requests from unbound sources.
  - Evidence: metrics/logs showing stable operation.

---

## 11 · Out of Scope (Verified Unchanged)

These systems are explicitly NOT modified by this spec. Verify they remain functional.

Traffic paths labeled by direction:
- **service** — inbound-facing
- **consumer** — outbound-initiating

- [ ] **O-1 — Customer/subscriber privacy tunnels (consumer).**
  - Verify xray passthrough still works (SNI routing, no decryption).
  - Evidence: traffic flows through xray unchanged.

- [ ] **O-2 — Mail/Qdrant/similar services (service).**
  - Verify incus proxy devices still route correctly.
  - Evidence: services accessible via existing paths.

- [ ] **O-3 — Netmaker mesh traffic (service — public API).**
  - Verify `OP_NETMK_*` iptables chains still functional.
  - Evidence: `iptables -L OP_NETMK_*` shows rules intact.

- [ ] **O-4 — Assistant container port 8090 (service — control plane).**
  - Verify dokodemo-door path still works.
  - Evidence: control-plane traffic flows correctly.

- [ ] **O-5 — wgcf-egress (consumer).**
  - Verify Cloudflare WARP egress path still works.
  - Evidence: outbound traffic routes correctly.

---

## Definition of Done

- [ ] `op-identity-shuttle` runit service running with `WG_INTERFACE=wg-lan`.
- [ ] TransportBindingIndex SHM file created and operational.
- [ ] Handshake watcher updates binding on every verified wg-lan handshake.
- [ ] Grants materialization detects staleness and auto-recovers.
- [ ] Per-registration identity containers provisioned on enrollment with alias.
- [ ] In-container verification logic validates against binding (fail-closed).
- [ ] Requests without valid binding rejected.
- [ ] Container lifecycle management (deprovision on revocation/expiry).
- [ ] Account deactivation/reactivation working without reprovisioning.
- [ ] WireGuard plugin emits D-Bus signals for peer events.
- [ ] OpenFlow identity tagging: `parse_match()`/`build_actions()` extended.
- [ ] Each container has dedicated OVS port with identity-tagged flows.
- [ ] Human-friendly alias generated for each container (display only, never authoritative).
- [ ] All E-* end-to-end verification tests pass.
- [ ] All O-* out-of-scope verifications confirm no regressions.
- [ ] 48-hour stability observation complete with no regressions.
- [ ] Customer tunnel traffic (consumer), mail/qdrant (service), netmaker mesh all unchanged.
