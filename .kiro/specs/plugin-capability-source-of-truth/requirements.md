# Requirements Document: Plugin as Sole Source of Truth + Full Capability Model

## Introduction

This feature establishes the plugin as the single, authoritative source of truth
for every object in the control plane. Today the system registers D-Bus objects,
gRPC routes, and present-state projections through three separate registrars,
the `PluginSchema` carries no method declarations, method dispatch is stringly-
typed and opaque, `PluginCapabilities` is duplicated across two crates, and
`capability_id` is plumbed but never enforced.

The outcome of this spec is a system where:
- A plugin declares its *complete* capability surface (methods, properties,
  signals, guarantees, and OSCAL classification) inside `PluginSchema` —
  the single source of truth.
- **op-projection** is the sole producer: it computes the full schema
  (capability surface + present-state) and emits it to SHM.
- **op-grpc-bridge** is the sole bridge owner: it reads SHM, registers the
  entire D-Bus/gRPC object tree, dispatches calls to `SchemaEngine.mutate`,
  validates every method against the schema, and enforces `capability_id`
  before execution.
- All redundant registrars, dead bus-name claims, and duplicate struct
  definitions are removed.

## Glossary

- **Plugin**: A Rust impl of the `StatePlugin` trait. The plugin is the only
  reason a D-Bus object, gRPC method, or SHM entry exists.
- **PluginSchema**: The `op-state-store` struct that is the single source of
  truth for a plugin's entire interface contract — fields, methods, signals,
  guarantees, subids, and OSCAL classification. Defined exclusively in
  `crates/op-plugins/src/state_plugins/plugin_schema_defs.rs`.
- **Capability Surface**: The complete, machine-enumerable set of everything an
  object can do: every method, property, signal, and guarantee it exposes.
- **MethodDecl**: A structured declaration of one method in the capability
  surface: name, argument schema, return schema, side-effect class, idempotency
  flag, and required `capability_id` to invoke.
- **SignalDecl**: A structured declaration of one emitted signal: name and
  payload schema.
- **PluginCapabilities**: Runtime guarantee flags for an object
  (`supports_rollback`, `supports_checkpoints`, `supports_verification`,
  `atomic_operations`). One definition, in `op-state-store`.
- **Producer**: `op-projection` — the only component that computes and writes
  schema and present-state to SHM. Reads plugin registrations; writes nothing
  to D-Bus.
- **Bridge**: `op-grpc-bridge` — the sole owner of `org.opdbus.v1` and the
  entire `/org/opdbus/v1/plugins/` object tree on the system bus.
- **SHM Layout**:
  - `/dev/shm/opdbus/schemas/<plugin_id>.json` — per-plugin capability schema.
  - `/dev/shm/live-schema.json` — combined monolith catalog (all plugins).
  - `/dev/shm/opdbus/.manifest.json` — `{ "catalog_hash": "<blake3>" }`.
  - `/dev/shm/opdbus/state/<plugin_id>.json` — present-state projection per plugin.
- **Catalog Hash**: A single Blake3 leaf+fold hash over all per-plugin schema
  files. Computed once by the producer; never recomputed by consumers.
- **SchemaEngine.mutate**: The real execution path in `op-grpc-bridge`'s
  `SchemaEngine` that applies a state mutation and appends it to the event
  chain. All method dispatch flows through this.
- **capability_id**: The caller's authorization token for invoking a method.
  Declared in `MethodDecl.required_capability`. Enforced at the bridge before
  `SchemaEngine.mutate` is called.
- **GhostbridgeInterceptor**: The gRPC interceptor that extracts
  `X-Ghostbridge-Footprint` / `X-Ghostbridge-Trace-ID` from metadata and
  attaches them as the caller's identity footprint.
- **The Sled**: 1:1 shared memory layout for identity state.
- **The Snowball**: The appended-only session ledger.
- **The Strike / Etch**: Blake3 hash computation for a footprint.
- **subid**: OSCAL operational taxonomy key in the format
  `<category>.<component-type>.<subject>.<verb>[.<facet>][@vN]`.
