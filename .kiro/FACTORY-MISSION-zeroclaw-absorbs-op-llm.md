# Factory Mission: Absorb `op-llm` into Zeroclaw (schemars-native, single-projection)

## Mission
Implement the spec at `.kiro/specs/zeroclaw-absorbs-op-llm/`
(`requirements.md`, `design.md`, `spec.md`, `tasks.md`). Fold `crates/op-llm`
into the **Zeroclaw plugin** so that Zeroclaw becomes the single
schema/D-Bus/gRPC authority for all LLM execution — provider catalog, model
routes, dynamic selection, execution authorization — and retire `op-llm` as an
independent crate.

`tasks.md` is the authoritative, dependency-ordered work breakdown (T-00 →
T-90). The workspace must build at each phase gate. Every task carries its
verification commands — honor them.

## Base: build on PR #14 (`plugin-capability`)
Branch off and target **PR #14 — "Plugin capability"**
(`https://github.com/repr0bated/operation-dbus-proto/pull/14`, head branch
`plugin-capability`, base `main`). PR #14 is the substrate this mission depends
on — do **not** reinvent any of it; integrate with it:

- `op-state-store::PluginSchema` already carries `MethodDecl`, `SignalDecl`, and
  `PluginCapabilities` — declare Zeroclaw's methods/signals/guarantees through
  these (the §3 D-Bus surface), not a parallel structure.
- `plugin_schema_defs.rs` + `with_caps(...)` is the existing schema-assembly
  point — the §8 `plugin_schema_from_schemars::<ZeroclawState>()` converter slots
  in here, replacing the `schema_from_state()` call for `zeroclaw` while reusing
  `with_caps` for methods/signals/guarantees.
- `op-grpc-bridge::SchemaRouter` already resolves destination/path/interface from
  schema and **denies `mutate` RPCs lacking the declared `required_capability`** —
  Zeroclaw's `SelectModel`/`SetProvider`/`AuthorizeExecution` enforcement rides
  this existing path; do not add a second enforcement point.
- `ShmWriter` already writes per-plugin schema + present-state to
  `/dev/shm/opdbus/`; `SchemaRouter` reads it. The §9 `GrpcBridgeProjectionHook`
  observes the **same** `SchemaEngine`/projection cycle — it is one more **view**
  of PR #14's single projection, never a second writer.
