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
mutation_engine.rs
    │
    ├──► write_projection()  → /dev/shm/opdbus/state/<plugin>/<key>  (ATOMIC FILE WRITE)
    │
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
mutation_engine.rs
    │
    ├──► write_projection()  → /dev/shm/opdbus/state/<plugin>/<key>  (unchanged)
    │
    └──► emit Updated signal on org.opdbus.v1.PluginV1 (SESSION bus)
              │
              ├──► op-web: re-reads shm path on signal receipt (SSE/reactive)
              ├──► op-web: reads shm path directly on HTTP request (non-reactive)
              └──► any future consumer: subscribes to signal, reads shm

Cold start:
    consumer ──► read /var/lib/opdbus/snapshots/latest (ONE Btrfs volume)
             ──► then subscribe to Updated signal (push only from here)
```

**No intermediate daemon. No polling. Source announces, consumers subscribe.**

---

## Component Design

### 1. Push Signal — `Updated` on `org.opdbus.v1.PluginV1`

#### Location

`crates/op-grpc-bridge/src/schema_router.rs` — this file already owns the
`org.opdbus.v1.PluginV1` interface on the session bus. The signal is added to the
existing zbus interface impl.

#### Signal Definition

```rust
// In the #[interface(name = "org.opdbus.v1.PluginV1")] impl block:

#[zbus(signal)]
async fn updated(&self, data_json: &str) -> zbus::Result<()>;
```

The `data_json` payload is:
```json
{"plugin": "zeroclaw", "key": "selected_model"}
```

Or for batch mutations:
```json
{"plugin": "zeroclaw", "keys": ["selected_model", "model_routes"]}
```

#### Emission Site

The signal is emitted from `mutation_engine.rs` after `write_projection()` succeeds.
The mutation engine already holds a D-Bus connection handle (used for other operations).
The signal emission call is:

```rust
// After write_projection(plugin, key, &value) at :230, :520, :1203
if let Some(iface_ref) = connection
    .object_server()
    .interface::<_, PluginV1Interface>("/org/opdbus/v1/plugins")
    .await
    .ok()
{
    let _ = PluginV1Interface::updated(
        iface_ref.signal_emitter(),
        &serde_json::to_string(&json!({"plugin": plugin, "key": key})).unwrap(),
    ).await;
}
```

If the connection is unavailable (early boot before grpc-bridge starts), the write
still succeeds — the signal is best-effort for reactivity, shm is the source of truth.

#### Bus Topology

- **Bus**: Session bus at `unix:path=/run/opdbus/session-bus.sock`
- **Well-known name**: `org.opdbus.v1.plugins` (owned by op-grpc-bridge)
- **Object path**: `/org/opdbus/v1/plugins`
- **Interface**: `org.opdbus.v1.PluginV1`
- **Signal**: `Updated(s)` where `s` is JSON string

---

### 2. Static Tree Reader — Replaces `projection_client.rs`

#### Module

New: `crates/op-web/src/state_tree.rs`

This is a thin file-read utility. It does NOT maintain a cache, does NOT poll, and does
NOT hold a D-Bus connection.

#### API

```rust
/// Read a single key from the static tree.
/// Returns None if the file does not exist (not yet mutated).
pub fn read_key(plugin: &str, key: &str) -> Option<serde_json::Value> {
    let path = format!("/dev/shm/opdbus/state/{plugin}/{key}");
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Read all keys for a plugin.
pub fn read_plugin(plugin: &str) -> serde_json::Value {
    let dir = format!("/dev/shm/opdbus/state/{plugin}");
    let mut map = serde_json::Map::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if let Ok(bytes) = std::fs::read(entry.path()) {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    let key = entry.file_name().to_string_lossy().into_owned();
                    map.insert(key, val);
                }
            }
        }
    }
    serde_json::Value::Object(map)
}

