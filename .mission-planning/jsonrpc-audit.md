# rovs-jsonrpc / rovs-ovsdb Transport Audit

> Audit date: 2026-06-04. Method: grep + file reads against `crates/` and vendored
> `rovs-jsonrpc-0.2.0` / `rovs-ovsdb-0.2.0` sources.
> Purpose: Find all code that bypasses `rovs-jsonrpc::Connection` / `rovs-ovsdb::Client`
> and does OVSDB JSON-RPC I/O by hand, identify why, and map migration targets.

---

## rovs-jsonrpc::Connection — what it provides

| Feature | Implementation |
|---|---|
| **Persistent session** | `Connection::new(stream: Stream)` — wraps a `Stream` in read/write halves, maintains `read_buf` + `next_id` counter + `pending_notifications` queue |
| **Brace-depth JSON framing** | `recv_message()` tracks `{`/`}` depth (respecting strings/escapes) because OVSDB **does not send newlines after responses** — this is the #1 framing gotcha that every hand-rolled client gets wrong differently |
| **Request/response matching** | `transact(method, params)` auto-assigns ID, sends, blocks for matching response ID, buffers interleaved notifications |
| **Notification queue** | `has_pending_notifications()`, `pop_notification()`, `drain_notifications()` — notifications received while waiting for responses are automatically buffered |
| **Send raw messages** | `send_message(&Message)`, `notify(method, params)` — low-level access when needed |
| **Typed error parsing** | `RpcError` handles both OVSDB `{"error": {...}, "details": ...}` and unixctl plain-string format |

## rovs-ovsdb::Client — what it adds on top

| Feature | Implementation |
|---|---|
| **Schema fetch + IDL replica** | `Client::connect(addr)` → `get_schema` → `start_monitor` → full in-memory replica of OVSDB tables |
| **Monitor V1/V2/V3** | `monitor`, `monitor_cond`, `monitor_cond_since` — all three OVSDB monitor protocols |
| **Transaction commit** | `commit(&mut Transaction)` — builds params, sends, processes result (uuid_map resolution) |
| **Blocking wait** | `wait()` — blocks for next update notification, handles echo internally |
| **Non-blocking drain** | `run()` — drains buffered notifications without blocking |
| **Echo reply** | Automatically sends echo replies when server pings (JSON-RPC "echo" method) |
| **Cancel monitor** | `cancel_monitor()` — sends `monitor_cancel` |

---

## Who ALREADY uses rovs-jsonrpc / rovs-ovsdb correctly

| Crate | File | Usage |
|---|---|---|
| `op-network` | `ovsdb.rs` (OvsdbClient, 991 lines) | `rovs_ovsdb::Client::connect_with_config()` + persistent IDL monitor pump + `Client::transact()` + `Client::commit()` + `Client::wait()` |
| `op-network` | `bin/op-ovsbr0-setup.rs` | `rovs_jsonrpc::Connection::new(stream)` + `Connection::transact()` — **the only direct user of rovs-jsonrpc** in the workspace |
| `op-network` | `bin/op-ovsbr0-afxdp.rs` | `rovs_ovsdb::Client` — bridge/port setup |

These are correct. Everything below is NOT using rovs-jsonrpc/rovs-ovsdb.

---

## Hand-rolled OVSDB JSON-RPC clients — detailed audit

### Client A: `op-jsonrpc/src/ovsdb.rs` (OvsdbClient)

**File:** `crates/op-jsonrpc/src/ovsdb.rs` (~446 lines)

**Pattern:** Per-call `UnixStream::connect(db.sock)` + `stream.shutdown()` + `read_to_end`

```rust
// Line 44-46: fresh connection per call
let mut stream = UnixStream::connect(&self.socket_path).await?;
// ...write request + "\n"...
stream.shutdown().await?;               // ← HALF-CLOSE
let mut response_bytes = Vec::new();
stream.read_to_end(&mut response_bytes).await?;  // ← read until EOF
```

**Why it doesn't use rovs-jsonrpc:**
1. `op-jsonrpc` doesn't depend on `rovs-*` at all
2. The half-close pattern (`shutdown` write side → `read_to_end`) is a **legacy OVSDB workaround** — OVSDB doesn't send newlines, so this client forces EOF by shutting down the write side. This is **destructive** — the connection can never be reused.
3. Has its own `simd_json`-based parsing with fallback (last-valid-line) for multi-line responses