- **handle_command**: The stringly-typed catch-all on the `Plugin` trait that
  this spec replaces with a structured capability surface.

## Requirements

---

### Requirement 1: Plugin Trait Declares Full Capability Surface

**User Story:** As a control-plane developer, I want each plugin to declare its
complete capability surface in its `PluginSchema` so that every D-Bus object,
gRPC method, and enforcement point derives from exactly one schema definition
and nothing exists without one.

#### Acceptance Criteria

1. WHEN a `StatePlugin` is registered, THE `PluginSchema` returned by its
   `schema()` method SHALL contain a `methods` map, a `signals` array, and a
   `guarantees` block in addition to the existing `fields`, `tags`, `subids`,
   and `immutable_paths`.

2. WHERE the `methods` map is present, EACH entry SHALL be a `MethodDecl`
   containing: `name` (string), `args` (JSON Schema object describing
   parameters), `returns` (JSON Schema object or `null`), `side_effect`
   (`"read"` | `"mutation"`), `idempotent` (bool), and
   `required_capability` (string | null).

3. WHERE the `signals` array is present, EACH entry SHALL be a `SignalDecl`
   containing: `name` (string), `payload` (JSON Schema object or `null`), and
   `subid` (string | null).

4. THE `guarantees` block SHALL contain exactly the four boolean fields
   `supports_rollback`, `supports_checkpoints`, `supports_verification`, and
   `atomic_operations` — replacing both existing `PluginCapabilities` structs.

5. WHEN the `PluginSchema` is serialized to SHM, THE `methods`, `signals`, and
   `guarantees` keys SHALL be present in the emitted JSON (empty object / empty
   array / all-false block if unused) so that consumers can enumerate them
   without null-checking absent keys.

6. IF a plugin's `schema()` returns `None`, THE system SHALL NOT register a
   D-Bus object, gRPC route, or SHM entry for that plugin.

7. THE `handle_command(command: &str, args: Value)` method on the `Plugin`
   trait SHALL be deprecated and replaced by `dispatch_method(method: &str,
   args: Value) -> Result<Value>`, which receives only method names declared in
   `PluginSchema.methods`.

---

### Requirement 2: Single `PluginCapabilities` Definition

**User Story:** As a developer, I want `PluginCapabilities` defined exactly once
so that guarantee metadata is never contradictory between crates.

#### Acceptance Criteria

1. WHEN the codebase is compiled, THERE SHALL be exactly one definition of
   `PluginCapabilities` — in `crates/op-state-store/src/plugin_schema.rs`.

2. THE duplicate definition in `crates/op-state/src/plugin.rs` SHALL be
   removed and all callers SHALL reference `op_state_store::PluginCapabilities`.

3. THE duplicate definition in `crates/op-plugins/src/plugin.rs` SHALL be
   removed and all callers SHALL reference `op_state_store::PluginCapabilities`.

4. WHILE the `PluginSchema.guarantees` block serves as the per-object capability
   declaration, THE standalone `PluginCapabilities` struct SHALL be kept as the
   in-memory runtime view used by `SchemaEngine` for call validation, derived
   from `PluginSchema.guarantees` at load time.

---

### Requirement 3: Producer Emits Complete Capability Schema to SHM

**User Story:** As a system operator, I want `op-projection` to be the sole
writer of all schema and present-state data to SHM so that every consumer reads
from one authoritative surface without polling.

#### Acceptance Criteria

1. WHEN `op-projection` starts, IT SHALL iterate every registered `StatePlugin`,
   invoke `plugin.schema()`, and for plugins returning `Some(schema)` write the
   full capability schema JSON to `/dev/shm/opdbus/schemas/<plugin_id>.json`.

2. AFTER writing all per-plugin files, `op-projection` SHALL write the combined
   monolith to `/dev/shm/live-schema.json` containing all plugins keyed by
   plugin_id.

3. AFTER writing both per-plugin files and the monolith, `op-projection` SHALL
   compute a single Blake3 catalog hash (leaf hash over each sorted per-plugin
   file, folded into one root) and write `{ "catalog_hash": "<hex>" }` to
   `/dev/shm/opdbus/.manifest.json` atomically (write to `.manifest.json.tmp`
   then rename).

