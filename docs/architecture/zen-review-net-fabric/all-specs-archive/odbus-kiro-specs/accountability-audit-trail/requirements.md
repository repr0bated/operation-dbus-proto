# Requirements: accountability-audit-trail

## Purpose

Replace the `Route::Accountability` stub in zeroclaw-gui with a real view that renders
the live reasoning audit trail (the `EventChain` data already recorded on every
`PluginV1.Call`) and surfaces raw event payloads for human PII review. The view is
architecturally decoupled from the Chat path — its own transport, its own store, no
shared module state with `crate::chat`.

The audit trail must be queryable through both the GUI (gRPC `EventChainService`) AND
through a proper D-Bus/MCP plugin method (`PluginV1.Call` on the `snowball` plugin),
so that external AI agents and MCP clients can query accountability data without
coupling to the gRPC proto surface.

## Context and Verified Baseline

### What already exists and is correct

- **EventChain is populated on every dispatch**: `MutationEngine` at
  `crates/op-grpc-bridge/src/mutation_engine.rs:63` holds
  `pub event_chain: Arc<RwLock<EventChain>>`. It writes via `record_method_call` at
  lines 525 and 971 (inside `mutate()` and `dispatch_method_call()`). Every
  `PluginV1.Call` that passes the capability gate produces a `ChainEvent` with
  `actor_id`, `capability_id`, `plugin_id`, `method_name`, `event_hash`, `timestamp`,
  `json_args_footprint`, `event_id`, and `decision` fields
  (`crates/op-state-store/src/event_chain.rs:124-165`).

- **gRPC EventChainService already exposes query/subscribe**:
  `crates/op-grpc-bridge/proto/operation.proto:55` defines `EventChainService` with
  `GetEvents` (filtered by plugin_id, event_id range, tags, decision; paginated via
  `limit` + `has_more`), `SubscribeEvents` (server-streaming), `VerifyChain`,
  `GetProof`, `ProveTagImmutability`, `GetSnapshot`, `CreateSnapshot`, and
  `SearchSemanticTrace`. The server implementation is at
  `crates/op-grpc-bridge/src/grpc_server.rs:1310-1470`.

- **zeroclaw-gui already has tonic gRPC client deps**: `Cargo.toml:25-30` lists
  `tonic 0.12`, `prost 0.13`, `prost-types 0.13`, `tonic-web 0.12`. The existing
  `chat::transport` module includes a proto at `src/proto/op_chat.chat.rs:14` and
  creates a tonic client channel. The same pattern applies for the Accountability
  transport.

- **The chat module pattern is the established idiom**: `crates/zeroclaw-gui/src/chat/`
  contains `mod.rs`, `store.rs`, `transport.rs`, `view.rs`. `ExplorerState` at
  `crates/zeroclaw-gui/src/views/mod.rs:13` holds `chat_store: ChatStore`. The route
  dispatcher at line 46 calls `chat::view::render_chat(ui, &mut explorer.chat_store, ctx)`.
  The Accountability module mirrors this structure exactly.

- **Stub renders today**: `crates/zeroclaw-gui/src/nav.rs:104` declares
  `it("Accountability", "✎", Route::Accountability)`. The route dispatcher at
  `crates/zeroclaw-gui/src/views/mod.rs:51` catches it with the `r => stub(...)` arm.
  `description()` at line 688 returns `"Reasoning audit trail and PII review."`.

- **The chat path is working end-to-end and must not be touched**:
  `Route::Chat => chat::view::render_chat(...)` (line 46), backed by
  `src/chat/transport.rs` which calls `op_chat.chat.ChatService` gRPC (`Send`,
  server-streaming). Server-side, `crates/op-grpc-bridge/src/chat_service.rs:274`
  (`ChatServiceImpl::send`) requires Ghostbridge identity and dispatches through the
  real `zeroclaw` plugin with `ChatInput` / `op_llm::ChatManager`. This is genuinely
  wired end-to-end.

- **On-disk StreamingSnowball** (`crates/op-snowball/src/streaming_snowball.rs:164`)
  has a `timing_subvol` field commented `// Audit trail (immutable history)`, written via
  `add_footprint` (line 247). But `mutation_engine.rs` never calls `add_footprint` — the
  two systems are disconnected today.

