# Design: accountability-audit-trail

## Architecture Decisions

Five design questions resolved below with decisions, rationale, and rejected alternatives.

---

### DQ-1: Which audit backend is authoritative for the Accountability view?

**Decision**: Unify. The in-memory `EventChain` remains the live-query source (served by
gRPC `EventChainService.GetEvents`). Additionally, each `record_method_call` now ALSO
writes the event durably to `StreamingSnowball`'s `timing_subvol`. On startup, the
chain is rebuilt by reading back the timing_subvol directory.

**Why both, not one**:
- The in-memory chain gives sub-millisecond query latency and supports the existing
  `GetEvents` / `SubscribeEvents` gRPC surface without change.
- The timing_subvol gives durability: events survive `op-grpc-bridge` restarts.
- The two are unified by writing both at the same call site (`dispatch_method_call`),
  not by polling or syncing.

**Data flow**:
```
PluginV1.Call arrives
  → schema_router validates + capability check
    → MutationEngine::dispatch_method_call()
      → event_chain.record_method_call(actor, plugin, method, cap, args)  [in-memory]
      → streaming_snowball.add_footprint(event_to_footprint(chain_event)) [on-disk]
      → dispatch actual method logic
      → return result with event_id + event_hash
```

**Startup rebuild**:
```
MutationEngine::new()
  → read timing_subvol/*.json sorted by timestamp
  → for each file: deserialize → EventChain::replay_event(chain_event)
  → chain state = full history from disk
```

**Rejected alternative — read only the in-memory chain (no durability)**:
Unacceptable for an audit trail. The chain would be empty after every restart, making the
"audit" claim a lie.

**Rejected alternative — read only the timing_subvol (query disk directly)**:
Would require building a query engine (filtering, pagination) over a directory of JSON
files. The in-memory chain + existing `GetEvents` implementation already provides this.

**Rejected alternative — new persistence layer (SQLite, CozoDB)**:
Over-engineering. The timing_subvol already exists and is purpose-built.

---

### DQ-2: What schema methods expose audit data through PluginV1.Call?

**Decision**: Add `query_events` and `verify_chain` methods to the `snowball` plugin.
This makes the audit trail queryable through the D-Bus/MCP plugin surface, not only
through the gRPC `EventChainService`.

**Why the `snowball` plugin**: Its doc comment
(`crates/op-plugins/src/state_plugins/snowball_plugin.rs:1-16`) explicitly states:
"This is the correct home for the capability... New backend capabilities register here
as plugins; the served gRPC surface stays the one shared route-builder." The audit trail
IS the streaming snowball's timing_subvol — this is literally what the plugin was
designed to expose.

