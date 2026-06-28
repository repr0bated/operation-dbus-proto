# Handoff — D-Bus Method Agent (T-20, T-21, T-22)

## T-20 Finding (recorded per spec instruction)
The gRPC-bridge refactor (PR #14) **auto-generates the D-Bus/gRPC method and
property surface from `PluginSchema`** — there is no per-method D-Bus boilerplate
to write. `SchemaBackedInterface::call` (in
`crates/op-grpc-bridge/src/schema_router.rs`, interface `org.opdbus.v1.PluginV1`)
does, in order:
1. method-existence gate (method must be in the plugin's `PluginSchema.methods`);
2. arg validation against the method's `args` JSON schema;
3. **capability enforcement** — a mutation method with `required_capability`
   is denied without the capability (R7.x);
4. dispatch via `SchemaEngine::dispatch_method_call`.

**Gap:** `SchemaEngine::dispatch_method_call`
(`crates/op-grpc-bridge/src/schema_engine.rs`) is **generic** — it records an
immutable accountability event (Blake3 footprint of the verbatim args, appended
under the event-chain write lock before returning, NFR-003), broadcasts the
change, and **echoes the args back**. It contains **no domain logic**, so
`SelectModel`/`AuthorizeExecution` would return an inert "success" with no
selection/authorization decision — which the mission forbids.

## Scope Completed
Per design §5 (Orchestration Layer is plugin-owned), the domain logic for the
declared methods now lives in `zeroclaw.rs` as `dispatch_zeroclaw_method`, the
single entry point the bridge routes a validated call to. Handlers read **only**
the in-memory `ZeroclawState` passed in (never `/dev/shm`, never a network call),
keeping execution decisions on the schema/state authority. There are **no
`match` arms on provider or model name** — branching is data-driven from the
declared `providers`/`model_routes`/`tools`.

| Method | Behavior |
|---|---|
| `GetState` / `GetModelRoutes` / `GetProviderCatalog` / `GetTools` | return the in-memory projection slice |
| `ResolveRoute` | resolve `hint` against declared routes; `RouteNotDeclared` for unknown hint, `RouteUnavailable` for declared-but-down, `ContextWindowExceeded` when over the route window |
| `SelectModel` (T-21) | run `selector::select_model`, then stamp `trace_id` (uuid v4) + `timestamp` (RFC3339) the selector intentionally leaves blank |
| `AuthorizeExecution` (T-21) | deterministic gate: provider declared, (provider, model) route declared, tool (if given) declared, route available; returns `{authorized, reason}` + an `ExecutionAuthorized`/`ExecutionDenied` signal |
| `SetProvider` / `SetModel` | validate target is declared (reject undeclared), surface the effective change + a `ProviderChanged`/`ModelChanged` signal |

`dispatch_zeroclaw_method` returns a `DispatchOutcome { result, signal }` so the
bridge can serialize `result` to the caller and emit the optional declared
signal through its accountability pipeline.

## Files Changed
- `crates/op-plugins/src/state_plugins/zeroclaw.rs` — added the Orchestration
  Layer: `DispatchSignal`, `DispatchOutcome`, `dispatch_zeroclaw_method`, and the
  per-method handlers + 8 unit tests. Imports `common::errors::ZeroclawError` and
  `common::selector::select_model`.

## Verification Commands Run
```
cargo check -p op-plugins                                          # green
cargo test -p op-plugins --lib state_plugins::zeroclaw::tests      # ok (10 passed)
cargo clippy -p op-plugins --lib                                   # clean (only pre-existing op-state warn)
```

## Known Risks / Blocked Items
- **Bridge wiring is NOT yet done.** `dispatch_zeroclaw_method` is the
  plugin-owned target, but `SchemaEngine::dispatch_method_call` does not yet call
  it. The integration (T-22) needs an extension point in the bridge: for
  `plugin_id == "zeroclaw"`, route the validated `(method, json_args)` to
  `op_plugins::state_plugins::zeroclaw::dispatch_zeroclaw_method` using
  `ZeroclawPlugin::current_state()` as the state source, then (a) merge the
  domain `result` into the dispatch return value and (b) emit `signal` if present.
  `op-grpc-bridge` already depends on `op-plugins`, so no new dependency edge.
- `SetProvider`/`SetModel` here **validate + surface** the effective change; the
  durable write must flow through the bridge `MutationEngine`/`mutate()` path so a
  single accountability event is recorded. Persisting `selected_provider`/
  `selected_model` into the projected present-state is a Projection-phase concern.
- Signal emission is returned as data (`DispatchOutcome.signal`); actual D-Bus
  `emit_signal` happens at the bridge integration.

## Next-Agent Dependencies
- Bridge/Projection Agent (T-15…T-18, T-22): consume `dispatch_zeroclaw_method`
  and `DispatchOutcome` from the bridge dispatcher; emit signals; persist
  `Set*` mutations via `MutationEngine`.
