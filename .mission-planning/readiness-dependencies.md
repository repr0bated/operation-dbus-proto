# Dependency Readiness Check

> Verified against vendored rovs-\* 0.2.0 sources at
> `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rovs-*-0.2.0/`
> and workspace source at `/home/jeremy/git/operation-dbus-proto/`.
> All findings cite real file paths + line numbers.

---

## rovs-openflow VConn — passive listen?

**CORRECTED: VConn is CLIENT-only (no `accept` constructor), NOT receive-only.**
VConn already performs full bidirectional I/O — `send_message` + `recv_message`, `send_flow` + `dump_flows`,
`send_flow_sync` (send+barrier loop), `monitor_flows` + `recv_flow_updates`, `echo`, `recv_packet_in`.
The OpenFlow protocol is inherently bidirectional on every connection. The gap is solely a
`VConn::from_accepted_stream(Stream)` constructor — VConn cannot **accept** a connection,
but once a connection exists, it can do everything.

### Evidence (vendored source)

| file | line(s) | finding |
|---|---|---|
| `rovs-openflow-0.2.0/src/vconn.rs` | 20–27 | `VConn::connect(addr)` is the **only** constructor; delegates to `Stream::connect()` |
| `rovs-openflow-0.2.0/src/vconn.rs` | 20 | `pub async fn connect(addr: &Address) -> Result<Self>` — no `listen`, `bind`, `accept` |
| `rovs-openflow-0.2.0/src/vconn.rs` | 60–70 | `send_message` + `recv_message` — full bidirectional, uses `stream.write_all` + `stream.read_exact` |
| `rovs-openflow-0.2.0/src/vconn.rs` | 100–130 | `send_flow` / `send_flow_sync` / `dump_flows` — all send+recv patterns |
| `rovs-openflow-0.2.0/src/vconn.rs` | 245–280 | `monitor_flows` + `recv_flow_updates` — continuous bidirectional monitor stream |
| `rovs-transport-0.2.0/src/stream.rs` | 22–36 | `Stream::connect()` branches on `Address::Unix/Tcp/Tls`; no `TcpListener` anywhere in the enum |
| `rovs-transport-0.2.0/src/stream.rs` | 36 | TLS is `Err(Error::Tls("TLS not yet implemented"))` — **RISK** for secure controller transport |
| `rovs-transport-0.2.0/src/stream.rs` | 16 | `Stream` is `enum { Unix(UnixStream), Tcp(TcpStream), Tls(...) }` — `TcpStream` can be constructed from an accepted connection |

### Locked implementation for passive listen (:6653) — Option A

**Add `VConn::from_accepted_stream()` to rovs-openflow** (upstream PR or local fork, ~20 lines).
Takes an already-connected `Stream`, performs passive-side handshake (recv Hello first, then send Hello).
Daemon uses a single `VConn` type for both active and passive connections — all VConn methods
(`send_flow`, `dump_flows`, `recv_packet_in`, `monitor_flows`, etc.) work identically on both paths.

```rust
// Proposed addition to vconn.rs
impl VConn {
    /// Accept an inbound OpenFlow connection on an already-connected stream.
    ///
    /// For use by a passive (listening) controller: accept via TcpListener,
    /// wrap the TcpStream in a Stream, then pass it here.
    pub async fn from_accepted_stream(stream: Stream) -> Result<Self> {
        let mut conn = Self {
            stream,
            version: Version::Of13,
            next_xid: 1,
        };
        conn.passive_handshake().await?;
        Ok(conn)
    }

    /// Passive-side handshake: receive Hello first, then respond.
    async fn passive_handshake(&mut self) -> Result<()> {
        let reply = self.recv_message().await?;
        if reply.header.msg_type != MessageType::Hello {
            return Err(Error::InvalidMessage("expected Hello from switch".into()));
        }
        self.version = std::cmp::min(self.version, reply.header.version);

        let hello = Message::new(
            self.version,
            MessageType::Hello,
            self.next_xid(),
            Bytes::new(),
        );
        self.send_message(&hello).await?;
        Ok(())
    }
}
```