**Why D-Bus methods and not only gRPC**: The repo's architecture mandates "D-Bus is the
only control plane" (CLAUDE.md line 44). The gRPC `EventChainService` is an internal
operational surface, but MCP clients, external AI agents, and `zcall` operators need the
audit trail through the canonical `PluginV1.Call` path. Without D-Bus methods:
- MCP tool exposure requires coupling to a proto service (violates the no-new-proto rule).
- `zcall` operators have no way to query events (there's no `zcall events` subcommand).
- AI agents in the control-plane chatbot cannot access the audit trail through tools.

**Scope limitation**: Only `query_events` and `verify_chain` are dispatched. The existing
7 snowball schema methods (DR snapshots, retention, rollback) remain un-wired — that
work belongs to the paused schema-methods sweep. The new dispatch arm checks the method
name and only handles the two new methods; all others fall through to the catch-all echo.

**Method signatures**:

```
query_events:
  Effect: Read
  Capability: snowball.read
  Subid: obs.service.snowball.events.query@v1
  Input:  QueryEventsInput { plugin_id?, from_event_id?, to_event_id?, limit?, decision? }
  Output: QueryEventsOutput { events: [AuditEventRecord], has_more, total_in_chain }

verify_chain:
  Effect: Read
  Capability: snowball.read
  Subid: obs.service.snowball.chain.verify@v1
  Input:  VerifyChainInput { from_event_id?, to_event_id? }
  Output: VerifyChainOutput { valid, events_verified, errors: [String] }
```

**Rejected alternative — new dedicated `accountability` plugin**:
Would add a new plugin file, new schema function, new blob. Over-engineering when
`snowball_plugin.rs` already exists and is explicitly designated for this.

**Rejected alternative — no D-Bus methods, only gRPC**:
Violates the "D-Bus is the only control plane" principle. Leaves MCP clients and
`zcall` operators without a path to the audit data.

---

### DQ-3: The zeroclaw-gui module boundary

**Decision**: New `crates/zeroclaw-gui/src/accountability/` module with:

```
accountability/
├── mod.rs          — pub mod declarations, re-exports
├── store.rs        — AccountabilityStore (event page + filters + pagination state)
├── transport.rs    — AccountabilityTransport (tonic EventChainService client)
└── view.rs         — render_accountability() (egui rendering)
```

The GUI uses the gRPC `EventChainService.GetEvents` path (direct, efficient, streaming).
This is the same pattern as the chat view calling `ChatService` directly via gRPC.
The D-Bus `query_events` method provides the same data for non-GUI consumers (MCP, zcall,
AI agents) — the two paths read the same underlying `EventChain`.

**ExplorerState integration**: `ExplorerState` gains:
```rust
pub accountability_store: crate::accountability::store::AccountabilityStore,
```

**Route dispatcher change** (`views/mod.rs:51`):
```rust
Route::Accountability => accountability::view::render_accountability(
    ui, &mut explorer.accountability_store, ctx
),
r => stub(ui, route_title(r), description(r)),
```

**Decoupling proof**: The `accountability` module does NOT import: `crate::chat`,
`op_chat`, `ChatStore`, `ChatTransport`, or any chat proto type.

**Rejected alternative — shared transport module for both chat and accountability**:
Would couple the two paths. They talk to different gRPC services.

---

### DQ-4: Minimal v1 PII review definition

**Decision**: v1 "PII review" = human-readable display of all `ChainEvent` metadata fields
in the Accountability view. No automation.

**What the operator sees for each event row**:
- Summary columns: timestamp, actor_id, plugin_id, method_name, decision (Allow/Deny).
- Expandable detail: all proto `ChainEvent` fields rendered as formatted JSON.
- The `json_args_footprint` (Blake3 hash) and `input_patch_hash` are displayed.

**Why hashes, not raw payloads**: The proto `ChainEvent` message does NOT carry the raw
`json_args` string — only its Blake3 hash. This is by design: the event chain is an
accountability ledger, not a payload store.

**What is explicitly deferred**:
- Automated PII detection on payloads.
- Integration with `ctl_plane_chatbot.rs`'s `pii_flagged` field.
- A "flag as PII concern" write-back action from the UI.

---

### DQ-5: snowball dispatch arm scope boundary

**Decision**: The new `"snowball"` dispatch arm in `MutationEngine::dispatch_method_call`
handles ONLY `query_events` and `verify_chain`. All other snowball methods (the existing
7: `list_snapshots`, `get_snapshot`, `create_snapshot`, `rollback`, `get_current_state`,
`set_retention`, `get_stats`) explicitly fall through to the catch-all echo.

**Implementation pattern**:
```rust
"snowball" => {
    match method {
        "query_events" => dispatch_snowball_query_events(&self.event_chain, json_args).await?,
        "verify_chain" => dispatch_snowball_verify_chain(&self.event_chain, json_args).await?,
        _ => serde_json::to_value(&parsed_value).unwrap_or(serde_json::Value::Null),
    }
}
```

The inner `_ =>` reproduces the outer catch-all behavior for the un-wired methods. This
ensures calling `list_snapshots` today produces the same echo behavior as before — no
regression, no silent breakage.

**Rationale**: The existing 7 methods require `StreamingSnowball` DR operations (real
snapshot creation, rollback, retention policy changes) which are complex and unrelated
to the audit-query concern. Bundling them would make this spec un-reviewable and risks
the demo timeline.

**Rejected alternative — full snowball dispatch arm**:
Out of scope. The 7 existing methods are complex DR operations that need their own
verification. They belong to the paused schema-methods sweep.

---

## Affected Files

| File | Change type |
|------|-------------|
| `crates/op-plugins/src/state_plugins/snowball_plugin.rs` | **Modify** — add `query_events` + `verify_chain` methods + Input/Output structs |
| `crates/op-grpc-bridge/src/mutation_engine.rs` | **Modify** — add `"snowball"` dispatch arm (scoped to 2 methods) |
| `crates/op-grpc-bridge/src/mutation_engine.rs` | **Modify** — add `add_footprint` durability call |
| `crates/op-grpc-bridge/src/mutation_engine.rs` | **Modify** — add startup chain rebuild from timing_subvol |
| `crates/op-grpc-bridge/Cargo.toml` | **Verify** — already depends on `op-snowball` (if not, add) |
| `crates/op-state-store/src/event_chain.rs` | **Modify** — add `replay_from_footprint` method |
| `crates/zeroclaw-gui/src/accountability/mod.rs` | **New** — module declaration |
| `crates/zeroclaw-gui/src/accountability/store.rs` | **New** — AccountabilityStore |
| `crates/zeroclaw-gui/src/accountability/transport.rs` | **New** — gRPC client |
| `crates/zeroclaw-gui/src/accountability/view.rs` | **New** — egui render function |
| `crates/zeroclaw-gui/src/main.rs` | **Modify** — `mod accountability;` |
| `crates/zeroclaw-gui/src/views/mod.rs` | **Modify** — Route::Accountability arm, ExplorerState field |
| `crates/zeroclaw-gui/build.rs` | **Modify** — include `operation.proto` for client codegen |

---

## Exact Signatures and JSON Shapes

### New schema methods (snowball_plugin.rs)

```rust
/// Input for `query_events` — paginated audit trail query.
///
/// OSCAL subid: sch.service.snowball.query-events-input@v1
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct QueryEventsInput {
    /// Filter by plugin_id (empty/None = all plugins).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    /// Return events with event_id >= this value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_event_id: Option<u64>,
    /// Return events with event_id <= this value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_event_id: Option<u64>,
    /// Max events to return. Default 50, max 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Filter by decision: "allow", "deny", or None for all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
}

/// A single event record in the query output.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AuditEventRecord {
    pub event_id: u64,
    pub event_hash: String,
    pub prev_hash: String,
    pub timestamp: String, // ISO 8601
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

/// Output for `query_events`.
///
/// OSCAL subid: sch.service.snowball.query-events-output@v1
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct QueryEventsOutput {
    pub events: Vec<AuditEventRecord>,
    pub has_more: bool,
    pub total_in_chain: u64,
}

/// Input for `verify_chain` — hash chain integrity check.
///
/// OSCAL subid: sch.service.snowball.verify-chain-input@v1
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VerifyChainInput {
    /// Verify from this event_id (0 or None = from genesis).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_event_id: Option<u64>,
    /// Verify to this event_id (0 or None = to latest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_event_id: Option<u64>,
}

/// Output for `verify_chain`.
///
/// OSCAL subid: sch.service.snowball.verify-chain-output@v1
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VerifyChainOutput {
    pub valid: bool,
    pub events_verified: u64,
    pub errors: Vec<String>,
}
```

Schema registration (inside `snowball_schema()` function):
```rust
schema.methods.insert(
    "query_events".to_string(),
    method_decl_from_schemars_with_output::<QueryEventsInput, QueryEventsOutput>(
        "query_events",
        SideEffect::Read,
        true,
        "snowball.read",
        "obs.service.snowball.events.query@v1",
    ),
);
schema.methods.insert(
    "verify_chain".to_string(),
    method_decl_from_schemars_with_output::<VerifyChainInput, VerifyChainOutput>(
        "verify_chain",
        SideEffect::Read,
        true,
        "snowball.read",
        "obs.service.snowball.chain.verify@v1",
    ),
);
```

### MutationEngine dispatch arm (mutation_engine.rs)

```rust
"snowball" => {
    match method {
        "query_events" => {
            dispatch_snowball_query_events(&self.event_chain, &parsed_value).await?
        }
        "verify_chain" => {
            dispatch_snowball_verify_chain(&self.event_chain, &parsed_value).await?
        }
        // Existing 7 methods remain un-dispatched until schema-methods sweep.
        _ => serde_json::to_value(&parsed_value).unwrap_or(serde_json::Value::Null),
    }
}
```

Free functions:
```rust
async fn dispatch_snowball_query_events(
    event_chain: &Arc<RwLock<EventChain>>,
    args: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let input: QueryEventsInput = serde_json::from_value(args.clone())
        .unwrap_or_default();

    let limit = input.limit.unwrap_or(50).min(100) as usize;
    let chain = event_chain.read().await;
    let total = chain.events().len() as u64;

    let events: Vec<AuditEventRecord> = chain.events().iter()
        .filter(|e| input.from_event_id.map_or(true, |id| e.event_id >= id))
        .filter(|e| input.to_event_id.map_or(true, |id| e.event_id <= id))
        .filter(|e| input.plugin_id.as_ref().map_or(true, |p| p.is_empty() || e.plugin_id == *p))
        .filter(|e| input.decision.as_ref().map_or(true, |d| {
            match d.as_str() {
                "allow" => e.decision == Decision::Allow,
                "deny" => e.decision == Decision::Deny,
                _ => true,
            }
        }))
        .take(limit + 1) // take one extra to detect has_more
        .map(chain_event_to_record)
        .collect();

    let has_more = events.len() > limit;
    let events = if has_more { events[..limit].to_vec() } else { events };

    Ok(serde_json::to_value(QueryEventsOutput { events, has_more, total_in_chain: total })?)
}

async fn dispatch_snowball_verify_chain(
    event_chain: &Arc<RwLock<EventChain>>,
    args: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let input: VerifyChainInput = serde_json::from_value(args.clone())
        .unwrap_or_default();

    let chain = event_chain.read().await;
    let result = chain.verify_range(
        input.from_event_id.unwrap_or(0),
        input.to_event_id.unwrap_or(0),
    );

    Ok(serde_json::to_value(VerifyChainOutput {
        valid: result.valid,
        events_verified: result.events_verified,
        errors: result.errors,
    })?)
}

fn chain_event_to_record(event: &ChainEvent) -> AuditEventRecord {
    AuditEventRecord {
        event_id: event.event_id,
        event_hash: event.event_hash.clone(),
        prev_hash: event.prev_hash.clone(),
        timestamp: event.timestamp.to_rfc3339(),
        actor_id: event.actor_id.clone(),
        capability_id: event.capability_id.clone().unwrap_or_default(),
        plugin_id: event.plugin_id.clone(),
        method_name: event.method_name.clone().unwrap_or_default(),
        operation_type: format!("{:?}", event.op),
        target: event.target.clone(),
        tags_touched: event.tags_touched.clone(),
        decision: format!("{:?}", event.decision),
        input_patch_hash: event.input_patch_hash.clone(),
        result_effective_hash: event.result_effective_hash.clone().unwrap_or_default(),
    }
}
```

### MutationEngine durability integration (mutation_engine.rs)

```rust
// After record_method_call at line 525 / 971:
{
    let footprint = event_to_footprint(chain_event);
    if let Some(ref snowball) = self.streaming_snowball {
        if let Err(e) = snowball.add_footprint(footprint).await {
            tracing::warn!("audit durability write failed: {e}");
        }
    }
}

fn event_to_footprint(event: &ChainEvent) -> PluginFootprint {
    let mut metadata = HashMap::new();
    metadata.insert("actor_id".into(), simd_json::json!(event.actor_id));
    metadata.insert("capability_id".into(), simd_json::json!(event.capability_id));
    metadata.insert("method_name".into(), simd_json::json!(event.method_name));
    metadata.insert("event_hash".into(), simd_json::json!(event.event_hash));
    metadata.insert("decision".into(), simd_json::json!(format!("{:?}", event.decision)));

    PluginFootprint {
        plugin_id: event.plugin_id.clone(),
        operation: event.method_name.clone().unwrap_or_else(|| "unknown".into()),
        timestamp: event.timestamp.timestamp_millis() as u64,
        data_hash: event.input_patch_hash.clone(),
        content_hash: event.event_hash.clone(),
        metadata,
        vector_features: vec![],
    }
}
```

---

## OSCAL Subid Assignments

| Item | Subid |
|------|-------|
| `AccountabilityStore` (view surface) | `exp.software.zeroclaw.accountability.render@v1` |
| `AccountabilityTransport` (gRPC client) | `obs.service.event-chain.query@v1` |
| `event_to_footprint` (durability write) | `evt.service.event-chain.persist@v1` |
| `rebuild_chain_from_disk` (startup) | `src.service.event-chain.rebuild@v1` |
| `render_accountability` (egui view) | `exp.software.zeroclaw.accountability.view@v1` |
| `query_events` schema method | `obs.service.snowball.events.query@v1` |
| `verify_chain` schema method | `obs.service.snowball.chain.verify@v1` |
| `QueryEventsInput` struct | `sch.service.snowball.query-events-input@v1` |
| `QueryEventsOutput` struct | `sch.service.snowball.query-events-output@v1` |
| `VerifyChainInput` struct | `sch.service.snowball.verify-chain-input@v1` |
| `VerifyChainOutput` struct | `sch.service.snowball.verify-chain-output@v1` |

---

## Communication Flow

```
PATH 1 — GUI (gRPC, efficient for streaming/pagination):

┌─────────────────────────────────────────────────────────────────┐
│  zeroclaw-gui (Accountability tab)                               │
│  AccountabilityTransport::fetch_page(channel, filter, tx)        │
│       │  gRPC: EventChainService.GetEvents(GetEventsRequest)      │
└───────┼───────────────────────────────────────────────────────────┘
        ▼
┌─────────────────────────────────────────────────────────────────┐
│  op-grpc-bridge (gRPC server)                                     │
│  EventChainService::get_events() → reads event_chain              │
└───────────────────────────────────────────────────────────────────┘

PATH 2 — MCP/D-Bus (PluginV1.Call, for agents and zcall):

  ./bin/zcall snowball query_events -a '{"limit":10,"plugin_id":"zeroclaw"}'
       │  D-Bus: PluginV1.Call("query_events", '{"limit":10,...}')
       │  on /org/opdbus/v1/plugins/snowball
       ▼
  op-grpc-bridge → schema_router validates + capability check
       │  MutationEngine::dispatch_method_call("snowball", "query_events", ...)
       │  → dispatch_snowball_query_events(&event_chain, args)
       │  → reads same Arc<RwLock<EventChain>>
       ▼
  Returns JSON: {"events":[...],"has_more":false,"total_in_chain":42}

Both paths read the SAME in-memory EventChain — consistency is guaranteed.

WRITE PATH (how events get into the chain):

  Any PluginV1.Call → schema_router validates
       ▼
  MutationEngine::dispatch_method_call()
       ├─► event_chain.write().record_method_call(...)  ← in-memory
       ├─► streaming_snowball.add_footprint(...)      ← on-disk (timing_subvol)
       └─► actual method dispatch → result
```

---

## What Does NOT Change

| Item | Reason |
|------|--------|
| `crate::chat` (store/transport/view) | Decoupled — Accountability has its own module |
| `ChatService` / `ChatServiceImpl` | Chat path untouched |
| `zeroclaw` plugin dispatch arm | Not involved |
| `snowball_plugin.rs` existing 7 methods | Declarations stay; dispatch deferred |
| `EventChainService` proto definition | Already has everything needed |
| `EventChainService` server implementation | Already handles GetEvents correctly |
| `op-state-store/src/event_chain.rs` structs | May add `replay_from_footprint` but existing API stays |
| Nav entry in `nav.rs:104` | Already declares Route::Accountability |
| `stub()` function in `views/mod.rs` | Other routes still use it |
| Any file under `crates/op-chat/` | Chat is independent |
| `op-gemma/src/ui_gallery.rs` | Explicitly paused |
| The 13-plugin schema-methods sweep | Explicitly paused |
