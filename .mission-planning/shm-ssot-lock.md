# SHM Single-Source-of-Truth Lock

> **Locked: 2026-06-04.** This is an architectural invariant, not a preference.
> Violations of this lock are bugs by definition.

## The Rule

**There is one source of truth: `PluginSchema → SchemaEngine → /dev/shm/live-schema.json`.**

No monitoring. No caches. No database replicas. No IDL snapshots. No in-process state stores.

The daemon is a transport/execution proxy. It holds `rovs-jsonrpc::Connection` and `VConn` for I/O
only. When a `transact` succeeds, the result is written into SchemaEngine → `/dev/shm`. That IS
the state update. Consumers read from `/dev/shm` (1:1 direct read, zero-copy, The Sled).

gRPC subscription signals mean "state changed in /dev/shm" — not "OVSDB update received".
Consumers receive the signal, then read current state from `/dev/shm` directly.

---

## What this deprecates — specific code sites

### 1. OVSDB IDL monitors — DEPRECATED

| Site | Pattern | Lines | Replacement |
|---|---|---|---|
| `op-network/ovsdb.rs:752` | `OvsdbClient::monitor_db("Open_vSwitch")` — `mpsc::Receiver<Value>` from IDL pump | ~90 | Delete. Consumers read from `/dev/shm`, subscribe to gRPC "state changed" |
| `op-jsonrpc/ovsdb.rs:445` | `OvsdbClient::monitor_db()` — second implementation, same pattern | ~30 | Delete. Same replacement |
| `op-grpc-bridge/schema_engine.rs:271` | `self.ovsdb.monitor_db("Open_vSwitch").await` → feeds `process_authoritative_change` | ~40 | Rework: daemon writes transact results → SchemaEngine → `/dev/shm`. No monitor. |
| `op-dbus-mirror/event_sources/ovsdb.rs:31` | `ovsdb.monitor_db("Open_vSwitch").await?` — 5-file streaming consumer | ~70 | Read from `/dev/shm` + gRPC "state changed" signal |

### 2. In-process state caches — DEPRECATED

| Site | Pattern | Lines | Replacement |
|---|---|---|---|
| `op-grpc-bridge/schema_engine.rs:66` | `state_cache: Arc<RwLock<HashMap<String, OwnedValue>>>` | ~20 | Delete. `/dev/shm` is the cache. |
| `op-grpc-bridge/schema_engine.rs:474-496` | `read_cached_state()`, `update_state_cache()`, `remove_cached_state()` | ~25 | Delete. Read from `/dev/shm` via SchemaEngine. |
| `op-plugins/full_system.rs:181` | `state_cache: Arc<RwLock<Option<FullSystemState>>>` | ~15 | Read from `/dev/shm` via SledReader. |

### 3. OVSDB IDL snapshots — DEPRECATED

| Site | Pattern | Lines | Replacement |
|---|---|---|---|
| `op-network/ovsdb.rs:896-940` | `idl_snapshot(idl: &rovs_ovsdb::Idl) → Value` — full table dump | ~45 | Delete. SchemaEngine writes to `/dev/shm` directly. |

### 4. SqlitePluginCatalog — DEPRECATED (per AGENTS.md §2)

| Site | Pattern | Lines | Replacement |
|---|---|---|---|
| `op-dbus-model/lib.rs:49-53` | `SqlitePluginCatalog` struct + `SqliteSchemaCatalog` type alias | ~80 | SchemaEngine + `/dev/shm`. AGENTS.md §2 already declares this obsolete. |
| `op-plugins/registry.rs:32` | `schema_catalog_store: Option<Arc<SqlitePluginCatalog>>` | ~20 | Delete. SchemaEngine is the catalog. |

### 5. NonNetDb — DELETED (stopgap, not renamed)

`NonNetDb` was an in-memory `HashMap<String, Vec<Value>>` pretending to be an OVSDB database
(`OpNonNet`) for non-OVS plugin state. It is **deleted entirely** — not absorbed, not renamed.

Non-OVS plugins (netmaker, wireguard, hardware, software, etc.) are just plugins. Their state
goes into `SchemaEngine → /dev/shm` alongside OVS state, governed by their own
`PluginSchema` entries in `plugin_schema_defs.rs`. No "NonNet" namespace, no fake OVSDB
facade, no `OpNonNet` JSON-RPC server.

