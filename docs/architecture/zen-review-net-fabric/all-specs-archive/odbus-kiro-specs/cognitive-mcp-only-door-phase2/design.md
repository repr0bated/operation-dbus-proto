# Design: cognitive-mcp-only-door-phase2

## Architecture Decisions

Five design questions resolved below. Phase 1 (`.kiro/specs/cognitive-mcp-bridge-only-door/`)
is assumed implemented; this spec covers only the delta.

---

### DQ-1: In-process dispatch — ToolRegistry construction in the bridge

**Decision**: `op-grpc-bridge` constructs a `CognitiveMcpServer` at startup and
holds `Arc<ToolRegistry>` in `MutationEngine`. The Phase 1 HTTP loopback is
replaced with a direct `tool_registry.execute(name, args).await` call.

**Why no new crate is needed** (superseding Phase 1's "Phase 2 gate"):

| Fact | Evidence |
|------|----------|
| Bridge already depends on `op-cognitive-mcp` | `crates/op-grpc-bridge/Cargo.toml:48` |
| Bridge already uses `op-cognitive-mcp` types | `crates/op-grpc-bridge/src/grpc_server.rs:13` (`use op_cognitive_mcp::QdrantSemanticShuttle`) |
| `op-cognitive-mcp` re-exports `op-mcp` | `crates/op-cognitive-mcp/Cargo.toml:11` |
| `ToolRegistry` is pub | `crates/op-mcp/src/tool_registry.rs:47` |
| `CognitiveMcpServer::tool_registry()` returns `Arc<ToolRegistry>` | `crates/op-cognitive-mcp/src/server.rs:227` |
| `cozo`, `qdrant-client`, `tonic`, `axum`, `zip` already linked transitively | `cargo tree -p op-grpc-bridge --depth 2` |

**Construction site**: `crates/op-grpc-bridge/src/grpc_server.rs` already
constructs `QdrantSemanticShuttle` at `:758`. The `CognitiveMcpServer::new()`
call is placed adjacent, using the same `db_path` env var
(`COGNITIVE_MCP_DB_PATH`, defaulting to
`/var/lib/op-cognitive-mcp/memory.db`).

```rust
// In BridgeServer::start() or equivalent init:
let cognitive_server = CognitiveMcpServer::new(
    &std::env::var("COGNITIVE_MCP_DB_PATH")
        .unwrap_or_else(|_| "/var/lib/op-cognitive-mcp/memory.db".into())
).await;

let tool_registry: Option<Arc<ToolRegistry>> = match cognitive_server {
    Ok(server) => {
        let reg = server.tool_registry();
        // Also extract context_engine for FR-4
        let ctx = server.context_engine();
        Some(reg)
    }
    Err(e) => {
        tracing::error!(error = %e, "CognitiveMcpServer init failed; cognitive_mcp tools unavailable");
        None
    }
};
```

**Degradation**: If `CognitiveMcpServer::new()` fails (CozoDB locked, Qdrant
unreachable), the bridge stores `tool_registry: None`. The dispatch arm returns:
```json
{"success": false, "error": "cognitive_mcp tool registry unavailable", ...}
```
All other 64 plugins remain fully functional. The bridge never panics or refuses
to start.

**Task-pool isolation**: Tool execution calls `tokio::spawn` inside the dispatch
arm so that long-running tools (e.g. `agent_shell_executor_exec`,
`ask_question`) do not block the bridge's D-Bus message loop. The spawned task
is `.await`ed with a timeout (30s default, configurable via
`COGNITIVE_TOOL_TIMEOUT_SECS`).

```rust
let result = tokio::time::timeout(
    Duration::from_secs(tool_timeout),
    tokio::spawn(async move { registry.execute(&tool_name, args).await })
).await;
```

**Per-method mapping table**: Unchanged from Phase 1. The 15 existing methods +
`invoke_tool` retain identical semantics. Only the transport changes: where
Phase 1 did `HTTP POST http://10.200.0.2:3003/mcp`, Phase 2 does
`tool_registry.execute(mapped_tool_name, mapped_args).await`.

For `list_tools`: instead of sending MCP `tools/list` over HTTP, call
`tool_registry.list(0, usize::MAX, None).await` directly.

For `get_health` / `get_config` / `set_config` / `restart_service`: these
remain in-process (read projection or apply-state), unchanged from Phase 1.

**Rejected alternatives**:

1. *Extract `op-mcp-registry` crate* — unnecessary; the dependency edge already
   exists. Adding an empty forwarding crate increases maintenance for zero benefit.

2. *Keep HTTP loopback to a still-running `op-cognitive-mcp`* — defeats the
   purpose of Phase 2. The process may not be running (--stdio mode only), and the
   listener is being deleted.

3. *D-Bus rendezvous with `op-cognitive-mcp`* — rejected in Phase 1 DQ-2 for the
   same reasons. No new bus name.

---

### DQ-2: What happens to `op-cognitive-mcp` as a service

**Decision**: `op-cognitive-mcp` **continues to exist as a runit service** but
its run script changes to `--stdio` mode only. It survives because:

1. The `.mcp.json` `op-cognitive-mcp` stdio entry uses it for local MCP access.
2. `rag-ingest` and `op-cog-admin` binaries link against the library but run
   independently — they do not depend on the service.
3. The service could be stopped (`sv stop op-cognitive-mcp`) without affecting
   the bridge, since tool execution is now in-process in `op-grpc-bridge`.

**Run script change** (`/etc/runit/sv/op-cognitive-mcp/run`):
```sh
#!/bin/sh
exec 2>&1
# After Phase 2: no network listeners. Stdio mode only (for local MCP clients).
# Tool execution is now in-process in op-grpc-bridge.
export COGNITIVE_MCP_DB_PATH=/var/lib/op-cognitive-mcp/memory.db
exec /usr/local/bin/op-cognitive-mcp --stdio
```

**However**: with the tool registry now constructed inside `op-grpc-bridge`, the
`op-cognitive-mcp --stdio` instance opens a **separate** CozoDB database handle
(same path, different process). CozoDB supports concurrent readers but only one
writer. This means:
- The `op-grpc-bridge` process is the primary writer (through tool execution).
- The `op-cognitive-mcp --stdio` process can read but write-conflicting tool
  calls may fail with a lock error.

**Resolution**: This is acceptable because `--stdio` mode is a debugging/fallback
path, not the production path. The production path is always through the bridge.
Document this limitation in the run script.

**Alternative considered**: Remove the runit service entirely (make
`op-cognitive-mcp` library-only). Deferred — the `.mcp.json` entry still uses
it, and removing a runit service is a separate operational concern.

---

### DQ-3: Context-awareness SSE — in-process in op-web

**Decision**: `op-web` hosts the context-awareness routes in-process by receiving
a `ContextAwarenessEngine` (and supporting state) from the same
`CognitiveMcpServer` constructed by the bridge.

**Mechanism**: Since `op-web` and `op-grpc-bridge` are the same process (verified:
`crates/op-web/src/bin/opdbus.rs` is the unified binary), they share the
`CognitiveMcpServer` instance. The route tree gets:

```rust
let context_router = op_cognitive_mcp::context_server::build_context_router(
    cognitive_server.context_engine(),
    cognitive_server.memory_store(),
    cognitive_server.session_manager(),
);
// Nest under /cognitive/context/
app = app.nest("/cognitive/context", context_router);
```

**Routes exposed**:
- `GET  /cognitive/context/stream/:session_id` — SSE push stream
- `GET  /cognitive/context/status/:session_id` — session status
- `POST /cognitive/context/record` — record activity
- `POST /cognitive/context/request_push` — request knowledge push
- `GET  /cognitive/context/health` — context engine health

**No proxy to `:3003`**: The engine runs in the same process. No HTTP hop.

**Rejected alternatives**:

1. *Keep proxying to a lightweight `:3003`* — defeats Phase 2's goal. The listener
   is being deleted.

2. *Move context engine to a new crate* — unnecessary complexity. The engine is
   already accessible via `CognitiveMcpServer::context_engine()`.

3. *Make context-awareness a D-Bus plugin method* — SSE (Server-Sent Events) is
   inherently HTTP; D-Bus request-reply semantics don't fit long-lived streams.
   Keep as HTTP routes on op-web.

---

### DQ-4: WaypipeTunnel relocation

**Decision**: Create a new runit service `op-waypipe-grpc` running the existing
standalone binary on the same port (`10.200.0.2:50052`).

**Why same port**: Laptop clients connecting via WireGuard mesh already have
`100.69.0.254:50052` or `10.200.0.2:50052` configured. Changing the port breaks
existing client configs. The `op-waypipe-grpc` binary already supports `serve`
subcommand with configurable listen address.

**New runit service** (`/etc/runit/sv/op-waypipe-grpc/run`):
```sh
#!/bin/sh
exec 2>&1
exec /usr/local/bin/op-waypipe-grpc serve --listen 10.200.0.2:50052
```

**Timing**: The `op-waypipe-grpc` service must be started BEFORE removing the
gRPC listener from `op-cognitive-mcp`, ensuring zero downtime for tunnel clients.

**Rejected alternatives**:

1. *Co-host on op-grpc-bridge's tonic server* — a waypipe tunnel crash would take
   down the entire control plane. Process isolation is worth the extra service.

2. *Move to a different port* — breaks existing client configs without benefit.

3. *Delete WaypipeTunnel entirely* — it has active laptop consumers.

---

### DQ-5: Schema field removal and blob resealing

**Decision**: Remove `http`, `grpc`, `http_enabled`, `grpc_enabled` from
`CognitiveMcpConfig`. Keep `wg_interface` (still used by the `--stdio` mode for
WireGuard identity detection) and `dbus_enabled` (still meaningful).

**Post-removal `CognitiveMcpConfig`**:
```rust
pub struct CognitiveMcpConfig {
    #[serde(default = "default_wg")]
    pub wg_interface: String,
    #[serde(default = "default_true")]
    pub dbus_enabled: bool,
}
```

**Blob resealing procedure**:
1. After code changes, `cargo build -p op-blob`.
2. Run `op-blob seal cognitive_mcp` — produces new blob at
   `/dev/shm/opdbus/plugin-blobs/cognitive_mcp.blob`.
3. `op-grpc-bridge` detects the new blob on next publish cycle and emits updated
   projection.
4. The projection at `/dev/shm/opdbus/projections/cognitive_mcp.json` will no
   longer contain `http_enabled`/`grpc_enabled`/`http`/`grpc`. Consumers already
   use `#[serde(default)]` on all fields, so deserialization remains backward-
   compatible.

**Consumers affected**:
- `op-cognitive-mcp/src/main.rs` — Phase 1 already removed the
  `cognitive_mcp_bind_config()` function that read the projection. After Phase 2,
  the process in `--stdio` mode does not read the projection at all.
- Any monitoring/dashboard reading the projection JSON directly will see the
  fields disappear. This is acceptable — the service no longer has those
  transports.

**The `apply_state` diff logic** in `cognitive_mcp.rs:200-283` handles
`http_enabled`/`grpc_enabled` field diffs. These match arms are deleted alongside
the fields.

**Rejected alternatives**:

1. *Keep fields as permanently-false* — adds dead schema weight. Clean deletion
   is preferred; the blob resealing is a mechanical step.

2. *Deprecation period with `#[deprecated]` on the struct fields* — Rust struct
   field deprecation is not well-supported. The Phase 1 deprecation period IS the
   deprecation period. Phase 2 is the deletion.

---

## Communication Flow (Phase 2)

```
MCP Client
    │  (stdio / HTTP / D-Bus — client's choice)
    ▼
op-web :8080 / op-mcp-server --stdio / busctl
    │  org.opdbus.v1.PluginV1.Call("invoke_tool", '{"tool_name":"X","arguments":{...}}')
    │  destination: org.opdbus.v1.plugins
    │  object path: /org/opdbus/v1/plugins/cognitive_mcp
    ▼
op-grpc-bridge (session bus owner: org.opdbus.v1.plugins)
    │  1. Method-existence gate: "invoke_tool" in schema ✓
    │  2. Arg validation: {tool_name: string, arguments: object} ✓
    │  3. Capability check: cognitive_mcp.invoke granted? ✓
    │  4. Event chain: record actor_id, capability_id, event_hash
    │  5. Dispatch: "cognitive_mcp" match arm → in-process call
    ▼
ToolRegistry::execute("X", {...})     [in-process, same PID as bridge]
    │
    ▼
Tool handler (e.g. CognitiveMemoryStore, CodeSearchTool, ...)
    │
    ▼
Result JSON → envelope build (:1015-1021) → PluginV1.Call return string
```

No `:3003` in the path. No `:50052` in the path. No HTTP loopback.

---

## Affected Files

| File | Change type |
|------|-------------|
| `crates/op-grpc-bridge/src/mutation_engine.rs` | Replace HTTP loopback with in-process `tool_registry.execute()` |
| `crates/op-grpc-bridge/src/grpc_server.rs` | Construct `CognitiveMcpServer`, store `Arc<ToolRegistry>` |
| `crates/op-web/src/routes/mod.rs` | Add `/cognitive/context` nest with `build_context_router` |
| `crates/op-cognitive-mcp/src/main.rs` | Delete transport match arms, flags; keep `--stdio` only |
| `crates/op-cognitive-mcp/src/server.rs` | Delete `start_http_server`, `start_grpc_server`, `start_dual`, `serve_cognitive_grpc` |
| `crates/op-cognitive-mcp/src/lib.rs` | Remove `pub mod interceptor`, `pub mod dbus_interface`, `pub mod client_config` |
| `crates/op-cognitive-mcp/src/interceptor.rs` | **Delete file** |
| `crates/op-cognitive-mcp/src/dbus_interface.rs` | **Delete file** |
| `crates/op-cognitive-mcp/src/client_config.rs` | **Delete file** |
| `crates/op-cognitive-mcp/src/grpc_service.rs` | **Delete file** |
| `crates/op-cognitive-mcp/examples/external_client.rs` | **Delete file** |
| `crates/op-plugins/src/state_plugins/cognitive_mcp.rs` | Remove `http`, `grpc`, `http_enabled`, `grpc_enabled` from `CognitiveMcpConfig`; remove `apply_state` diff arms |
| `/etc/runit/sv/op-cognitive-mcp/run` | Rewrite: `--stdio` only, remove bind env vars |
| `/etc/runit/sv/op-waypipe-grpc/run` | **New file**: standalone WaypipeTunnel service |
| `deploy/config/cognitive-mcp-clients.json` | Remove `:3003`/`:50052` endpoints; update to bridge paths |

---

## What Does NOT Change

| Item | Reason |
|------|--------|
| `ToolRegistry` / `Tool` trait in `op-mcp` | Execution substrate; used as-is |
| Tool registration in `server.rs::new()` | Unchanged; tools register into the same `Arc<ToolRegistry>` |
| `notebooklm.rs` | Registers tools; reachable through in-process registry |
| `rag_pipeline.rs`, `voyage.rs` | RAG tools work identically through registry |
| Sealed blob catalog | Written by `op-blob`, read-only from bridge |
| 16 schema methods | Unchanged; only transport under dispatch changes |
| `context_awareness.rs` / `context_server.rs` | Code unchanged; just hosted in a different process |
| `fwd-3003` runit service | **Deleted** in Phase 2 — see FR-2. Targets the xray container (`10.200.0.1`) correctly, but xray exposes no port-3003 listener, so nothing reaches it |
| Bus name `org.opdbus.v1.plugins` | Sole well-known name; no additions |
| `.mcp.json` `op-cognitive-mcp` stdio entry | Still works; uses the binary in stdio mode |
| `rag-ingest` / `op-cog-admin` binaries | Library users; unaffected |

---

## Verified System Facts (as of 2026-07-29)

| Claim | Status | Evidence |
|-------|--------|----------|
| `op-grpc-bridge` depends on `op-cognitive-mcp` | ✅ Verified | `Cargo.toml:48` |
| Bridge already links `cozo`, `qdrant-client`, `axum`, `tonic`, `zip` | ✅ Verified | `cargo tree -p op-grpc-bridge --depth 2` |
| `CognitiveMcpServer::tool_registry()` is pub | ✅ Verified | `server.rs:227` |
| `CognitiveMcpServer::context_engine()` is pub | ✅ Verified | `server.rs:247` |
| `CognitiveMcpServer::memory_store()` is pub | ✅ Verified | `server.rs:219` |
| `CognitiveMcpServer::session_manager()` is pub | ✅ Verified | `server.rs:235` |
| `build_context_router` returns `axum::Router` | ✅ Verified | `context_server.rs:75` |
| `10.200.0.1` is the xray server (incus container), the intended relay hop | ✅ Verified | `/etc/xray/xray_config.json` dokodemo-door `listen=10.200.0.1:{8443,8081,6334,8090}` → `10.200.0.2:<same>`; `incus-ct-xray` runit service |
| `fwd-3003` targets the xray hop correctly but xray has no port-3003 listener | ✅ Verified | `/etc/runit/sv/fwd-3003/run`; probe `10.200.0.1:3003` REFUSED; xray inbounds cover only 8443/8081/6334/8090/8444 |
| Mesh chain is `100.69.0.254:<p>` → `10.200.0.1:<p>` → `10.200.0.2:<p>` | ✅ Verified | tcpfwd args + xray dokodemo-door config |
| `op-web` reachable from mesh at `:8080` with no forwarder | ✅ Verified | `curl http://100.69.0.254:8080/mcp/compact` returned the tool list; nftables admits `100.69.0.0/16` to 8080 |
| "assistant" is a deprecated label denoting this host | ⚠️ Per operator | xray `to-assistant-ui` → `10.200.0.2:8080`, `to-assistant-grpc` → `10.200.0.2:8090` |
| `serve_cognitive_grpc` co-hosts WaypipeTunnel | ✅ Verified | `server.rs:271` (`op_waypipe_grpc::build_tunnel_service`) |
| `op-waypipe-grpc` has standalone binary | ✅ Verified | `crates/op-waypipe-grpc/src/bin/op-waypipe-grpc.rs` |
| `client_config.rs` only used from `examples/external_client.rs` | ✅ Verified | `grep -rn` shows no other consumers |
| `dbus_interface.rs` never registered on bus | ✅ Verified | Phase 1 + no `ConnectionBuilder` call in codebase |
| `CognitiveGrpcService` has no tool registry | ✅ Verified | `grpc_service.rs:38-60` (memory/session/quota/gemini only) |
| NotebookLM tools registered into ToolRegistry | ✅ Verified | `notebooklm.rs:121` `register_notebooklm_tools(registry)` |
| `reqwest` already linked transitively | ✅ Verified | `cargo tree -p op-grpc-bridge --depth 2` shows `reqwest v0.11.27` |
| Host runs runit, not s6, not systemd | ✅ Verified | `sudo sv status op-cognitive-mcp` works |
| No env-dir for op-cognitive-mcp | ✅ Verified | env vars set inline in `/etc/runit/sv/op-cognitive-mcp/run` |
| `QdrantSemanticShuttle::new()` is fallible, non-fatal | ✅ Verified | `server.rs:51-57` logs warning, sets `None` |
| MutationEngine envelope built at `:1015-1021` | ✅ Verified | `mutation_engine.rs:1015-1021` |
| op-web and op-grpc-bridge share a binary | ✅ Verified | `crates/op-web/src/bin/opdbus.rs` |