4. WHEN plugin state changes, `op-projection` SHALL write the updated present-
   state JSON to `/dev/shm/opdbus/state/<plugin_id>.json` and update the
   manifest hash.

5. THE catalog hash SHALL be computed by `op-projection` only. Consumers SHALL
   read `catalog_hash` from `.manifest.json`; they SHALL NOT recompute it from
   the schema files.

6. `op-projection` SHALL NOT request or hold the `org.opdbus.v1` well-known bus
   name, register D-Bus objects on the plugins path, or serve any D-Bus
   interface for plugin state.

---

### Requirement 4: Bridge Is Sole Owner of the Plugins Bus and Object Tree

**User Story:** As an operator, I want `op-grpc-bridge` to own `org.opdbus.v1`
and be the single registrar of all `/org/opdbus/v1/plugins/*` objects so that
there is no ambiguity about which process answers calls to a plugin object.

#### Acceptance Criteria

1. WHEN `op-grpc-bridge` starts, IT SHALL request the well-known bus name
   `org.opdbus.v1` on the system D-Bus. No other process in the workspace SHALL
   request this name.

2. WHEN the bridge holds `org.opdbus.v1`, IT SHALL read
   `/dev/shm/live-schema.json` (falling back to per-plugin files) and register
   one `SchemaBackedInterface` object per plugin at
   `/org/opdbus/v1/plugins/<plugin_id>`.

3. THE `SchemaBackedInterface` SHALL expose every `MethodDecl` from the plugin's
   capability surface as a callable D-Bus method on interface
   `org.opdbus.v1.Plugin.<PluginName>`.

4. THE `SchemaBackedInterface` SHALL expose every `SignalDecl` from the plugin's
   capability surface as an emittable D-Bus signal.

5. WHEN the bridge reads a new `catalog_hash` from `.manifest.json` (detected
   on inbound connection, not by polling), IT SHALL reload SHM and re-register
   changed objects without restarting the service.

6. THE `op-dbus-mirror` crate SHALL NOT register any object under
   `/org/opdbus/v1/plugins/`. It SHALL NOT claim the bus name `org.opdbus.v1`.
   It SHALL retain the `org.opdbus.v1.mirror` name for mirror-management
   interfaces only.

7. THE `op-openvswitch-daemon` crate SHALL NOT claim the bare name
   `org.opdbus.v1`. It SHALL use `org.opdbus.v1.plugins.ovsdb` exclusively.

8. THE `op-state` crate's D-Bus name claim for `org.opdbus.v1` SHALL be deleted.
   The crate has no s6 service and the name claim is dead code.

---

### Requirement 5: Real Method Dispatch Through SchemaEngine.mutate

**User Story:** As a developer, I want every method call received by the bridge
to be dispatched to `SchemaEngine.mutate` (not return a stub) so that mutations
are recorded in the event chain and reflected in state.

#### Acceptance Criteria

1. WHEN a D-Bus or gRPC caller invokes a method on a plugin object, THE bridge
   SHALL call `SchemaEngine.mutate(plugin_id, method_name, json_args,
   capability_id, actor_id)` and return its result.

2. THE stub `Ok(r#"{"success": true}"#)` in `SchemaBackedInterface::call` SHALL
   be removed and replaced with a real call to `SchemaEngine.mutate`.

3. IF `SchemaEngine.mutate` returns an error, THE bridge SHALL propagate the
   error to the caller as a `zbus::fdo::Error::Failed` (D-Bus) or a gRPC
   `Status::internal` respectively.

4. THE `json_args` argument to `SchemaEngine.mutate` SHALL be the verbatim JSON
   string passed by the caller, not a default or placeholder.

5. WHEN a mutation completes, `SchemaEngine.mutate` SHALL append an immutable
   event to the event chain (`EventChain.record`) including `actor_id`,
   `plugin_id`, `method_name`, `capability_id`, and a Blake3 footprint of
   `json_args`.

---

### Requirement 6: Method Validation Against Schema Capability Surface