/// Walk the entire state tree (for dashboard dump).
pub fn read_all() -> serde_json::Value {
    let mut tree = serde_json::Map::new();
    if let Ok(plugins) = std::fs::read_dir("/dev/shm/opdbus/state") {
        for plugin_dir in plugins.flatten() {
            if plugin_dir.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let name = plugin_dir.file_name().to_string_lossy().into_owned();
                tree.insert(name.clone(), read_plugin(&name));
            }
        }
    }
    serde_json::Value::Object(tree)
}
```

No D-Bus. No network. Pure filesystem reads of files that `write_projection()` already
maintains atomically (write-to-tmp + rename).

---

### 3. Call Site Rewrites (op-web)

#### Zeroclaw Subtree

**Before** (all 6 call sites):
```rust
let proj = projection_client.get_projection("zeroclaw").await?;
let selected = proj.get("selected_model")...;
```

**After**:
```rust
let selected = state_tree::read_key("zeroclaw", "selected_model");
```

Specific files:
- `zeroclaw_routes.rs:76` — reads model routes
- `routes/llm.rs:80` — reads selected model
- `handlers/chat.rs:140`, `:188` — reads selected model for chat dispatch
- `routes/chat.rs:74`, `:109` — reads selected model
- `handlers/zeroclaw.rs:403` — reads model config

#### System Metrics

**Before**:
```rust
let mem = projection_client.get_projection("system").await?.get("memory");
let load = projection_client.get_projection("system").await?.get("load");
```

**After**:
```rust
let mem = state_tree::read_key("system", "memory");
let load = state_tree::read_key("system", "load");
```

#### Whole-Tree Dump

**Before**:
```rust
let all = projection_client.get_all_projections().await?;
```

**After**:
```rust
let all = state_tree::read_all();
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

#### Snapshot Production

Already exists in `op-blockchain`. The mutation engine calls snapshot rotation on a
cadence (configurable, default: every 100 mutations or 5 minutes, whichever comes first).
This spec does not modify the snapshot cadence — it only requires the consumer can read
the result.

---

### 5. Crate Deletion

#### `crates/op-projection` — DELETE

19 files. Remove from:
- `Cargo.toml` workspace members (line 37)
- `Cargo.toml` workspace dependencies (line 79, if present)
- `install/3tched-artix-s6-install.sh` (service setup for op-projection)
- `install/3tched-artix-runit-install.sh` (runit sv dir for op-projection)

The Btrfs snapshot code referenced from op-projection (lives in op-blockchain) is NOT
deleted — only the call path through op-projection's binary is removed. The mutation
engine retains its own path to the same code.

#### `crates/op-dbus-mirror` — DELETE

Dead crate. No binary, no runit service, sole workspace dependent is itself.
Remove from:
- `Cargo.toml` workspace members
- Any cross-references in other Cargo.toml dep tables

This supersedes `.kiro/specs/op-dbus-mirror-event-session-refactor/`. That spec's
goals (event-driven, no polling) are achieved by the simpler `Updated` signal design
without requiring a mirror daemon.

#### `crates/op-web/src/projection_client.rs` — DELETE

Entire file. All imports and usages rewritten to `state_tree::read_key` /
`state_tree::read_all`.

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

- `op_core::projection_shm::write_projection` — KEPT. This is the write half. Already
  correct. Already called from mutation_engine at all three sites.
- `/dev/shm/opdbus/state/` file layout — KEPT. Consumers that read this today continue
  to work.
- `op-blockchain` Btrfs snapshot code — KEPT. Only the call path from op-projection is
  removed; the mutation_engine's path is retained.
- `op-grpc-bridge` schema_router.rs — MODIFIED (gains `Updated` signal), not deleted.
- Runit service dirs for other services — UNCHANGED. Only the op-projection service
  dir is removed.

---

## Risks and Mitigations

| Risk | Mitigation |
|---|---|
| Signal lost during burst mutations | Shm is source of truth. Consumer re-reads on next request. Signal is for reactivity, not correctness. |
| Shm file read races with write | `write_projection` uses atomic rename (write tmp, rename). Readers see complete files or previous version. |
| Cold start with empty snapshot | Consumer proceeds with empty state. First mutations populate via signals. Present-state today is already empty — no regression. |
| Btrfs snapshot code becomes unreachable after deletion | Proven reachable in Phase 0 (Task 0.4) BEFORE deletion proceeds. Phase 2 is gated on this proof. |
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
| `crates/op-core/src/mutation_engine.rs` | MODIFY | Emit signal after write_projection |
| `crates/op-web/src/zeroclaw_routes.rs` | MODIFY | Use state_tree::read_key |
| `crates/op-web/src/routes/llm.rs` | MODIFY | Use state_tree::read_key |
| `crates/op-web/src/handlers/chat.rs` | MODIFY | Use state_tree::read_key |
| `crates/op-web/src/routes/chat.rs` | MODIFY | Use state_tree::read_key |
| `crates/op-web/src/handlers/zeroclaw.rs` | MODIFY | Use state_tree::read_key |
| `crates/op-web/src/handlers/dashboard.rs` | MODIFY | Use state_tree::read_key + read_all |
| `Cargo.toml` (workspace root) | MODIFY | Remove op-projection, op-dbus-mirror members |
| `install/3tched-artix-runit-install.sh` | MODIFY | Remove op-projection service setup |
| `install/3tched-artix-s6-install.sh` | MODIFY | Remove op-projection service setup |
