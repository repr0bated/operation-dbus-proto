# Design — remove-projection-static-tree

## Architecture Question: Can the Projection Layer Be Removed Without Losing Functionality?

**Yes. The projection layer is a read-only lens that re-reads what the mutation engine
already writes. Every read it serves can be served directly from the shm static tree
that the mutation engine already maintains. Removing projection removes an unnecessary
intermediate, a 5-second polling loop, a system-bus binding on the wrong bus, and
~19 source files.**

---

## Before (Current Architecture)

```
op-grpc-bridge/mutation_engine.rs
    │
    ├──► write_projection()  → /dev/shm/opdbus/projections/<plugin_id>.json  (ATOMIC FILE WRITE)
    │                          (NOTE: previously the projections/ dir; readers
    │                           were pointed at state/ — the two never met)
    └──► (no signal emitted)
                                              ╔══════════════════════════╗
                                              ║   op-projection daemon   ║
                                              ║                          ║
op-web/projection_client.rs ──5s poll──►      ║ org.opdbus.ProjectedObj  ║
                                              ║ (SYSTEM bus)             ║
                                              ║                          ║
                                              ║ reads shm, reformats,   ║
                                              ║ re-serves via D-Bus      ║
                                              ╚══════════════════════════╝
```

Problems:
- `projection_client.rs` polls every 5 seconds — stale by definition.
- The projection daemon is on the SYSTEM bus; the rest of the plane is on the SESSION bus.
- The daemon re-reads what mutation already wrote — adds latency, zero value.
- `op-dbus-mirror` was intended to complement this but is dead: no binary, no service.

---

## After (Target Architecture)

```
op-grpc-bridge/mutation_engine.rs
    │
    ├──► write_projection()  → /dev/shm/opdbus/state/<plugin_id>.json
    │                          (one file per plugin, whole present-state object,
    │                           atomic temp+rename)
    │
    └──► emit Updated signal on org.opdbus.v1.PluginV1 (SESSION bus)
              at object path /org/opdbus/v1/plugins/<plugin_id>
              payload: {"plugin","key"} or {"plugin","keys":[...]}
              │
              ├──► op-web: re-reads shm path on signal receipt (SSE/reactive)
              ├──► op-web: reads shm path directly on HTTP request (non-reactive)
              └──► any future consumer: subscribes to signal, reads shm

Cold start:
    consumer ──► read /var/lib/opdbus/snapshots/latest (ONE Btrfs volume,
                 produced externally at deploy time — see §4)
             ──► then subscribe to Updated signal (push only from here)
```

**No intermediate daemon. No polling. Source announces, consumers subscribe.**

---

## Component Design

### 1. Push Signal — `Updated` on `org.opdbus.v1.PluginV1`

#### Location

`crates/op-grpc-bridge/src/schema_router.rs` — this file already owns the
`org.opdbus.v1.PluginV1` interface on the session bus. The signal is added to the
existing zbus interface impl (`SchemaBackedInterface::updated`).

#### Signal Definition

```rust
// In the #[zbus::interface(name = "org.opdbus.v1.PluginV1")] impl block:

#[zbus(signal)]
pub async fn updated(signal_emitter: &zbus::object_server::SignalEmitter<'_>, data_json: &str) -> zbus::Result<()>;
```

The `data_json` payload identifies what changed — a single mutated member:
```json
{"plugin": "zeroclaw", "key": "selected_model"}
```

Or the top-level subtrees affected by a whole-state write:
```json
{"plugin": "zeroclaw", "keys": ["selected_model", "model_routes"]}
```

#### Emission Site

The signal is emitted from `crates/op-grpc-bridge/src/mutation_engine.rs` (the
mutation engine lives in op-grpc-bridge, NOT op-core) after each
`write_projection()` succeeds, via the engine's `emit_updated_signal(plugin_id,
key, keys)` helper. The engine receives the session-bus connection from server
startup (`set_signal_bus`), and the signal is emitted on the per-plugin object
path:

```rust
// After write_projection(plugin_id, json) at the three write sites:
self.emit_updated_signal(&plugin_id, member_name.as_deref(), keys).await;
// payload: {"plugin": plugin_id, "key": member}        (single-member mutation)
//       or {"plugin": plugin_id, "keys": [...]}        (whole-state write)
```

If the connection is unavailable (early boot before the bridge registers
interfaces), the write still succeeds — the signal is best-effort for
reactivity, shm is the source of truth.

#### Bus Topology

- **Bus**: Session bus at `unix:path=/run/opdbus/session-bus.sock`
- **Well-known name**: `org.opdbus.v1.plugins` (owned by op-grpc-bridge)
- **Object path**: `/org/opdbus/v1/plugins/<plugin_id>` (one object per plugin)
- **Interface**: `org.opdbus.v1.PluginV1`
- **Signal**: `Updated(s)` where `s` is JSON string

---

### 2. Static Tree Reader — Replaces `projection_client.rs`

#### Module

New: `crates/op-web/src/state_tree.rs`

This is a thin file-read utility. It does NOT maintain a cache, does NOT poll, and does
NOT hold a D-Bus connection.

#### API (as implemented)

`write_projection()` stores ONE file per plugin
(`/dev/shm/opdbus/state/<plugin_id>.json`) containing the plugin's whole
present-state object — the per-key directory layout originally sketched here
does not match the writer. The reader API mirrors that:

```rust
/// Read a plugin's full present-state. None if not yet mutated (REQ-3.4).
pub fn read_plugin(plugin: &str) -> Option<simd_json::OwnedValue>

