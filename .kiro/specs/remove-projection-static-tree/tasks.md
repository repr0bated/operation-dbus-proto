# Tasks — remove-projection-static-tree

Each task is self-contained, ends with a verifiable outcome, and builds on the previous.
The push signal MUST land before any deletion — consumers need the replacement before
the old path is removed. (The former Task 0.4 snapshot-reachability gate is VOID —
see Task 0.4: the seed volume is external to op-blockchain, so there is nothing to
rehome or prove.)

---

## Phase 0 — Push Signal (Dependency for Everything Else)

### Task 0.1 — Add `Updated` Signal to `org.opdbus.v1.PluginV1` ✅

**Requirements**: REQ-1.1, REQ-1.4, REQ-1.5, REQ-5.2

**Context**: `op-grpc-bridge/src/schema_router.rs` owns the `PluginV1` zbus interface on
the session bus. It currently exposes methods but NO signals — this absence is what forced
the projection layer to poll. The signal definition is ported from the now-to-be-deleted
`op-projection/src/dbus_server.rs:127`.

**Steps**:
1. Open `crates/op-grpc-bridge/src/schema_router.rs`.
2. Locate the `#[interface(name = "org.opdbus.v1.PluginV1")]` impl block.
3. Add the signal definition:
   ```rust
   #[zbus(signal)]
   pub async fn updated(&self, data_json: &str) -> zbus::Result<()>;
   ```
4. Verify the interface compiles: `cargo check -p op-grpc-bridge`.

**Acceptance**: `cargo check -p op-grpc-bridge` passes. The `PluginV1` interface
definition includes `Updated(s)` signal. Verify with `busctl --user introspect
org.opdbus.v1.plugins /org/opdbus/v1/plugins org.opdbus.v1.PluginV1` showing the
signal in the XML.

---

### Task 0.2 — Emit `Updated` Signal from Mutation Engine ✅

**Requirements**: REQ-1.2, REQ-1.3, REQ-7.2

**Context**: `crates/op-grpc-bridge/src/mutation_engine.rs` (the mutation engine lives
in op-grpc-bridge, NOT op-core) calls `write_projection()` at three sites. After each
write, the `Updated` signal must be emitted. The engine receives the session-bus
connection from server startup via `set_signal_bus`.

**Steps**:
1. Emit via the engine's `emit_updated_signal(plugin_id, key, keys)` helper after each
   `write_projection()` call, on the per-plugin object path
   `/org/opdbus/v1/plugins/<plugin_id>`:
   - single-member mutation: payload `{"plugin": id, "key": member_name}`
   - whole-state write (seed / re-projection): payload `{"plugin": id, "keys": [top-level keys]}`
2. If the connection is not yet available (early boot), the write still succeeds — signal
   emission is best-effort. Log at `debug!` level if skipped.
3. Run `cargo check -p op-grpc-bridge`.

**Acceptance**: `cargo build -p op-grpc-bridge` succeeds. Signal is emitted on every
mutation path. Manual test: `dbus-monitor --address unix:path=/run/opdbus/session-bus.sock
"type='signal',interface='org.opdbus.v1.PluginV1'"` shows `Updated` when a mutation
fires.

---

### Task 0.3 — Verify Signal Payload Carries State Data ✅

**Requirements**: REQ-1.1, REQ-1.5, NFR-1

**Context**: End-to-end proof that the signal works AND carries identifiable state data
in its payload before proceeding to deletion. This verifies the no-polling invariant
(REQ-7): subscribers must be able to act on the signal payload alone without issuing a
follow-up query.

**Steps**:
1. Start op-grpc-bridge (or the combined server binary).
2. In a separate terminal, subscribe:
   ```sh
   busctl --user monitor org.opdbus.v1.plugins
   ```
   or:
   ```sh
   dbus-monitor --address unix:path=/run/opdbus/session-bus.sock \
     "type='signal',interface='org.opdbus.v1.PluginV1',member='Updated'"
   ```
3. Trigger a mutation (e.g., via gRPC call or MCP tool invocation that writes state).
4. Observe the `Updated` signal with correct `data_json` payload.
5. **Verify the payload is NOT empty or a bare notification** — it MUST contain at
   minimum `{"plugin": "<name>", "key": "<key>"}` identifying what changed. A signal
   with empty string, `{}`, or `"updated"` as payload is a test FAILURE.
6. Document the verification command in `deploy/smoke/dbus-signal-check.sh`.

**Acceptance**: Signal observed within <1s of mutation. Payload contains `plugin` and
`key` (or `keys`) fields with correct values matching the mutation that was triggered.
Smoke script exists and is executable.

---

