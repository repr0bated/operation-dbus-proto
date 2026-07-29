# Requirements — dead-signal-and-tool-cleanup

## Context

`.kiro/specs/remove-projection-static-tree/` shipped and is live (commit `39bde466`,
deployed 2026-07-29, verified against the running host — `op-web-server` on the new
binary, service healthy, `op-projection` service directory gone). It left two loose
ends, both discovered by direct code inspection after deployment, neither caught by
that spec's own completion criteria:

1. **The `Updated` signal it added has zero subscribers.** `org.opdbus.v1.PluginV1`
   gained an `Updated(data_json: &str)` signal
   (`crates/op-grpc-bridge/src/schema_router.rs:834-835`), correctly emitted from all
   3 mutation write sites via `emit_updated_signal()`
   (`crates/op-grpc-bridge/src/mutation_engine.rs:159-188`, called at lines 277, 571,
   1253). A workspace-wide search for a receiver (`receive_updated`, `MatchRule`,
   `signal_stream`, anything binding to this signal) returns nothing. The signal is
   correctly built and fires correctly into a void.

2. **op-tools still targets the D-Bus interface `remove-projection-static-tree`
   deleted.** That prior spec's own NFR-3 claimed "no external consumer uses
   [`org.opdbus.ProjectedObjectV1`] (only `projection_client.rs` did)" — this was
   wrong. Two op-tools code paths reference it:
   - `crates/op-tools/src/discovery/projection_engine.rs:157-166` — a branch inside
     `ProjectionEngine` (which DOES run live, instantiated at op-web startup,
     `crates/op-web/src/state.rs:638` and `:654`) that matches on
     `iface.name == "org.opdbus.ProjectedObjectV1"`. Since `op-projection` — the sole
     owner of that interface — is deleted, this D-Bus interface no longer exists
     anywhere on either bus. The branch is not broken (no error, no panic); it simply
     never matches, silently registering zero plugin-projection tools forever.
   - `crates/op-tools/src/builtin/plugin_projection.rs` (209 lines) — the
     `PluginProjectionTool` the dead branch above would have constructed, plus
     `register_plugin_projection_tools()`, which has **zero real callers** —
     confirmed by workspace search; the only two hits outside its own file are doc
     comments in `crates/op-plugins/src/state_plugins/compact_mcp.rs:6` and
     `cognitive_mcp.rs:6` describing it as something that "can expose" state, never
     invoking it.

Prior spec's own completion criteria already flagged this half-honestly: "Zero
references to `ProjectedObjectV1` ... remain in op-web (op-tools has legacy tool defs
— out of scope)." This spec closes that gap.

---

## Decision — Disposition of the `Updated` Signal

Two live, independent reactive paths now exist for "state changed":

| Path | Transport | Scope | Status |
|---|---|---|---|
| `change_tx` (mutation_engine.rs:65) → gRPC `subscribe` (grpc_server.rs:874) → op-web SSE bridge (state.rs:321) | in-process broadcast + gRPC stream | browser / anything with gRPC access | **live today**, predates this spec entirely (SIGNALS.md 2026-07-22 confirms it in production) |
| `Updated` signal on `org.opdbus.v1.PluginV1` | raw D-Bus signal, session bus | anything with D-Bus access but NOT gRPC | **built, zero subscribers** |

**REQ-D1** The `Updated` signal MUST NOT be consumed by adding a D-Bus subscriber
inside `op-web` or `op-grpc-bridge`. Both processes already have live reactivity via
`change_tx`; a D-Bus round-trip to notify a process of its own in-process broadcast is
pure overhead with no capability gain. This is the "don't add abstractions beyond what
the task requires" line from CLAUDE.md, applied.

**REQ-D2** The signal's actual justification is CLAUDE.md's own invariant: "D-Bus is
the only control plane." A process with D-Bus access but no gRPC client (an operator
at a terminal, a future agent that only speaks D-Bus) must be able to observe state
changes without polling. The signal exists to serve THAT consumer, not to duplicate
op-web's SSE path.

**REQ-D3** The signal MUST remain defined and emitted exactly as-is
(`schema_router.rs:834`, `mutation_engine.rs:159-188`) — no code change to the
emit side. It is correct.

**REQ-D4** A real, minimal external consumer MUST exist to prove the signal is not
theoretical dead weight: a smoke-test script using `busctl` or `dbus-monitor` against
the live session bus socket (`/run/opdbus/session-bus.sock`), demonstrating a
mutation → signal → payload roundtrip from a process outside op-web/op-grpc-bridge.
This was partially done already (`remove-projection-static-tree` Task 0.3 verified
the payload manually) but no persisted script exists — `deploy/smoke/dbus-signal-check.sh`
was specified but never confirmed written. This spec makes it durable.

**REQ-D5** Document the signal's purpose (D-Bus-native consumers only, not an op-web
internal dependency) directly in the doc comment above the signal definition in
`schema_router.rs`, so the next person reading the code doesn't repeat this
investigation.

---

## Requirements — op-tools Cleanup

**REQ-T1** The `if iface.name == "org.opdbus.ProjectedObjectV1"` branch in
`crates/op-tools/src/discovery/projection_engine.rs:157-166` MUST be deleted. The
interface it matches against no longer exists on any bus. `ProjectionEngine` itself
is NOT deleted — it performs other live discovery work; only this dead branch goes.

**REQ-T2** `crates/op-tools/src/builtin/plugin_projection.rs` MUST be deleted in its
entirety. After REQ-T1, its sole constructor call site (`PluginProjectionTool::new_generic`
in the deleted branch) is gone, and `register_plugin_projection_tools()` already has
zero real callers. Nothing depends on this file surviving.

**REQ-T3** `pub mod plugin_projection;` MUST be removed from
`crates/op-tools/src/builtin/mod.rs:17`.

**REQ-T4** The doc-comment references to `register_plugin_projection_tools` in
`crates/op-plugins/src/state_plugins/compact_mcp.rs:6` and
`crates/op-plugins/src/state_plugins/cognitive_mcp.rs:6` MUST be updated — they
describe a function that will no longer exist. Either remove the sentence or repoint
it at the actual current mechanism these files use to expose state as MCP tools
(verify what that is before editing — do not guess).

**REQ-T5** `cargo build --workspace` MUST pass after all deletions. `cargo check -p
op-tools -p op-plugins` MUST pass with zero warnings referencing
`plugin_projection` or `ProjectedObjectV1`.

---

## Non-Functional Requirements

**NFR-1** No behavior change to any currently-working tool. `ProjectionEngine`'s other
discovery branches (non-`ProjectedObjectV1` interfaces) are untouched.

**NFR-2** No new dependencies, no new crates.

**NFR-3** This spec does not touch `crates/op-web` — its reactivity path is already
correct and is explicitly out of scope (see REQ-D1).

---

## Verification Baseline (as of 2026-07-29, this investigation)

- `39bde466` is `HEAD` on `agent/zeroclaw-runtime-routing`, matches `origin` exactly.
- `op-web-server` redeployed, service healthy, `GET /` → 200.
- `cargo build --workspace` (no `--all-targets`) passes clean.
- `cargo check --workspace --all-targets` has exactly one pre-existing, unrelated
  failure (`examples/ovs_native_rust.rs`, `OvsdbDbusClient` naming — traced to commit
  `e32ecacc`, predates this work by multiple sessions). Not in scope here.
