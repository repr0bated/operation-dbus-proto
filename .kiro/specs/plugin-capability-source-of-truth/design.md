# Design Document: Plugin as Sole Source of Truth + Full Capability Model

## Overview

This document describes the architecture, data flow, type changes, and migration
path for establishing the plugin as the sole source of truth for every object in
the control plane.

The core invariant: **No plugin → no schema → no object.** The `PluginSchema`
is the object's complete interface contract. Every D-Bus object, gRPC route,
SHM entry, and capability enforcement point derives from one schema, computed
in one place, owned by one process.

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────────────┐
│  Plugin Definitions                                                        │
│  crates/op-plugins/src/state_plugins/plugin_schema_defs.rs               │
│                                                                            │
│  fn wireguard_plugin_schema() -> PluginSchema { ... }                     │
│    .methods: { "SetKey": MethodDecl, "GetStatus": MethodDecl }            │
│    .signals: [ SignalDecl { name: "KeyRotated", ... } ]                   │
│    .guarantees: { supports_rollback: true, ... }                          │
│    .fields: { ... }                                                        │
│    .subids: { "SetKey": "mut.network.wireguard.set-key@v1", ... }         │
└──────────────────────────┬───────────────────────────────────────────────┘
                           │ schema() call at startup
                           ▼
┌──────────────────────────────────────────────────────────────────────────┐
│  PRODUCER: op-projection                                                   │
│                                                                            │
│  Reads plugin registrations (StatePlugin trait)                           │
│  Computes full capability schema + present-state snapshot                 │
│                                                                            │
│  Writes (atomic):                                                          │
│    /dev/shm/opdbus/schemas/<plugin_id>.json   ← capability schema         │
│    /dev/shm/live-schema.json                   ← combined monolith         │
│    /dev/shm/opdbus/state/<plugin_id>.json     ← present-state             │
│    /dev/shm/opdbus/.manifest.json             ← { catalog_hash }          │
│                                                                            │
│  Does NOT: claim org.opdbus.v1, register D-Bus objects, serve state       │
└──────────────────────────┬───────────────────────────────────────────────┘
                           │ SHM (direct read)
                           ▼
┌──────────────────────────────────────────────────────────────────────────┐
│  BRIDGE: op-grpc-bridge  (sole owner of org.opdbus.v1)                   │
│                                                                            │
│  Reads SHM once at startup; re-reads on manifest hash change              │
│  Registers SchemaBackedInterface per plugin on:                           │
│    /org/opdbus/v1/plugins/<plugin_id>                                     │
│    interface: org.opdbus.v1.Plugin.<PluginName>                           │
│                                                                            │
│  On inbound call:                                                          │
│    1. GhostbridgeInterceptor → extract footprint + session_id             │
│    2. Validate method ∈ PluginSchema.methods  (Req 6)                     │
│    3. Validate json_args against MethodDecl.args  (Req 6.3)               │
│    4. Enforce capability_id  (Req 7)                                       │
│    5. SchemaEngine.mutate(plugin_id, method, json_args,                   │
│                           capability_id, actor_id)  (Req 5)               │
│    6. Return result to caller                                              │
│                                                                            │
│  gRPC surface: every MethodDecl auto-exposed via SchemaRouter             │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## Data Flow: Plugin → Producer → SHM → Bridge → gRPC/D-Bus

### Step 1 — Plugin declares capability surface (compile-time)

`plugin_schema_defs.rs` returns a `PluginSchema` with `methods`, `signals`,
`guarantees`, `fields`, and `subids`. This is the only place a plugin's
interface contract is defined.

### Step 2 — Producer writes SHM (startup + on-change)

`op-projection` iterates `StatePlugin::schema()` for every registered plugin,
serializes the `PluginSchema` (now including `methods`, `signals`, `guarantees`)
and writes the SHM layout. The final step is an atomic write of `.manifest.json`
with the `catalog_hash`. Consumers detect staleness by comparing the stored hash
with the one in `.manifest.json`.

### Step 3 — Bridge bootstraps from SHM (startup)

`op-grpc-bridge` reads `/dev/shm/live-schema.json`, deserializes each entry
into a `PluginSchema`, and calls `SchemaRouter::register_objects` to mount one
`SchemaBackedInterface` per plugin on the system bus. The bridge requests the
well-known name `org.opdbus.v1` at this point.

### Step 4 — Bridge handles inbound call (runtime)

