# Tasks: accountability-audit-trail

Each task is independently verifiable. Complete them in order — each step's output is the
next step's input. No implementation code is written outside the named files.

---

## Task 1 — Add `query_events` and `verify_chain` methods to blockchain plugin schema

**Crate:** `op-plugins`
**File:** `crates/op-plugins/src/state_plugins/blockchain_plugin.rs`

### What to add

1. New Input/Output structs (inside or adjacent to the existing `blockchain_schema()`
   function, matching the local struct pattern used by `ListSnapshotsOutput` etc.):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, Default)]
pub struct QueryEventsInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_event_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_event_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AuditEventRecord {
    pub event_id: u64,
    pub event_hash: String,
    pub prev_hash: String,
    pub timestamp: String,
    pub actor_id: String,
    pub capability_id: String,
    pub plugin_id: String,
    pub method_name: String,
    pub operation_type: String,
    pub target: String,
    pub tags_touched: Vec<String>,
    pub decision: String,
    pub input_patch_hash: String,
    pub result_effective_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct QueryEventsOutput {
    pub events: Vec<AuditEventRecord>,
    pub has_more: bool,
    pub total_in_chain: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, Default)]
pub struct VerifyChainInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_event_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_event_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VerifyChainOutput {
    pub valid: bool,
    pub events_verified: u64,
    pub errors: Vec<String>,
}
```

2. Two new `schema.methods.insert` calls after the existing 7:

```rust
schema.methods.insert(
    "query_events".to_string(),
    method_decl_from_schemars_with_output::<QueryEventsInput, QueryEventsOutput>(
        "query_events",
        SideEffect::Read,
        true,
        "blockchain.read",
        "obs.service.blockchain.events.query@v1",
    ),
);
schema.methods.insert(
    "verify_chain".to_string(),
    method_decl_from_schemars_with_output::<VerifyChainInput, VerifyChainOutput>(
        "verify_chain",
        SideEffect::Read,
        true,
        "blockchain.read",
        "obs.service.blockchain.chain.verify@v1",
    ),
);
```

### Verification

```bash
cargo check -p op-plugins
cargo clippy -p op-plugins --all-targets -- -D warnings
# After rebuild + restart of op-grpc-bridge:
./bin/zcall methods blockchain | grep -E "query_events|verify_chain"
# Expected: both methods listed with effect=read, capability=blockchain.read
```

---

## Task 2 — Add `"blockchain"` dispatch arm in MutationEngine (scoped to 2 methods)

**Crate:** `op-grpc-bridge`
**File:** `crates/op-grpc-bridge/src/mutation_engine.rs`

### What to add

1. A new match arm in `dispatch_method_call` BEFORE the `_ =>` catch-all:

```rust
"blockchain" => {
    match method {
        "query_events" => {
            dispatch_blockchain_query_events(&self.event_chain, &parsed_value).await?
        }
        "verify_chain" => {
            dispatch_blockchain_verify_chain(&self.event_chain, &parsed_value).await?
        }
        _ => serde_json::to_value(&parsed_value).unwrap_or(serde_json::Value::Null),
    }
}
```

2. The two dispatch functions and `chain_event_to_record` helper as specified in design.md.

3. Import `QueryEventsInput`, `QueryEventsOutput`, `VerifyChainInput`, `VerifyChainOutput`,
   `AuditEventRecord` from `op_plugins::state_plugins::blockchain_plugin` (or define locally
   if the types are not `pub` — adjust visibility accordingly).

### Key design points

- The inner `_ =>` for un-handled blockchain methods reproduces the outer catch-all's
  echo behavior. This is intentional — the existing 7 methods stay un-wired.
- `query_events` reads `self.event_chain` directly (same data as gRPC `GetEvents`).
- `verify_chain` reuses the chain's existing `verify_range` or equivalent logic (check
  if `EventChain` already has this; if not, implement inline with hash-chain walking).
- Default limit = 50, max = 100, clamped silently.

### Verification

```bash
cargo check -p op-grpc-bridge
cargo clippy -p op-grpc-bridge --all-targets -- -D warnings
# After rebuild + restart:
./bin/zcall blockchain query_events -a '{"limit": 5}'
# Expected: JSON with "events" array (possibly empty if chain is fresh), "has_more", "total_in_chain"
# NOT an echo of the input args.
./bin/zcall blockchain verify_chain -a '{}'
# Expected: {"valid": true, "events_verified": N, "errors": []}
./bin/zcall blockchain list_snapshots -a '{}'
# Expected: STILL echoes args (existing methods remain un-wired — scope boundary)
```

---

## Task 3 — Add `operation.proto` client codegen to zeroclaw-gui

**Crate:** `zeroclaw-gui`
**File:** `crates/zeroclaw-gui/build.rs`

### What to change

Add `operation.proto` (from `crates/op-grpc-bridge/proto/`) to the tonic-build
compilation list so that `EventChainServiceClient` and all its request/response types
are generated for the zeroclaw-gui binary.

The chat transport already demonstrates this pattern: `build.rs` compiles
`crates/op-chat/proto/chat.proto` and the generated code is included via
`include!("../proto/op_chat.chat.rs")` in `src/chat/transport.rs`.

For `operation.proto`, configure tonic-build to:
1. Generate client code only (no server).
2. Output to `src/proto/` (matching the existing layout).
3. Handle the `google.protobuf.Timestamp` and `google.protobuf.Struct` imports
   (already available via `prost-types` in Cargo.toml).

### Verification

```bash
cargo check -p zeroclaw-gui
# Expected: compiles without errors. Generated proto types are available.
```

---

## Task 4 — Create `accountability/mod.rs` module declaration

**Crate:** `zeroclaw-gui`
**Files:**
- `crates/zeroclaw-gui/src/accountability/mod.rs` (new)
- `crates/zeroclaw-gui/src/main.rs` (modify — add `mod accountability;`)

### What to create

```rust
//! Accountability module — reasoning audit trail and PII review surface.
//!
//! Architecturally decoupled from `crate::chat`: separate store, separate
//! transport, no shared state. Queries the `EventChainService` gRPC surface
//! directly (same pattern as chat queries `ChatService`).
//!
//! The D-Bus `blockchain.query_events` method provides the same audit data
//! for MCP clients and zcall operators; the GUI uses gRPC for efficiency.