- **snowball plugin has schema methods but no dispatch arm**:
  `crates/op-plugins/src/state_plugins/snowball_plugin.rs:343-410` declares 7 methods
  (`create_snapshot`, `list_snapshots`, `get_snapshot`, `rollback`, `get_current_state`,
  `set_retention`, `get_stats`). However, `"snowball"` does NOT appear as a match arm
  in `crates/op-grpc-bridge/src/mutation_engine.rs` — it falls to the `_ =>` catch-all
  (line 1082: `_ => serde_json::to_value(&parsed_value)`) which echoes args back.

- **snowball_plugin.rs is the sanctioned home for audit chain capabilities**: Its doc
  comment (`crates/op-plugins/src/state_plugins/snowball_plugin.rs:1-16`) explicitly
  states: "This is the correct home for the capability the Lovable frontend mistakenly
  called as a standalone `operation.snowball.v1.SnowballService` gRPC package — there
  is no such proto and there must not be one. New backend capabilities register here as
  plugins."

### What is broken

1. **No Accountability-specific view code exists anywhere in the tree**: The route
   renders the generic `stub()` function (line 65 of `views/mod.rs`) whose body prints
   "View not yet ported. Wire this view to the ZeroClaw core (gRPC/D-Bus) and render
   real state here."

2. **EventChain is RAM-only**: `record_method_call` appends to an in-memory `Vec<ChainEvent>`
   (`crates/op-state-store/src/event_chain.rs:469`). Nothing persists it to disk. On
   process restart, the chain is empty. The `StreamingSnowball` timing_subvol IS
   durable but is not connected to the per-method-call EventChain writes.

3. **No plugin method exposes EventChain for querying via D-Bus**: `events()` (line 750)
   and `events_for_plugin()` (line 722) are Rust-only accessors on the struct. The only
   external query path is the gRPC `EventChainService.GetEvents` — but this is a separate
   proto service, not a `PluginV1.Call` schema method. No plugin's `schema.methods` map
   includes an "audit query" method. This means MCP clients and external AI agents have
   no path to the audit trail through the D-Bus control plane.

4. **snowball plugin's existing methods are not dispatched**: The 7 existing schema
   methods (`list_snapshots`, `get_snapshot`, etc.) fall to the catch-all echo — calling
   them via `zcall snowball list_snapshots` echoes the args back, not real data.

5. **No PII detection/redaction/review code exists**: `grep -ril "\bpii\b" crates/ --include=*.rs`
   returns references in comments and schema field descriptions (`ctl_plane_chatbot.rs:358`
   has a `pii_flagged: bool` field and some `[PII]` annotations in field docs) but no
   automated detection or redaction logic anywhere.

### What is NOT broken and must not be touched

- **The Chat path**: `crate::chat` module (store/transport/view), `ChatService` gRPC,
  `ChatServiceImpl::send`, the `zeroclaw` plugin dispatch arm. The Accountability feature
  is architecturally DECOUPLED — no shared module state, separate transport, separate store.

- **EventChain recording logic**: `record_method_call` at `mutation_engine.rs:525,971` and
  the `ChainEvent` struct. These work correctly; the spec adds a query surface, not a
  write surface.

- **The gRPC EventChainService**: Already works, already serves `GetEvents` and
  `SubscribeEvents`. The Accountability view uses this for the GUI path.

- **snowball_plugin.rs existing 7 schema methods**: Their declarations stay as-is.
  Wiring their full dispatch (DR snapshots, retention, rollback) is part of the paused
  13-plugin schema-methods sweep, NOT this spec. This spec only adds NEW audit-query
  methods and a minimal dispatch arm for them.

- **The 13-plugin schema-methods sweep**: Explicitly paused until this spec lands.

- **op-gemma ui_gallery zeroclaw wiring**: Explicitly paused.

---

## Functional Requirements

### FR-1: New `accountability` module in zeroclaw-gui

A new module at `crates/zeroclaw-gui/src/accountability/` mirroring the `src/chat/` pattern:

- `mod.rs` — module declaration, re-exports.
- `store.rs` — `AccountabilityStore`: holds the current page of `ChainEvent` records,
  pagination cursor, active filters (plugin_id, time range, actor_id, decision), and a
  `should_repaint()` flag. Append-only design consistent with `ChatStore`.