A gRPC client (or D-Bus peer) calls a method. The `GhostbridgeInterceptor`
extracts the footprint. The bridge runs the validation + capability enforcement
pipeline (see Architecture diagram) before calling `SchemaEngine.mutate`.

### Step 5 — Mutation recorded

`SchemaEngine.mutate` applies the change, writes a `StateChange` to the
`EventChain` (with `actor_id`, `capability_id`, Blake3 footprint), and broadcasts
the change to any registered `StatePublisher` listeners (e.g. for Qdrant
semantic indexing).

---

## Capability Schema Format (JSON)

A serialized `PluginSchema` written to SHM will include the following new keys.
The existing keys (`name`, `category`, `version`, `description`, `fields`,
`dependencies`, `immutable_paths`, `tags`, `dialect`, `mutation_index`, `subids`)
are unchanged.

```jsonc
{
  "name": "wireguard",
  "version": "1.0.0",
  // ... existing fields ...

  "methods": {
    "SetKey": {
      "name": "SetKey",
      "args": {
        "type": "object",
        "properties": {
          "public_key": { "type": "string", "minLength": 44 },
          "private_key": { "type": "string" }
        },
        "required": ["public_key"]
      },
      "returns": {
        "type": "object",
        "properties": {
          "applied": { "type": "boolean" }
        }
      },
      "side_effect": "mutation",
      "idempotent": true,
      "required_capability": "wireguard.set-key",
      "subid": "mut.network.wireguard.set-key@v1"
    },
    "GetStatus": {
      "name": "GetStatus",
      "args": { "type": "object", "properties": {} },
      "returns": {
        "type": "object",
        "properties": {
          "connected": { "type": "boolean" },
          "endpoint": { "type": "string" }
        }
      },
      "side_effect": "read",
      "idempotent": true,
      "required_capability": null,
      "subid": "obs.network.wireguard.get-status@v1"
    }
  },

  "signals": [
    {
      "name": "KeyRotated",
      "payload": {
        "type": "object",
        "properties": {
          "new_public_key": { "type": "string" }
        }
      },
      "subid": "evt.network.wireguard.key-rotated@v1"
    }
  ],

  "guarantees": {
    "supports_rollback": true,
    "supports_checkpoints": false,
    "supports_verification": true,
    "atomic_operations": true
  }
}
```

---

## Rust Type Changes

### `PluginSchema` (op-state-store)

Add three fields to `PluginSchema`:

```rust
// in crates/op-state-store/src/plugin_schema.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodDecl {
    pub name: String,
    pub args: serde_json::Value,    // JSON Schema object
    pub returns: Option<serde_json::Value>,
    pub side_effect: SideEffect,
    pub idempotent: bool,
    pub required_capability: Option<String>,
    pub subid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffect {
    Read,
    Mutation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalDecl {
    pub name: String,
    pub payload: Option<serde_json::Value>,
    pub subid: String,
}

// Guarantee flags — the single canonical definition
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginCapabilities {
    pub supports_rollback: bool,
    pub supports_checkpoints: bool,
    pub supports_verification: bool,
    pub atomic_operations: bool,
}

// Added to PluginSchema:
pub struct PluginSchema {
    // ... existing fields unchanged ...
    #[serde(default)]
    pub methods: HashMap<String, MethodDecl>,
    #[serde(default)]
    pub signals: Vec<SignalDecl>,
    #[serde(default)]
    pub guarantees: PluginCapabilities,
}
```

### `Plugin` trait (op-plugins)

`handle_command` is deprecated; `dispatch_method` is the replacement:

```rust
// Replaces: async fn handle_command(&self, command: &str, args: Value) -> Result<Value>
async fn dispatch_method(&self, method: &str, args: serde_json::Value) -> Result<serde_json::Value> {
    Err(anyhow::anyhow!("Method '{}' not implemented by plugin '{}'", method, self.name()))
}
```

The bridge calls `dispatch_method` only after schema validation and capability
enforcement have passed.

### `PluginCapabilities` deduplication

- Remove `pub struct PluginCapabilities` from `crates/op-state/src/plugin.rs`
  (4-field version).
- Remove `pub struct PluginCapabilities` from `crates/op-plugins/src/plugin.rs`
  (8-field version with `can_read`, `can_write`, etc.).
- All callers use `op_state_store::PluginCapabilities` (the 4-field guarantee
  struct, now also embedded as `PluginSchema.guarantees`).
