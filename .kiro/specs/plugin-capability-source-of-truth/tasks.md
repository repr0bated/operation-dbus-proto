# Implementation Plan: Plugin as Sole Source of Truth + Full Capability Model

## Overview

Five ordered phases. Every checkpoint requires `cargo build --workspace` to
succeed before moving to the next task. Phase boundaries are s6-service deploy
points. Tasks within a phase may overlap; tasks across phases must not.

---

## Phase 1 — Schema Types and PluginCapabilities Deduplication

- [ ] 1.1 Add `MethodDecl`, `SideEffect`, `SignalDecl` types to
      `crates/op-state-store/src/plugin_schema.rs`
  - Define `MethodDecl { name, args, returns, side_effect, idempotent,
    required_capability, subid }` with `Serialize, Deserialize, Debug, Clone`.
  - Define `SideEffect` enum `{ Read, Mutation }` (snake_case serde).
  - Define `SignalDecl { name, payload, subid }` with same derives.
  - _Requirements: 1.1, 1.2, 1.3, 11.3_

- [ ] 1.2 Move `PluginCapabilities` (4-field guarantee struct) to
      `crates/op-state-store/src/plugin_schema.rs` as the canonical definition
  - `pub struct PluginCapabilities { supports_rollback, supports_checkpoints,
    supports_verification, atomic_operations }` — derives `Default`.
  - Re-export from `op_state_store` crate root.
  - _Requirements: 2.1, 2.4_

- [ ] 1.3 Add `methods`, `signals`, `guarantees` fields to `PluginSchema`
  - `pub methods: HashMap<String, MethodDecl>` with `#[serde(default)]`.
  - `pub signals: Vec<SignalDecl>` with `#[serde(default)]`.
  - `pub guarantees: PluginCapabilities` with `#[serde(default)]`.
  - _Requirements: 1.1, 11.1, 11.2_

- [ ] 1.4 Remove duplicate `PluginCapabilities` from `crates/op-state/src/plugin.rs`
  - Delete the 4-field struct definition.
  - Update all `op-state` callers to `use op_state_store::PluginCapabilities`.
  - _Requirements: 2.2_

- [ ] 1.5 Remove duplicate `PluginCapabilities` from `crates/op-plugins/src/plugin.rs`
  - Delete the 8-field struct definition (`can_read`, `can_write`, etc.).
  - Update all `op-plugins` callers to `use op_state_store::PluginCapabilities`.
  - _Requirements: 2.3_

- [ ] 1.6 Checkpoint — workspace builds clean
  - `cargo build --workspace`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`

---

## Phase 2 — Populate Capability Surface in Plugin Schema Defs

- [ ] 2.1 Add `dispatch_method` to the `Plugin` / `StatePlugin` trait; deprecate
      `handle_command`
  - Add `async fn dispatch_method(&self, method: &str, args: serde_json::Value)
    -> Result<serde_json::Value>` with a default impl returning `Err`.
  - Mark `handle_command` `#[deprecated]`.
  - _Requirements: 1.7_

- [ ] 2.2 Populate `methods`, `signals`, `guarantees` for network plugins
      (wireguard, net, rtnetlink, openflow, ovsdb_bridge, ovsdb_daemon) in
      `plugin_schema_defs.rs`
  - Declare every callable method as a `MethodDecl` with `args` JSON Schema,
    `returns`, `side_effect`, `idempotent`, `required_capability`, and `subid`.
  - Declare every emitted signal as a `SignalDecl`.
  - Set `guarantees` booleans appropriately per plugin.
  - Add subid entries for every method and signal to `PluginSchema.subids`.
  - _Requirements: 1.1–1.6, 10.1–10.5_

- [ ] 2.3 Populate `methods`, `signals`, `guarantees` for container/service
      plugins (incus, s6, unix_socket, xray, privacy_router, privacy_routes)
  - Same pattern as 2.2.
  - _Requirements: 1.1–1.6, 10.1–10.5_

- [ ] 2.4 Populate `methods`, `signals`, `guarantees` for identity/auth plugins
      (wireguard, keypair, adc, gcloud_adc, agent_config, endpoint)
  - Same pattern as 2.2.
  - _Requirements: 1.1–1.6, 10.1–10.5_