Daemon's passive-listen code:
```rust
let listener = TcpListener::bind(":6653").await?;
loop {
    let (tcp_stream, _peer) = listener.accept().await?;
    let stream = Stream::Tcp(tcp_stream);
    let vconn = VConn::from_accepted_stream(stream).await?;
    // Now use vconn.send_flow(), vconn.dump_flows(), etc. — identical API to active VConn
}
```

**Existing controller.rs reference** (template for the accept loop structure, even though VConn handles all wire protocol):

| file | line(s) | technique |
|---|---|---|
| `crates/op-network/src/controller.rs` | 316–320 | `TcpListener::bind(listen_addr)` + `listener.accept().await` loop |
| `crates/op-network/src/controller.rs` | 296–358 | `OpenFlowController::run()` — accept loop + `tokio::spawn(handle_connection)` per peer |

**Wire-encoding types rovs-openflow already exposes (re-usable by daemon under either option):**

| type | source file | purpose |
|---|---|---|
| `Message` | `rovs-openflow-0.2.0/src/message.rs:111` | Complete OF message with `Header` + `body: Bytes`; `.encode()` → `BytesMut` |
| `Header` | `message.rs:13` | 8-byte header with `version`, `msg_type`, `length`, `xid`; `encode()`/`decode()` |
| `MessageType` | `message.rs:29` | All OF1.0–1.5 message types (Hello=0 … MeterMod=29, Bundle*=33–34) |
| `Version` | `lib.rs:65` | `Of10..Of15` enum with `wire_version()` |
| `Flow` | `flow.rs` | Flow modification with `.to_message(version, xid)` |
| `FlowMonitorRequest` | `flow_monitor.rs:128` | Builder for NXST_FLOW_MONITOR; `.to_message(version, xid)` |
| `FlowUpdate` / `FlowUpdateFull` | `flow_monitor.rs:87–117` | Parsed monitor reply events (Added/Deleted/Modified) |
| `PacketIn` / `PacketOut` | `packet_in.rs` / `packet_out.rs` | Async message handling |
| `Match` | `match_fields.rs` | OXM/NXM match field builder |
| `Action` / `ActionList` | `action/mod.rs` | Action encoding including Nicira extensions |

**What the daemon must implement from scratch (under either option):**

1. `tokio::net::TcpListener::bind(":6653")` + accept loop
2. Passive handshake sequence (Option A: built into `VConn::from_accepted_stream`; Option B: manual as in controller.rs)
3. Connection lifecycle / Reconnect state machine (controller.rs uses `rovs_transport::Reconnect`)

---

## rovs-jsonrpc / rovs-ovsdb streaming vs polling

**CONFIRMED: Poll-only notification model.** There is no async stream/subscription primitive.
Notifications are buffered while waiting for response matching and must be drained explicitly.

### rovs-jsonrpc notification API

| method | file:line | signature | semantics |
|---|---|---|---|
| `pop_notification` | `connection.rs:194` | `fn pop_notification(&mut self) -> Option<Request>` | FIFO pop one buffered notification |
| `drain_notifications` | `connection.rs:200` | `fn drain_notifications(&mut self) -> impl Iterator<Item = Request>` | Drain all buffered |
| `has_pending_notifications` | `connection.rs:185` | `fn has_pending_notifications(&self) -> bool` | Check if any buffered |
| `pending_notification_count` | `connection.rs:190` | `fn pending_notification_count(&self) -> usize` | Count |
| `recv_message` | `connection.rs:114` | `async fn recv_message(&mut self) -> Result<Message>` | **Blocking read** from socket — returns `Message::Request` (notification) or `Message::Response` |

**Key insight:** Notifications are only buffered *as a side effect* of calling `transact()`. There is no background reader. To receive unsolicited OVSDB update notifications, the caller must call `recv_message()` directly (blocking) or call `transact()` periodically to trigger buffering.

### rovs-ovsdb client API (wraps rovs-jsonrpc)