- The 8-field access-control fields (`can_read`, `can_write`, `can_delete`,
  `requires_root`, `supported_platforms`) are superseded by `MethodDecl
  .required_capability` per method.

### `SchemaEngine.mutate` signature (op-grpc-bridge)

Rename `_capability_id` to `capability_id` and surface it in the event chain:

```rust
pub async fn mutate(
    &self,
    plugin_id: &str,
    method: &str,
    json_args: &str,
    capability_id: Option<&str>,
    actor_id: &str,
) -> Result<serde_json::Value, MutationError>
```

### `SchemaBackedInterface::call` (op-grpc-bridge)

Remove the `{"success": true}` stub. After schema validation and capability
check, call `self.engine.mutate(...)` and return the result:

```rust
async fn call(&self, method: String, json_args: String) -> zbus::fdo::Result<String> {
    // 1. Validate method in schema
    // 2. Validate json_args against MethodDecl.args
    // 3. Enforce capability_id
    // 4. self.engine.mutate(...)
    // 5. serde_json::to_string(&result)
}
```

---

## SHM Layout (canonical)

```
/dev/shm/
├── live-schema.json                         ← combined monolith (all plugins)
└── opdbus/
    ├── .manifest.json                       ← { "catalog_hash": "<blake3>" }
    ├── schemas/
    │   ├── wireguard.json
    │   ├── xray.json
    │   └── ...                              ← one file per plugin
    └── state/
        ├── wireguard.json                   ← present-state snapshot
        ├── xray.json
        └── ...
```

The manifest is the only file consumers need to poll-free-detect staleness.
The bridge compares the hash it has cached against the one in `.manifest.json`
at the start of each inbound connection.

---

## D-Bus Ownership Handoff Sequence

The following describes the live-bus transition from the current three-registrar
state to the single-bridge-owner state. This is a zero-downtime migration
performed by s6 service restart ordering.

```
Current state (before):
  PID 2194  org.opdbus.v1.plugins  (op-projection)
  op-dbus-mirror              → also registers /org/opdbus/v1/plugins/* objects
  op-grpc-bridge              → also registers /org/opdbus/v1/plugins/* objects
  op-openvswitch-daemon       → claims org.opdbus.v1 (bare)

Target state (after):
  op-grpc-bridge              → org.opdbus.v1 (sole owner)
                                /org/opdbus/v1/plugins/* (sole registrar)
  op-dbus-mirror              → org.opdbus.v1.mirror (mirror-management only)
  op-openvswitch-daemon       → org.opdbus.v1.plugins.ovsdb only
  op-projection               → no bus name; SHM producer only
```

Migration order (s6 controlled):

1. **Deploy op-projection changes** (no bus name, writes SHM). Restart service.
   Bus state is unchanged; SHM now has canonical schema+state.

2. **Deploy op-dbus-mirror changes** (remove plugin-object registration, remove
   `org.opdbus.v1` name claim). Restart service. Plugin objects may briefly be
   unregistered on the bus — acceptable during migration window.

3. **Deploy op-grpc-bridge changes** (request `org.opdbus.v1`, read SHM,
   register all plugin objects, real dispatch). Restart service. Bridge now
   owns the bus name and all objects.

4. **Deploy op-openvswitch-daemon changes** (remove bare name claim). Restart.

5. **Verify** with `busctl list | grep opdbus` that only `org.opdbus.v1` (bridge)
   and `org.opdbus.v1.plugins.ovsdb` appear.

---

## Capability Enforcement Pipeline (runtime detail)

```
Inbound gRPC call
      │
      ▼
GhostbridgeInterceptor
  → X-Ghostbridge-Footprint       → ctx.footprint
  → X-Ghostbridge-Trace-ID        → ctx.session_id
      │
      ▼
SchemaBackedInterface::call(method, json_args)
  1. schema = SHM cache[plugin_id]
  2. decl = schema.methods.get(method)
         → None  → UnknownMethod / NOT_FOUND
  3. validate json_args against decl.args (jsonschema crate)
         → Err   → InvalidArgs / INVALID_ARGUMENT
  4. if decl.required_capability.is_some():
       check ctx.footprint grants capability
         → Deny  → AccessDenied / PERMISSION_DENIED
  5. engine.mutate(plugin_id, method, json_args,
                   decl.required_capability, ctx.footprint)
  6. return Ok(serde_json::to_string(&result))
```

---