**User Story:** As an operator, I want every method invocation validated against
the capability surface declared in the schema so that unknown methods are
rejected before dispatch.

#### Acceptance Criteria

1. WHEN a caller invokes a method, THE bridge SHALL look up the `MethodDecl` for
   that method name in `PluginSchema.methods` before dispatching.

2. IF the method name is not present in `PluginSchema.methods`, THE bridge SHALL
   return `zbus::fdo::Error::UnknownMethod` (D-Bus) or gRPC `NOT_FOUND` without
   calling `SchemaEngine.mutate`.

3. IF the caller's JSON arguments do not conform to `MethodDecl.args` (validated
   against the declared JSON Schema), THE bridge SHALL return
   `zbus::fdo::Error::InvalidArgs` or gRPC `INVALID_ARGUMENT`.

4. THE method validation SHALL use the schema loaded from SHM at bridge startup
   (refreshed on manifest hash change). It SHALL NOT re-read schema files on
   every call.

5. WHERE `PluginSchema.methods` is empty (no methods declared), ALL method
   invocations on that object SHALL be rejected as `UnknownMethod`.

---

### Requirement 7: capability_id Enforced at the Bridge

**User Story:** As a security engineer, I want `capability_id` enforced at the
bridge before any mutation executes so that callers without the required
capability cannot invoke privileged methods.

#### Acceptance Criteria

1. WHEN a gRPC call arrives, THE `GhostbridgeInterceptor` SHALL extract the
   identity footprint (`X-Ghostbridge-Footprint`) and session ID
   (`X-Ghostbridge-Trace-ID` / `X-WireGuard-Pubkey`) from request metadata and
   attach them to the request context.

2. WHEN the bridge resolves the `MethodDecl` for a method call, IT SHALL read
   `MethodDecl.required_capability`. If `required_capability` is non-null, IT
   SHALL verify the caller's footprint grants that capability.

3. IF the caller's identity footprint does not grant the `required_capability`,
   THE bridge SHALL return `zbus::fdo::Error::AccessDenied` (D-Bus) or gRPC
   `PERMISSION_DENIED` and SHALL NOT call `SchemaEngine.mutate`.

4. THE `_capability_id` parameter in `SchemaEngine.mutate` SHALL be renamed to
   `capability_id` and used in the event chain record and in the enforcement
   check (Requirements 7.2–7.3).

5. WHERE `MethodDecl.required_capability` is null, THE bridge SHALL allow
   invocation by any authenticated caller without a capability check.

6. THE capability enforcement SHALL happen at the bridge layer only. Plugin
   `dispatch_method` implementations SHALL trust that the bridge has already
   enforced the capability before calling them.

---

### Requirement 8: Remove Redundant Registrars and Dead Name Claims

**User Story:** As an operator, I want a single, unambiguous D-Bus ownership
model so that I can tell exactly which process answers a call to any plugin
object without reading four crates.

#### Acceptance Criteria

1. WHEN the workspace builds, THE `op-dbus-mirror` crate SHALL NOT contain any
   call to `request_name("org.opdbus.v1")` or any registration of objects under
   `/org/opdbus/v1/plugins/`.

2. WHEN the workspace builds, THE `op-state` crate SHALL NOT contain any
   `request_name("org.opdbus.v1")` call. The dead name claim SHALL be deleted.

3. WHEN the workspace builds, THE `op-openvswitch-daemon` crate SHALL NOT
   contain `request_name("org.opdbus.v1")`. It SHALL claim
   `org.opdbus.v1.plugins.ovsdb` only.

4. THE orphan `/usr/local/bin/opdbus` binary and any s6 service definition for
   it SHALL be identified and either reconciled to a crate or deleted via a
   follow-up deploy script. The spec SHALL document this as a required cleanup
   task.

5. AFTER this refactor, `cargo grep 'org.opdbus.v1"'` across all crates SHALL
   return exactly one `request_name` call site — in `op-grpc-bridge`.

---

### Requirement 9: Present-State in SHM, Not Peer-to-Peer D-Bus