| method | file:line | signature | semantics |
|---|---|---|---|
| `run` | `client.rs:216` | `async fn run(&mut self) -> Result<bool>` | **Non-blocking drain** of buffered notifications only; returns `true` if any updates processed. Does NOT read from socket. |
| `wait` | `client.rs:244` | `async fn wait(&mut self) -> Result<()>` | **Blocking** — calls `recv_message()` in a loop until an update notification arrives. Handles echo keepalives internally. |
| `idl` | `client.rs:85` | `fn idl(&self) -> &Idl` | Access the in-memory replica after `run()` or `wait()` |
| `transact` | `client.rs:310` | `async fn transact(&mut self, operations: Value) -> Result<Value>` | Send RPC; notifications are buffered as side-effect |

### How the daemon will drain updates for gRPC push

The existing `OvsdbClient::monitor_db()` pattern (ovsdb.rs:749–840) is the **canonical approach**:

1. Open a dedicated `rovs_ovsdb::Client` via `Client::connect_with_config(addr, config)` (ovsdb.rs:192–199)
2. Send initial snapshot via `idl_snapshot(client.idl())` (ovsdb.rs:800)
3. Loop: `client.wait().await` → `idl_snapshot(client.idl())` → send to `mpsc::channel` (ovsdb.rs:808–825)
4. Reconnection via `rovs_transport::Reconnect` state machine (ovsdb.rs:760–795)

The daemon will replicate this pattern, but instead of sending to an mpsc channel, it will push into:
- gRPC `OvsdbMirror::Monitor` server-streaming RPC (already exists!)
- SchemaEngine `change_tx` broadcast (already wired!)

For OpenFlow flow updates, the daemon will use:
- `VConn::monitor_flows(request)` → initial snapshot
- `VConn::recv_flow_updates()` → blocking loop → push into gRPC

**Async API the daemon will call:**

```
// OVSDB path
client.wait().await          → Result<()>           // blocks until update
client.idl().change_seqno()  → u64                  // check if changed
idl_snapshot(client.idl())   → serde_json::Value    // full snapshot

// OpenFlow flow monitor path
vconn.monitor_flows(request).await → Result<Vec<FlowUpdate>>   // initial
vconn.recv_flow_updates().await    → Result<Vec<FlowUpdate>>   // ongoing
```

---

## gRPC subscription infrastructure (does it exist?)

**CONFIRMED: Extensive subscription infrastructure already exists.** The gap is only
wiring the new daemon's live data into the existing broadcast channels.

### What already exists

| RPC | proto file:line | server impl | mechanism |
|---|---|---|---|
| `OvsdbMirror::Monitor` | `operation.proto:496` | `grpc_server.rs:1365–1467` | `tokio::sync::broadcast` receiver from `SchemaEngine::change_tx`, filtered for `/org/opdbus/v1/ovsdb` paths |
| `StateSync::Subscribe` | `operation.proto:17` | `grpc_server.rs:459–489` | Same broadcast, generic state change |
| `StateSync::SubscribeSignals` | `operation.proto:48` | `grpc_server.rs:763–825` | Same broadcast, filtered for `ChangeType::Signal` |
| `StateSync::SubscribeEvents` | `operation.proto:60` | `grpc_server.rs:862–` | Same broadcast, event chain |
| `PluginService::Watch` | `registry.proto:35` | `grpc_server.rs:2057–2225` | Broadcast + registry filter |
| `DbusMirror::Watch` | `operation.proto:751` | `grpc_server.rs:4465–4596` | `zbus::SignalStream` → broadcast |

### How SchemaEngine currently consumes OvsdbClient

| file | line(s) | mechanism |
|---|---|---|
| `schema_engine.rs:21` | import | `use op_network::ovsdb::OvsdbClient` |
| `schema_engine.rs:74` | field | `pub ovsdb: Arc<OvsdbClient>` |
| `schema_engine.rs:268–307` | `start()` spawn | Subscribes to `self.ovsdb.monitor_db("Open_vSwitch")` → receives `mpsc::Receiver<Value>` → parses table updates → calls `process_authoritative_change()` |
| `schema_engine.rs:338–347` | `mutate()` | Routes "create_bridge"/"add_port" method calls through `self.ovsdb` directly |
| `grpc_server.rs:1519–1532` | `ovsdb_call()` | Routes list_dbs/get_schema/transact through **D-Bus proxy** (`/org/opdbus/v1/ovsdb`, `org.opdbus.OvsdbV1`), **not** through `OvsdbClient` directly |