## Plugin Autogeneration Lifecycle (Requirement 12)

### State Machine

```
                    ┌─────────────────────────────────────────────────────────┐
                    │  Unknown plugin name referenced                          │
                    └──────────────────────────┬──────────────────────────────┘
                                               │ idempotency check (Req 12.2)
                                               ▼
                    ┌──────────────────────────────────────────────────────┐
                    │  pending_research                                     │
                    │  draft created, written to CozoDB (Req 12.16)        │
                    └────────────────────────┬─────────────────────────────┘
                                             │  Gemma.ResearchCapabilitySurface
                                             │  (Req 12.4, 13.1–13.3)
                           ┌─────────────────┼────────────────────┐
                           ▼ error           │ ok                 ▼ structural validation fails
                ┌──────────────────┐         │           ┌─────────────────────┐
                │ research_failed  │         ▼           │  synthesis_invalid  │
                │ pending_human_   │  CapabilitySurface  │  pending_human_     │
                │ review: true     │  Draft assembled    │  review: true       │
                └──────┬───────────┘  (Req 12.7–12.8)   └────────┬────────────┘
                       │ manual retry                             │ manual retry
                       │ or RequestRevision                       │ or RequestRevision
                       └──────────────┐   ┌──────────────────────┘
                                      ▼   ▼
                    ┌─────────────────────────────────────────────────────────┐
                    │  draft_pending_review                                    │
                    │  QUARANTINED — bridge does NOT register (Req 12.10)     │
                    └───────────────────────┬─────────────────────────────────┘
               ┌───────────────────────────┼────────────────────────────────┐
               │ RejectDraft               │ ApproveDraft                   │ RequestRevision
               ▼                           ▼                                ▼
  ┌─────────────────────┐   ┌──────────────────────────────┐  ┌─────────────────────┐
  │  rejected           │   │  approved                    │  │  pending_research   │
  │  revision allowed   │   │  persisted as PluginSchema   │  │  (Gemma re-invoked) │
  │  with new info      │   │  in plugin store (Req 12.11) │  └─────────────────────┘
  │  (Req 12.3, 12.12)  │   │                              │
  └─────────────────────┘   │  op-projection picks up      │
                            │  writes SHM (Req 12.14)      │
                            │  bridge registers live object │
                            │  (Req 12.15)                 │
                            └──────────────────────────────┘
```

### Quarantine Rule

A draft in any state other than `approved` is quarantined. The bridge, on
reading `live-schema.json` from SHM, MUST only serve plugins whose definition
comes from the approved plugin store. The autogeneration draft store is separate
from the canonical plugin registry. Approval is the only gate that moves a
schema from draft store → plugin store → SHM → live bridge object.

### `CapabilitySurfaceDraft` Shape

The JSON returned by `Gemma.ResearchCapabilitySurface` and stored in the draft:

```jsonc
{
  "methods": [
    {
      "name": "GetStatus",
      "args": { "type": "object", "properties": {} },
      "returns": { "type": "object" },
      "side_effect": "read",
      "idempotent": true,
      "required_capability": null,
      "subid": "obs.service.<plugin>.get-status@v1"
    }
  ],
  "properties": {
    "status": { "field_type": "String", "required": true, "description": "..." }
  },
  "signals": [
    { "name": "StateChanged", "payload": { "type": "object" }, "subid": "evt.service.<plugin>.state-changed@v1" }
  ],
  "guarantees": {
    "supports_rollback": false,
    "supports_checkpoints": false,
    "supports_verification": true,
    "atomic_operations": false
  },
  "subids": { "GetStatus": "obs.service.<plugin>.get-status@v1" },
  "tags": ["auto-generated"]
}
```

### Durable Draft Store

Drafts are persisted in CozoDB under the `auto_gen_drafts` relation. Fields:
`plugin_id` (key), `revision` (int), `status` (string), `requested_info`,
`capability_surface_draft` (JSON), `review_reason`, `pending_human_review`
(bool), `created_at`, `updated_at`. This survives service restarts (Req 12.16).

---

## Gemma Plugin Architecture (Requirement 13)

### Gemma as a First-Class StatePlugin

Gemma is registered in `default_registry.rs` as `Arc::new(GemmaPlugin::new())`
under the key `"gemma"`. Its object path is `/org/opdbus/v1/plugins/gemma`.
Its schema is defined exclusively in `plugin_schema_defs.rs` as
`gemma_plugin_schema()`, following the one-schema-file rule.