**User Story:** As a developer, I want present-state projections to be written to
SHM by the producer and read from SHM by the bridge so that state propagation
is deterministic and does not rely on D-Bus push paths.

#### Acceptance Criteria

1. WHEN `op-projection` computes a present-state snapshot for a plugin, IT SHALL
   write it to `/dev/shm/opdbus/state/<plugin_id>.json`.

2. WHEN the bridge needs present-state for a plugin object (e.g. to answer a
   `GetProperties` call), IT SHALL read it from
   `/dev/shm/opdbus/state/<plugin_id>.json`.

3. THE bridge SHALL NOT watch for D-Bus `PropertiesChanged` signals from
   `op-projection` or any other component to obtain present-state. SHM is the
   authoritative channel.

4. THE bridge SHALL NOT poll SHM files on a timer. Present-state is read on
   inbound connection arrival or explicit method call, not proactively.

5. WHEN the present-state file for a plugin does not exist in SHM, THE bridge
   SHALL return an empty properties object for that plugin, not an error.

---

### Requirement 10: Subid Taxonomy on Every Capability

**User Story:** As a compliance officer, I want every method, signal, property,
and guarantee in the capability surface to carry an OSCAL subid so that the
entire functional surface can be mapped to compliance controls.

#### Acceptance Criteria

1. EACH `MethodDecl` in `PluginSchema.methods` SHALL carry a `subid` field
   following the taxonomy `<category>.<component-type>.<subject>.<verb>[@vN]`
   where `category` is one of `src`, `prj`, `sch`, `mut`, `obs`, `evt`, `exp`.

2. Methods with `side_effect = "mutation"` SHALL carry a subid with category
   `mut`. Methods with `side_effect = "read"` SHALL carry a subid with category
   `obs`.

3. EACH `SignalDecl` SHALL carry a subid with category `evt`.

4. THE `PluginSchema.subids` map SHALL register the subid for each method and
   signal by name (consistent with the existing per-field subid registration).

5. WHEN the schema is serialized, the `subids` map SHALL include an entry for
   every declared method and signal. CI SHALL enforce uniqueness of subids
   across the registry.

---

### Requirement 11: Schema Serialization Completeness for SHM

**User Story:** As a bridge developer, I want `PluginSchema` to serialize all
capability fields to JSON so that the SHM file is the complete, self-contained
contract the bridge needs with no out-of-band lookups.

#### Acceptance Criteria

1. WHEN `PluginSchema` is serialized with `serde_json`, THE output SHALL include
   `methods`, `signals`, and `guarantees` keys.

2. IF `methods` is empty, THE serialized JSON SHALL include `"methods": {}`.
   IF `signals` is empty, THE serialized JSON SHALL include `"signals": []`.
   IF `guarantees` is all-false, THE serialized JSON SHALL include
   `"guarantees": { "supports_rollback": false, ... }`.

3. THE `MethodDecl` and `SignalDecl` types SHALL derive `Serialize`,
   `Deserialize`, `Debug`, and `Clone`.

4. WHEN the bridge deserializes a per-plugin schema from SHM, IT SHALL be able
   to reconstruct the full capability surface — including methods, args schemas,
   required capabilities, signals, and guarantees — without consulting any
   in-process plugin registry.

---

### Requirement 12: Plugin Autogeneration Lifecycle

**User Story:** As a control-plane operator, I want an unknown plugin reference to
trigger a complete, governed autogeneration lifecycle — from Gemma-researched
capability synthesis through human/agent review to live projection — so that
the system can onboard new objects without manual schema authoring, while
guaranteeing that no draft is ever served until it is approved.

#### Acceptance Criteria

**Trigger and idempotency**

1. WHEN a component references a plugin name that has no registered `StatePlugin`
   and no approved definition in the plugin store, THE system SHALL initiate the
   autogeneration lifecycle for that name and set the draft status to
   `pending_research`.

2. IF a draft already exists for the same plugin name (in any non-terminal
   state), THE system SHALL NOT initiate a second autogeneration run. It SHALL
   return the existing draft's status to the caller.