### What must be built for subscription-as-mutate-trigger

The current flow is:

```
OvsdbClient.monitor_db() → mpsc::Receiver → SchemaEngine.start() task
  → process_authoritative_change() → change_tx broadcast
    → grpc_server SubscribeSignals/Monitor streams
```

**What's missing for the new daemon:**

1. **OpenFlow flow update → SchemaEngine broadcast**: No equivalent path exists.
   Need: daemon spawns `VConn::recv_flow_updates()` loop → calls `SchemaEngine::process_authoritative_change()` with OF flow changes → enters broadcast.

2. **Subscription signal as mutate trigger**: `SubscribeSignals` currently **emits** signals but doesn't **trigger** mutations.
   The `mutate()` method on SchemaEngine accepts `ChangeType::MethodCall` to write through OVSDB.
   What's needed: wire incoming OVSDB/OF notifications so that when a subscription signal arrives
   (e.g., bridge created externally), it calls `mutate()` with `ChangeType::PropertySet`
   to update the SchemaEngine's in-memory state. This is already partially done in
   `schema_engine.rs:268–307` (OVSDB path) but not for OpenFlow.

3. **D-Bus object for rovs daemon**: The gRPC `ovsdb_call()` routes through D-Bus at
   `/org/opdbus/v1/ovsdb` (grpc_server.rs:1521–1526). The new daemon must register D-Bus objects
   at `/org/opdbus/rovs/jsonrpc` and `/org/opdbus/rovs/openflow` per the mission spec.

4. **No proto definitions for OpenFlow streaming**: There are no gRPC proto messages
   for OpenFlow flow updates, PacketIn, or PortStatus events. New proto messages and
   a new gRPC service (e.g., `OpenFlowMirror`) must be added.

---

## Cargo / build graph impact

### op-network is the ONLY crate depending on rovs-*

Verified by `rg -n 'rovs-' --glob '**/Cargo.toml'` — only `crates/op-network/Cargo.toml` lines 41–45.

### Where to declare the new daemon bin

The daemon should be declared in `crates/op-network/Cargo.toml` alongside the existing `[[bin]]` entries:

```toml
[[bin]]
name = "op-rovs-daemon"
path = "src/bin/op-rovs-daemon.rs"
```

Existing bins in op-network (all at `crates/op-network/src/bin/`):

| bin | path |
|---|---|
| `op-of-controller` | `src/bin/op-of-controller.rs` |
| `op-xdp-wg` | `src/bin/op-xdp-wg.rs` |
| `op-ovsbr0-afxdp` | `src/bin/op-ovsbr0-afxdp.rs` |
| `op-ovsbr0-setup` | `src/bin/op-ovsbr0-setup.rs` |

### bindgen (for --enable-advanced-protocols)

- **Not present anywhere.** `rg 'bindgen' --glob '**/Cargo.toml'` returns nothing.
- **OVS C headers ARE present** on this host:
  - `/usr/local/include/openflow/openflow.h`, `openflow-1.0.h` through `openflow-1.5.h`
  - `/usr/local/include/openvswitch/ofpbuf.h`, `ofp-actions.h`, `ofp-flow.h`, etc. (20+ headers)
  - `/usr/local/include/openflow/nicira-ext.h`, `intel-ext.h`, `netronome-ext.h` (vendor extensions)
- **vswitch.ovsschema IS present** at `/usr/local/share/openvswitch/vswitch.ovsschema`
- **bindgen must be added** as a `build-dependency` in op-network/Cargo.toml if advanced-protocols
  (raw C struct bindings) are needed. Alternatively, rovs-openflow already covers OF1.0–1.5 wire
  types in pure Rust; bindgen is only needed for **OVS-specific C extensions** not yet in rovs-openflow
  (e.g., `ofp-ed-props.h`, `ofp-ct.h`).

### op-grpc-bridge dependency on op-network