- [ ] 2.5 Populate `methods`, `signals`, `guarantees` for observability/AI
      plugins (cognitive_mcp, compact_mcp, ctl_plane_chatbot, memory, btrfs,
      knowledge, factory, zeroclaw, antigravity, antigravity_chat, cron,
      fail2ban, workflows, schema_renderer, oscal_subid_registry,
      mail_server, proxy_server, web_ui, users, software, service,
      sess_decl, proxmox, hardware)
  - Same pattern as 2.2.
  - _Requirements: 1.1–1.6, 10.1–10.5_

- [ ] 2.6 Verify no inline schema definitions exist outside `plugin_schema_defs.rs`
  - `grep -r "PluginSchema {" crates/op-plugins/src/state_plugins/` should show
    only `schema()` methods that call `super::plugin_schema_defs::*`, never
    inline struct literals.
  - _Requirements: 1.1 (AGENTS.md one-schema-file rule)_

- [ ] 2.7 Checkpoint — workspace builds and tests pass
  - `cargo build --workspace`
  - `cargo test --workspace --all-targets --all-features`

---

## Phase 3 — Producer Writes Full Capability Schema to SHM

- [ ] 3.1 Create `/dev/shm/opdbus/` directory hierarchy at startup in
      `op-projection`
  - Ensure `schemas/` and `state/` subdirectories exist (tmpfs; no Btrfs I/O).
  - _Requirements: 3.1, NFR 1.1_

- [ ] 3.2 Write per-plugin capability schema JSON to
      `/dev/shm/opdbus/schemas/<plugin_id>.json`
  - Iterate registered plugins, call `plugin.schema()`, serialize with
    `serde_json::to_vec_pretty` (drop `simd_json` from this path), write file.
  - Skip plugins returning `None` from `schema()`.
  - _Requirements: 1.6, 3.1, 11.1, 11.2_

- [ ] 3.3 Write combined monolith to `/dev/shm/live-schema.json`
  - Collect all per-plugin schemas into a `HashMap<String, PluginSchema>` and
    serialize atomically (write to `live-schema.json.tmp`, rename).
  - _Requirements: 3.2_

- [ ] 3.4 Compute Blake3 catalog hash and write `.manifest.json` atomically
  - Hash computation: sort per-plugin filenames, leaf-hash each file's bytes
    with Blake3, fold to one root hash.
  - Write `{ "catalog_hash": "<hex>" }` to
    `/dev/shm/opdbus/.manifest.json.tmp`, then rename.
  - _Requirements: 3.3, 3.5, NFR 2.2_

- [ ] 3.5 Write present-state projections to `/dev/shm/opdbus/state/<id>.json`
      and refresh manifest hash after each state change
  - `op-projection` calls `plugin.get_state()` and writes result.
  - Update manifest hash after writing state.
  - _Requirements: 3.4_

- [ ] 3.6 Remove D-Bus object registration from `op-projection`
  - Delete any call to `conn.object_server().at(...)` under the plugins path.
  - Remove any `request_name("org.opdbus.v1.plugins")` call.
  - _Requirements: 3.6_

- [ ] 3.7 Checkpoint — `op-projection` builds; SHM files verified by unit test
  - Unit test: start SchemaEngine, register a test plugin with a `MethodDecl`,
    confirm `live-schema.json` contains `"methods"` key and `.manifest.json`
    contains `"catalog_hash"`.
  - `cargo build -p op-projection && cargo test -p op-projection`

---

## Phase 4 — Bridge Becomes Sole Owner

- [ ] 4.1 Bridge requests `org.opdbus.v1` well-known bus name at startup
  - In `op-grpc-bridge` main/init, call `conn.request_name("org.opdbus.v1")`.
  - _Requirements: 4.1_

- [ ] 4.2 Bridge reads `live-schema.json` (with per-plugin fallback) and builds
      `SchemaRouter` routes from full `PluginSchema` including `methods` map
  - `SchemaRouter::build_route` already calls `extract_methods` but reads from a
    `"methods"` key that is currently absent → now populated (Phase 2).
  - Confirm `route.methods` is non-empty for plugins with declared methods.
  - _Requirements: 4.2, 6.4_

- [ ] 4.3 `SchemaBackedInterface` exposes every `MethodDecl` as a D-Bus method
  - `SchemaBackedInterface::call` dispatches to `dispatch_method` by method name.
  - `SchemaBackedInterface` registers one D-Bus signal per `SignalDecl`.
  - _Requirements: 4.3, 4.4_

- [ ] 4.4 Bridge detects manifest hash change on inbound connection and reloads
      SHM without restart
  - Cache last-seen `catalog_hash`; on each inbound connection compare against
    `.manifest.json`. If changed, call `SchemaRouter::reload()`.
  - _Requirements: 4.5, NFR 1.1 (no polling)_

