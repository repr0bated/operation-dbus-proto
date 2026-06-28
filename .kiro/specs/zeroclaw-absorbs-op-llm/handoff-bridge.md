# Handoff — Bridge Agent (Phase 1b, T-15…T-18, + T-22 wiring)

## Dependency-graph deviation (important)
The mission text places the `SchemaProjectionHook` trait in `op-projection` and
has `op-grpc-bridge` implement it. **The real dependency edge is the reverse**:
`op-projection` already depends on `op-grpc-bridge` (and `op-plugins`). Putting the
trait in `op-projection` and implementing it in the bridge would create a crate
cycle, which the spec's actual hard rule (§10) forbids. So the trait lives in
`op-grpc-bridge::zeroclaw_projection` (the bridge already depends on `op-plugins`
for `ZeroclawState`). `op-projection` is untouched; the graph stays acyclic.
The user-approved name is honored: trait `SchemaProjectionHook` with `apply()`,
push-only — no observer/watcher/poll/index vocabulary anywhere.

## Scope Completed
- **T-15/T-16 `crates/op-grpc-bridge/src/zeroclaw_projection.rs` (new):**
  - `trait SchemaProjectionHook { fn apply(&self, plugin_id, schema_json, state_json); }`
    — synchronous push; the impl spawns the async D-Bus work.
  - `GrpcBridgeProjectionHook` holds `Arc<OnceCell<Connection>>` + the registered
    object-path sets for diffing. It does **not** hold `Arc<SchemaEngine>` and never
    reads `/dev/shm`.
  - `apply_projection(&self, &ZeroclawState)` registers one managed object per
    `ModelRoute` at `/org/opdbus/v1/plugins/zeroclaw/routes/<elem>` (interface
    `org.opdbus.v1.ModelRoute`) and per `Provider` at `.../providers/<id>`
    (interface `org.opdbus.v1.Provider`), then unregisters objects whose
    route/provider disappeared. Registration failures log + retry next cycle.
  - **Property names are enumerated from `schemars::schema_for!(ModelRoute)` /
    `schema_for!(Provider)`**, never hardcoded (`schema_property_names` +
    `project_object`).
  - **Route path keying:** route hints are NOT unique (e.g. `code` via both
    openrouter and factory), so the path element is the `(hint, provider, model)`
    triple, not bare `<hint>`. Documented deviation from the illustrative spec path.
  - Wired in `SchemaRouter`: `set_projection_hook(...)` + invocation at the end of
    `register_objects()` using `ZeroclawPlugin::current_state()` as the in-memory
    source. Registered at startup in `bin/op-grpc-bridge.rs`.
- **T-17 proto + passthrough:**
  - `proto/operation.proto`: added `ModelRouteProto`, `ProviderProto`,
    `ZeroclawProjection` (subid `exp.service.zeroclaw-bridge.grpc-stream@v1`).
  - `schema_passthrough.rs`: `SchemaPassthroughService::zeroclaw_projection(name)`
    builds the proto from in-memory `ZeroclawState` (`None` for other plugins).
- **T-18 smoke:** `crates/op-grpc-bridge/tests/zeroclaw-busctl-smoke.sh` (calls
  `GetModelRoutes`, introspects `.../routes`, calls `SelectModel`, asserts
  `selected_model`). Requires a live bridge; not run by `cargo test`.
- **T-22 dispatch wiring (folded in):** `SchemaEngine::dispatch_method_call` now,
  for `plugin_id == "zeroclaw"`, routes the validated `(method, json_args)` to
  `op_plugins::…::dispatch_zeroclaw_method` against `ZeroclawPlugin::current_state()`
  and returns the **real domain result** instead of echoing args. The single
  accountability event is still recorded (Blake3 footprint, NFR-003). Dispatch
  errors propagate (`anyhow::Error` → `fdo::Error::Failed`). `SetProvider`/
  `SetModel` merge their effective change into the authoritative state cache via
  `merge_into_state_cache` so readers observe the new selection.

## Files Changed
- `crates/op-grpc-bridge/src/zeroclaw_projection.rs` (new) + 6 tests.
- `crates/op-grpc-bridge/src/lib.rs` — module + re-exports.
- `crates/op-grpc-bridge/src/schema_router.rs` — `projection_hook` field,
  `set_projection_hook`, invocation in `register_objects`, `warn` import.
- `crates/op-grpc-bridge/src/schema_engine.rs` — zeroclaw dispatch wiring +
  `merge_into_state_cache`.
- `crates/op-grpc-bridge/src/schema_passthrough.rs` — `zeroclaw_projection()`.
- `crates/op-grpc-bridge/src/bin/op-grpc-bridge.rs` — hook registration at startup.
- `crates/op-grpc-bridge/proto/operation.proto` — projection messages.
- `crates/op-grpc-bridge/Cargo.toml` — added `schemars`.
- `crates/op-grpc-bridge/tests/zeroclaw-busctl-smoke.sh` (new).
- `crates/op-plugins/src/state_plugins/zeroclaw.rs` — `current_state()` made `pub`.

## Verification Commands Run
```
cargo check -p op-grpc-bridge                              # green
cargo test  -p op-grpc-bridge --lib -- --test-threads=1    # 56 passed, 0 failed
cargo clippy -p op-grpc-bridge --lib                       # clean (my files)
```

## Known Risks / Blocked Items
- **Pre-existing flaky tests:** `interceptor::tests::interceptor_extracts_footprint_and_session_id`
  and `schema_router::tests::required_capability_check_allows_granted` fail under
  parallel `cargo test` on the **baseline too** (race on shared sled/capability
  grants global state). They pass in isolation and with `--test-threads=1`. Not
  caused by this work; flag for T-80.
- **Signal emission** (`ExecutionAuthorized`/`ProviderChanged`/…): `dispatch_zeroclaw_method`
  returns the intended signal in `DispatchOutcome.signal`, but `dispatch_method_call`
  has no D-Bus `SignalContext`, so emission is not yet wired. `SchemaBackedInterface`
  already has `emit_signal`; wiring the signal through the `call` path is the
  remaining step.
- **Durable Set\* persistence:** currently merged into the in-memory state cache
  (observable to readers); the durable NonNetDb write via `MutationEngine`/`mutate()`
  is still TODO (kept single-event to avoid double accountability records).
- The route/provider sub-objects expose a single read-only `properties` D-Bus
  property (JSON), mirroring `SchemaBackedInterface`'s dynamic-schema convention
  rather than native per-field D-Bus properties.

## Next-Agent Dependencies
- Provider Agent (T-40…T-42): adapters move under zeroclaw; the projection +
  dispatch surface is ready to back real execution.
- T-60/T-61: projection verification can introspect `.../routes` + `.../providers`.
