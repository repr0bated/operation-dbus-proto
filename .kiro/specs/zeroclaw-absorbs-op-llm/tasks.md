# Tasks — zeroclaw-absorbs-op-llm

Execution model: **orchestrated multi-agent**. The Coordinator Agent sequences
agent phases, manages the integration branch, and owns the final acceptance
gate. Agents work concurrently within a phase only when their inputs do not
depend on each other. All agents deposit handoff files per spec.md §12 before
the next phase proceeds.

---

## Phase 0 — Baseline (Coordinator Agent)

### T-00: Inventory `op-llm` consumers and establish baseline

**Agent:** Coordinator
**Inputs:** None
**Actions:**
1. Run `rg 'op_llm::' crates -g '*.rs'` across workspace; record all consumer
   files and import paths. Also run `rg 'op-llm' crates -g Cargo.toml` to
   find all crate dependencies.
2. Run `cargo build --workspace` and `cargo test --workspace --all-targets`
   baseline; record pass/fail counts.
3. Verify `/dev/shm/opdbus/schemas/zeroclaw.json` write path is active (via
   SchemaEngine projection) in current state.
4. Commit baseline with a `[baseline]` tag.

**Verification:**
```bash
rg 'op_llm::' crates -g '*.rs' -l
rg 'op-llm' crates -g 'Cargo.toml'
cargo build --workspace
cargo test --workspace --all-targets --all-features 2>&1 | tail -20
```

**Gate:** Baseline test counts recorded; consumer list in handoff.

---

## Phase 1 — Schema Extension (Schema Agent)

### T-10: Extend `ModelRoute` with selector fields

**Agent:** Schema
**File:** `crates/op-plugins/src/state_plugins/common/llm_projection.rs`
**Actions:**
1. The cost field is resolved (spec §2a): `ModelRoute.cost_profile` — a
   budget-class **string**, unit-neutral. No `cost_per_token`, no currency math.
2. Add fields to `ModelRoute`: `cost_profile`, `effort_level`, `latency_class`,
   `privacy_tier`, `context_window`, `health_score`, `fallback_routes`,
   `tool_support`.
3. `ModelRoute` MUST carry `#[derive(Serialize, Deserialize, schemars::JsonSchema)]`.
   Every new field MUST also be visible to `schemars` — no `#[serde(skip)]`
   on schema-contract fields, no manual `impl JsonSchema`.
4. Record each field's subid in the `PluginSchema.subids` map (the single
   authority — spec §13.1), keyed by field name, using the values in §13.2.
   Do NOT rely on `x-oscal-subid` source annotations.
5. Add `serde(default)` on all new fields.

**Verification:**
```bash
cargo check -p op-plugins
cargo test -p op-plugins -- zeroclaw_schema_golden
```

### T-11: Add `SelectorPolicy` struct and `SelectionInput`/`SelectionOutput`

**Agent:** Schema
**Files:**
- `crates/op-plugins/src/state_plugins/common/llm_projection.rs`
  (add `SelectorPolicy`, `SelectionInput`, `SelectionOutput`, `SelectionEvent`)
- `crates/op-plugins/src/state_plugins/zeroclaw.rs`
  (add `selector_policy: SelectorPolicy` to `ZeroclawState`)
**Actions:**
1. Define all four structs with `#[derive(Serialize, Deserialize, schemars::JsonSchema)]`.
   `schemars::JsonSchema` is mandatory — without it these types are invisible
   to D-Bus method shape generation, MCP tool parameters, and gRPC proto fields.
2. Record struct/field subids in the `PluginSchema.subids` map (spec §13.1)
   using the values in §13.2.
3. Add `selector_policy` field to `ZeroclawState`.
4. Update `zeroclaw_schema_golden()` to include `selector_policy` defaults.
5. Confirm `schemars::schema_for!(ZeroclawState)` contains `selector_policy`,
   `selection_input`, and `selection_output` definitions in output.