- [ ] 4.5 Replace `{"success": true}` stub in `SchemaBackedInterface::call` with
      real `SchemaEngine.mutate` dispatch
  - After validation + capability check, call
    `self.engine.mutate(plugin_id, &method, &json_args, capability_id, actor_id)`.
  - Propagate errors as `zbus::fdo::Error::Failed`.
  - _Requirements: 5.1, 5.2, 5.3, 5.4_

- [ ] 4.6 Rename `_capability_id` to `capability_id` in `SchemaEngine.mutate`
      and thread it into the `EventChain.record` call
  - Update signature: `pub async fn mutate(..., capability_id: Option<&str>, ...)`.
  - Pass `capability_id` to `chain.record(...)`.
  - _Requirements: 5.5, 7.4_

- [ ] 4.7 Add method validation: unknown method → reject before dispatch
  - `route.methods.get(&method).ok_or(UnknownMethod)?`
  - _Requirements: 6.1, 6.2, 6.5_

- [ ] 4.8 Add arg validation: `json_args` vs `MethodDecl.args` JSON Schema
  - Use `jsonschema` crate. Validate `serde_json::from_str(&json_args)` against
    `MethodDecl.args`. Reject with `InvalidArgs` on failure.
  - _Requirements: 6.3_

- [ ] 4.9 Implement capability enforcement in `GhostbridgeInterceptor`
  - Extract `X-Ghostbridge-Footprint` and `X-Ghostbridge-Trace-ID` from gRPC
    metadata; attach to request context.
  - In `SchemaBackedInterface::call`, if `decl.required_capability.is_some()`,
    verify the footprint grants it. Reject with `AccessDenied` / `PERMISSION_DENIED`.
  - Allow null `required_capability` without check.
  - _Requirements: 7.1, 7.2, 7.3, 7.5, 7.6_

- [ ] 4.10 Remove plugin-object registration from `op-dbus-mirror`
  - Delete all `conn.object_server().at("/org/opdbus/v1/plugins/...")` calls.
  - Remove `request_name("org.opdbus.v1")` from `op-dbus-mirror`.
  - Retain `org.opdbus.v1.mirror` name claim for mirror-management interfaces.
  - _Requirements: 4.6, 8.1_

- [ ] 4.11 Remove bare `org.opdbus.v1` name claim from `op-openvswitch-daemon`
  - The daemon shall claim `org.opdbus.v1.plugins.ovsdb` only.
  - _Requirements: 4.7, 8.3_

- [ ] 4.12 Remove dead `org.opdbus.v1` name claim from `op-state`
  - Delete the `request_name("org.opdbus.v1")` call in `op-state`. No s6 service
    exists for it; the code is unreachable.
  - _Requirements: 4.8, 8.2_

- [ ] 4.13 Bridge reads present-state from SHM for `GetProperties` calls
  - `SchemaBackedInterface::get_property` reads from
    `/dev/shm/opdbus/state/<plugin_id>.json`.
  - Missing file → return empty object, not an error.
  - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5_

- [ ] 4.14 Checkpoint — live bus ownership verified
  - `cargo build --workspace`
  - Manual: `busctl list | grep opdbus` shows exactly `org.opdbus.v1` (bridge)
    and `org.opdbus.v1.plugins.ovsdb`.
  - Automated: unit test calls a declared method on a test plugin via the bridge;
    confirms `SchemaEngine.mutate` is called with correct args (not stub).

---

## Phase 5 — Enforcement, Cleanup, and CI Gates

- [ ] 5.1 Document orphan `/usr/local/bin/opdbus` binary
  - Identify which crate or external package installs it. Add a note to
    `docs/orphan-opdbus-binary.md` with instructions to either point it at the
    bridge binary or delete it via a `deploy/` uninstall script.
  - _Requirements: 8.4_

- [ ] 5.2 Add CI check: exactly one `request_name("org.opdbus.v1")` site in
      workspace
  - `grep -r 'request_name.*org\.opdbus\.v1"' crates/ | grep -v plugins.ovsdb
    | wc -l` must equal 1.
  - _Requirements: 8.5_

- [ ] 5.3 Add CI check: subid uniqueness across all `PluginSchema.subids` entries
  - Collect all subid values from `plugin_schema_defs.rs` tests; assert no
    duplicates.
  - _Requirements: 10.5_