- `transport.rs` — `AccountabilityTransport`: tonic gRPC client for `EventChainService`.
  Connects to the same gRPC endpoint the chat transport uses (the op-grpc-bridge gRPC
  server on the session bus socket / loopback). Exposes `fetch_page()` (calls `GetEvents`)
  and `subscribe()` (calls `SubscribeEvents`, server-streaming, pushes new events into
  the store channel).
- `view.rs` — `render_accountability(ui, store, ctx)`: renders the event table with
  columns for timestamp, actor_id, plugin_id, method_name, decision, event_hash. Includes
  filter controls and pagination. Raw `json_args_footprint` / `input_patch_hash` are
  displayed for human review (this is the v1 "PII review" — see FR-5).

**Acceptance criteria**: `cargo check -p zeroclaw-gui` passes with the new module.
`grep -rn "mod accountability" crates/zeroclaw-gui/src/` shows the module declaration.
The module compiles without any `#[allow(dead_code)]` suppressions (all public items are
reachable from the route dispatcher).

### FR-2: Route dispatcher wires Route::Accountability to the real view

Replace the `r => stub(...)` catch-all handling of `Route::Accountability` in
`crates/zeroclaw-gui/src/views/mod.rs:51` with a dedicated arm:

```rust
Route::Accountability => accountability::view::render_accountability(ui, &mut explorer.accountability_store, ctx),
```

`ExplorerState` gains a new field `pub accountability_store: AccountabilityStore`
(same pattern as `chat_store: ChatStore`).

**Acceptance criteria**: The route no longer falls through to `stub()`. Building and
running zeroclaw-gui shows the Accountability view when navigating to that route.
`cargo clippy -p zeroclaw-gui --all-targets -- -D warnings` passes (modulo existing
unrelated warnings already present in the tree).

### FR-3: AccountabilityTransport calls EventChainService.GetEvents

The transport module includes the generated `operation.proto` client types and calls:

1. `GetEvents` with `GetEventsRequest { from_event_id, to_event_id, limit, plugin_id, tags, decision_filter }` — this is the primary paginated query for the view.
2. Optionally `SubscribeEvents` (server-streaming) for live-tail mode — if the user
   enables it, new events push into the store without re-fetching.

The proto is already compiled for `op-grpc-bridge`; zeroclaw-gui includes the generated
Rust code for the client side (same pattern as `include!("../proto/op_chat.chat.rs")`
in `chat/transport.rs`).

**Acceptance criteria**: `cargo check -p zeroclaw-gui` compiles with the transport.
A manual test: launching zeroclaw-gui and navigating to Accountability renders events
(or an empty state with "no events yet" if the chain is empty). Triggering a
`PluginV1.Call` (e.g. `./bin/zcall cognitive_mcp get_health`) then refreshing shows the
new event in the table.

### FR-4: Pagination and filtering — no unbounded "return everything" call

The `AccountabilityStore` issues `GetEvents` with a default `limit` of 50 events per page.
The view provides:

- **Time range filter**: converted to `from_event_id` / `to_event_id` by binary-searching
  the known event_ids (or just using monotonic event_id as a proxy for time ordering).
- **Plugin filter**: a dropdown populated from the known plugin list (reading the
  projection directory or hardcoded from the route catalog for v1).
- **Actor filter**: text input for exact actor_id match (no server-side support today —
  client-side filter on the returned page, acceptable for limit=50 pages).
- **Decision filter**: Allow / Deny / All (maps directly to `GetEventsRequest.decision_filter`).
- **Pagination**: "Older" / "Newer" buttons adjusting `from_event_id` / `to_event_id`.

**Acceptance criteria**: The view never issues a `GetEvents` with `limit = 0` (which
means unbounded). Each request has `limit <= 100`.

### FR-5: Minimal v1 "PII review" — raw payload surface for human inspection

v1 PII review is defined as: **surface the raw event metadata for human inspection**.
There is no automated PII detection, no redaction engine, no classifier. The view displays:

- `json_args_footprint` (Blake3 hash of the raw call arguments) — allows correlation
  with the actual payload if the operator has access to logs.
- `input_patch_hash` — the hash of the serialized input patch.
- `actor_id` — who initiated the call.
- `capability_id` — what capability was used.
- `method_name` — what method was called.
- `plugin_id` — what plugin was targeted.