**Verification:**
```bash
cargo check -p op-plugins
cargo test -p op-plugins -- zeroclaw
```

### T-12: Add `ZeroclawError` enum

**Agent:** Schema
**File:** `crates/op-plugins/src/state_plugins/zeroclaw.rs` (or a new
`crates/op-plugins/src/state_plugins/common/errors.rs`)
**Actions:**
1. Define `ZeroclawError` with all variants from spec.md §14.
2. Derive `thiserror::Error` AND `schemars::JsonSchema` — the error schema
   is surfaced in MCP tool responses and D-Bus method error payloads.
3. Annotate with `sch.software.zeroclaw-error.schema@v1`.

**Verification:**
```bash
cargo check -p op-plugins
```

### T-13: Generate Zeroclaw `PluginSchema` from schemars (spec §8)

**Agent:** Schema
**File:** `crates/op-plugins/src/state_plugins/plugin_schema_defs.rs`
**Actions:**
1. Add `plugin_schema_from_schemars::<T: schemars::JsonSchema>(name, category,
   version, description, methods, signals, guarantees, subids) -> PluginSchema`.
   It calls `schemars::schema_for!(T)` and maps each top-level JSON-Schema
   property to a `FieldSchema` (type, required, default, description) via
   `PluginSchemaBuilder`, resolving `$ref`/`$defs` for nested structs, arrays,
   and `Option`.
2. Rewrite `zeroclaw_plugin_schema()` to call
   `plugin_schema_from_schemars::<ZeroclawState>(...)`, supplying the declared
   `methods` (spec §3), `signals`, `guarantees`, and the `subids` map (§13.1).
   Do NOT use `schema_from_state()` for Zeroclaw. Method arg/return shapes are
   referenced via `schema_for!(SelectionInput)` / `schema_for!(SelectionOutput)`.
3. Subid authority is the `PluginSchema.subids` map — populate it for every
   field/method/signal from spec §13.2. No `x-oscal-subid` source annotations.
4. Add a golden `#[test]` (`zeroclaw_schema_golden`): assert the generated
   `PluginSchema.fields` matches the `schemars::schema_for!(ZeroclawState)`
   property set and includes `model_routes`, `providers`, `selector_policy`;
   assert `subids` values are unique.

**Verification:**
```bash
cargo check -p op-plugins
cargo test -p op-plugins -- zeroclaw_schema_golden
# subid uniqueness is asserted in-test against PluginSchema.subids (not source grep)
```

**Handoff:** `handoff-schema.md`

---

## Phase 1b — gRPC-Bridge D-Bus Projection (Bridge Agent) — starts after T-13 green

### T-15: Implement `GrpcBridgeProjectionHook`

**Agent:** Bridge
**File:** `crates/op-grpc-bridge/src/zeroclaw_projection.rs` (new)
**Actions:**
1. Define `GrpcBridgeProjectionHook` implementing `SchemaProjectionObserver`
   (spec §9.2/§9.3) over a `zbus::Connection`; it holds the last registered
   object set for diffing. It does NOT hold `Arc<SchemaEngine>` and does NOT
   read `/dev/shm`.
2. Implement `apply_projection(&self, state: &ZeroclawState)`:
   - Register one D-Bus managed object per `ModelRoute` at
     `/org/opdbus/v1/plugins/zeroclaw/routes/<hint>` implementing interface
     `org.opdbus.v1.ModelRoute`; one per `Provider` at
     `/org/opdbus/v1/plugins/zeroclaw/providers/<id>` implementing
     `org.opdbus.v1.Provider`.
   - **Property names and types are enumerated from
     `schemars::schema_for!(ModelRoute)` / `schema_for!(Provider)` — never
     hardcoded.** (Illustrative only — the schemars set currently yields
     `hint, model, provider, available, effort_level, latency_class,
     privacy_tier, context_window, health_score, …` for a route and
     `id, route, kind, …` for a provider; the code must read these from
     schemars, not from a literal list.)
   - Diff against previous cycle: unregister objects whose `hint`/`id` no
     longer appears in `state`.
