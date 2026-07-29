# Requirements — remove-projection-static-tree

## Context

The odbus control plane has a projection layer (`crates/op-projection`) that maintains
a read-only materialised view of state. It works by polling the D-Bus system bus every
5 seconds, re-reading all plugin state, and writing a projection snapshot. This design
has multiple fatal flaws:

1. **Polling** — the projection client (`op-web/src/projection_client.rs`) queries
   `org.opdbus.ProjectedObjectV1` on the system bus every 5 seconds. This violates the
   "no polling" principle: the source knows when it mutates and must announce.
2. **Wrong bus** — the projection lives on the system bus; everything else runs on the
   session bus at `unix:path=/run/opdbus/session-bus.sock`.
3. **Authority confusion** — a read-only lens cannot author state, yet projection_client
   queries it as if it were a primary source.
4. **Dead code** — `op-dbus-mirror` is an orphan crate (no binary, no runit service,
   sole dependent is itself).

The correct architecture already exists in embryonic form:
`op_core::projection_shm::write_projection` is called at mutation sites
(`mutation_engine.rs:230`, `:520`, `:1203`). The mutation engine already pushes state
into `/dev/shm/opdbus/state/<plugin>/<key>` atomically. What is missing is a D-Bus
**signal** that tells consumers "this subtree changed" so they can read the new value
from shm without polling.

This spec removes the projection layer entirely and replaces the consumer path with a
push-notified static tree.

---

## Blast Radius

7 call sites in 6 `op-web` files collapse to 3 distinct reads:

| Read pattern | Files | Shm path |
|---|---|---|
| Zeroclaw subtree (model routes, selected model) | `zeroclaw_routes.rs:76`, `routes/llm.rs:80`, `handlers/chat.rs:140+188`, `routes/chat.rs:74+109`, `handlers/zeroclaw.rs:403` | `/dev/shm/opdbus/state/zeroclaw/*` |
| `system.memory` + `system.load` | `handlers/dashboard.rs:40`, `:41` | `/dev/shm/opdbus/state/system/{memory,load}` |
| Whole-tree dump | `handlers/dashboard.rs:115` | `/dev/shm/opdbus/state/` (directory walk) |

All other `get_projection` / `get_all_projections` calls are internal to `op-projection`
and die with the crate.

---

## Functional Requirements

### REQ-1 — Push Signal on Mutation

**REQ-1.1** The D-Bus service `org.opdbus.v1.PluginV1` (owned by `op-grpc-bridge` on
the session bus `unix:path=/run/opdbus/session-bus.sock`) MUST emit a signal
`Updated(data_json: &str)` whenever any plugin state is written to shm. The signal
payload is a JSON object: `{"plugin": "<name>", "key": "<key>"}`.

**REQ-1.2** The signal MUST be emitted from the same codepath that calls
`write_projection()` in `mutation_engine.rs`. It MUST NOT be emitted from a polling
loop, a timer, or a deferred queue.

**REQ-1.3** If multiple keys are written atomically in one mutation batch, the signal
MAY be emitted once with an array payload: `{"plugin": "<name>", "keys": ["k1","k2"]}`.
A per-key signal is also acceptable.

**REQ-1.4** The signal interface is `org.opdbus.v1.PluginV1`. The signal name is
`Updated`. This matches the signature previously defined in
`op-projection/src/dbus_server.rs:127` and is being ported, not invented.

**REQ-1.5** The `Updated` signal MUST carry the new state data as its payload — not
merely a notification that something changed. The ported signature is
`updated(data_json: &str)` where `data_json` is a JSON object containing at minimum
`{"plugin": "<name>", "key": "<key>"}` (or `"keys"` for batch). This is a stated
invariant: if the signal were ever reduced to a bare "something changed" notification,
every subscriber would need to read back to discover what changed, reintroducing polling
through the side door and silently voiding REQ-7. Subscribers MUST be able to act on
the signal payload alone without an additional query.

---

### REQ-2 — Static Tree Read (Consumer Side)

**REQ-2.1** Consumers (op-web route handlers) MUST read plugin state directly from the
shm static tree at `/dev/shm/opdbus/state/<plugin>/<key>`. They MUST NOT call a D-Bus
method to retrieve state.

**REQ-2.2** Consumers that need reactivity (e.g., SSE push to frontend) MUST subscribe
to the `Updated` signal and re-read the relevant shm path on receipt.

**REQ-2.3** The whole-tree dump (`handlers/dashboard.rs:115`) MUST walk
`/dev/shm/opdbus/state/` and aggregate all key files. No D-Bus call required.

**REQ-2.4** Consumers MUST NOT cache shm state in-process beyond the lifetime of a
single HTTP request handler invocation unless they are subscribed to `Updated` and
invalidate on receipt.

---

### REQ-3 — Cold-Start Hydration

**REQ-3.1** On process join (op-web start, new subscriber), the initial state MUST be
read from a single rotating Btrfs snapshot volume. This is ONE atomic volume read, not a
D-Bus fan-out or iterative query.

**REQ-3.2** After the initial volume read, the consumer transitions to push-only mode —
no further volume reads, no polling.

**REQ-3.3** The Btrfs seed volume is produced by the mutation engine at a cadence
independent of this spec (existing snapshot logic in `op-blockchain`). This spec requires
only that the consumer can read it at startup.