The grpc-bridge currently depends on `OvsdbClient` via:
- `schema_engine.rs:21` → `use op_network::ovsdb::OvsdbClient`
- `bin/op-grpc-bridge.rs:18` → `use op_network::ovsdb::OvsdbClient`

When `OvsdbClient` is deleted from op-network (mission goal), `op-grpc-bridge` must be rewired
to call the daemon's D-Bus objects instead. The `ovsdb_call()` pattern (D-Bus proxy) already
exists as the primary gRPC path, so the main change is removing the `Arc<OvsdbClient>` field
from SchemaEngine and ensuring the D-Bus objects are registered by the new daemon.

---

## Risks / blockers for milestone planning

### M1 — Daemon (D-Bus objects + passive OF listener + OVSDB mirror)

| # | risk | severity | evidence |
|---|---|---|---|
| 1 | **VConn `from_accepted_stream` addition**: must add ~20-line constructor to rovs-openflow (upstream PR or local fork). Locked Option A per user. Not a blocker — code is straightforward and tested pattern exists in controller.rs. | **RISK** (low — ~20 lines, clear spec) | `vconn.rs:20`, `controller.rs:316–358` |
| 2 | **TLS not implemented in rovs-transport**: `Stream::connect` returns error for `Address::Tls`. If the daemon needs TLS for remote OVSDB connections, this must be added to rovs-transport or worked around. | **RISK** | `stream.rs:36` |
| 3 | **rovs-ovsdb `run()` is non-blocking drain only**: does NOT read from socket. Background pump must call `wait()` (blocking) in a loop, same as existing `monitor_db()` pattern. | **OK** — pattern already established | `ovsdb.rs:749–840` |
| 4 | **OvsdbClient is shared by SchemaEngine**: deleting it requires rewiring SchemaEngine to call D-Bus objects instead. The D-Bus proxy path (`ovsdb_call()`) already exists. | **RISK** (requires careful rewiring) | `schema_engine.rs:74`, `grpc_server.rs:1519` |
| 5 | **No OpenFlow gRPC service proto**: must add `OpenFlowMirror` service + messages for flow updates, PacketIn, PortStatus. | **RISK** (new proto, but pattern exists in OvsdbMirror) | `operation.proto:496` is the template |

### M2 — gRPC transport (streaming subscriptions + mutate triggers)

| # | risk | severity | evidence |
|---|---|---|---|
| 6 | **OpenFlow flow updates have no SchemaEngine integration**: `recv_flow_updates()` produces `FlowUpdate` structs but nothing pushes them into the `change_tx` broadcast. Must add analogous path to the OVSDB monitor loop. | **RISK** (new code, but pattern is clear) | `schema_engine.rs:268–307` |
| 7 | **`broadcast` channel lag**: existing code handles `RecvError::Lagged` by logging + continuing. High-frequency OF events could overwhelm subscribers. May need configurable buffer sizes. | **RISK** | `grpc_server.rs:489`, `1432` |
| 8 | **Subscription-as-mutate-trigger not wired**: `SubscribeSignals` emits but doesn't trigger mutations. Need to wire incoming notifications → `SchemaEngine::mutate()` or `process_authoritative_change()`. | **RISK** (new wiring, architecture is clear) | `grpc_server.rs:763–825` |

### M7 — Advanced-protocols / bindgen

| # | risk | severity | evidence |
|---|---|---|---|
| 9 | **bindgen not in build graph**: must add `build-dependency` + `build.rs` to op-network. | **OK** — straightforward addition | No Cargo.toml has bindgen |
| 10 | **OVS C headers present but may diverge from rovs-openflow types**: bindgen-generated structs may conflict with hand-rolled Rust types in rovs-openflow. Must carefully namespace or use conditional compilation. | **RISK** | Headers at `/usr/local/include/openvswitch/`; types in `rovs-openflow-0.2.0/src/` |
| 11 | **vswitch.ovsschema available** for schema-driven validation at build time. | **OK** | `/usr/local/share/openvswitch/vswitch.ovsschema` |

---

*End of dependency readiness check. All findings verified from source unless marked "inferred".*