3. Hook is invoked by `SchemaEngine` via `register_projection_observer` after
   every Zeroclaw projection cycle (the in-memory state is passed in), or
   invoke from `SchemaRouter`'s existing cycle loop.
4. Record subids in the `PluginSchema.subids` map from spec.md §13.2.

**Verification:**
```bash
cargo check -p op-grpc-bridge
busctl introspect org.opdbus.v1 /org/opdbus/v1/plugins/zeroclaw/routes
```

### T-16: Wire hook via the `SchemaProjectionObserver` trait (no crate cycle)

**Agent:** Bridge
**Files:** `crates/op-projection/src/…` (trait + registry), `crates/op-grpc-bridge/src/zeroclaw_projection.rs`
**Actions:**
1. Define `pub trait SchemaProjectionObserver { fn on_zeroclaw_projection(&self,
   schema: &PluginSchema, state: &ZeroclawState); }` in `op-projection` (spec
   §9.2). `SchemaEngine` holds `Vec<Arc<dyn SchemaProjectionObserver>>` and a
   `register_projection_observer(...)`; it invokes observers **after** each
   projection cycle, passing the **in-memory** `PluginSchema`/`ZeroclawState` —
   NOT a re-read of `/dev/shm`.
2. `GrpcBridgeProjectionHook` implements `SchemaProjectionObserver`; on callback
   it spawns `apply_projection(state)`. The bridge registers its hook at startup.
3. `op-grpc-bridge` depends on `op-projection` only — `op-projection` MUST NOT
   depend on `op-grpc-bridge` (spec §10). Verify no new cycle.
4. Registration/D-Bus failures log a warning and retry next cycle; never abort
   the SchemaEngine cycle. The `/dev/shm` snapshot is never on this path.

**Verification:**
```bash
cargo check -p op-grpc-bridge
cargo check -p op-projection   # confirm no op-grpc-bridge dependency edge
cargo test -p op-grpc-bridge -- zeroclaw_projection
```

### T-17: Add `ZeroclawProjection` proto message and wire SchemaPassthroughService

**Agent:** Bridge
**Files:**
- `crates/op-grpc-bridge/proto/operation.proto` — add `ZeroclawProjection` message
- `crates/op-grpc-bridge/src/schema_passthrough.rs` — route `"zeroclaw"` plugin
  name to `ZeroclawProjection` stream
**Actions:**
1. Add to `operation.proto`:
   ```protobuf
   message ModelRouteProto {
     string hint = 1;
     string model = 2;
     string provider = 3;
     bool available = 4;
     string effort_level = 5;
     string privacy_tier = 6;
     uint32 context_window = 7;
   }
   message ProviderProto {
     string id = 1;
     string name = 2;
     string transport = 3;
     bool available = 4;
   }
   message ZeroclawProjection {
     string schema_json = 1;
     repeated ModelRouteProto model_routes = 2;
     repeated ProviderProto providers = 3;
   }
   ```
2. In `SchemaPassthroughService::resolve_route`, when `plugin_name == "zeroclaw"`,
   deserialize `ZeroclawState` and populate a `ZeroclawProjection` proto for
   streaming. Annotate with `exp.service.zeroclaw-bridge.grpc-stream@v1`.

**Verification:**
```bash
cargo check -p op-grpc-bridge
cargo test -p op-grpc-bridge -- schema_passthrough
```

### T-18: Verify Refactro can call Zeroclaw via busctl

**Agent:** Bridge
**Actions:**
1. Write a shell integration test script at
   `crates/op-grpc-bridge/tests/zeroclaw-busctl-smoke.sh` that:
   - Calls `GetModelRoutes` on the top-level plugin object.
   - Introspects `/org/opdbus/v1/plugins/zeroclaw/routes` to confirm sub-objects registered.
   - Calls `SelectModel` with a minimal JSON payload.
   - Confirms response is valid JSON with `selected_model` field.