3. WHERE the existing draft is in state `rejected`, THE system SHALL allow
   re-initiation only when the caller explicitly supplies revised `requested_info`,
   creating a new draft revision (`revision` counter incremented) and setting
   status back to `pending_research`.

**Research phase — Gemma**

4. WHEN status is `pending_research`, THE system SHALL invoke Gemma (via the
   `gemma_plugin` D-Bus object at `/org/opdbus/v1/plugins/gemma`) with the
   plugin name and `requested_info` to research the object's properties and
   synthesize its full capability surface.

5. THE research call to Gemma SHALL produce a structured
   `CapabilitySurfaceDraft` containing: `methods` (array of `MethodDecl`-shaped
   objects with `name`, `args`, `returns`, `side_effect`, `idempotent`,
   `required_capability`, `subid`), `properties` (field map), `signals` (array
   of `SignalDecl`-shaped objects), `guarantees` (4-bool block), `subids` (map),
   and `tags` (array).

6. IF Gemma is unavailable or returns an error, THE system SHALL set draft
   status to `research_failed`, record `review_reason` with the error detail,
   set `pending_human_review: true`, and halt the lifecycle at that point until
   a manual retry or re-initiation is requested.

**Synthesis phase**

7. WHEN Gemma returns a `CapabilitySurfaceDraft`, THE system SHALL construct a
   full `PluginSchema` from the draft (merging the synthesized capability surface
   with autogeneration metadata fields) and set status to `draft_pending_review`.

8. THE synthesized `PluginSchema` SHALL pass the same structural validation as a
   hand-authored schema: all `MethodDecl` entries must have a non-empty `subid`
   following the OSCAL taxonomy, `side_effect` must be `"read"` or `"mutation"`,
   and each `SignalDecl` must have a `subid` with category `evt`.

9. IF structural validation fails, THE system SHALL set draft status to
   `synthesis_invalid`, record the validation errors in `review_reason`, set
   `pending_human_review: true`, and NOT advance to review.

**Review and approval**

10. WHILE a draft's status is `draft_pending_review`, `research_failed`, or
    `synthesis_invalid`, THE bridge SHALL NOT register a D-Bus object, gRPC
    route, or SHM entry for that plugin name. The draft is quarantined.

11. WHEN a reviewer (human or authorized agent) calls `ApproveDraft` on the
    autogeneration manager with the draft's `plugin_id`, THE system SHALL:
    (a) persist the synthesized `PluginSchema` as the canonical plugin definition
    in the plugin store (the sole source of truth); (b) register the plugin as a
    live `StatePlugin`; (c) set draft status to `approved`.

12. WHEN a reviewer calls `RejectDraft` with an optional `reason`, THE system
    SHALL set draft status to `rejected`, record `reason` in `review_reason`, and
    remove the draft from the active autogeneration queue.

13. WHEN a reviewer calls `RequestRevision` with corrective `requested_info`,
    THE system SHALL set draft status to `pending_research` and re-enter the
    research phase (Gemma re-invoked) without incrementing the `plugin_id`.

**Persistence and projection**

14. WHEN a draft transitions to `approved`, `op-projection` SHALL detect the
    new plugin registration, write its capability schema to
    `/dev/shm/opdbus/schemas/<plugin_id>.json`, include it in
    `/dev/shm/live-schema.json`, and update `/dev/shm/opdbus/.manifest.json`.

15. WHEN the bridge detects the manifest hash change (per Requirement 4.5), IT
    SHALL register the new plugin object at `/org/opdbus/v1/plugins/<plugin_id>`
    and begin serving its capability surface without restart.

16. THE autogeneration draft store SHALL be durable (persisted in CozoDB, not
    in-memory only) so that drafts survive service restarts.

---

### Requirement 13: Gemma Owns Object-Property / Capability-Surface Research

**User Story:** As a control-plane architect, I want Gemma to be the single
schema-reasoning brain for all capability-surface work — subid classification,
tag routing, UI perspective generation, and now object-property research for
autogeneration — so that all schema reasoning is concentrated in one plugin and
never fanned out to generic or bespoke handlers.

#### Acceptance Criteria

**Gemma as the autogeneration research authority**

