# Handoff — Schema Agent (Phase 1, T-10…T-13)

## Scope Completed
WS1 schemars-native schema. The Zeroclaw `PluginSchema` is now generated
**entirely** from `schemars::schema_for!(ZeroclawState)` and is **defined in the
plugin** (`zeroclaw.rs`), not in `plugin_schema_defs.rs`. The plugin IS the
schema.

- T-10: `ModelRoute` selector fields (`cost_profile`, `effort_level`,
  `latency_class`, `privacy_tier`, `context_window`, `health_score`,
  `fallback_routes`, `tool_support`) confirmed present in
  `common/llm_projection.rs`, all `#[serde(default)]` + `JsonSchema`.
- T-11: `SelectorPolicy`/`SelectionInput`/`SelectionOutput`/`SelectionEvent`
  confirmed in `common/llm_projection.rs`. `ZeroclawState` **retyped** from
  untyped `simd_json::Value` fields to typed schemars structs
  (`#[serde(flatten)] projection: LlmProjection`, typed `transport: LlmTransport`,
  added `selector_policy: SelectorPolicy`), derives `JsonSchema`. Legacy
  `/opdbus/v1/...` transport/source strings fixed to canonical
  `/org/opdbus/v1/...`.
- T-12: `ZeroclawError` added in `common/errors.rs` (thiserror + JsonSchema,
  subid `sch.software.zeroclaw-error.schema@v1`).
- T-13: generic `plugin_schema_from_schemars::<T>()` converter added to
  `plugin_schema_defs.rs` (shared infra, per spec §8); `zeroclaw_plugin_schema()`
  **moved into `zeroclaw.rs`** and rewritten to call it with the §3 method
  surface, §3 signals, guarantees, and the §13.2 `subids` map. Golden test
  `zeroclaw_schema_golden` added and passing.

## Files Changed
- `crates/op-plugins/src/state_plugins/common/mod.rs` (new) — declares
  `errors`, `llm_projection`.
- `crates/op-plugins/src/state_plugins/common/errors.rs` (new) — `ZeroclawError`.
- `crates/op-plugins/src/state_plugins/mod.rs` — `pub mod common;`.
- `crates/op-plugins/src/state_plugins/zeroclaw.rs` — typed `ZeroclawState`,
  rebuilt `current_state()`, `zeroclaw_plugin_schema()` definition + golden test.
- `crates/op-plugins/src/state_plugins/plugin_schema_defs.rs` — added generic
  `plugin_schema_from_schemars` + schemars walker helpers; made
  `read_method`/`mutation_method`/`signal_decl`/`empty_args` `pub(crate)`;
  removed the old `schema_from_state`-based `zeroclaw_plugin_schema`; registry
  now calls `crate::state_plugins::zeroclaw::zeroclaw_plugin_schema()`.

## Contract Changes
- `ZeroclawState` is now a typed schemars contract type (breaking for any code
  that read its fields as raw JSON via `ZeroclawState` struct fields — none do;
  consumers use `query_current_state() -> Value`, unchanged).
- New generic converter `plugin_schema_from_schemars<T: JsonSchema>(name,
  category, version, description, methods, signals, guarantees, subids)`.
- D-Bus method surface (§3): `GetState`, `ResolveRoute`, `GetProviderCatalog`,
  `GetModelRoutes`, `GetTools` (read); `SelectModel`, `AuthorizeExecution`,
  `SetProvider`, `SetModel` (mutation, each with `required_capability`).
  `SelectModel` args = `schema_for!(SelectionInput)`, returns =
  `schema_for!(SelectionOutput)`. Signals: `ProviderChanged`, `ModelChanged`,
  `RouteHealthChanged`, `ExecutionAuthorized`, `ExecutionDenied`.
- subids: full field-level + struct-level map in `zeroclaw.rs`; method/signal
  subids backfilled by the converter.

## Verification Commands Run
```
cargo check -p op-plugins                                  # green
cargo test -p op-plugins --lib zeroclaw_schema_golden      # ok (1 passed)
```
Golden test asserts: `PluginSchema.fields` == schemars property set (flatten
merge handled), includes `model_routes`/`providers`/`selector_policy`/`router`/
`tools`, `subids` values unique, and `SelectModel` arg/return shapes derive from
`SelectionInput`/`SelectionOutput`.

## Known Risks / Blocked Items
- The converter resolves `#/definitions/*` refs and merges `allOf`/object
  properties (covers `#[serde(flatten)]`). Verified against `ZeroclawState`; if a
  future schemars upgrade changes flatten representation, the golden test will
  catch it.
- `selection_input`/`selection_output` are surfaced via method arg/return shapes
  (not as top-level `ZeroclawState` properties); the golden test verifies them
  through `SelectModel.args`/`returns` rather than the top-level property set.

## Next-Agent Dependencies
- Bridge Agent (T-15…T-18): `SchemaProjectionObserver` trait in `op-projection`;
  `GrpcBridgeProjectionHook` enumerating property names from
  `schema_for!(ModelRoute)`/`schema_for!(Provider)`.
- Router Agent (T-30): `common/selector.rs` `select_model` (declare
  `pub mod selector;` in `common/mod.rs`); `ZeroclawError` is ready.
- D-Bus Agent (T-20…T-22): method handlers; the declared method surface is in
  place — confirm whether the bridge auto-generates handlers from `PluginSchema`.