A "Details" expandable row shows all available `ChainEvent` fields in a JSON viewer
(using egui's `CollapsingHeader` or similar). This gives the human operator enough
information to audit whether sensitive data flowed through a method call, decide whether
to investigate further, and correlate with external PII inventories.

**What is explicitly deferred to a follow-up spec (NOT this spec)**:
- Automated PII detection/classification on event payloads.
- Redaction of PII from displayed fields.
- Integration with `ctl_plane_chatbot.rs`'s `pii_flagged: bool` field.
- A "flag as PII" action button that would write back to the event chain.

**Acceptance criteria**: The Accountability view renders every `ChainEvent` field that
the proto `ChainEvent` message exposes (event_id, prev_hash, event_hash, timestamp,
actor_id, capability_id, plugin_id, schema_version, operation_type, target,
tags_touched, decision, deny_reason, input_patch_hash, result_effective_hash). An
expandable detail row shows all fields.

### FR-6: Durability decision — EventChain persists via StreamingSnowball

**Decision**: Unify the two audit backends. On every `record_method_call`, after appending
to the in-memory `Vec<ChainEvent>`, also call `StreamingSnowball::add_footprint` to
write the event durably to the `timing_subvol`. This gives:

- **Live-queryable**: the in-memory `EventChain` serves real-time queries via
  `EventChainService.GetEvents`.
- **Durable across restarts**: the `timing_subvol` Btrfs directory holds one JSON file
  per event, surviving process restarts.
- **Rebuild on startup**: on `MutationEngine` initialization, read the `timing_subvol`
  directory and replay events into the in-memory chain (sorted by timestamp). This
  restores the chain state after restart.

This unification lives in `crates/op-grpc-bridge/src/mutation_engine.rs`, NOT in the
zeroclaw-gui crate. The GUI is a pure consumer via gRPC.

**Rationale**: The `timing_subvol` was explicitly designed for this purpose (its comment
reads `// Audit trail (immutable history)`). The `PluginFootprint` struct
(`crates/op-snowball/src/footprint.rs:54`) is general enough to carry method-call event
data: it has `plugin_id`, `operation`, `timestamp`, `data_hash`, `content_hash`, and a
`metadata: HashMap<String, OwnedValue>` field for arbitrary data.

**Rejected alternative**: Keep them separate, GUI reads only in-memory chain. Rejected
because the chain is empty after every restart — unacceptable for an audit trail that
must survive process recycling.

**Rejected alternative**: Write a new persistence layer (SQLite, CozoDB, separate file).
Rejected because the timing_subvol already exists, is Btrfs-backed, and was designed for
this exact purpose.

**Acceptance criteria**: After `op-grpc-bridge` restarts, previously-recorded events are
visible in the Accountability view (fetched via `GetEvents`). The `timing_subvol`
directory contains one JSON file per event.

### FR-7: Schema-declared audit query methods on the `snowball` plugin

New methods are added to the `snowball` plugin schema
(`crates/op-plugins/src/state_plugins/snowball_plugin.rs`) to expose the audit trail
through `PluginV1.Call`. This makes the audit trail queryable by MCP clients, external
AI agents, and any consumer of the D-Bus plugin surface — not only by gRPC clients.

The `snowball` plugin is the sanctioned home for this capability per its own doc comment
(lines 1-16: "This is the correct home... New backend capabilities register here as plugins").

Two new methods:

1. **`query_events`** — paginated, filtered query of the event chain.
   - **Effect**: `Read` (pure query, no state mutation).
   - **Capability**: `snowball.read`.
   - **Subid**: `obs.service.snowball.events.query@v1`.
   - **Input**: `QueryEventsInput { plugin_id: Option<String>, from_event_id: Option<u64>, to_event_id: Option<u64>, limit: Option<u32>, decision: Option<String> }`.
   - **Output**: `QueryEventsOutput { events: Vec<AuditEventRecord>, has_more: bool, total_in_chain: u64 }`.
   - `AuditEventRecord` contains: `event_id`, `event_hash`, `prev_hash`, `timestamp`,
     `actor_id`, `capability_id`, `plugin_id`, `method_name`, `operation_type`, `target`,
     `tags_touched`, `decision`, `input_patch_hash`, `result_effective_hash`.
   - Default limit: 50. Max limit: 100. Exceeding max clamps silently.

2. **`verify_chain`** — integrity verification of the hash chain.
   - **Effect**: `Read`.
   - **Capability**: `snowball.read`.
   - **Subid**: `obs.service.snowball.chain.verify@v1`.
   - **Input**: `VerifyChainInput { from_event_id: Option<u64>, to_event_id: Option<u64> }`.
   - **Output**: `VerifyChainOutput { valid: bool, events_verified: u64, errors: Vec<String> }`.

These methods are dispatched by a NEW `"snowball"` match arm in
`MutationEngine::dispatch_method_call` — but ONLY for `query_events` and `verify_chain`.
The existing 7 methods (`list_snapshots`, `get_snapshot`, etc.) remain un-dispatched
(falling to the catch-all echo) until the paused schema-methods sweep wires them.

**Acceptance criteria**:
- `./bin/zcall methods snowball` shows `query_events` and `verify_chain` alongside the
  existing 7 methods.
- `./bin/zcall snowball query_events -a '{"limit": 10}'` returns a JSON object with
  `events` array and `has_more` boolean — NOT an echo of the input.
- `./bin/zcall snowball verify_chain -a '{}'` returns `{"valid": true, ...}`.
- `./bin/zcall snowball list_snapshots` STILL echoes args (existing methods remain
  un-wired — confirming the scope boundary).

### FR-8: snowball_plugin existing methods remain un-dispatched

The existing 7 schema methods (`create_snapshot`, `list_snapshots`, `get_snapshot`,
`rollback`, `get_current_state`, `set_retention`, `get_stats`) remain un-dispatched.
The new `"snowball"` dispatch arm handles ONLY `query_events` and `verify_chain`;
all other snowball methods fall through to the catch-all echo.

This is the explicit scope boundary: wiring the full snowball dispatch (which involves
`StreamingSnowball` DR/snapshot operations) belongs to the paused 13-plugin
schema-methods sweep, not this spec.

**Acceptance criteria**: No new match logic for `list_snapshots`, `get_snapshot`,
`rollback`, `get_current_state`, `set_retention`, `get_stats`, or `create_snapshot`.

---

## Non-Functional Requirements

### NFR-1: No new crate dependencies in zeroclaw-gui

The `EventChainService` proto client is generated from `operation.proto` which is already
in the workspace. `tonic`, `prost`, `prost-types` are already in zeroclaw-gui's Cargo.toml.
The build script includes the proto — no new external crates.

### NFR-2: Decoupled from Chat — no shared module state

`crate::accountability` does NOT import from `crate::chat`. `AccountabilityStore` shares
no fields with `ChatStore`. The two modules may share the tonic `Channel` connection (since
both talk to the same gRPC server) but have no logical coupling.

### NFR-3: No polling in the GUI

The view fetches on user interaction (navigate to tab, click refresh, page forward/back)
or via `SubscribeEvents` server-streaming push. No `std::thread::sleep` loops, no timers
polling `GetEvents` on an interval.

### NFR-4: Event chain append is synchronous with dispatch

The `add_footprint` call for durability happens in the same `dispatch_method_call` code
path, after `record_method_call` succeeds and before the response is returned to the
caller. It is NOT deferred to a background task. If `add_footprint` fails (disk error),
the event is still in the in-memory chain and the dispatch succeeds — durability failure
is logged at `warn!` level but does not block the call.

### NFR-5: OSCAL subid coverage

New items carry subids:
- `AccountabilityStore` view surface: `exp.software.zeroclaw.accountability.render@v1`
- `AccountabilityTransport` client: `obs.service.event-chain.query@v1`
- The durability write path: `evt.service.event-chain.persist@v1`
- `query_events` schema method: `obs.service.snowball.events.query@v1`
- `verify_chain` schema method: `obs.service.snowball.chain.verify@v1`

---

## Non-Goals (explicitly out of scope)

- **Rewiring `op-gemma/src/ui_gallery.rs`** to use zeroclaw for real generation — paused,
  not this spec.
- **The 13-plugin schema-methods sweep** — paused until this spec's tasks land.
- **Wiring the `snowball` dispatch arm for its existing 7 schema methods** (DR
  snapshots, retention, rollback) — belongs to the schema-methods sweep. This spec ONLY
  wires `query_events` and `verify_chain`.
- **Automated PII detection/classification** — no code for this exists; explicitly deferred.
- **Deleting or refactoring the stub() function** — other routes still use it.
- **Modifying `chat_service.rs` or the `zeroclaw` plugin dispatch** — the chat path is
  not touched.