1. WHEN the autogeneration lifecycle reaches the `pending_research` state, THE
   system SHALL invoke Gemma's `ResearchCapabilitySurface` method (declared in
   Gemma's `PluginSchema.methods`) and SHALL NOT call `create_agent("search-specialist")`
   or any other generic agent for this purpose.

2. THE `query_elements_via_agent` function in `crates/op-plugins/src/auto_create.rs`
   SHALL be replaced by a call to the Gemma plugin's D-Bus method
   `ResearchCapabilitySurface` via `SchemaEngine` dispatch, using the plugin's
   registered D-Bus object path.

3. THE `ResearchCapabilitySurface` method SHALL accept `{ "plugin_name": string,
   "requested_info": string }` as its `args` schema and SHALL return a
   `CapabilitySurfaceDraft` (Requirement 12.5 shape) as its `returns` schema.

4. THE `ResearchCapabilitySurface` `MethodDecl` SHALL carry
   `side_effect: "read"`, `idempotent: true`, `required_capability:
   "gemma.research"`, and `subid: "obs.service.gemma.research-capability@v1"`.

**Gemma as a plugin / StatePlugin**

5. Gemma SHALL be registered as a `StatePlugin` with a `PluginSchema` defined in
   `plugin_schema_defs.rs` (the single source of truth file). Its `schema()`
   method SHALL call `super::plugin_schema_defs::gemma_plugin_schema()`.

6. THE Gemma plugin's `PluginSchema` SHALL declare ALL of Gemma's schema-reasoning
   responsibilities as `MethodDecl` entries: subid classification
   (`ClassifySubid`), tag routing (`RouteByTags`), UI perspective generation
   (`GenerateUiPerspectives` covering the four perspectives: Data/Numeric,
   Spatial/Layout, User Flow, Context/Aesthetic), and capability-surface research
   (`ResearchCapabilitySurface`).

7. IF Gemma's `PluginSchema` does not declare `ResearchCapabilitySurface` in its
   `methods` map, THE system SHALL refuse to complete the autogeneration research
   phase and SHALL set the draft to `research_failed` with reason
   `"gemma.ResearchCapabilitySurface not declared in schema"`.

**Schema reasoning concentration**

8. WHEN any component requires subid classification, tag routing, UI rendering
   hints, or capability-surface synthesis, IT SHALL route the request to the
   Gemma plugin's D-Bus object. No other component SHALL implement schema
   reasoning logic independently.

9. THE Gemma plugin's `dispatch_method` implementation SHALL be the sole
   execution path for all schema-reasoning operations. Bespoke handler code
   outside the `GemmaPlugin` struct for these operations SHALL NOT exist.

10. WHEN `cargo build --workspace` succeeds, THERE SHALL be no call site of
    `create_agent("search-specialist")` in `crates/op-plugins/src/auto_create.rs`.

---

## Non-Functional Requirements

### NFR 1: Zero-Copy, No Polling

1. ALL reads of schema and present-state SHALL be direct file reads from SHM
   (`/dev/shm/`). No JSON-RPC polling loops, no D-Bus watchers for state.
2. The catalog hash SHALL be compared on inbound connection arrival, not on a
   timer, before dispatching a method call.

### NFR 2: Atomicity and Durability

1. EVERY mutation dispatched through `SchemaEngine.mutate` SHALL be appended
   to the immutable event chain before the call returns success to the caller.
2. The manifest file write SHALL be atomic (tmp + rename) to prevent consumers
   from reading a partial hash.

### NFR 3: No SQL

1. Durability is the per-mutation event chain (CozoDB→RocksDB). No SQLite
   anywhere. No Btrfs-snapshot backups. No parallel persistence.

### NFR 4: Zero-Trust Identity

1. The caller's identity footprint (`Argon2(PSK, salt=WG-pubkey)`) is the
   primary gate. Capability enforcement is additive, not a replacement.
2. All container I/O is over Unix Domain Sockets. No container gets a NIC/IP.

### NFR 5: Build Correctness

1. `cargo build --workspace` SHALL succeed at every task checkpoint.
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   SHALL produce zero warnings after the refactor is complete.
