# Transport Audit — Code That Bypasses `rovs-transport` Unnecessarily

> Audit date: 2026-06-04. Method: grep + file reads against `crates/`.
> Purpose: Find all code that does raw socket I/O to OVS/OpenFlow endpoints when
> it should be using `rovs-transport::Stream` / `rovs-openflow::VConn` — the
> reason it doesn't is because `VConn::from_accepted_stream()` didn't exist as an
> option when the code was written. With Option A locked, these become migration
> targets.

---

## Summary: 5 distinct hand-rolled transport implementations found

| # | Crate | File | What it does | Should use | Effort |
|---|---|---|---|---|---|
| 1 | `op-network` | `controller.rs` | `TcpListener::accept` + manual OF1.3 handshake + `Message::encode()`/`Header::decode()` + `Flow::to_message()` on raw `TcpStream` | `TcpListener::accept → Stream::Tcp → VConn::from_accepted_stream` | **Low** — VConn handles everything |
| 2 | `op-jsonrpc` | `ovsdb.rs` + `ovsdb_jsonrpc.rs` | `UnixStream::connect(db.sock)` + manual JSON-RPC read_line/write + `stream.shutdown()` for each request | `Stream::Unix → rovs_jsonrpc::Connection` or `rovs_ovsdb::Client` | **Medium** — different JSON-RPC framing |
| 3 | `op-tools` | `builtin/ovsdb.rs` | `UnixStream::connect(db.sock)` + `BufReader::read_line` + manual `simd_json` per-call | `Stream::Unix → rovs_jsonrpc::Connection` | **Medium** — same framing issue |
| 4 | `op-chat` | `tool_loader.rs` | `std::net::TcpStream::connect_timeout(:6653)` — raw TCP **probe only** (not sending OF data) | `VConn::connect` or D-Bus proxy `RovsOpenFlowProxy::echo` | **Low** — just a liveness check |
| 5 | `op-network` | `openflow.rs` (`OpenFlowClient`) | **Already uses VConn correctly** via `VConn::connect` — this is the model pattern | N/A — already correct | N/A |

---

## 1. `controller.rs` — Passive OpenFlow controller (the primary case)

**File:** `crates/op-network/src/controller.rs` (358 lines)

**Current approach:** `TcpListener::bind(:6653)` → `listener.accept()` → per-connection `handle_connection()` that:
- Manually reads OF header (8 bytes) + body via `TcpStream::read_exact`
- Manually builds Hello, FeaturesRequest, PortDesc, FlowMod, EchoReply via `Message::new().encode()`
- Manually parses PortDesc multipart reply (64-byte chunk parsing)
- Drives OF1.3 handshake: recv Hello → send Hello → send FeaturesRequest → recv FeaturesReply
- Keeps alive: match EchoRequest → build EchoReply
- Installs flows: `build_flow_mod_add(in_port, out_port, priority, xid)` → `Flow::add().to_message().encode()`

**Why it doesn't use VConn:** `VConn::connect()` is active/outbound only — no passive-accept constructor existed. With `VConn::from_accepted_stream()`, the entire `handle_connection()` becomes:

```rust
async fn handle_connection(stream: Stream, flows: Arc<Vec<...>>) -> Result<()> {
    let mut vconn = VConn::from_accepted_stream(stream).await?;
    // Port discovery
    let port_map = discover_ports_via_vconn(&mut vconn).await?;
    // Delete all + install flows
    vconn.send_flow(&Flow::delete()).await?;
    for (in_name, out_name, priority) in flows.iter() {
        // ...resolve port numbers from port_map...
        vconn.send_flow_sync(&flow).await?;
    }
    // Keepalive — VConn handles echo internally in every recv call
    loop { vconn.recv_message().await?; /* VConn auto-replies echo */ }
}
```

**What becomes dead code after migration:**
- `build_raw_msg()`, `build_hello()`, `build_features_request()`, `build_port_desc_request()`, `build_echo_reply()`, `build_flow_mod_add()`, `build_flow_mod_delete_all()`
- `recv_msg()` / `send_msg()` raw wire helpers
- `RawMsg` struct
- `discover_ports()` — replaced by a VConn-based port discovery (still needs multipart, but through `recv_message()`/`send_message()` instead of raw `read_exact`)