/// Walk the state dir; map keyed by plugin_id (".json" stripped, dotfiles skipped).
pub fn read_all() -> HashMap<String, simd_json::OwnedValue>

/// Shared directory-walk used by read_all and read_seed_volume.
pub fn read_all_from_path(base: &str) -> HashMap<String, simd_json::OwnedValue>

/// One-time cold-start read of the seed volume; empty map if missing.
pub fn read_seed_volume() -> HashMap<String, simd_json::OwnedValue>
```

Consumers navigate the returned state object for the subkeys they need
(e.g. `state.get("model_routes")`). Uses `simd_json` per workspace convention.

No D-Bus. No network. Pure filesystem reads of files that `write_projection()` already
maintains atomically (write-to-tmp + rename).

---

### 3. Call Site Rewrites (op-web)

#### Zeroclaw Subtree (actual call sites)

**Before**:
```rust
let proj = projection_client.get_projection("zeroclaw").await?;
```

**After**:
```rust
let zeroclaw = state_tree::read_plugin("zeroclaw")?;   // whole state object
```

Actual files (the originally listed `routes/llm.rs`, `handlers/chat.rs`,
`routes/chat.rs` had no projection_client usages — verified by grep):
- `zeroclaw_routes.rs:76` — reads `model_routes` from the state object
- `handlers/zeroclaw.rs:402` — reads `projection` (providers, model_routes, tools)

#### System Metrics

**After**:
```rust
let mem = state_tree::read_plugin("system.memory");   // /dev/shm/opdbus/state/system.memory.json
let load = state_tree::read_plugin("system.load");    // /dev/shm/opdbus/state/system.load.json
```

(`system.memory` / `system.load` are plugin ids, not plugin+key pairs.)

#### Whole-Tree Dump

**After** (`handlers/dashboard.rs:114`):
```rust
let projections = state_tree::read_all();
```

---

### 4. Cold-Start Hydration

#### Seed Volume

Path: `/var/lib/opdbus/snapshots/latest` — a Btrfs subvolume containing a snapshot of
`/dev/shm/opdbus/state/` at the time of the last rotation.

#### Consumer Startup Sequence

```
1. Read /var/lib/opdbus/snapshots/latest → populate in-memory state (if snapshot exists)
2. Subscribe to Updated signal on session bus
3. From this point: only signal-driven reads from /dev/shm/opdbus/state/
```

For op-web (stateless HTTP handlers), step 1 is optional — each request reads shm
directly. The seed volume matters for consumers that maintain an in-memory aggregate
(e.g., a future SSE stream that needs to send a full initial frame).

#### Snapshot Production — EXTERNAL to op-snowball

The seed volume has **no relationship to `op-snowball`**. The snowball is the
per-mutation durability chain; the seed volume is a deploy-time artifact. It is
produced outside the runtime code in this workspace (Btrfs snapshot/send of the
state tree at install/deploy time — the install path itself is `btrfs send`,
the legacy shell install scripts being deprecated). This spec only requires the
consumer read path (`state_tree::read_seed_volume()`), which treats a missing
volume as first boot (empty tree, REQ-3.4). No snapshot cadence is defined or
modified by this spec, and no op-snowball code is reachable from, or required
by, the mutation engine for seed production.

---

### 5. Crate Deletion

#### `crates/op-projection` — DELETE

19 files. Remove from:
- `Cargo.toml` workspace members
- `Cargo.toml` workspace dependencies (if present)

(Install scripts are deprecated — see the note at the end of this section.)

`op-snowball` is entirely untouched by this spec — the seed volume is external
to it (see §4). Nothing needs rehoming because nothing in the runtime depended
on the projection binary for snapshot production.

#### `crates/op-dbus-mirror` — DELETE

Dead crate. No binary, no runit service, sole workspace dependent is itself.
Remove from:
- `Cargo.toml` workspace members
- Any cross-references in other Cargo.toml dep tables

This supersedes `.kiro/specs/op-dbus-mirror-event-session-refactor/`. That spec's
goals (event-driven, no polling) are achieved by the simpler `Updated` signal design
without requiring a mirror daemon.

#### `crates/op-web/src/projection_client.rs` — DELETE

Entire file. All imports and usages rewritten to `state_tree::read_plugin` /
`state_tree::read_all`.

#### Install scripts — NO ACTION (deprecated)

The shell install scripts (`3tched-artix-*-install.sh`, `install/`) are
deprecated: installation is a `btrfs send` of a prepared image. Their stale
op-projection / op-dbus-mirror references die with the scripts and are out of
scope for this spec.

---

### 6. Bus Contention Resolution

| Before | After |
|---|---|
| `op-projection/dbus_server.rs:271` claims `org.opdbus.v1.plugins` on SYSTEM bus | Deleted. Claim disappears. |
| `op-grpc-bridge/schema_router.rs:698` owns `org.opdbus.v1.PluginV1` on SESSION bus | Sole owner. Gains `Updated` signal. |

Post-change, exactly one process owns the plugin interface: `op-grpc-bridge` on the
session bus. No contention. No system bus usage for plugin state.

---

## Data Flow Diagram

```
┌─────────────────────┐
│   mutation_engine    │
│                      │
│  mutate(plugin, k, v)│
│    │                 │
│    ├─► write_projection(plugin, k, v)    ──► /dev/shm/opdbus/state/{plugin}/{k}
│    │                 │
│    └─► emit Updated signal               ──► D-Bus session bus
│                      │                         │
└─────────────────────┘                         │
                                                 │
        ┌────────────────────────────────────────┘
        │
        ▼