| Site | Pattern | Lines | Replacement |
|---|---|---|---|
| `op-jsonrpc/nonnet.rs` | `NonNetDb` — in-memory HashMap + broadcast channels | ~480 | Delete. Plugin state → SchemaEngine → `/dev/shm` |
| `op-jsonrpc/nonnet_staging.rs` | `OpNonNet` JSON-RPC server — fake OVSDB query iface | ~150 | Delete. D-Bus + `/dev/shm` replaces it. OVSDB-compat concerns are handled by plugin schema definitions. |
| `op-grpc-bridge/schema_engine.rs:75` | `pub nonnet: Arc<NonNetDb>` — in-process replica | ~15 | Delete. Plugin state → SchemaEngine → `/dev/shm` |
| `op-grpc-bridge/schema_engine.rs:254-270` | `nonnet_rx = self.nonnet.subscribe()` | ~20 | Delete. gRPC signals from `/dev/shm` writes |
| `op-web/bin/op-dbus.rs:36` | `let nonnet = Arc::new(NonNetDb::new())` | ~5 | Delete. SchemaEngine owns all plugin state. |
| `op-dbus-mirror/jsonrpc_interface.rs:234` | `get_schema(["OpNonNet"])` query | ~10 | Delete. No OpNonNet. Schema from PluginSchema. |
| `op-dbus-mirror/bin/ovs-dbus-init.rs:53` | `nonnet.load_from_plugins(&plugin_state)` | ~5 | Delete. SchemaEngine loads plugin schemas. |

### 6. rovs-ovsdb::Client IDL methods — NOT USED by daemon

| Method | Status | Reason |
|---|---|---|
| `Client::start_monitor()` | NOT USED | Daemon doesn't maintain IDL replica |
| `Client::wait()` | NOT USED | No monitor to wait on |
| `Client::run()` | NOT USED | No IDL to drain |
| `Client::idl()` | NOT USED | State is in `/dev/shm`, not in IDL |
| `Client::fetch_schema()` | NOT USED | PluginSchema IS the schema |
| `Client::cancel_monitor()` | NOT USED | No monitor to cancel |

The daemon uses `rovs-jsonrpc::Connection::transact()` and `Connection::notify()` only — the
raw I/O primitives. `rovs-ovsdb::Client` is not part of the daemon design.

---

## What IS the source of truth

```
PluginSchema (in plugin_schema_defs.rs)
   • netmaker_plugin_schema()
   • wireguard_plugin_schema()
   • ovsdb_bridge_plugin_schema()
   • hardware_plugin_schema()
   • ... (30+ plugins — no "NonNet" namespace)
     │
     ▼
SchemaEngine (op-projection/src/schema_engine.rs)
     │
     ▼ register_schema() / validate_schema()
     │
     ▼ write_schemas_to_shm()
     │
/dev/shm/live-schema.json    ←  THE SLED  ←  ONE SOURCE OF TRUTH
     │
     ▼ 1:1 direct read (zero-copy)
     │
Consumers: UI, gRPC, snowball, MCP, plugins, op-dbus-mirror, op-web
```

- **`op-projection::SchemaEngine`** — the authoritative registry. Validates schemas, persists
  catalog to `/dev/shm`, maintains audit trail with Blake3 footprints. ALL plugin state — OVS
  and non-OVS — flows through this single engine.
- **`/dev/shm/live-schema.json`** — the materialized catalog. This is what consumers read.
  No distinction between "OVS state" and "NonNet state" — it's all just plugin state.
- **`op-grpc-bridge::SchemaEngine`** — **reworked** to be a thin write-through: daemon transact
  result → `op-projection::SchemaEngine::write_schemas_to_shm()` → gRPC "state changed" signal.
  No `OvsdbClient`, no `NonNetDb`, no `state_cache`, no `monitor_db`.
- **NonNetDb is deleted** — it was a stopgap. Non-OVS plugins (netmaker, wireguard, hardware,
  etc.) are just plugins with their own `PluginSchema` entries. Their state goes into SchemaEngine
  alongside OVS state. No "OpNonNet" fake-OVSDB facade, no separate namespace.

---

## Implications for milestones

- **M2 (gRPC transport):** gRPC subscription signals originate from `/dev/shm` writes, not from
  OVSDB monitor subscriptions. The daemon writes to SchemaEngine → `/dev/shm` → emits signal.
- **M3 (OvsdbClient delete):** `monitor_db()` consumers don't migrate to a new monitor — they
  migrate to `/dev/shm` reads + gRPC signals. This is a different API shape than the current
  `mpsc::Receiver<Value>` stream.
- **M5 (schema plugin):** `rovs_commands_plugin_schema()` defines the schema that governs what
  OVSDB operations are valid. This IS the schema — no runtime `fetch_schema()` needed.
- **op-grpc-bridge rework:** Must happen before or during M3. Currently owns `Arc<OvsdbClient>`,
  `Arc<NonNetDb>`, `state_cache`, `monitor_db` subscription — all deprecated by this lock.