**Note:** VConn's `recv_message()` does NOT auto-reply echo — the caller must handle echo. But `send_flow_sync`, `dump_flows`, `barrier`, `recv_packet_in` all handle echo internally. The keepalive loop still needs explicit echo handling, same as today.

---

## 2. `op-jsonrpc/src/ovsdb.rs` + `ovsdb_jsonrpc.rs` — Direct OVSDB socket clients

**Files:**
- `crates/op-jsonrpc/src/ovsdb.rs` (446 lines) — `OvsdbClient` with `UnixStream::connect` + `read_to_end`/`write_all`
- `crates/op-jsonrpc/src/ovsdb_jsonrpc.rs` (~200 lines) — `OvsdbClient` with `UnixStream::connect` + `BufReader::read_line`

**Current approach:** Both construct a fresh `UnixStream::connect(db.sock)` **per RPC call** — no persistent connection. They:
- Serialize JSON-RPC request manually (`simd_json::to_string`)
- Write it + `"\n"` to the raw `UnixStream`
- For `ovsdb.rs`: `stream.shutdown()` then `read_to_end` (reads entire response as one blob)
- For `ovsdb_jsonrpc.rs`: `BufReader::read_line` (newline-delimited)
- Parse response with `simd_json`

**Why they don't use rovs:** These are in `op-jsonrpc` which doesn't depend on rovs at all. They predate the rovs integration and were written as standalone RFC 7047 clients.

**Migration target:** Replace with `rovs_jsonrpc::Connection` over `Stream::Unix(db.sock)` — which handles JSON-RPC framing, id tracking, and notification parsing. Or route through the daemon's D-Bus `Transact` method (the D-Bus-first option per AGENTS.md).

**Challenge:** These clients create a **new connection per call** (no persistent session). `rovs_jsonrpc::Connection` is session-oriented (maintains an ID counter and notification queue). The per-call pattern could either:
- (a) Be converted to persistent `Connection` objects (more efficient, aligns with rovs model)
- (b) Route through the daemon's D-Bus/gRPC surface (AGENTS.md preferred path)

**Also note:** `ovsdb.rs` line 446 has a `monitor_db()` that opens a **long-lived** `UnixStream` + spawns a reader task — this is exactly the streaming/monitor use case that should go through gRPC subscriptions (M2).

---

## 3. `op-tools/src/builtin/ovsdb.rs` — LLM tool OVSDB client

**File:** `crates/op-tools/src/builtin/ovsdb.rs` (~350 lines)

**Current approach:** Another `OvsdbClient` (3rd distinct implementation!) that:
- `UnixStream::connect(db.sock)` per call
- `BufReader::read_line` for response
- Manual `simd_json` construction of OVSDB transactions (`insert Bridge`, `mutate Open_vSwitch`, etc.)
- Implements high-level tools: `ovs_create_bridge`, `ovs_delete_bridge`, `ovs_list_bridges`, `ovs_add_port`, `ovs_delete_port`, `ovs_list_ports`, `ovs_get_bridge`

**Why it doesn't use rovs:** `op-tools` doesn't depend on `rovs-*`. It was written as a standalone tool using only `simd_json` + `tokio::net::UnixStream`.

**Migration target:** Route through the daemon's D-Bus `Transact` method. The tool implementations already construct raw OVSDB JSON-RPC payloads — they just need to send them through the proxy instead of directly opening a socket. This is the D-Bus-first path.

**The tool payloads are already in the correct format:** e.g., `ovs_create_bridge` constructs `{"op":"insert","table":"Bridge",...}` and calls `self.transact(operations)`. That payload can be sent verbatim through `RovsJsonRpcProxy::transact("transact", params)`.

---

## 4. `op-chat/src/tool_loader.rs` — OpenFlow liveness probes

**File:** `crates/op-chat/src/tool_loader.rs` lines 1589, 1827, 1896

**Current approach:** `std::net::TcpStream::connect_timeout(SocketAddr::from(([127,0,0,1], 6653)), Duration::from_millis(500))` — a **sync** TCP connect just to check if the port is open. No OpenFlow data is sent. Fallback to port 6633.

**Why it doesn't use rovs:** This is a bare liveness check, not an OpenFlow session. But per AGENTS.md "D-Bus first", the check should be:
- `RovsOpenFlowProxy::echo()` via D-Bus (daemon already connected → knows if OF switch is alive)
- Or a D-Bus property on the openflow object (e.g. `Connected` boolean)