2. Document the call pattern in `handoff-bridge.md` (see below).

**Verification:**
```bash
bash crates/op-grpc-bridge/tests/zeroclaw-busctl-smoke.sh
```

**Handoff:** `handoff-bridge.md`

---

## Phase 2 — D-Bus Methods (D-Bus Agent) — starts after T-13 green

### T-20: Implement `ResolveRoute`, `GetModelRoutes`, `GetProviderCatalog`, `GetTools`

**Agent:** D-Bus
**File:** `crates/op-plugins/src/state_plugins/zeroclaw.rs`
**Actions:**
1. Add D-Bus method handlers (read-only) for `ResolveRoute`,
   `GetModelRoutes`, `GetProviderCatalog`, `GetTools`.
2. Each returns JSON-serialized state fields from `ZeroclawState`.
3. Annotate with obs-category subids.
4. Verify whether the gRPC-bridge refactor already generates these surfaces
   from `PluginSchema`; if so, record in handoff and skip manual
   handler boilerplate for those methods.

**Verification:**
```bash
cargo check -p op-plugins
cargo test -p op-plugins -- dbus
```

### T-21: Implement `SelectModel` and `AuthorizeExecution` with signals

**Agent:** D-Bus
**File:** `crates/op-plugins/src/state_plugins/zeroclaw.rs`
**Actions:**
1. `SelectModel`: calls `selector::select_model(&input, &state)` (from T-30);
   records `actor_id` in audit metadata via `MutationEngine`; emits
   `ModelChanged` signal. State mutations go through `MutationEngine`.
2. `AuthorizeExecution`: validates provider+model (and optional tool) exist in
   schema via `SchemaEngine`; emits `ExecutionAuthorized` or `ExecutionDenied`;
   returns `bool + reason`.
3. `SetProvider` / `SetModel`: route through `MutationEngine` for validated
   state mutation; emit corresponding signals.
4. Schema projection (`/dev/shm/opdbus/schemas/zeroclaw.json`) is written by
   `SchemaEngine` — do NOT write it from method handlers directly.

**Note:** T-21 depends on T-30 (Router Agent) for `select_model` function
signature. D-Bus Agent may stub the call with a placeholder until T-30 is
complete; real wiring happens in T-22.

**Verification:**
```bash
cargo check -p op-plugins
cargo test -p op-plugins -- authorize_execution
```

### T-22: Wire `SelectModel` to real `selector::select_model` (after T-30)

**Agent:** D-Bus (integration step with Router Agent)
**Verification:**
```bash
cargo check -p op-plugins
cargo test -p op-plugins -- select_model
```

**Handoff:** `handoff-dbus.md`

---

## Phase 2 (parallel) — Dynamic Router (Router Agent) — starts after T-13 green

### T-30: Implement `selector::select_model` pure function

**Agent:** Router
**File:** `crates/op-plugins/src/state_plugins/common/selector.rs` (new)
**Actions:**
1. Implement Phase 1 hard filters per spec.md §4 / design.md §4.
   Cost filtering uses `ModelRoute.cost_profile` (budget class) vs
   `SelectionInput.cost_policy` — string/ordinal comparison only.
   Do NOT use `cost_per_token` or invent unit math.
2. Implement Phase 2 scoring (no `match` on provider/model name).
3. Return `SelectionOutput` on success; `ZeroclawError::NoCandidateAfterFiltering`
   or specific hard-filter errors on failure.
4. Pure function: `fn select_model(input: &SelectionInput, state: &ZeroclawState) -> Result<SelectionOutput, ZeroclawError>`.

**Verification:**
```bash
cargo check -p op-plugins
cargo test -p op-plugins -- selector::
```

### T-31: Add model-selection focused tests