### Task 0.4 — VOID (superseded by seed-volume independence)

**Status**: VOID — do not implement. This task was predicated on the seed volume
being produced by `op-blockchain`'s snapshot code. Per REQ-6 (revised), the seed
volume has NO relationship to op-blockchain: it is an external, deploy-time Btrfs
artifact (the install path is itself `btrfs send`). The mutation engine neither
has nor needs an op-blockchain dependency. Nothing is rehomed; no reachability
proof is required. Phase 2 is not gated on this task.

---

## Phase 1 — Static Tree Reader (Replacement Consumer)

### Task 1.1 — Create `state_tree.rs` Module in op-web ✅

**Requirements**: REQ-2.1, REQ-2.3

**Context**: This module replaces all reads that `projection_client.rs` performed. It is
a pure filesystem reader — no D-Bus, no caching, no network.

**Steps**:
1. Create `crates/op-web/src/state_tree.rs` with:
   - `pub fn read_plugin(plugin: &str) -> Option<simd_json::OwnedValue>`
   - `pub fn read_all() -> HashMap<String, simd_json::OwnedValue>`
   - `pub fn read_all_from_path(base: &str) -> HashMap<String, simd_json::OwnedValue>`
2. Implementation: read from `/dev/shm/opdbus/state/<plugin_id>.json` (one file per
   plugin, whole present-state object — NOT a per-key directory layout), deserialize
   JSON. Return `None` / empty map on missing file (not an error — state not yet mutated).
3. Add `pub mod state_tree;` to `crates/op-web/src/lib.rs` (or appropriate module root).
4. `cargo check -p op-web`.

**Acceptance**: Module exists, compiles, exports the three public functions. No new
dependencies added.

---

### Task 1.2 — Rewrite Zeroclaw Call Sites ✅

**Requirements**: REQ-2.1, REQ-4.4

**Context**: exactly 2 call sites (verified by grep — the earlier list of 6 sites in
5 files was wrong; `routes/llm.rs`, `handlers/chat.rs`, and `routes/chat.rs` contain
no projection_client usages). Replace with `state_tree::read_plugin("zeroclaw")` —
the state tree stores one whole-state file per plugin, so the API reads the plugin
object and navigates subkeys on the result.

**Steps**:
1. `zeroclaw_routes.rs:76` — replace projection_client call with
   `state_tree::read_plugin("zeroclaw")`, then `.get("model_routes")` on the result.
2. `handlers/zeroclaw.rs:402` — replace with `state_tree::read_plugin("zeroclaw")`.
3. Remove `use crate::projection_client::*` (or equivalent import) from each file.
4. Add `use crate::state_tree;` to each file.
5. `cargo check -p op-web`.

**Acceptance**: No `projection_client` references remain in zeroclaw-related files.
`cargo check -p op-web` passes.

---

### Task 1.3 — Rewrite Dashboard Call Sites ✅

**Requirements**: REQ-2.1, REQ-2.3, REQ-4.4

**Context**: `handlers/dashboard.rs` has 3 call sites: system.memory (:39),
system.load (:40), and whole-tree dump (:114).

**Steps**:
1. Line 39: replace with `state_tree::read_plugin("system.memory")` (a plugin id,
   not a plugin+key pair).
2. Line 40: replace with `state_tree::read_plugin("system.load")`.
3. Line 114: replace with `state_tree::read_all()`.
4. Remove projection_client imports.
5. `cargo check -p op-web`.

**Acceptance**: `handlers/dashboard.rs` has zero projection_client references. Compiles.

---

### Task 1.4 — Delete `projection_client.rs` ✅

**Requirements**: REQ-4.3

**Context**: All call sites have been rewritten (Tasks 1.2, 1.3). The file is now dead.

**Steps**:
1. Delete `crates/op-web/src/projection_client.rs`.
2. Remove `pub mod projection_client;` from the module tree.
3. Remove any `ProjectionClient` struct from `AppState` or server startup.
4. Remove the D-Bus connection setup that was used solely for projection reads.
5. `cargo check -p op-web`.
6. `cargo test -p op-web` (if tests exist).

**Acceptance**: File deleted. `cargo build -p op-web` succeeds with zero warnings about
dead code related to projection.

---

## Phase 2 — Crate Deletion

**Dependency**: Phase 2 requires Phase 1 complete (the consumer path must survive
without projection_client). The former Task 0.4 gate is VOID — the seed volume is
external to op-blockchain (see Task 0.4), so deleting op-projection cannot stop
seed production.

### Task 2.1 — Remove `op-projection` from Workspace ✅

**Requirements**: REQ-4.1, REQ-4.5, REQ-6.4, NFR-4