┌───────────────────────────────────┐       ┌──────────────────────────┐
│  op-web (HTTP request handler)    │       │  op-web (SSE subscriber) │
│                                   │       │                          │
│  state_tree::read_key(p, k)       │       │  on Updated signal:      │
│    └─► read /dev/shm/.../p/k      │       │    read_key(p, k)        │
│                                   │       │    push to SSE stream    │
└───────────────────────────────────┘       └──────────────────────────┘

Cold start:
┌───────────────────────────────────┐
│  read /var/lib/opdbus/snapshots/  │
│  latest (ONE Btrfs volume read)   │
│    └─► populate initial state     │
│                                   │
│  then: subscribe Updated signal   │
│  (push only from here on)         │
└───────────────────────────────────┘
```

---

## What Is NOT Changing

- `op_core::projection_shm::write_projection` — KEPT as the single write door.
  RETARGETED: it now writes `/dev/shm/opdbus/state/<plugin_id>.json` (where
  schema_router and state_tree read) instead of the orphaned `projections/` dir.
- One-file-per-plugin layout (`<plugin_id>.json`, whole state object) — KEPT.
- `op-snowball` — UNTOUCHED. It is the per-mutation durability chain and has no
  relationship to the cold-start seed volume (§4).
- `op-grpc-bridge` schema_router.rs — MODIFIED (gains `Updated` signal), not deleted.
- The broadcast `StateChange` payload on the gRPC event stream — UNCHANGED (the
  `{"data","_introspection"}` composite is retained there; only the shm file holds
  raw present state).
- Install scripts — UNCHANGED (deprecated; install is `btrfs send`).

---

## Risks and Mitigations

| Risk | Mitigation |
|---|---|
| Signal lost during burst mutations | Shm is source of truth. Consumer re-reads on next request. Signal is for reactivity, not correctness. |
| Shm file read races with write | `write_projection` uses atomic rename (write tmp, rename). Readers see complete files or previous version. |
| Cold start with empty snapshot | Consumer proceeds with empty state. First mutations populate via signals. Present-state today is already empty — no regression. |
| Seed volume never produced | Acceptable: it is an external deploy-time artifact (§4). Missing volume = first boot, handled by REQ-3.4. No runtime code depends on it. |
| Signal payload reduced to bare notification | REQ-1.5 mandates data payload. Task 0.3 verifies payload content. Regression would void REQ-7 by reintroducing polling through the side door. |

---

## File Ownership Summary

| File | Action | Reason |
|---|---|---|
| `crates/op-projection/` (19 files) | DELETE | Entire projection layer removed |
| `crates/op-dbus-mirror/` | DELETE | Dead code, superseded |
| `crates/op-web/src/projection_client.rs` | DELETE | Polling client on wrong bus |
| `crates/op-web/src/state_tree.rs` | CREATE | Direct shm reader (replacement) |
| `crates/op-grpc-bridge/src/schema_router.rs` | MODIFY | Add `Updated` signal to PluginV1 |
| `crates/op-grpc-bridge/src/mutation_engine.rs` | MODIFY | Emit signal after write_projection; write raw present state |
| `crates/op-core/src/projection_shm.rs` | MODIFY | Retarget write path to `/dev/shm/opdbus/state/` |
| `crates/op-web/src/zeroclaw_routes.rs` | MODIFY | Use state_tree::read_plugin |
| `crates/op-web/src/handlers/zeroclaw.rs` | MODIFY | Use state_tree::read_plugin |
| `crates/op-web/src/handlers/dashboard.rs` | MODIFY | Use state_tree::read_plugin + read_all |
| `Cargo.toml` (workspace root) | MODIFY | Remove op-projection, op-dbus-mirror members |

(`routes/llm.rs`, `handlers/chat.rs`, `routes/chat.rs` were listed in an earlier
draft; they contain no projection_client usages. Install scripts are deprecated —
install is `btrfs send` — and are intentionally untouched.)