pub mod store;
pub mod transport;
pub mod view;

pub use store::AccountabilityStore;
```

Add `mod accountability;` to the appropriate location in `main.rs` (check the existing
`mod chat;` declaration and mirror it).

### Verification

```bash
cargo check -p zeroclaw-gui
# (may require store/transport/view stubs — create them in tasks 5-7)
```

Note: Tasks 4-7 are a logical unit. Create all files before verifying compilation.

---

## Task 5 — Create `accountability/store.rs`

**Crate:** `zeroclaw-gui`
**File:** `crates/zeroclaw-gui/src/accountability/store.rs` (new)

### What to create

The `AccountabilityStore` struct and supporting types as specified in design.md:

- `AuditFilter` — filter state (plugin_id, decision, event_id range, limit default 50).
- `DecisionFilter` enum — All / Allow / Deny.
- `AuditEvent` — deserialized event from the proto `ChainEvent` message.
- `AccountabilityStore` — holds events vec, filter, has_more, loading flag, error,
  frame receiver channel, expanded detail set.
- `AccountabilityFrame` enum — Page / Error / StreamEvent.
- `impl AccountabilityStore` with `drain_frames()`, `should_repaint()`,
  `request_fetch()`, `page_forward()`, `page_back()`.

### Verification

```bash
cargo check -p zeroclaw-gui
```

---

## Task 6 — Create `accountability/transport.rs`

**Crate:** `zeroclaw-gui`
**File:** `crates/zeroclaw-gui/src/accountability/transport.rs` (new)

### What to create

Include the generated proto client code and implement `AccountabilityTransport` with:
- `fetch_page(channel, filter, tx)` — spawns tokio task calling
  `EventChainServiceClient::get_events`.
- `proto_to_audit_event(e: ChainEvent) -> AuditEvent` — field-by-field conversion.

Mirror the connection pattern from `chat/transport.rs`.

### Verification

```bash
cargo check -p zeroclaw-gui
```

---

## Task 7 — Create `accountability/view.rs`

**Crate:** `zeroclaw-gui`
**File:** `crates/zeroclaw-gui/src/accountability/view.rs` (new)

### What to create

```rust
pub fn render_accountability(
    ui: &mut egui::Ui,
    store: &mut AccountabilityStore,
    ctx: &egui::Context,
) { ... }
```

The view renders: header, filter bar, event table (Event ID | Timestamp | Actor |
Plugin | Method | Decision | Hash), detail expansion (all fields as JSON),
pagination buttons, loading/empty/error states. Uses project theme constants.

### Verification

```bash
cargo check -p zeroclaw-gui
cargo clippy -p zeroclaw-gui --all-targets -- -D warnings
```

---

## Task 8 — Wire Route::Accountability in views/mod.rs

**Crate:** `zeroclaw-gui`
**File:** `crates/zeroclaw-gui/src/views/mod.rs`

### What to change

1. Add `use crate::accountability;` import.
2. Add field to `ExplorerState`:
   ```rust
   pub accountability_store: crate::accountability::store::AccountabilityStore,
   ```
3. Add dedicated match arm BEFORE the `r => stub(...)` catch-all:
   ```rust
   Route::Accountability => accountability::view::render_accountability(
       ui, &mut explorer.accountability_store, ctx
   ),
   ```

### Verification

```bash
cargo check -p zeroclaw-gui
cargo clippy -p zeroclaw-gui --all-targets -- -D warnings
grep -n "Route::Accountability" crates/zeroclaw-gui/src/views/mod.rs
# Expected: shows the new dedicated arm, NOT inside the catch-all.
```

---

## Task 9 — Add `add_footprint` durability call in MutationEngine

**Crate:** `op-grpc-bridge`
**File:** `crates/op-grpc-bridge/src/mutation_engine.rs`

### What to change

1. Ensure `MutationEngine` holds a reference to `StreamingBlockchain`:
   ```rust
   pub streaming_blockchain: Option<Arc<StreamingBlockchain>>,
   ```
   Initialize from `OPDBUS_BLOCKCHAIN_PATH` env var or default path.

2. After each `record_method_call` (lines 525 and 971), add the `add_footprint` call.

3. Add the `event_to_footprint` conversion function as specified in design.md.

4. Verify `op-grpc-bridge/Cargo.toml` depends on `op-blockchain`.

### Important constraints

- Durability failure does NOT fail the dispatch — logs at `warn!` level.
- Synchronous with the dispatch (same code path, not deferred).

### Verification

```bash
cargo check -p op-grpc-bridge
cargo clippy -p op-grpc-bridge --all-targets -- -D warnings
# After rebuild + restart:
./bin/zcall cognitive_mcp get_health
ls /var/lib/opdbus/blockchain/timing/ | tail -3
# Expected: new JSON file appeared in timing_subvol
```

---

## Task 10 — Add startup chain rebuild from timing_subvol

**Crate:** `op-grpc-bridge`, `op-state-store`
**Files:** `crates/op-grpc-bridge/src/mutation_engine.rs`, `crates/op-state-store/src/event_chain.rs`

### What to change

1. Add `replay_from_footprint(&mut self, json: &serde_json::Value)` to `EventChain` in
   `event_chain.rs` — reconstructs and appends a `ChainEvent` from persisted JSON,
   using stored `event_hash` and `prev_hash` (not recomputing).

2. In `MutationEngine` startup, call `rebuild_chain_from_disk`:
   - Read `timing_subvol/*.json`, sort by timestamp, replay each.
   - Set `next_event_id` to `max(replayed) + 1`.
   - Skip malformed files with `warn!` log.
   - Log total count at `info!`.

### Verification

```bash
cargo check -p op-grpc-bridge
cargo check -p op-state-store
# After rebuild + restart:
./bin/zcall blockchain query_events -a '{"limit": 5}'
# Expected: events from BEFORE the restart are visible (rebuilt from disk).
```

---

## Task 11 — Integration: full build + end-to-end smoke test

**Verify the complete change set compiles and both query paths return real data.**

```bash
# 1. Full workspace check
cargo check --workspace

# 2. Clippy on affected crates
cargo clippy -p op-plugins -p op-grpc-bridge -p op-state-store -p zeroclaw-gui \
  --all-targets -- -D warnings

# 3. Verify blockchain methods appear:
./bin/zcall methods blockchain | grep -E "query_events|verify_chain"
# Expected: both listed

# 4. D-Bus path — query audit trail:
./bin/zcall blockchain query_events -a '{"limit": 10}'
# Expected: JSON with events array (not echo)

# 5. D-Bus path — verify chain integrity:
./bin/zcall blockchain verify_chain -a '{}'
# Expected: {"valid": true, "events_verified": N, "errors": []}

# 6. Scope boundary — existing methods still echo:
./bin/zcall blockchain list_snapshots -a '{}'
# Expected: echoes args (NOT a real result)

# 7. Verify route is wired (not hitting stub):
grep -n "Route::Accountability =>" crates/zeroclaw-gui/src/views/mod.rs
# Expected: accountability::view::render_accountability(...)

# 8. Verify module structure:
ls crates/zeroclaw-gui/src/accountability/
# Expected: mod.rs store.rs transport.rs view.rs

# 9. Verify decoupling (no chat imports in accountability):
grep -rn "crate::chat\|ChatStore\|ChatTransport\|ChatService" \
  crates/zeroclaw-gui/src/accountability/
# Expected: no matches

# 10. Verify durability:
./bin/zcall cognitive_mcp get_health  # trigger an event
ls /var/lib/opdbus/blockchain/timing/ | tail -3
# Expected: timing_subvol has JSON files

# 11. After restart, events survive:
# (restart op-grpc-bridge, then query again)
./bin/zcall blockchain query_events -a '{"limit": 5}'
# Expected: events from before restart are present

# 12. GUI: launch zeroclaw-gui, navigate to Accountability tab
# Expected: event table renders (or empty state if fresh), NOT the stub text
```

### Acceptance criteria

- `cargo check --workspace` exits 0.
- `cargo clippy` on affected crates produces no new `-D warnings` failures.
- `./bin/zcall blockchain query_events` returns real event data (not echo).
- `./bin/zcall blockchain verify_chain` returns integrity result.
- `./bin/zcall blockchain list_snapshots` STILL echoes (scope boundary).
- The `accountability/` module exists with 4 files.
- `Route::Accountability` dispatches to the real view, not `stub()`.
- No import from `crate::chat` exists in `crate::accountability`.
- The timing_subvol contains JSON files after method calls.
- After a restart, the event chain is non-empty (rebuilt from disk).
- The zeroclaw-gui Accountability tab renders an event table.

---

## Summary Table

| Task | Crate(s) | File(s) | Type |
|------|----------|---------|------|
| 1 — Schema methods | op-plugins | blockchain_plugin.rs | Modify (add methods + structs) |
| 2 — Dispatch arm | op-grpc-bridge | mutation_engine.rs | Modify (add blockchain arm) |
| 3 — Proto codegen | zeroclaw-gui | build.rs | Modify |
| 4 — Module declaration | zeroclaw-gui | accountability/mod.rs, main.rs | New + Modify |
| 5 — Store | zeroclaw-gui | accountability/store.rs | New |
| 6 — Transport | zeroclaw-gui | accountability/transport.rs | New |
| 7 — View | zeroclaw-gui | accountability/view.rs | New |
| 8 — Route wiring | zeroclaw-gui | views/mod.rs | Modify |
| 9 — Durability write | op-grpc-bridge | mutation_engine.rs | Modify |
| 10 — Startup rebuild | op-grpc-bridge, op-state-store | mutation_engine.rs, event_chain.rs | Modify |
| 11 — Integration test | all | — | Verify |

---

## Explicitly Not Done (see Non-Goals in requirements.md)

- The existing 7 blockchain methods remain un-dispatched (scope boundary: inner `_ =>`
  echo). Their wiring belongs to the paused schema-methods sweep.
- No automated PII detection or redaction.
- No changes to `crate::chat`, `ChatService`, or the `zeroclaw` plugin.
- No changes to `op-gemma/src/ui_gallery.rs`.
- No plugin-schema-methods sweep work beyond the 2 new methods.