**Migration:** Trivial — replace `TcpStream::connect_timeout` with a D-Bus property check or proxy method.

---

## 5. `op-network/src/openflow.rs` (`OpenFlowClient`) — ALREADY CORRECT

**File:** `crates/op-network/src/openflow.rs:98`

**Current approach:** `OpenFlowClient::connect(SocketAddr)` → `rovs_transport::Address::Tcp {...}` → `VConn::connect(&rovs_addr)` → uses `vconn.send_flow_sync`, `vconn.echo`, `vconn.dump_flows`, etc.

**This is the model pattern.** Once `VConn::from_accepted_stream` exists, the passive side in `controller.rs` becomes symmetric with this client — both use the same `VConn` type, same methods.

---

## Additional: `op-jsonrpc/src/server.rs` — OVSDB JSON-RPC server

**File:** `crates/op-jsonrpc/src/server.rs:153` — `TcpListener::bind(addr)` for a JSON-RPC **server** (not a client to OVSDB).

This is an OVSDB-compatible JSON-RPC **server** that external consumers connect to. NOT a bypass — it's a different role (serving OVSDB-style RPCs). Not a migration target.

---

## Consolidated migration plan for transport

### Phase 1 — With `VConn::from_accepted_stream` (M1)
| Target | Change |
|---|---|
| `controller.rs` | Replace entire passive-handshake + wire-encoding with `VConn::from_accepted_stream`. Delete `build_*`, `recv_msg`, `send_msg`, `RawMsg`. Keep `OpenFlowController` accept-loop structure, but each connection uses a VConn. |
| `openflow.rs` `OpenFlowClient` | No change — already correct. |

### Phase 2 — With daemon D-Bus/gRPC surface (M3–M4)
| Target | Change |
|---|---|
| `op-jsonrpc/src/ovsdb.rs` + `ovsdb_jsonrpc.rs` | Route through daemon's `RovsJsonRpcProxy::transact` instead of direct socket. Eliminate 2 hand-rolled OVSDB clients. |
| `op-tools/src/builtin/ovsdb.rs` | Route through `RovsJsonRpcProxy::transact` — payloads already in correct OVSDB format. Eliminate 3rd hand-rolled client. |
| `op-chat/src/tool_loader.rs` | Replace `TcpStream::connect_timeout` with D-Bus property check or `RovsOpenFlowProxy::echo`. |

### Key insight
There are **3 independent OVSDB client implementations** in the codebase (`op-network::ovsdb::OvsdbClient`, `op-jsonrpc::ovsdb::OvsdbClient`, `op-tools::builtin::ovsdb::OvsdbClient`), plus 2 in `op-jsonrpc` (`ovsdb.rs` + `ovsdb_jsonrpc.rs`). All talk to the same `/run/openvswitch/db.sock`. The daemon + proxy pattern unifies them into a single transport path through D-Bus → daemon → rovs → db.sock.

For OpenFlow, the split is cleaner: `OpenFlowClient` (active, via VConn) is already correct; `controller.rs` (passive) becomes correct with `VConn::from_accepted_stream`; and the liveness probes in `op-chat` become D-Bus checks.

---

## Risks

1. **`op-jsonrpc` clients use per-call connections.** rovs-jsonrpc/rovs-ovsdb are session-oriented (persistent connection + ID tracking). Migrating means either keeping a persistent `Connection` per consumer or routing through the daemon (which maintains the persistent connection). The daemon approach is simpler and aligns with AGENTS.md.

2. **`ovsdb.rs` (op-jsonrpc) `monitor_db()` is a long-lived stream.** This is the exact consumer that needs gRPC subscriptions (M2). It can't be a per-call D-Bus method — it needs a streaming transport. The daemon + gRPC subscription path is the correct replacement.

3. **`controller.rs` VConn migration may lose some multi-message optimization.** The current `discover_ports()` does a tightly-coupled request/reply exchange. With VConn, the same exchange uses `send_message`/`recv_message` which is functionally identical but requires handling messages that arrive between request and reply (echo, packet_in, etc.) — VConn's higher-level methods already handle this, but a raw `recv_message()` loop would need echo handling. The `dump_flows_filtered` and `send_flow_sync` methods handle echo internally, so using those is preferred over raw `recv_message`.
