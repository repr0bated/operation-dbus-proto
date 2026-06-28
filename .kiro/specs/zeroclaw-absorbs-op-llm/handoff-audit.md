# Handoff — Review/Bypass Audit (T-80)

## Pre-existing known risks (flagged)

### Flaky capability-grant tests (interceptor.rs)
Two tests fail under parallel execution on a clean baseline:
- `interceptor::tests::interceptor_extracts_footprint_and_session_id`
- `schema_router::tests::required_capability_check_allows_granted`

Root cause: race on shared sled/capability-grants global state when tests run in
parallel. They pass in isolation and with `--test-threads=1`. These are
environment-dependent test isolation issues, not regressions from this mission.

### op-state clippy warning
`crates/op-state/src/dbus_server.rs:202` emits `empty_line_after_doc_comments`
(rejects `-D warnings`). This is pre-existing tech debt in op-state, out of scope
for this mission since op-state was not modified.

## Signal emission pending (T-22 wire)
`DispatchOutcome.signal` is populated by the plugin-owned dispatch handlers (e.g.,
`ExecutionAuthorized`, `ProviderChanged`) but the bridge `call` path has no
`SignalContext`, so actual D-Bus signal emission is not yet wired through. This
is documented in `handoff-dbus.md` and is a straightforward follow-up: pass the
signal through `SchemaEngine` or have `SchemaBackedInterface::call` emit it.

## Single schema source satisfied
- `op-plugins` defines the Zeroclaw schema (`schemars::schema_for!(ZeroclawState)`)
- `op-llm/src/schema.rs` re-exports it as an include (embedded, one source)
- The bridge auto-generates D-Bus/gRPC surface from this shared schema
- No duplicate schema definitions exist for LLM routing

## Workspace status
```
cargo check --workspace                           # green
cargo test -p op-plugins --lib -- --test-threads=1 # 18 passed
cargo test -p op-grpc-bridge --lib -- --test-threads=1 # 56 passed
cargo test -p op-llm --lib                       # passes (no op-llm unit tests)
cargo clippy -p op-grpc-bridge --lib -p op-plugins --lib
                                               # clean on changed files
```