**Agent:** Router
**Tests to write:**
- `should_select_lowest_cost_for_equivalent_capability`
- `should_respect_effort_ceiling`
- `should_exclude_unavailable_routes`
- `should_enforce_privacy_tier`
- `should_reject_undeclared_route`
- `should_return_fallback_routes_ordered`
- `should_select_explicit_provider_when_authorized`

**Verification:**
```bash
cargo test -p op-plugins -- selector:: --nocapture
```

**Handoff:** `handoff-router.md`

---

## Phase 3 — Provider Absorption (Provider Absorption Agent) — starts after T-13 green

### T-40: Determine target module location (pre-implementation spike)

**Agent:** Provider Absorption
**Actions:**
1. Inspect the SchemaEngine/MutationEngine/gRPC-bridge dispatch boundary in
   the current repo to identify the existing module that hosts Zeroclaw plugin
   execution. This is a read-only spike — do not move any files yet.
2. Confirm the target module satisfies the three-layer boundary (spec.md §5):
   - The Contract Layer (`op-plugins` schema types) must remain separate from
     adapter runtime (HTTP/streaming) behavior.
   - If placing adapters in `op-plugins` would cause the schema crate to own
     runtime HTTP clients, identify the correct Orchestration/Adapter host
     module in a different crate instead.
   - If no existing module cleanly satisfies this boundary, document the gap
     and record a short boundary spike result — do NOT create a new crate.
3. Confirm the identified module does NOT have an existing authority role (no
   D-Bus object, no schema registration) that would make it a second authority.
4. Record confirmed target module path in `handoff-provider-absorption.md`.
   Include: why this module satisfies the layer boundary, and which layer
   (Orchestration or Adapter) it hosts.
5. Do NOT create a new crate.

### T-41: Move provider adapter files

**Agent:** Provider Absorption
**Source:** `crates/op-llm/src/{anthropic,gemini,gemini_cli,factory,openclaw,assistant,gcloud_adc}.rs`
**Target:** determined in T-40
**Actions:**
1. Move files; update `mod` declarations and import paths.
2. Keep all adapter behavior identical.
3. Any config/options struct in the moved files (e.g. `AnthropicConfig`,
   `GeminiConfig`, `OpenClawConfig`) MUST add `#[derive(schemars::JsonSchema)]`
   if not already present. These are schema-contract types — they must be
   visible to the D-Bus/MCP/gRPC surface.
4. Add `pub use` re-exports in `op-llm` for each moved type
   (`LlmProvider`, `ChatMessage`, `ToolDefinition`, `ProviderType`, etc.)
   so existing callers still compile — **compat shim**.

**Verification:**
```bash
cargo check -p op-llm    # must still be green (via shim)
cargo check -p op-plugins
cargo check -p op-chat
```

### T-42: Adapter behavioral tests at new location

**Agent:** Provider Absorption
**Tests to write/move:**
- `should_translate_openai_tool_call_response`
- `should_translate_anthropic_content_block`
- `should_handle_streaming_delta`
- `should_reject_call_without_api_key`

**Verification:**
```bash
cargo test -p op-plugins -- provider_adapter
```

**Handoff:** `handoff-provider-absorption.md`

---

## Phase 4 — Caller Migration (Caller Migration Agent) — starts after T-41 + T-22 green

### T-50: Migrate `op-chat`

**Agent:** Caller Migration
**Files:** `crates/op-chat/src/`
**Actions:**
1. Replace `op_llm::provider::LlmProvider` import with Zeroclaw D-Bus call for
   route resolution + `AuthorizeExecution`.
2. Replace `op_llm::chat::ChatManager` usage with Zeroclaw `SelectModel` +
   absorbed provider adapter.
3. Remove `op-llm` from `crates/op-chat/Cargo.toml`.

**Verification:**
```bash
cargo check -p op-chat
cargo test -p op-chat
```

### T-51: Migrate `op-web` and any remaining callers