- [ ] 5.4 Remove `simd_json` from `op-state-store/src/plugin_schema.rs`
      serialization paths that now use `serde_json`
  - The `generate_template`, `to_json_schema`, and `to_contract_json_schema`
    methods use `simd_json::value::owned::Object`. Replace with `serde_json::Map`.
  - Keep `simd_json` only where the existing crate API requires it (e.g.
    `SchemaEngine` state cache for gRPC performance path).
  - _Requirements: NFR 1.1 (consistency)_

- [ ] 5.5 Write property-based tests for the capability enforcement pipeline
  - Property: for any `MethodDecl` with `required_capability = Some(c)`, a call
    from a footprint that does not grant `c` SHALL return `AccessDenied`.
  - Property: for any `MethodDecl` with `required_capability = None`, the call
    SHALL reach `SchemaEngine.mutate`.
  - Property: for any method name not in `PluginSchema.methods`, the call SHALL
    return `UnknownMethod`.
  - _Requirements: 6.1, 6.2, 7.2, 7.3, 7.5_

- [ ] 5.6 Write unit tests for `PluginSchema` serialization completeness
  - Test: serialize a `PluginSchema` with one `MethodDecl` and one `SignalDecl`;
    deserialize; assert `methods`, `signals`, and `guarantees` round-trip intact.
  - Test: empty `methods` serializes as `{}`, empty `signals` as `[]`.
  - _Requirements: 11.1, 11.2, 11.4_

- [ ] 5.7 Write unit test for SHM manifest atomicity
  - Test: concurrent write + read of `.manifest.json`; assert the reader never
    sees a partial write (tmp+rename guarantee).
  - _Requirements: NFR 2.2_