**Deficiencies vs rovs-jsonrpc::Connection:**
- **No persistent connection** — TCP/Unix handshake per call (3-way overhead on every transact)
- **No request/response ID matching** — hardcoded `"id": 0` on every request, can't pipeline
- **No notification buffering** — can't receive OVSDB `update` notifications at all
- **Broken framing** — `shutdown()` + `read_to_end` only works for request/response; **completely unusable** for `monitor` (long-lived stream)
- **No echo handling** — OVSDB sends periodic `"method":"echo"` requests; this client ignores them, which can cause the server to disconnect

**Has `monitor_db()`:** Yes (line ~446) — but it opens a **second** `UnixStream` with `BufReader::read_line` (line-oriented, NOT brace-depth). This only works if OVSDB happens to newline-terminate monitor updates, which is **not guaranteed** per RFC 7047. This is fragile.

**Migration target:** Route through daemon `RovsJsonRpcProxy::transact` (D-Bus-first path) or use `rovs-ovsdb::Client` with persistent IDL monitor.

---

### Client B: `op-jsonrpc/src/ovsdb_jsonrpc.rs` (OvsdbClient)

**File:** `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs` (~200 lines)

**Pattern:** Per-call `UnixStream::connect(db.sock)` + `BufReader::read_line`

```rust
// Line 24: fresh connection per call
let mut stream = UnixStream::connect(&self.socket_path).await?;
// ...write request + "\n"...
let mut response_line = String::new();
reader.read_line(&mut response_line).await?;  // ← line-oriented read
```

**Why it doesn't use rovs-jsonrpc:** Same as Client A — `op-jsonrpc` doesn't depend on rovs.