**Agent:** Caller Migration
**Actions:**
1. Audit remaining `op_llm::` hits from T-00 baseline.
2. Migrate each to Zeroclaw D-Bus surface.
3. Remove `op-llm` from their `Cargo.toml` dependencies.

**Verification:**
```bash
rg 'op_llm::' crates -g '*.rs'
# must output zero lines outside op-llm itself
cargo check --workspace
```

### T-52: Update factory provider routes

**Agent:** Caller Migration
**Actions:**
1. Factory is a provider category in Zeroclaw's provider catalog (alongside
   OpenRouter, Kilocode, Opencode, etc.). Factory model routes are declared in
   Zeroclaw's schema as provider/model routes.
2. Update any factory-specific code to read `model_routes[]` from Zeroclaw
   schema projection via D-Bus `GetModelRoutes`. Do NOT create a separate
   D-Bus object or control plane for factory.
3. Remove any direct `op-llm` imports from factory-related code.

**Verification:**
```bash
cargo check -p op-plugins
cargo test -p op-plugins -- factory
```

### T-53: Remove compat shim and retire `op-llm`

**Agent:** Caller Migration
**Actions:**
1. Remove `pub use` re-exports from `op-llm`.
2. Remove `op-llm` from workspace `[members]` in root `Cargo.toml`.
3. Confirm no remaining references to `op_llm::` outside the now-deleted crate.

**Verification:**
```bash
cargo build --workspace
rg 'op_llm::' crates -g '*.rs'
# must be empty
```

**Handoff:** `handoff-callers.md`

---

## Phase 4 (parallel) — Projection Verification (Projection Agent) — starts after T-21 green

### T-60: Verify `/dev/shm/opdbus/schemas/zeroclaw.json` write path

**Agent:** Projection
**Actions:**
1. Confirm `SchemaEngine` writes `/dev/shm/opdbus/schemas/zeroclaw.json`
   atomically (tmp + rename) as a derived `PluginSchema` projection.
2. Confirm no individual plugin directly writes to that path.
3. Verify file contents match D-Bus `SchemaJson` property value.

**Verification:**
```bash
cargo test -p op-plugins -- projection_write
# manual: start plugin, check /dev/shm/opdbus/schemas/zeroclaw.json exists
```

### T-61: Verify monolithic all-plugins catalog

**Agent:** Projection
**Actions:**
1. Confirm `SchemaEngine`'s aggregation pass writes
   `/dev/shm/live-schema.json` and includes the `zeroclaw` schema.
2. Confirm individual plugins do NOT write `/dev/shm/live-schema.json`.
3. Write a test that reads the monolithic file and asserts `zeroclaw` key
   present.

**Verification:**
```bash
cargo test -p op-plugins -- monolithic_projection
```

**Handoff:** `handoff-projection.md`

---

## Phase 5 — Test Agent (after all absorption + migration complete)

### T-70: Undeclared-route enforcement tests

**Agent:** Test
**Tests to write:**
- `should_deny_execution_for_undeclared_provider`
- `should_deny_execution_for_undeclared_model`
- `should_deny_execution_for_undeclared_tool`
- `should_deny_execution_for_unavailable_route`

### T-71: Subid uniqueness CI test

**Agent:** Test
**Actions:**
1. Add a `#[test]` that collects all values from the `PluginSchema.subids` map
   (the single subid authority — spec §13.1) and asserts no duplicates.

### T-72: Projection integrity tests

**Agent:** Test
**Tests:** file exists after `apply`, content is valid JSON, contains
`model_routes`, `selector_policy`, `tools`.

### T-73: Provider adapter regression tests (full pass)

**Agent:** Test
**Tests:** All moved adapter tests still green at new location.

**Verification:**
```bash
cargo test --workspace --all-targets --all-features 2>&1 | tail -30
```

**Handoff:** `handoff-tests.md`

---

## Phase 6 — Review Agent (final gate)