```
┌──────────────────────────────────────────────────────────────────────────┐
│  GemmaPlugin  (StatePlugin impl)                                         │
│  path: /org/opdbus/v1/plugins/gemma                                      │
│                                                                           │
│  Declared methods (all in gemma_plugin_schema() MethodDecl map):         │
│    ClassifySubid          obs.service.gemma.classify-subid@v1            │
│    RouteByTags            obs.service.gemma.route-by-tags@v1             │
│    GenerateUiPerspectives obs.service.gemma.generate-ui-perspectives@v1  │
│    ResearchCapabilitySurface obs.service.gemma.research-capability@v1    │
└──────────────────────────────────────────────────────────────────────────┘
         ▲                                    ▲
         │ dispatch_method (bridge-validated) │
         │                                    │
  schema-renderer / subid-registry     autogeneration lifecycle
  existing callers                     (Req 12.4, query_elements_via_agent
                                        replacement — Req 13.1–13.2)
```

### Replacing `query_elements_via_agent`

The current seam in `auto_create.rs`:

```rust
// BEFORE (removed):
let agent = create_agent("search-specialist", agent_id)?;

// AFTER (replacement):
// Route through the bridge to the Gemma plugin's D-Bus object.
// SchemaEngine dispatch validates the method and enforces capability_id.
let result = schema_engine
    .mutate(
        "gemma",
        "ResearchCapabilitySurface",
        &serde_json::to_string(&json!({
            "plugin_name": name,
            "requested_info": requested_info,
        }))?,
        Some("gemma.research"),
        actor_id,
    )
    .await?;
```

`query_elements_via_agent` is deleted. The call goes through `SchemaEngine.mutate`
(the only valid dispatch path), which validates the method against Gemma's
declared `MethodDecl`, enforces `required_capability: "gemma.research"`, and
returns the `CapabilitySurfaceDraft` JSON.

### UI Perspective Methods

`GenerateUiPerspectives` takes a `plugin_id` and returns four rendering hints:

| Perspective      | Purpose                                      |
|-----------------|----------------------------------------------|
| Data/Numeric    | Chart types, numeric field mappings          |
| Spatial/Layout  | Grid/masonry/list column recommendations     |
| User Flow       | Form step ordering, action button placement  |
| Context/Aesthetic| Theme, density, animation mode              |

These were previously handled by `SchemaRendererPlugin` heuristics. Gemma
provides the reasoning; `SchemaRendererPlugin` continues to own the field-type
→ component mapping table. The UI renderer calls `Gemma.GenerateUiPerspectives`
for layout decisions, then applies `SchemaRendererPlugin`'s `field_mappings` for
component selection.

---

### Phase 1 — Schema types and deduplication (no runtime change)
- Add `MethodDecl`, `SignalDecl`, `PluginCapabilities` (4-field) to
  `op-state-store`.
- Add `methods`, `signals`, `guarantees` fields to `PluginSchema`.
- Remove duplicate `PluginCapabilities` from `op-state` and `op-plugins`.
- `cargo build --workspace` green.

### Phase 2 — Populate capability surface in schema defs
- For each of the 68 plugins in `plugin_schema_defs.rs`, add `methods`,
  `signals`, and `guarantees` declarations.
- Add subids for every new method and signal.
- `cargo test --workspace` green.

### Phase 3 — Producer writes full schema to SHM
- `op-projection`: write per-plugin capability JSON, monolith, and manifest.
- Remove D-Bus object registration from `op-projection`.
- `cargo build -p op-projection` green.

### Phase 4 — Bridge becomes sole owner
- `op-grpc-bridge`: request `org.opdbus.v1`, read SHM, register full
  `SchemaBackedInterface` tree with real `dispatch_method` call path.
- Replace `{"success": true}` stub with `SchemaEngine.mutate`.
- Add capability enforcement pipeline.
- `op-dbus-mirror`: remove plugin-object registration and `org.opdbus.v1` claim.
- `op-openvswitch-daemon`: remove bare name claim.
- `op-state`: remove dead name claim.

### Phase 5 — Enforcement and cleanup
- Rename `_capability_id` → `capability_id` everywhere.
- Add `jsonschema` validation of `json_args` against `MethodDecl.args`.
- Delete dead `op-state` bus name code. Document orphan `opdbus` binary cleanup.
- CI subid uniqueness check.
- Full `cargo clippy` + `cargo test` clean.