**Deficiencies vs rovs-jsonrpc::Connection:**
- All the same as Client A (no persistent connection, no ID matching, no notifications)
- **Line-oriented framing is WRONG for OVSDB** — OVSDB responses may not be newline-terminated (this is documented in `rovs-jsonrpc::connection.rs`'s doc comment). This client can **hang indefinitely** on certain OVSDB responses.
- Uses `simd_json` for parsing; `rovs-jsonrpc` uses `serde_json` (both work, but inconsistent)

**Migration target:** Same as Client A.

---

### Client C: `op-tools/src/builtin/ovsdb.rs` (OvsdbClient)

**File:** `crates/op-tools/src/builtin/ovsdb.rs` (~350 lines)

**Pattern:** Per-call `UnixStream::connect(db.sock)` + `BufReader::read_line` (same framing as Client B)

```rust
// Line 45: fresh connection per call
let stream = UnixStream::connect(&self.socket_path).await?;
let (reader, mut writer) = stream.into_split();
let mut reader = BufReader::new(reader);
// ...write request + "\n"...
let mut response_str = String::new();
reader.read_line(&mut response_str).await?;  // ← line-oriented
```

**Why it doesn't use rovs-jsonrpc:** `op-tools` doesn't depend on rovs.

**Deficiencies vs rovs-jsonrpc::Connection:**
- All the same as Client B (line-oriented framing is wrong, per-call connection overhead)
- **Implements high-level OVSDB operations** (`create_bridge`, `delete_bridge`, `add_port`, `delete_port`, `list_bridges`, `list_ports`, `get_bridge`) by constructing raw OVSDB JSON payloads (`{"op":"insert","table":"Bridge",...}`)
- The **payloads are already in correct OVSDB format** — they just need to be sent through the proxy instead of directly opening a socket

**Key insight:** This client's OVSDB payload construction is exactly what `RovsJsonRpcProxy::transact("transact", params)` needs. The migration is literally "change the transport, keep the payload."

**Migration target:** `RovsJsonRpcProxy::transact` via D-Bus. Payloads are already correct.

---

### Client D: `op-network/src/ovsdb.rs` (OvsdbClient — the one being deleted)

**File:** `crates/op-network/src/ovsdb.rs` (991 lines)

**Pattern:** Persistent `rovs_ovsdb::Client` with background IDL monitor pump + `Arc<Mutex<Option<Client>>>`

**This one IS using rovs correctly** — it wraps `rovs_ovsdb::Client` and adds:
- Lazy connection initialization
- Background `tokio::spawn` IDL pump task
- Thread-safe `Arc<Mutex<...>>` for shared access
- High-level methods (`bridge_exists`, `create_bridge`, `add_port`, `monitor_db`, etc.)
- `transact_simd()` shim for `simd_json::OwnedValue` compatibility

**But it's being deleted** because:
1. It embeds business logic (bridge CRUD) that belongs in plugins
2. It maintains an in-process OVSDB IDL replica — a **second source of truth** competing with `/dev/shm`
3. In the target architecture, the daemon owns a `rovs-jsonrpc::Connection` for **execution only** (transact/notify). It does NOT maintain an `rovs-ovsdb::Client` with IDL monitor. When a transact succeeds, the result is written into `SchemaEngine → /dev/shm` — that IS the state update. Consumers read from `/dev/shm` (1:1 direct read, zero-copy).

**Migration target:** Consumers switch to `RovsJsonRpcProxy::transact` for reads/writes and read current state from `/dev/shm` via `SchemaEngine`. The `monitor_db()` `mpsc::Receiver` pattern is replaced by: (a) gRPC "state changed in /dev/shm" subscription signals, (b) consumer reads `/dev/shm` directly upon signal. The high-level `create_bridge`/`add_port` helpers move into the consuming plugins (which construct their own OVSDB payloads).

---

## The framing problem — why every hand-rolled client is subtly broken

OVSDB (RFC 7047) does **not** send newlines after JSON responses. This is confirmed by `rovs-jsonrpc::connection.rs`'s doc comment:

> OVSDB servers do not send newlines after JSON responses. Instead, this implementation uses brace-depth tracking to detect complete JSON objects.

The three hand-rolled clients each handle this differently:

| Client | Framing strategy | Works? |
|---|---|---|
| `op-jsonrpc/ovsdb.rs` | `shutdown()` write side → `read_to_end()` until EOF | Yes for request/response (but kills connection) |
| `op-jsonrpc/ovsdb_jsonrpc.rs` | `BufReader::read_line()` | **NO** — can hang if OVSDB omits newline |
| `op-tools/builtin/ovsdb.rs` | `BufReader::read_line()` | **NO** — same bug |
| `op-jsonrpc/ovsdb.rs` `monitor_db()` | `BufReader::read_line()` on a persistent stream | **NO** — same bug, monitor updates may not be newline-delimited |
| `rovs-jsonrpc::Connection` | Brace-depth tracking on persistent buffer | **YES** — correct per RFC 7047 |

Two of the three hand-rolled clients have a **latent framing bug** that can cause them to hang indefinitely on certain OVSDB responses. `rovs-jsonrpc` solved this correctly; none of the hand-rolled clients did.

---

## Consolidated migration map

| Client | Crate | Lines | Framing bug? | Has monitor? | Migration |
|---|---|---|---|---|---|
| A `op-jsonrpc/ovsdb.rs` | op-jsonrpc | ~446 | No (shutdown workaround) | Yes (`monitor_db`) | Route through daemon D-Bus `transact` for reads/writes. Read state from `/dev/shm`. gRPC "state changed" signal replaces `monitor_db`. **Delete client.** |
| B `op-jsonrpc/ovsdb_jsonrpc.rs` | op-jsonrpc | ~200 | **YES** (line-oriented) | No | Route through daemon D-Bus `transact`. Read state from `/dev/shm`. **Delete client.** |
| C `op-tools/builtin/ovsdb.rs` | op-tools | ~350 | **YES** (line-oriented) | No | Route through daemon D-Bus `transact` (payloads already correct OVSDB format). **Delete client.** |
| D `op-network/ovsdb.rs` | op-network | ~991 | No (uses rovs correctly) | Yes (via rovs_ovsdb IDL) | **Delete client + IDL monitor.** Daemon uses `rovs-jsonrpc::Connection` for execution only (no IDL replica). Transact results → `SchemaEngine → /dev/shm`. Consumers read from `/dev/shm` + gRPC "state changed" signals. |

**Net effect:** 4 OVSDB client implementations (~2000 lines) → **0** (consumers use D-Bus/gRPC + `/dev/shm`). The daemon holds 1 `rovs-jsonrpc::Connection` internally for execution. No OVSDB IDL replica anywhere.

---

## What the daemon should expose (rovs-jsonrpc surface mapped to D-Bus)

From `rovs-jsonrpc::Connection`:

| D-Bus method on `/org/opdbus/rovs/jsonrpc` | Maps to `Connection` method | Notes |
|---|---|---|
| `Transact(method: s, params: s)` | `Connection::transact(method, params)` | Primary consumer method. Result written to `/dev/shm` via SchemaEngine. |
| `Notify(method: s, params: s)` | `Connection::notify(method, params)` | For cancel_monitor etc. |
| `Pop_notification()` → `s` | `Connection::pop_notification()` | Returns JSON; for advanced consumers |
| `Has_pending_notifications()` → `b` | `Connection::has_pending_notifications()` | Poll check |
| `Pending_notification_count()` → `u` | `Connection::pending_notification_count()` | |

**The daemon does NOT expose `rovs-ovsdb::Client` methods.** `rovs-ovsdb::Client` maintains an IDL
replica — that is a second source of truth. The daemon uses `rovs-jsonrpc::Connection` for
execution only. State lives in `PluginSchema → SchemaEngine → /dev/shm`. Consumers:
1. Call `Transact` to execute OVSDB operations (the daemon writes the result to `/dev/shm`)
2. Read current state from `/dev/shm` (1:1 direct read, zero-copy)
3. Subscribe to gRPC "state changed in /dev/shm" signals (not OVSDB monitor updates)

**What happens to `rovs-ovsdb::Client`'s functionality:**

| `rovs-ovsdb::Client` method | Target architecture path |
|---|---|
| `transact()` | `RovsJsonRpcProxy::Transact("transact", params)` — same underlying `Connection::transact` |
| `commit(&mut Transaction)` | Consumer constructs Transaction JSON, sends via `Transact` |
| `fetch_schema()` / `schema()` | PluginSchema in `plugin_schema_defs.rs` — no need to fetch OVSDB schema at runtime |
| `start_monitor()` | **NOT NEEDED.** State is in `/dev/shm`, not in an IDL replica |
| `wait()` / `run()` | **NOT NEEDED.** Daemon doesn't maintain an IDL replica |
| `cancel_monitor()` | **NOT NEEDED.** No monitor to cancel |
| `send_echo_reply()` | `Connection` handles echo automatically if the daemon needs to keep the session alive |
| `list_dbs()` | `RovsJsonRpcProxy::Transact("list_dbs", [])` — one-shot query |

**Key design point:** The daemon's D-Bus surface exposes the **raw rovs-jsonrpc primitives** (`transact`, `notify`, `send_message`, notification polling). The **rovs-ovsdb::Client** is an internal daemon implementation detail — consumers don't call its methods directly; they construct OVSDB JSON payloads and send them through `Transact`.

---

## Risks

1. **Clients B and C have a framing bug** — they can hang on non-newline-terminated OVSDB responses. This is a **present correctness issue**, not just a migration target. These clients are currently in production use and may silently fail under certain OVSDB server behaviors.

2. **Client A's `monitor_db()` uses `BufReader::read_line`** — same framing bug for monitor subscriptions. OVSDB `update` notifications are not guaranteed to be newline-delimited.

3. **`op-grpc-bridge::SchemaEngine` owns `Arc<OvsdbClient>` + `monitor_db("Open_vSwitch")` subscription** — this is a **second source of truth** competing with `/dev/shm`. Its `process_authoritative_change()` maintains its own `state_cache: HashMap` and `change_tx: broadcast::Sender<StateChange>`. In the target architecture, this entire pattern is replaced by: daemon transact result → write to `SchemaEngine → /dev/shm` → gRPC "state changed" signal. No OVSDB monitor, no IDL replica, no `state_cache`.

4. **Transaction payload compatibility** — Client C constructs OVSDB payloads like `{"op":"insert","table":"Bridge","row":{...},"uuid-name":"new_bridge"}`. These are **wire-compatible** with `rovs_jsonrpc::Connection::transact()` — they're standard RFC 7047 JSON-RPC. Migration is a transport swap, not a payload rewrite.

5. **`op-jsonrpc` crate doesn't depend on rovs** — migrating Clients A and B means either (a) adding rovs deps to op-jsonrpc (against the daemon-first design), or (b) routing through the daemon's D-Bus surface (correct path per AGENTS.md). Option (b) is the right one.

6. **`OvsdbClient::monitor_db()` consumers** — `op-dbus-mirror` (5 files) and `op-grpc-bridge` subscribe to `monitor_db()`'s `mpsc::Receiver<serde_json::Value>`. In the target architecture, they must read from `/dev/shm` instead and subscribe to gRPC "state changed" signals. This is a non-trivial migration: the `monitor_db()` stream delivers per-table OVSDB updates; the `/dev/shm` path delivers the full schema catalog. Consumers that need per-table diffing must compare snapshots or the daemon must write per-table SHM regions.

7. **Schema fetch at runtime** — `rovs-ovsdb::Client::fetch_schema()` loads the OVSDB server's schema. In the target architecture, PluginSchema in `plugin_schema_defs.rs` IS the schema (no runtime fetch needed). But this means PluginSchema must stay in sync with OVSDB's actual schema. If OVSDB adds a column, PluginSchema must be updated to match.