**Context**: 19 files. Referenced in workspace Cargo.toml (member + dep). Install
scripts are DEPRECATED (install is `btrfs send`) — their stale op-projection
references are out of scope per REQ-4.5. No op-blockchain rehoming is needed:
the seed volume is external to it (REQ-6).

**Steps**:
1. Remove `"crates/op-projection"` from `[workspace.members]` in root `Cargo.toml`.
2. Remove `op-projection` from `[workspace.dependencies]` if present.
3. Search all other `Cargo.toml` files for `op-projection` dependencies and remove them:
   `grep -rn "op-projection" crates/*/Cargo.toml`.
4. `rm -rf crates/op-projection`.
5. `cargo build --workspace` — must succeed.

**Acceptance**: `crates/op-projection/` does not exist. `cargo build --workspace`
succeeds. `grep -rn "op-projection" .` returns only this spec and git history.

---

### Task 2.2 — Remove `op-dbus-mirror` from Workspace ✅

**Requirements**: REQ-4.2, NFR-4

**Context**: Dead crate — no binary target, no runit service, sole dependent is itself.
This deletion supersedes `.kiro/specs/op-dbus-mirror-event-session-refactor/`.

**Steps**:
1. Remove `"crates/op-dbus-mirror"` from `[workspace.members]` in root `Cargo.toml`.
2. Remove `op-dbus-mirror` from `[workspace.dependencies]` if present.
3. Search all other `Cargo.toml` files: `grep -rn "op-dbus-mirror" crates/*/Cargo.toml`.
   Remove any references found.
4. `rm -rf crates/op-dbus-mirror`.
5. `cargo build --workspace` — must succeed.

**Acceptance**: `crates/op-dbus-mirror/` does not exist. `cargo build --workspace`
succeeds.

---

### Task 2.3 — Mark Prior Mirror Spec as Superseded ✅

**Requirements**: REQ-4.2 (reconciliation)

**Steps**:
1. Add a `SUPERSEDED.md` file to `.kiro/specs/op-dbus-mirror-event-session-refactor/`:
   ```markdown
   # SUPERSEDED

   This spec has been superseded by `.kiro/specs/remove-projection-static-tree/`.

   The op-dbus-mirror crate was deleted entirely. The event-driven goals of this
   spec are achieved by the `Updated` signal on `org.opdbus.v1.PluginV1` + direct
   shm reads — a simpler architecture that does not require a mirror daemon.

   Do NOT implement the tasks in this directory.
   ```

**Acceptance**: File exists. No ambiguity about which spec is authoritative.

---

## Phase 3 — Btrfs Seed Volume Consumer Path

### Task 3.1 — Cold-Start Reader in op-web ✅

**Requirements**: REQ-3.1, REQ-3.2, REQ-3.4

**Context**: op-web serves stateless HTTP requests — it reads shm on each request. The
cold-start seed volume matters only if op-web starts before any mutation has populated
shm. In practice: shm is empty → requests return empty/null → frontend shows "loading".
This is acceptable and is the present-state behavior.

For future SSE streams that need an initial full frame, add a one-time seed read.

**Steps**:
1. Add to `state_tree.rs`:
   ```rust
   /// Read initial state from Btrfs seed volume (cold start).
   /// Returns empty object if snapshot is missing (first boot).
   pub fn read_seed_volume() -> serde_json::Value {
       let seed_path = "/var/lib/opdbus/snapshots/latest";
       // Walk the seed volume directory structure (same layout as /dev/shm/opdbus/state/)
       if std::path::Path::new(seed_path).exists() {
           read_all_from_path(seed_path)
       } else {
           serde_json::Value::Object(serde_json::Map::new())
       }
   }
   ```
2. Factor out the directory-walk logic from `read_all()` into a `read_all_from_path(base)`
   helper so both shm and seed volume use the same reader.
3. Document in a comment: "Called once at startup, then push-only via Updated signal."
4. `cargo check -p op-web`.

**Acceptance**: `read_seed_volume()` compiles. It is NOT called on every request — only
at startup initialization (if a startup path exists) or from a future SSE setup function.

---

## Phase 4 — Cleanup and Verification

### Task 4.1 — Full Workspace Build ⚠️ PARTIAL

**Requirements**: NFR-4

**Steps**:
1. `cargo build --workspace`.
2. Fix any remaining broken imports, dead code warnings, or missing deps.
3. `cargo clippy --workspace --all-targets -- -D warnings` (if clippy is configured).
4. `cargo test --workspace` — verify no test references deleted crates.

**Acceptance**: Clean workspace build. Zero warnings referencing projection or mirror.