**REQ-3.4** If the seed volume is empty or missing (first boot), the consumer MUST
proceed with an empty state tree and hydrate solely from subsequent `Updated` signals.
This is the present-state today (`/dev/shm/opdbus/state/` is empty, `GetAllProperties`
returns `"{}"`). This is correct, not a bug.

---

### REQ-4 — Deletion of Projection Infrastructure

**REQ-4.1** `crates/op-projection` (19 files) MUST be deleted entirely. All workspace
references (`Cargo.toml:37`, `:79`) MUST be removed.

**REQ-4.2** `crates/op-dbus-mirror` MUST be deleted entirely. This supersedes
`.kiro/specs/op-dbus-mirror-event-session-refactor/` — that spec is voided by this
deletion. Its tasks are NOT to be completed.

**REQ-4.3** `crates/op-web/src/projection_client.rs` MUST be deleted entirely.

**REQ-4.4** All call sites in op-web that reference `projection_client` MUST be rewritten
to read directly from the shm static tree per REQ-2.

**REQ-4.5** References in install scripts (`install/3tched-artix-s6-install.sh`,
`install/3tched-artix-runit-install.sh`) to op-projection or its binary MUST be removed.

---

### REQ-5 — Bus Contention Resolution

**REQ-5.1** `op-projection/src/dbus_server.rs:271` claims `org.opdbus.v1.plugins` —
this claim dies with the crate. No action beyond deletion.

**REQ-5.2** `op-grpc-bridge/src/schema_router.rs:698` is the live owner of
`org.opdbus.v1.PluginV1` on the session bus. The `Updated` signal (REQ-1) is added here.

**REQ-5.3** After deletion, exactly ONE process owns `org.opdbus.v1.PluginV1`: the
`op-grpc-bridge` binary on the session bus. No other process may claim this name.

---

### REQ-6 — Btrfs Snapshot Rehoming

**REQ-6.1** The Btrfs subvolume/snapshot code currently in `op-blockchain` (reachable
only via the `op-projection` binary path) MUST be preserved. It is NOT deleted.

**REQ-6.2** The snapshot code MUST remain callable from the mutation engine for seed
volume rotation. Its current location in `op-blockchain` is acceptable; if it is
unreachable after op-projection deletion, it must be re-exported or moved.

**REQ-6.3** The seed volume path is `/var/lib/opdbus/snapshots/latest`. The mutation
engine writes to it; consumers read from it at cold start only.

**REQ-6.4** Snapshot reachability MUST be proven BEFORE `crates/op-projection` is
deleted. The Btrfs subvolume/snapshot path currently fires only via the projection binary
(`Created BTRFS subvolume: "/var/lib/opdbus/blockchain/state"`, `Streaming blockchain
initialized ... every 15 minutes interval`, `Created snapshot: SNP-state-000001`). If
Phase 2 proceeds without this proof, seed-volume production stops silently and cold start
degrades permanently to the REQ-3.4 empty-tree path — an invisible regression. This
verification is a hard gate on Phase 2.

---

### REQ-7 — No Polling Anywhere

**REQ-7.1** No code path introduced or retained by this change may use a timer, sleep
loop, or periodic query to discover state changes. The mutation source announces; the
consumer subscribes.

**REQ-7.2** The existing `write_projection()` calls in `mutation_engine.rs` are retained
— they are the PUSH side. They do not poll.

**REQ-7.3** Health-check endpoints that report "is the service alive" are NOT polling —
they respond to inbound requests. These are acceptable.

---

## Non-Functional Requirements

**NFR-1 — Latency**: Signal delivery from mutation to consumer MUST complete within the
D-Bus session bus dispatch latency (< 10ms typical on localhost Unix socket).

**NFR-2 — No New Dependencies**: This work uses `zbus` (already in workspace), shm file
I/O (std), and Btrfs snapshot reads (already in workspace). No new crates.

**NFR-3 — Backward Compatibility**: External consumers that read
`/dev/shm/opdbus/state/<plugin>/<key>` today continue to work unchanged. The only
breaking change is removal of the D-Bus interface `org.opdbus.ProjectedObjectV1` on the
system bus — no external consumer uses it (only `projection_client.rs` did).

**NFR-4 — Idempotent Deletion**: Removing `op-projection` and `op-dbus-mirror` from the
workspace must not break `cargo build --workspace`. All inter-crate dependency edges
must be verified clean before deletion.

---

## Flag Only (Out of Scope)

**FLAG-1 — rusqlite**: `op-cache` and `op-introspection` depend on `rusqlite`. This
spec flags their existence but does NOT modify them. A separate spec will address
migration to CozoDB/RocksDB.

**FLAG-2 — Xray config**: `/etc/xray/xray_config.json` is not written by any tooling
in this change. This constraint is noted for avoidance.

---

## Reconciliation with Prior Specs

**`.kiro/specs/op-dbus-mirror-event-session-refactor/`**: That spec planned to evolve
op-dbus-mirror into an event-driven session model. This spec supersedes it entirely.
op-dbus-mirror is dead code and is deleted. The event-driven model it aspired to is
achieved by the `Updated` signal on `org.opdbus.v1.PluginV1` + shm static tree — a
simpler, correct implementation that does not require a dedicated mirror daemon.