- [ ] 5.8 Final checkpoint — all gates green
  - `cargo build --workspace --release`
  - `cargo test --workspace --all-targets --all-features`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo fmt --all -- --check`
  - CI subid uniqueness check passes.
  - CI single `request_name` check passes.

---

## Phase 6 — Plugin Autogeneration Lifecycle and Gemma Plugin

These tasks must follow Phase 2 (schema types available) and Phase 4 (bridge
dispatch live). Each checkpoint requires `cargo build --workspace`.

- [ ] 6.1 Add `GemmaPlugin` struct and `gemma_plugin_schema()` to
      `plugin_schema_defs.rs`
  - Define `gemma_plugin_schema()` returning a `PluginSchema` with four
    `MethodDecl` entries: `ClassifySubid`, `RouteByTags`,
    `GenerateUiPerspectives`, `ResearchCapabilitySurface`.
  - `ResearchCapabilitySurface`: `args: { plugin_name: string, requested_info:
    string }`, `returns: CapabilitySurfaceDraft`, `side_effect: "read"`,
    `idempotent: true`, `required_capability: "gemma.research"`,
    `subid: "obs.service.gemma.research-capability@v1"`.
  - All four methods carry subids following the OSCAL taxonomy.
  - `GemmaPlugin::schema()` calls `super::plugin_schema_defs::gemma_plugin_schema()`.
  - _Requirements: 13.5, 13.6_

- [ ] 6.2 Implement `GemmaPlugin` as a `StatePlugin` in
      `crates/op-plugins/src/state_plugins/gemma.rs`
  - Implement `dispatch_method` for all four methods. `ResearchCapabilitySurface`
    runs the LLM reasoning via the local ollama/gemma4 route from the zeroclaw
    plugin state. Other methods move their existing logic from bespoke callers.
  - Register `GemmaPlugin` in `default_registry.rs` under key `"gemma"`.
  - Export from `state_plugins/mod.rs`.
  - _Requirements: 13.5, 13.8, 13.9_

- [ ] 6.3 Checkpoint — Gemma plugin compiles and is registered
  - `cargo build --workspace`
  - Unit test: `GemmaPlugin::schema()` returns a `PluginSchema` where
    `methods.contains_key("ResearchCapabilitySurface")` is true.

- [ ] 6.4 Define `CapabilitySurfaceDraft` type in `crates/op-plugins/src/auto_create.rs`
  - `pub struct CapabilitySurfaceDraft { methods, properties, signals,
    guarantees, subids, tags }` — derives `Serialize, Deserialize, Debug, Clone`.
  - This is the shape Gemma returns and the synthesizer consumes.
  - _Requirements: 12.5, 12.7_

- [ ] 6.5 Add durable draft store: `AutoGenDraft` persisted in CozoDB
  - Define `AutoGenDraft { plugin_id, revision, status, requested_info,
    capability_surface_draft, review_reason, pending_human_review, created_at,
    updated_at }`.
  - Add CozoDB relation `auto_gen_drafts` with `plugin_id` as primary key.
  - Implement `DraftStore::get`, `DraftStore::upsert`, `DraftStore::list`.
  - _Requirements: 12.16_

- [ ] 6.6 Implement autogeneration state machine in `auto_create.rs`
  - `AutoPlugin::create_from_requested_info` becomes the state machine entry.
    States: `pending_research` → `draft_pending_review` |
    `research_failed` | `synthesis_invalid`.
  - Idempotency check: if a draft for the name exists in a non-terminal state,
    return it. If `rejected`, require `requested_info` change + increment
    `revision`.
  - Persist each state transition to CozoDB via `DraftStore::upsert`.
  - _Requirements: 12.1, 12.2, 12.3, 12.16_

- [ ] 6.7 Replace `query_elements_via_agent` with Gemma dispatch
  - Delete the `create_agent("search-specialist", ...)` call and the
    `AgentTask` construction in `query_elements_via_agent`.
  - Call `SchemaEngine::mutate("gemma", "ResearchCapabilitySurface", args,
    Some("gemma.research"), actor_id)` instead.
  - On Gemma unavailable/error: set status `research_failed`,
    `pending_human_review: true`.
  - _Requirements: 12.4, 12.6, 13.1, 13.2, 13.10_

- [ ] 6.8 Implement synthesis phase: `CapabilitySurfaceDraft` → `PluginSchema`
  - After successful Gemma response, construct the full `PluginSchema` merging
    the `CapabilitySurfaceDraft` fields with autogeneration metadata fields.
  - Run structural validation (all `MethodDecl` subids present, valid
    `side_effect`, all `SignalDecl` subids have category `evt`).
  - On failure: set status `synthesis_invalid`, record errors in `review_reason`.
  - _Requirements: 12.7, 12.8, 12.9_

- [ ] 6.9 Checkpoint — research + synthesis path compiles
  - `cargo build --workspace`
  - Unit test: mock Gemma returning a minimal `CapabilitySurfaceDraft`; assert
    the output `PluginSchema` contains the drafted methods and passes structural
    validation.

- [ ] 6.10 Implement review actions: `ApproveDraft`, `RejectDraft`,
      `RequestRevision`
  - `ApproveDraft(plugin_id)`: persists synthesized `PluginSchema` to the plugin
    store, calls `PluginRegistry::register`, sets draft status `approved`.
  - `RejectDraft(plugin_id, reason)`: sets status `rejected`, records reason.
  - `RequestRevision(plugin_id, new_requested_info)`: sets status
    `pending_research`, re-enters research phase.
  - _Requirements: 12.11, 12.12, 12.13_

- [ ] 6.11 Enforce quarantine in bridge and producer
  - `op-projection`: skip plugins whose `plugin_id` is present in the draft
    store with status other than `approved`. Approved plugins flow through the
    normal SHM write path.
  - Bridge: add assertion in `SchemaRouter::register_objects` that the schema
    source is the canonical plugin store, not the draft store.
  - _Requirements: 12.10, 12.14, 12.15_

- [ ] 6.12 Validate Gemma schema declaration guard
  - In the autogeneration research entry point, before calling Gemma, check
    that `gemma` plugin is registered and its schema declares
    `ResearchCapabilitySurface`. If not, set draft to `research_failed` with
    reason `"gemma.ResearchCapabilitySurface not declared in schema"`.
  - _Requirements: 13.7_

- [ ] 6.13 Add CI check: no `search-specialist` call sites in `auto_create.rs`
  - `grep -r 'search-specialist' crates/op-plugins/src/auto_create.rs` must
    return empty.
  - _Requirements: 13.10_

- [ ] 6.14 Write property-based and integration tests for autogeneration lifecycle
  - Test: re-initiation of an active draft returns the existing draft
    (idempotency).
  - Test: `rejected` draft with same `requested_info` does NOT re-initiate.
  - Test: `rejected` draft with new `requested_info` increments `revision`.
  - Test: draft in `draft_pending_review` causes bridge to return `UnknownMethod`
    for any method on that plugin name.
  - Test: `ApproveDraft` causes producer to write SHM and bridge to register
    the object.
  - _Requirements: 12.1–12.15_

- [ ] 6.15 Final checkpoint — full Phase 6 green
  - `cargo build --workspace --release`
  - `cargo test --workspace --all-targets --all-features`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - No `create_agent("search-specialist")` in `auto_create.rs`.
  - Gemma plugin present in `busctl list` output after bridge start.