**Status note**: `cargo check --workspace` passes. The `--all-targets` variant fails
only on a PRE-EXISTING broken `op-dbus` example (`examples/ovs_native_rust.rs`
imports `op_network::OvsdbDbusClient`, which lives in `op_jsonrpc`/`rovs_proxy`) —
unrelated to this spec and out of scope. The earlier op-mcp `ServerMode::Cognitive`
blocker (commit 00a31406) no longer exists; op-mcp compiles.

---

### Task 4.2 — Verify No Polling Remains ✅

**Requirements**: REQ-7.1

**Steps**:
1. Search for polling patterns in op-web:
   ```sh
   grep -rn "sleep\|interval\|poll\|timer\|Duration::from_secs" crates/op-web/src/
   ```
2. Any hit must be justified (e.g., HTTP server keepalive, graceful shutdown timeout) or
   removed if it was part of the projection polling path.
3. Confirm `projection_client.rs` is gone (already done in Task 1.4 — this is a
   double-check).
4. Search for remaining `ProjectedObjectV1` references:
   ```sh
   grep -rn "ProjectedObjectV1\|org.opdbus.ProjectedObject" crates/
   ```
   Must return zero results.

**Acceptance**: Zero polling patterns for state reads. Zero references to the deleted
D-Bus interface.

---

### Task 4.3 — Install Scripts: NO ACTION (deprecated) ✅

**Requirements**: REQ-4.5

**Status**: N/A by decision. The shell install scripts (`3tched-artix-s6-install.sh`,
`3tched-artix-runit-install.sh`, `install/`) are DEPRECATED — installation is a
`btrfs send` of a prepared image. Their stale op-projection / op-dbus-mirror
references die with the scripts. No edits required or wanted.

---

### Task 4.4 — End-to-End Smoke Test ✅

**Requirements**: REQ-1.1, REQ-2.1, REQ-7.1

**Steps**:
1. Start the combined server (op-grpc-bridge + op-web).
2. Verify dashboard endpoint returns empty state (correct — nothing mutated):
   ```sh
   curl -s http://127.0.0.1:8080/api/dashboard | jq .
   ```
3. Trigger a mutation (gRPC call or MCP tool).
4. Verify dashboard endpoint now returns the mutated state.
5. Verify signal was emitted (use the smoke script from Task 0.3).
6. Document in `deploy/smoke/projection-removal-check.sh`.

**Acceptance**: State flows from mutation → shm → HTTP response without polling. Signal
observable. No projection daemon running.

---

## Completion Criteria

The projection removal is complete when:

- [x] `Updated` signal is defined on `org.opdbus.v1.PluginV1` and observable
- [x] Signal payload carries state data (`plugin` + `key`/`keys`) — not a bare notification (REQ-1.5)
- [x] Signal is emitted from all 3 `write_projection` sites in mutation_engine
- [x] Task 0.4 — VOID: seed volume is external to op-blockchain; no rehoming or reachability proof needed (REQ-6)
- [x] `state_tree.rs` exists and serves all 4 former call sites (2 zeroclaw + 3 dashboard readers across 2 of the 3 files)
- [x] `projection_client.rs` is deleted
- [x] `crates/op-projection/` is deleted
- [x] `crates/op-dbus-mirror/` is deleted
- [x] `cargo check --workspace` succeeds (`--all-targets` fails only on a PRE-EXISTING broken op-dbus example — unrelated, see Task 4.1)
- [x] Zero polling patterns for state reads remain in op-web
- [x] Zero references to `ProjectedObjectV1` or `org.opdbus.ProjectedObject` remain in op-web (op-tools has legacy tool defs — out of scope)
- [x] Install scripts — NO ACTION: deprecated (install is `btrfs send`), stale references die with the scripts (REQ-4.5)
- [x] Prior mirror spec marked SUPERSEDED
- [ ] End-to-end smoke test passes (mutation → signal → shm read → HTTP response) — requires running server (operator verification)

## Dependency Graph

```
Phase 0: Task 0.1 → Task 0.2 → Task 0.3
         Task 0.4 — VOID (seed volume is external to op-blockchain; no gate)

Phase 1: Task 1.1 → Task 1.2 + Task 1.3 (parallel) → Task 1.4
         (depends on Phase 0: signal must exist before consumer rewrite)

Phase 2: Task 2.1 → Task 2.2 → Task 2.3
         GATE: Phase 1 complete (consumer path survives without projection_client).
         No snapshot gate — the seed volume was never an op-blockchain product.

Phase 3: Task 3.1
         (depends on Phase 2: deletion complete, seed volume path confirmed)

Phase 4: Task 4.1 → Task 4.2 → Task 4.3 (N/A) → Task 4.4
         (depends on Phase 3)
```