- Bus name `org.opdbus.v1` is owned by `op-grpc-bridge` (per PR #14); the UDS +
  `uds_identity_interceptor` carry caller identity. Use them as-is.

All mission work is authored as commits on top of `plugin-capability` and merged
into PR #14 (or a stacked PR based on it) — never a fresh branch off `main`.

The two design pillars this mission exists to enforce:

1. **schemars is the sole schema generator.** The Zeroclaw `PluginSchema` is
   produced **entirely** from `schemars::schema_for!(ZeroclawState)` via the
   `plugin_schema_from_schemars::<T>()` converter (spec §8) — `schema_from_state()`
   is **not** used for Zeroclaw. Every schema-surface struct derives
   `schemars::JsonSchema`. D-Bus method shapes, gRPC proto fields, MCP tool
   params, UI renderers, and the per-route/per-provider D-Bus property types all
   derive from that one schemars output. A struct without `JsonSchema` is
   invisible and cannot route.

2. **Exactly one projection, owned by `op-projection::SchemaEngine`.** The D-Bus
   object tree under `/org/opdbus/v1/plugins/zeroclaw/...` is a **view** the
   grpc-bridge materializes from the **in-memory** `PluginSchema` handed to it
   via the `SchemaProjectionObserver` trait (spec §9). `/dev/shm/opdbus/schemas/
   zeroclaw.json` is a non-authoritative discovery snapshot — written by
   SchemaEngine, **never read back** to build the tree or make execution
   decisions. There is no second projection and no recomputation.

## Method: TDD-orchestrated multi-agent
Use the enabled skills: `tdd-orchestrator`, `test-automator`,
`spec-to-code-compliance`, `rust-pro`, `rust-async-patterns`,
`systems-programming-rust-project`, `orchestrate-batch-refactor`.

For each task group: **RED** (write tests from the spec's acceptance criteria;
they must fail first) → **GREEN** (minimum to pass) → **REFACTOR** (keep the
workspace compiling) → **VERIFY** (`spec-to-code-compliance` against the task's
verification block) before advancing. Each agent deposits its handoff file
(`handoff-*.md`, spec §12) before the next phase proceeds.

## Orchestration (workstreams → phases in `tasks.md`)
Parallelize within a workstream; gate between them on a green `cargo check` of
the named crates.

- **WS0 — Baseline** (T-00, Coordinator): inventory `op_llm::` consumers
  (24 files across op-chat/op-web/op-mcp-proxy) + the 4 `Cargo.toml` deps; record
  `cargo build/test` baseline; commit `[baseline]`.
- **WS1 — Schema, schemars-native** (T-10…T-13, Schema Agent): typed
  `ZeroclawState`/`LlmProjection`/`ModelRoute`/`Provider`/`SelectorPolicy`/
  `Selection{Input,Output,Event}`/`ZeroclawError`, all deriving `JsonSchema`;
  add the `plugin_schema_from_schemars::<ZeroclawState>()` converter and rewrite
  `zeroclaw_plugin_schema()` to use it; subids live in the `PluginSchema.subids`
  map (spec §13.1); golden test asserts fields == schemars properties + unique
  subids. Gate: `cargo check -p op-plugins` green.
- **WS1b — Bridge D-Bus projection** (T-15…T-18, Bridge Agent): define
  `SchemaProjectionObserver` in `op-projection`; `GrpcBridgeProjectionHook`
  in `op-grpc-bridge` registers one D-Bus object per route/provider with
  property names **enumerated from `schema_for!(ModelRoute)`/`schema_for!(Provider)`**
  (no hardcoded field strings); diff-based register/unregister; `ZeroclawProjection`
  proto + `SchemaPassthroughService` stream. `op-projection` MUST NOT depend on
  `op-grpc-bridge`. Gate: `busctl introspect …/zeroclaw/routes` shows objects.
- **WS2 — D-Bus methods** (T-20…T-22, D-Bus Agent): `ResolveRoute`,
  `GetModelRoutes`, `GetProviderCatalog`, `GetTools` (read), `SelectModel`,
  `AuthorizeExecution`, `SetProvider`, `SetModel` (mutation via `MutationEngine`,
  with `actor_id` + signals). Method shapes from schemars.
- **WS3 — Dynamic router** (T-30…T-31, Router Agent): pure
  `selector::select_model(&SelectionInput, &ZeroclawState) -> Result<…, ZeroclawError>`
  — Phase-1 hard filters + Phase-2 scoring, no `match` on provider/model name,
  `cost_profile` (budget class) only, no `cost_per_token`.
- **WS4 — Provider absorption** (T-40…T-42, Absorption Agent): spike to confirm
  the execution-host module (record in `handoff-provider-absorption.md`); move
  the 9 adapter files; config structs derive `JsonSchema`; compat `pub use` shim
  in `op-llm`. No new crate.
- **WS5 — Caller migration & retirement** (T-50…T-53, Caller Agent): migrate
  op-chat, op-web, factory routes, remaining callers to the Zeroclaw D-Bus
  surface; remove the shim; drop `op-llm` from workspace members.
- **WS6 — Projection/Test/Review** (T-60…T-90): single-projection verification,
  undeclared-route enforcement, subid uniqueness, full bypass audit, acceptance.

## Hard rules (do not violate — these are why naïve attempts fail here)
- **schemars or it doesn't exist.** Never hand-build a `PluginSchema` field set
  for Zeroclaw, never reach for `schema_from_state()` here, never `impl JsonSchema`
  manually for a contract type. Adding a field to `ZeroclawState` is the ONLY way
  to change the schema.
- **One projection, one writer.** Only `SchemaEngine` writes projection
  artifacts. The bridge/file/gRPC stream are read-only **views** of the one
  in-memory `PluginSchema`. Never read `/dev/shm` on an execution or
  object-registration path. Never recompute the schema in a consumer.
- **No crate cycle.** `op-grpc-bridge → op-projection` only; the observer trait
  is the seam. `op-plugins` (schema authority) never depends on `op-grpc-bridge`
  and never owns provider HTTP clients.
- **No stubs, no placeholders, no "for now".** A registered D-Bus object that
  returns fake success, or `AuthorizeExecution` that validates against an empty
  set, is a defect. Inert code that *looks* wired is the recurring failure mode.
- **Nothing executes that isn't declared.** No undeclared provider/model/route/
  tool may run. No static `match effort {…}` model picker. Factory is a provider
  category in the catalog — never a second D-Bus object or BYOM control plane.
- **Canonical addressing only:** bus `org.opdbus.v1`, paths
  `/org/opdbus/v1/plugins/zeroclaw[/routes/<hint>|/providers/<id>]`. Fix the
  legacy `/opdbus/v1/...` transport string in `zeroclaw.rs`.

## Definition of done
`cargo build --workspace` green **without** `op-llm`; `rg 'op_llm::' crates -g '*.rs'`
empty; `cargo clippy --workspace --all-targets --all-features -- -D warnings`
clean; `cargo test --workspace` green; all nine `handoff-*.md` present;
`busctl` can `GetModelRoutes` / `SelectModel` / `AuthorizeExecution` and
introspect the route/provider sub-objects.