### T-80: Full bypass audit

**Agent:** Review
**Checklist:**
- [ ] Zero `Command::new` for live state in plugin/service code
- [ ] Zero direct live-state file reads outside bootstrap scripts
- [ ] Zero `op_llm::` imports outside deleted crate
- [ ] Zero static `match effort {` or `match provider_type {` in selection path
- [ ] Zero `/dev/shm` reads in execution-path decisions (D-Bus used instead)
- [ ] Zero `cost_per_token` references (cost uses Zeroclaw-native field names)
- [ ] Zero separate factory D-Bus object or BYOM control plane (factory is a provider category)
- [ ] Zero `ZeroclawPlugin::apply` as projection write point (SchemaEngine owns it)
- [ ] Zero `cargo grep` commands (real `rg` commands used instead)
- [ ] Every field/method/signal has a subid in the generated `PluginSchema.subids`
      map (spec §13.1); `subids` values are unique (asserted in `zeroclaw_schema_golden`)
- [ ] No `x-oscal-subid` source annotations relied on as authority (subids map is authority)
- [ ] All handoff files present and non-empty
- [ ] **schemars is the sole generator: `zeroclaw_plugin_schema()` calls
      `plugin_schema_from_schemars::<ZeroclawState>()`, NOT `schema_from_state()`**
- [ ] **schemars: every schema-surface struct derives `schemars::JsonSchema`** — run:
      `rg 'pub struct.*\n.*serde' crates/op-plugins/src/state_plugins/ -A2 | grep -v JsonSchema`
      and confirm zero hits for structs that carry `Serialize`/`Deserialize`.
- [ ] **schemars: no `impl JsonSchema` manual overrides** for schema-contract types —
      run `rg 'impl.*JsonSchema' crates/ --include="*.rs"` and justify any hit.
- [ ] **schemars: `schemars::schema_for!(ZeroclawState)` contains `model_routes`,
      `selector_policy`, `providers`, `selection_input`, `selection_output`** —
      add this as a `#[test]` in `op-plugins` if not already present.
- [ ] **one projection: D-Bus tree built from in-memory `PluginSchema` via the
      observer (§9.2), never from `/dev/shm`; `op-projection` has no
      `op-grpc-bridge` dependency edge (§10)**
- [ ] **schemars: absorbed provider adapter config structs** (`AnthropicConfig`,
      `GeminiConfig`, etc.) derive `JsonSchema` at their new location.
- [ ] **schemars: `GrpcBridgeProjectionHook` D-Bus property names derived from
      `schemars` output**, not hardcoded strings.
- [ ] Layer boundary: no provider adapter owns model selection or route resolution
- [ ] Layer boundary: no Orchestration Layer owns provider wire formats or auth headers
- [ ] Layer boundary: no Contract Layer owns HTTP client implementations
- [ ] Layer boundary: no adapter reads `/dev/shm` or D-Bus live state directly
- [ ] Provider adapters confirmed in correct layer host (not mixed into `op-plugins` schema authority)
- [ ] `handoff-provider-absorption.md` documents confirmed target module and layer rationale
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean
- [ ] `cargo test --workspace --all-targets --all-features` green

**Verification:**
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

**Handoff:** `handoff-review.md`

---

## Phase 7 — Final Acceptance (Coordinator Agent)

### T-90: Integration acceptance

**Agent:** Coordinator
**Actions:**
1. Confirm all eight handoff files exist and are complete.
2. Run full workspace build and test suite.
3. Confirm `op-llm` is absent from workspace members.
4. Confirm `/dev/shm/opdbus/schemas/zeroclaw.json` write path is green.
5. Close spec with `[accepted]` tag.

**Verification:**
```bash
cargo build --workspace --release
cargo test --workspace --all-targets --all-features
ls .kiro/specs/zeroclaw-absorbs-op-llm/handoff-*.md | wc -l
# must be 8
```
