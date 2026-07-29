# Requirements: cognitive-mcp-only-door-phase2

## Purpose

Kill the `:3003` HTTP and `:50052` gRPC listeners owned by `op-cognitive-mcp`,
delete the transport-selection flags (`--no-http`, `--no-grpc`) and all dead
transport code, and relocate the context-awareness SSE surface and WaypipeTunnel
gRPC service so that `op-grpc-bridge` (via its D-Bus bus name
`org.opdbus.v1.plugins`) is truly the **only door** to cognitive MCP tool
execution.

This is Phase 2 of the spec at
`.kiro/specs/cognitive-mcp-bridge-only-door/` (hereafter "Phase 1"). Phase 1
adds `invoke_tool`, wires the HTTP-loopback dispatch arm, deprecates the
listeners, and migrates MCP client configs. Phase 2 deletes what Phase 1
deprecated.

## Prerequisite

Phase 2 cannot merge until:
1. Phase 1 is fully implemented and merged.
2. Phase 1's equivalence script (`bin/verify-bridge-equivalence.sh`) has
   captured a passing baseline proving every tool reachable via `:3003` is
   equally reachable via `./bin/zcall cognitive_mcp invoke_tool`.

## Context and Verified Baseline

### Phase 1 "Phase 2 gate" — superseded

Phase 1 `design.md` DQ-5 states:

> Phase 2 cannot start until `op-mcp-registry` crate exists and `op-grpc-bridge`
> links it directly.

**This gate is superseded.** The extraction is unnecessary because:

- `crates/op-grpc-bridge/Cargo.toml:48` already declares
  `op-cognitive-mcp = { path = "../op-cognitive-mcp" }`.
- `crates/op-grpc-bridge/src/grpc_server.rs:13` uses
  `use op_cognitive_mcp::QdrantSemanticShuttle;` and `:758` constructs one.
- `op-cognitive-mcp` depends on `op-mcp` (`crates/op-cognitive-mcp/Cargo.toml:11`),
  making `ToolRegistry` (`crates/op-mcp/src/tool_registry.rs:47`) transitively
  available to the bridge.
- `crates/op-cognitive-mcp/src/server.rs:227` exposes
  `pub fn tool_registry(&self) -> Arc<ToolRegistry>`.
- Heavy transitive deps (`cozo`, `qdrant-client`, `axum`, `tonic`, `zip`) are
  **already linked** into `op-grpc-bridge` (confirmed via `cargo tree -p
  op-grpc-bridge --depth 2`). The "heavy deps" concern is moot.

The correct Phase 2 approach is: construct a `CognitiveMcpServer` (or just its
`ToolRegistry`) in-process inside `op-grpc-bridge` and call
`ToolRegistry::execute` directly, eliminating the HTTP loopback. No new crate
is needed.

### What already exists and is correct

- **Session bus**: `unix:path=/run/opdbus/session-bus.sock`. Bus name
  `org.opdbus.v1.plugins` owned by `op-grpc-bridge` (PID 29396). Single
  interface `org.opdbus.v1.PluginV1` with `Call(ss) -> s`. Verified via
  `./bin/zcall tree` (65 plugin objects) and `./bin/zcall introspect cognitive_mcp`.

- **op-cognitive-mcp is runit-supervised**: `/etc/runit/sv/op-cognitive-mcp/run`
  exists; `sudo sv status op-cognitive-mcp` shows PID 1195 up for >56 hours.
  Env vars set inline in the run script (no env-dir):
  `COGNITIVE_MCP_BIND=10.200.0.2:3003`,
  `COGNITIVE_MCP_GRPC_BIND=10.200.0.2:50052`,
  `COGNITIVE_MCP_DB_PATH=/var/lib/op-cognitive-mcp/memory.db`.

- **Listeners on `10.200.0.2`** (svc0 WireGuard interface):
  - `10.200.0.2:3003` — HTTP/SSE MCP + context-awareness SSE
    (`crates/op-cognitive-mcp/src/server.rs:146`)
  - `10.200.0.2:50052` — gRPC: `CognitiveToolService` + `WaypipeTunnel` +
    tonic reflection + health
    (`crates/op-cognitive-mcp/src/server.rs:253`)

- **Netmaker mesh forwarder**: `fwd-3003` runit service (PID 1250) runs
  `/usr/local/libexec/3tched/tcpfwd.py 100.69.0.254 3003 10.200.0.1 3003`.
  `10.200.0.1` is the **xray server** (incus container `incus-ct-xray`), which is
  the intended relay hop to this host — see FR-2 for the full topology. The
  forwarder is correctly targeted, but xray exposes no listener for port 3003, so
  the chain dead-ends. Nothing currently reaches `:3003` from the mesh.

- **Tool registry**: 406 tools registered at runtime via
  `CognitiveToolRegistry::register_all` (:61), `typed_tools` (:68),
  `register_code_tools` (:103) in `crates/op-cognitive-mcp/src/server.rs`.

- **`CognitiveMcpConfig`**
  (`crates/op-plugins/src/state_plugins/cognitive_mcp.rs:44-58`): struct with
  `http`, `grpc`, `wg_interface`, `http_enabled`, `grpc_enabled`, `dbus_enabled`.

- **Transport flags in `main.rs`**: `--no-grpc` (:60), `--no-http` (:64),
  `--stdio` (:68). The `exit 1` guard at `:181` fires when both transports
  are disabled AND `--stdio` was not passed.

- **`start_http_server`** (:146): builds `RegistryExecutor`, mounts
  `build_context_router` (context-awareness SSE), serves on given addr.

- **`start_grpc_server`** (:179): builds `CognitiveGrpcService` (memory/session/
  quota/gemini only) + `WaypipeTunnel` service from `op-waypipe-grpc`.

- **`start_dual`** (:192): spawns gRPC task + starts HTTP.

- **`serve_cognitive_grpc`** (:253): tonic server hosting
  `CognitiveToolServiceServer` + `WaypipeTunnel` + reflection + health, all
  behind Ghostbridge interceptor and CORS.

- **WaypipeTunnel** (`op-waypipe-grpc` crate): co-hosted on `:50052`. Has its
  own standalone binary at `crates/op-waypipe-grpc/src/bin/op-waypipe-grpc.rs`
  and can be served independently.

- **`CognitiveMcpInterface`** (`crates/op-cognitive-mcp/src/dbus_interface.rs:27`):
  dead code — defined but never registered on any bus connection.

- **`client_config.rs`**: `CognitiveMcpClient` + connection pool + circuit
  breaker for dialing `:3003`. Only used from
  `crates/op-cognitive-mcp/examples/external_client.rs`.

- **`op-web` on `:8080`**: already provides `/mcp/compact` (4 meta-tools:
  `list_tools`, `search_tools`, `get_tool_schema`, `execute_tool`), verified at
  `crates/op-web/src/mcp.rs:111` and `crates/op-web/src/routes/mod.rs:271,303`.

- **`cognitive_mcp_bind_config()`** (`crates/op-cognitive-mcp/src/main.rs:90-105`):
  reads projection as bind directive. Phase 1 removes this; Phase 2 assumes it
  is already gone.

### What is broken (Phase 2 perspective)

1. **`:3003` is still network-reachable** on `10.200.0.2`, bypassing the bridge's
   schema validation, capability checks, and event-chain recording. Authenticated
   only by Ghostbridge header presence check
   (`crates/op-cognitive-mcp/src/interceptor.rs:20-30`).

2. **`:50052` is still network-reachable** on `10.200.0.2`, serving
   `CognitiveGrpcService` (memory/session/quota — no tool registry) AND
   WaypipeTunnel (waypipe remoting) without bridge enforcement.

3. **Phase 1's HTTP-loopback dispatch** in `MutationEngine` calls
   `http://10.200.0.2:3003/mcp` — this dependency must be replaced with
   in-process `ToolRegistry::execute` before `:3003` can die.

4. **Phase 1's context-awareness proxy** in `op-web` routes
   `/cognitive/context/` back to `:3003` — must be replaced with in-process or
   library call.

5. **Transport flags and dead code**: `--no-http`, `--no-grpc`, `start_http_server`,
   `start_grpc_server`, `start_dual`, `serve_cognitive_grpc`, `interceptor.rs`,
   `dbus_interface.rs`, `client_config.rs` — all dead once listeners die.

6. **`CognitiveMcpConfig` schema fields** (`http_enabled`, `grpc_enabled`):
   removing them changes the sealed blob's schema hash, affecting downstream
   projection consumers.

### What must NOT be touched

- `ToolRegistry` / `Tool` trait / `RegistryExecutor` in `crates/op-mcp/src/tool_registry.rs`.
- Tool registration pipeline in `server.rs::new()` (:38-116).
- `notebooklm.rs` MCP sidecar bridge (registered as tools in the registry).
- The 16 schema methods (15 existing + `invoke_tool` from Phase 1).
- Sealed blob catalog (`/dev/shm/opdbus/plugin-blobs/`).
- `op-state-store` legacy catalog.
- Bus name: must remain `org.opdbus.v1.plugins` only.
- `--stdio` mode (kept for debugging; see justification in FR-3).
- `rag-ingest` binary (`crates/op-cognitive-mcp/src/bin/rag-ingest.rs`) — CLI
  tool using the library; unaffected.
- `op-cog-admin` binary.

---

## Functional Requirements

### FR-1: In-process tool dispatch replaces HTTP loopback

The Phase 1 `cognitive_mcp` dispatch arm in
`crates/op-grpc-bridge/src/mutation_engine.rs` (currently doing HTTP POST to
`http://10.200.0.2:3003/mcp`) is replaced with a direct in-process call to
`ToolRegistry::execute`.

Construction:
- `op-grpc-bridge` constructs a `CognitiveMcpServer` once during startup
  (alongside the existing `QdrantSemanticShuttle` construction at
  `crates/op-grpc-bridge/src/grpc_server.rs:758`).
- The resulting `Arc<ToolRegistry>` is stored in `MutationEngine`'s state.
- The dispatch arm calls `tool_registry.execute(tool_name, arguments).await`
  directly — no HTTP, no D-Bus hop, no new process.

Degradation behavior:
- If `QdrantSemanticShuttle` or CozoDB is unreachable at startup, the
  `CognitiveMcpServer::new()` constructor already handles this gracefully
  (`:51` logs a warning, sets `qdrant_shuttle = None`, continues). The bridge
  still starts and serves all 65 plugins. Tool calls that need Qdrant return
  a tool-layer error (envelope `success: false`), NOT a bridge crash.
- Tool execution runs on the bridge's tokio runtime. Long-running tools
  (agent shell execution, network I/O) execute on `tokio::spawn` tasks; they do
  not block the dispatch path for other plugins.

The per-method mapping table from Phase 1's design (15 methods + `invoke_tool`)
is **unchanged in shape** — only the transport under it changes from HTTP to
direct function call.

**Acceptance criteria**: `./bin/zcall cognitive_mcp invoke_tool -a
'{"tool_name":"cognitive_memory","arguments":{"operation":"list_namespaces"}}'`
returns a valid envelope with `success: true`, `event_id > 0`, non-empty
`event_hash` — verified by checking the envelope at
`crates/op-grpc-bridge/src/mutation_engine.rs:1015-1021`. No HTTP traffic to
`:3003` is generated (verifiable by stopping `op-cognitive-mcp` and confirming
the call still succeeds).

### FR-2: Netmaker mesh clients retain MCP access

**Topology (verified 2026-07-29 from `/etc/xray/xray_config.json`)**

`10.200.0.1` is the **xray server**, running in an incus container
(`incus-ct-xray`). It is the bridge between xhttp and gRPC. It binds
`10.200.0.1` and relays to this host at `10.200.0.2`:

```
dokodemo-door  10.200.0.1:8443 → 10.200.0.2:8443   (tag grpc-uplink)
dokodemo-door  10.200.0.1:8081 → 10.200.0.2:8081   (tag netmaker-api-uplink)
dokodemo-door  10.200.0.1:6334 → 10.200.0.2:6334   (tag qdrant)
dokodemo-door  10.200.0.1:8090 → 10.200.0.2:8090   (tag assistant)
vless/xhttp    :8444 path /xhttp, domain-routed via freedom redirects:
   api.*        → 10.200.0.2:8081     assistant-grpc.* → 10.200.0.2:8090
   dashboard.*  → 10.200.0.2:8080     assistant.*      → 10.200.0.2:8080
   qdrant.*     → 10.200.0.2:6333     broker.*         → 10.200.0.2:8083
   (default rule: block)
```

The mesh ingress chain is therefore three hops, and it is **intentional**:

```
netmaker client → 100.69.0.254:<port>   (tcpfwd.py on the 3tched wg interface)
                → 10.200.0.1:<port>     (xray container, xhttp↔gRPC bridge)
                → 10.200.0.2:<port>     (this host)
```

Inbound mesh traffic arrives over the `3tched` WireGuard interface
(`100.69.0.254/16`, listen port 51821). The peer carrying `allowed ips
100.69.0.0/16` has endpoint `129.153.134.63:443` — the decoy Oracle VPS acting as
the netmaker egress.

**Correction to an earlier draft of this spec**: a previous version claimed the
`fwd-*` services had a "template bug" pointing at the wrong host, and that
`10.200.0.1` was an unrelated container peer. Both claims were wrong.
`fwd-* → 10.200.0.1` is correct by design — that is the xray hop.

**Why `fwd-3003` nonetheless does not work**: xray has **no listener for port
3003**. Its dokodemo-door inbounds cover only 8443, 8081, 6334 and 8090. So the
forwarder relays to a port the xray container never exposed.

Probe results align exactly with the xray config, which confirms the diagnosis:

| Forwarder | → `10.200.0.1:<port>` | xray listener? | Probe |
|---|---|---|---|
| `fwd-6334` | 6334 | yes (qdrant) | OPEN |
| `fwd-8090` | 8090 | yes (assistant) | OPEN |
| `fwd-8444` | 8444 | yes (xhttp vless in) | OPEN |
| `fwd-3003` | 3003 | **no** | REFUSED |
| `fwd-6333` | 6333 | no (xray has 6334) | REFUSED |
| `fwd-8091` | 8091 | no (xray has 8081) | REFUSED |
| `fwd-28082` | 28082 | no | REFUSED |

Evidence for `:3003` specifically:
- `curl http://100.69.0.254:3003/mcp` returns an **empty body** — `tcpfwd.py`
  accepts, fails upstream at the xray hop, and closes.
- `curl http://10.200.0.2:3003/mcp` returns a valid MCP `initialize` response
  (`protocolVersion 2024-11-05`, `serverInfo cognitive-mcp 0.4.0`).

Consequences for this spec:
- **No working mesh traffic currently reaches `:3003`.** `fwd-3003` can be deleted
  without breaking a live consumer.
- It must be **deleted, not completed**. The alternative — adding an xray
  dokodemo-door for 3003 — would finish building a door this spec exists to close.
- Any mesh client reaching `10.200.0.2:3003` directly (bypassing both hops) *is*
  live and must be retargeted.

**Migration path.** Use the direct route — verified working, no new forwarder or
xray inbound required:

`http://100.69.0.254:8080/mcp/compact` — `op-web` binds `0.0.0.0:8080` and nftables
admits `ip saddr 100.69.0.0/16` to port 8080, so mesh clients reach it without any
forwarder. Verified: returned the tool list.

An xray xhttp route to the same destination also exists
(`assistant.3tched.com` / `assistant.ghostbridge.tech` → `10.200.0.2:8080`), and the
`to-assistant-grpc` redirect reaches the bridge's gRPC-Web port
(`10.200.0.2:8090`). Note that **"assistant" is a deprecated label that denotes
this host**; these routes are recorded here for accuracy but should not be treated
as the forward-looking name. Prefer the direct `:8080` route for new client
configuration.

Mesh clients speak HTTP/MCP JSON-RPC while the bridge door is tonic/gRPC; `op-web`
supplies the HTTP→gRPC translation on `:8080`. Clients expecting 406 flat tools
must adapt to the 4-meta-tool surface (`list_tools`, `search_tools`,
`get_tool_schema`, `execute_tool`) — an intentional surface change, since the
bridge path is the only authorized path.

**Acceptance criteria**: `fwd-3003` service directory is deleted and no `tcpfwd.py`
process listens on `100.69.0.254:3003`. No xray dokodemo-door for 3003 is added.
Any documented reference to `:3003` is updated to `:8080/mcp/compact` in
`deploy/config/cognitive-mcp-clients.json`. A mesh client reaches
`http://100.69.0.254:8080/mcp/compact` and its call is recorded in the bridge event
chain with a non-zero `event_id`.

### FR-2a: Authenticate the MCP ingress (PREREQUISITE)

**This gates FR-3.** `:8080/mcp/compact` is currently **unauthenticated**, and this
spec makes it the sole MCP ingress for mesh clients. Verified 2026-07-29:

- `crates/op-web/src/routes/mod.rs:324` applies
  `security::ip_security_middleware` globally, but that middleware only resolves
  an `AccessZone` and inserts it as a request extension, then calls
  `next.run(request)` **unconditionally** — it denies nothing
  (`crates/op-web/src/middleware/security.rs:202-222`).
- `AccessZone` is consumed only by `groups_admin.rs` and `handlers/pair.rs`. The
  MCP handlers never read it, so no zone check applies to `/mcp/*`.
- Confirmed empirically: `curl http://100.69.0.254:8080/mcp/compact` with no auth
  headers and no Ghostbridge headers returned the full tool list.
- `op-web` binds `0.0.0.0`, but nftables restricts the port to the mesh:
  `ip saddr 100.69.0.0/16 tcp dport { 3003, 6333, 6334, 8080, ... } accept`.
  **The port is therefore not internet-exposed** — the host's public interface is
  `pub0 188.68.58.237/22`, and only 22, 25, 80, 143, 443, 465, 587, 993 and
  UDP 51821 are accepted there.

**Severity, stated precisely**: this is not remote-internet RCE. The practical
exposure is that **any peer on the netmaker mesh can execute shell commands**
without presenting a credential, and no per-client identity is recorded at the
ingress. That is a lateral-movement and attribution problem, not an emergency
perimeter hole. The firewall is doing real work here and should be credited as a
compensating control.

`execute_tool` reaches the full registry, including `agent_shell_executor_exec`
and `agent_python_executor_run`. The bridge capability gate does not substitute
for ingress auth: `cognitive_mcp.invoke` is a single blanket grant, so it does not
differentiate callers.

By contrast, `:8090` does enforce the Ghostbridge interceptor
(`op-grpc-bridge/src/interceptor.rs:98`); its responses carry
`x-ghostbridge-footprint` and `x-ghostbridge-trace-id`.

**Requirement**: before `:3003` is removed, `/mcp/*` on `:8080` must enforce one
of the following (options 1 and 2 are complementary and preferred):
1. `AccessZone` enforcement restricting `/mcp/*` to netmaker/loopback zones.
2. Ghostbridge footprint header validation equivalent to `:8090`.
3. Binding the MCP routes to `100.69.0.254` only rather than `0.0.0.0` (reduces
   exposure but does not authenticate).

Note that the bridge's capability gate does not substitute for this:
`cognitive_mcp.invoke` is a single blanket grant, so it does not differentiate
callers.

**Removing `:3003` without this would widen the exposed surface rather than
narrow it** — today the `:3003` path at least runs the cognitive `interceptor.rs`.

**Acceptance criteria**: an unauthenticated request to `/mcp/compact` from a
non-allowed zone returns 401 or 403 (currently 200). A netmaker-zone request still
succeeds.


### FR-2b: CognitiveToolService must be relocated too (conflicts with FR-5 as written)

**This gates the `serve_cognitive_grpc` deletion in FR-3 and must be reconciled
with FR-5.**

`:50052` hosts **two** gRPC services, not one. Verified at
`crates/op-cognitive-mcp/src/server.rs:287-298`:

```rust
tonic::transport::Server::builder()
    .accept_http1(true)
    .layer(cors)
    .add_service(tonic_web::enable(
        CognitiveToolServiceServer::with_interceptor(
            grpc_service,
            crate::interceptor::ghostbridge_interceptor,   // <-- enforcement
        ),
    ))
    .add_service(waypipe)                                  // <-- FR-5 covers this
    .add_service(tonic_web::enable(reflection))
    .add_service(tonic_web::enable(health_service))
    .serve(socket_addr)
```

FR-5 relocates only `waypipe` into `op-waypipe-grpc`. As written, that **strands
`CognitiveToolService`**, which has a live consumer: Xray routes the
`mcp.internal` subdomain to `10.200.0.2:50052` targeting
`operation.cognitive.v1.CognitiveToolService`
(`crates/op-identity/src/schema_bridge.rs:1454-1458`). The service is defined at
`crates/op-cognitive-mcp/proto/cognitive.proto:12` and serves the NotebookLM RPCs
(`ask_question`, `list_notebooks`, `select_notebook`, `get_notebook`).

Note also that FR-3 deletes `interceptor.rs`, which is the enforcement wrapper on
`CognitiveToolService`. Any relocation must not silently drop it.

**Requirement**: pick one and record it in the design.

- **(a) Move `CognitiveToolService` to the bridge's `:50051`**, behind the bridge's
  own Ghostbridge interceptor (`op-grpc-bridge/src/interceptor.rs:98`), and
  re-point the Xray route. The bridge already runs a tonic server with reflection
  on `:50051` (`op-grpc-bridge/src/server.rs:46`:
  `DEFAULT_BIND_ADDR = "0.0.0.0:8090,0.0.0.0:50051"`). The gRPC service name,
  package, and method signatures stay identical, so `mcp.internal` clients need
  only a new address. **Preferred** — consistent with the only-door goal and
  preserves enforcement.
- **(b) Have `op-waypipe-grpc` host both services on `:50052`.** Avoids the Xray
  change and keeps external `:50052` clients working unmodified, but keeps the
  NotebookLM surface outside the bridge, and requires re-implementing the
  ghostbridge interceptor in the new binary (FR-3 deletes the original).

Option (a) is preferred; if (b) is chosen, FR-5's scope and service name must be
updated since `op-waypipe-grpc` would no longer be waypipe-only.

**Rejected alternative**: converting the four RPCs into `notebooklm` plugin schema
methods. Cleaner in principle (everything behind `PluginV1.Call`) but a breaking
change for `mcp.internal` clients, and it overlaps the separate `notebooklm`
plugin spec. Deferred.

**Acceptance criteria**: `operation.cognitive.v1.CognitiveToolService` is reachable
via gRPC reflection at its new location, a NotebookLM RPC answers there, the Xray
route resolves to that location, and the call passes through a Ghostbridge
interceptor. If option (a): `grep -n '50052' crates/op-identity/src/schema_bridge.rs`
returns no match.

### FR-3: Listener and flag deletion

The following are deleted:

| Item | Location | Reason |
|------|----------|--------|
| `start_http_server` | `server.rs:146` | Listener function dead |
| `start_grpc_server` | `server.rs:179` | Listener function dead |
| `start_dual` | `server.rs:192` | Listener function dead |
| `serve_cognitive_grpc` | `server.rs:253` | gRPC server builder dead |
| `--no-http` flag | `main.rs:64` | No HTTP transport to disable |
| `--no-grpc` flag | `main.rs:60` | No gRPC transport to disable |
| `http_enabled`/`grpc_enabled` match arms | `main.rs:181-200` | Transport selection dead |
| `exit 1` guard | `main.rs:181-183` | Both-disabled case impossible |
| `CognitiveMcpConfig.http_enabled` | `cognitive_mcp.rs:52` | Schema field dead |
| `CognitiveMcpConfig.grpc_enabled` | `cognitive_mcp.rs:54` | Schema field dead |
| `CognitiveMcpConfig.http` | `cognitive_mcp.rs:46` | Bind addr dead |
| `CognitiveMcpConfig.grpc` | `cognitive_mcp.rs:48` | Bind addr dead |
| `interceptor.rs` | whole module | Ghostbridge gRPC interceptor dead |
| `dbus_interface.rs` | whole module | Never registered; dead code |
| `client_config.rs` | whole module | HTTP client to `:3003` dead |

**`--stdio` is kept.** Justification: the `.mcp.json` `op-cognitive-mcp` entry
uses `--stdio` to run a local MCP server (same DB, no network). This is a
library-use mode — stdin/stdout JSON-RPC talking to the `ToolRegistry`. It does
not open any TCP listener and is useful for local debugging/attach.

**Schema hash change**: Removing `http_enabled`, `grpc_enabled`, `http`, `grpc`
from `CognitiveMcpConfig` changes the struct layout. The sealed blob must be
re-sealed with `op-blob` after the schema change. The projection at
`/dev/shm/opdbus/projections/cognitive_mcp.json` will no longer contain these
fields. Consumers reading the projection must tolerate their absence (they are
already `#[serde(default)]` in reader code; verified at
`crates/op-cognitive-mcp/src/main.rs:97` which deserializes with
`serde_json::from_slice::<CognitiveMcpConfig>` — removing fields from the
*source* breaks nothing when the consumer has defaults).

**Acceptance criteria**: After deletion, `op-cognitive-mcp` starts with
`--stdio` only. `cargo check -p op-cognitive-mcp` passes. No unused-import or
dead-code warnings from the deletions remain.

### FR-4: Context-awareness SSE surface relocated in-process

Phase 1 adds an `op-web` proxy at `/cognitive/context/` that proxies to `:3003`.
Phase 2 replaces that proxy with **in-process hosting** of the context engine:

- `op-web` (or `op-grpc-bridge`) constructs a `ContextAwarenessEngine`
  (from `crates/op-cognitive-mcp/src/context_awareness.rs:399`) using the same
  `CognitiveMcpServer` instance that provides the tool registry.
- `build_context_router` (`crates/op-cognitive-mcp/src/context_server.rs:75`)
  produces an `axum::Router` which is merged into `op-web`'s route tree under
  `/cognitive/context/`.
- Routes: `/cognitive/context/stream/:session_id` (SSE),
  `/cognitive/context/status/:session_id`, `/cognitive/context/record`,
  `/cognitive/context/request_push`, `/cognitive/context/health`.
- No HTTP proxy to `:3003`; the engine runs in the same process.

**Acceptance criteria**: After `:3003` is dead,
`curl http://127.0.0.1:8080/cognitive/context/health` returns 200.
SSE subscription to `/cognitive/context/stream/test-session` opens successfully.

### FR-5: WaypipeTunnel relocated to its own service

`op-waypipe-grpc` has a standalone binary
(`crates/op-waypipe-grpc/src/bin/op-waypipe-grpc.rs`). A new runit service
runs it on a dedicated port (e.g. `10.200.0.2:50052` or a new port), replacing
the co-hosted instance inside `op-cognitive-mcp`'s gRPC server.

Alternatively, WaypipeTunnel can be co-hosted on `op-grpc-bridge`'s tonic
server (the bridge already runs tonic on the session bus socket). Decision:
prefer a separate runit service for process isolation — a waypipe tunnel crash
must not take down the bridge.

**Acceptance criteria**: `grpcurl -plaintext 10.200.0.2:50052
list op.waypipe.v1.WaypipeTunnel` succeeds after the migration. The tunnel is
no longer served by `op-cognitive-mcp`'s PID.

### FR-6: Orphaned code deletion

| Code | Decision | Reason |
|------|----------|--------|
| `dbus_interface.rs` | **Delete** | Never registered on any connection; Phase 1 confirmed dead code |
| `CognitiveGrpcService` in `grpc_service.rs` | **Delete** | Only consumer was `:50052` gRPC; no tool registry; NotebookLM queries now go through `invoke_tool` → `ask_question` tool |
| `client_config.rs` (pool + circuit breaker) | **Delete** | Only consumer was `examples/external_client.rs`; external clients now use the bridge |
| `interceptor.rs` (Ghostbridge gRPC) | **Delete** | Only used by `serve_cognitive_grpc` which is deleted |
| `examples/external_client.rs` | **Delete** | Uses deleted `client_config.rs` API |
| `deploy/config/cognitive-mcp-clients.json` | **Update** | Remove `:3003` and `:50052` endpoints; point to bridge/op-web paths |

`notebooklm.rs` is **kept** — it registers tools into the `ToolRegistry` (called
from `server.rs::new()` indirectly through `CognitiveToolRegistry::register_all`
or separately). These tools are now reachable through the in-process registry.

**Acceptance criteria**: `cargo check -p op-cognitive-mcp` passes with no dead-code
warnings from these deletions. `grep -r 'client_config\|dbus_interface\|interceptor'
crates/op-cognitive-mcp/src/lib.rs` returns nothing.

### FR-7: Verification that the door is actually closed

An acceptance test proving the "only door" property:

1. `ss -lntp | grep -E '3003|50052'` shows **zero** lines for `op-cognitive-mcp`'s
   PID on any address (including `10.200.0.2`, `127.0.0.1`, `0.0.0.0`).
   The `100.69.0.254:3003` line from `fwd-3003` (PID 1250) remains — it
   forwards to a container, not to the host.

2. Every tool previously reachable via `http://10.200.0.2:3003/mcp` `tools/call`
   is reachable via:
   ```bash
   ./bin/zcall cognitive_mcp invoke_tool -a '{"tool_name":"<name>","arguments":{...}}'
   ```

3. Each such call returns an envelope carrying:
   - `event_id > 0`
   - Non-empty `event_hash`
   (built in `crates/op-grpc-bridge/src/mutation_engine.rs:1015-1021`)

4. Phase 1's `bin/verify-bridge-equivalence.sh` baseline output (captured before
   Phase 2 merge) is compared with the Phase 2 output. Tool results must have
   equivalent JSON shapes.

**Acceptance criteria**: The verification script exits 0. Manual `ss` inspection
confirms zero cognitive-mcp TCP listeners.

---

## Non-Functional Requirements

### NFR-1: Bridge must not fail closed

If CozoDB or Qdrant is unreachable at bridge startup, the bridge still starts
and serves all 65 plugins. Cognitive tool calls return envelope
`{"success": false, "error": "..."}`, not a process crash or a D-Bus timeout on
unrelated plugins.

### NFR-2: No new crate

No `op-mcp-registry` extraction. The existing `op-cognitive-mcp` dependency in
`op-grpc-bridge` is sufficient. The `ToolRegistry` is accessed through
`CognitiveMcpServer::tool_registry()`.

### NFR-3: Tool execution does not block the bridge event loop

Tool execution (which may include agent shell runners, network I/O, LLM calls)
runs on spawned tasks. The bridge's D-Bus dispatch loop remains responsive to
other plugins during long-running tool calls.

### NFR-4: No Python

**Hard constraint: no Python is permitted.** This is stronger than "no new Python" —
the existing Python forwarder layer is itself a violation to be eliminated, not
preserved.

Current Python in the mesh ingress path (verified 2026-07-29):

| PID | Process | Role |
|---|---|---|
| 1250 | `tcpfwd.py 100.69.0.254 3003 10.200.0.1 3003` | dead-ends (xray has no :3003) |
| 1157 | `tcpfwd.py ... 28082 10.200.0.1 28082` | dead-ends |
| 1158 | `tcpfwd.py ... 6333 10.200.0.1 6333` | dead-ends (xray has 6334) |
| 1268 | `tcpfwd.py ... 8091 10.200.0.1 8091` | dead-ends (xray has 8081) |
| 1211 | `tcpfwd.py ... 6334 10.200.0.1 6334` | working |
| 1289 | `tcpfwd.py ... 8444 10.200.0.1 8444` | working |
| 1233 | `tcpfwd.py ... 8081 127.0.0.1 8081` | working (local target) |
| 1848 | `udsfwd.py 10.200.0.2 8091 /var/lib/assistant-controlplane/http.sock` | TCP→UDS shim |

**In scope for this spec**: `fwd-3003` is deleted (FR-2). That removes one Python
process and is required by the single-door goal independently of this constraint.
No new Python is introduced, and no acceptance criterion in this spec may invoke
`python3` — use `jq` (`/usr/bin/jq`) for JSON assertions.

**Out of scope but recorded — full de-Pythoning of the forwarder layer.** The
remaining seven processes are pre-existing infrastructure whose replacement is its
own work item. The replacement requires no daemon at all:

- **`tcpfwd.py` → nftables DNAT.** These are plain TCP port forwards
  (`100.69.0.254:<port>` → `10.200.0.1:<port>`). nftables already manages this
  host's ruleset and already has `table ip xray_egress` with `forward` and
  `postrouting` chains for the xray container. That table has **no `prerouting`
  chain**, which is exactly where the DNAT belongs:

  ```
  chain prerouting {
      type nat hook prerouting priority dstnat; policy accept;
      iifname "3tched" tcp dport { 6334, 8444 } dnat to 10.200.0.1
  }
  ```

  Kernel-level, zero userspace processes, and it collapses several supervised runit
  services into one declarative rule. Only ports xray actually serves should be
  listed — the dead-ending ports are deleted rather than translated.

- **`udsfwd.py` → `socat` or native UDS.** nftables cannot DNAT to a unix socket.
  Options, in order of preference: (a) have the consumer speak the unix socket
  directly, (b) have `assistant-controlplane` listen on TCP, (c) `socat
  TCP-LISTEN:8091,fork,reuseaddr UNIX-CONNECT:/var/lib/assistant-controlplane/http.sock`
  (C, not Python). Note this path serves the deprecated "assistant" surface and may
  be removable outright.

`jq`, `socat`, and `nft` are all present on the host — no new dependency is needed
for any of the above.

### NFR-5: OSCAL subid coverage

No new methods or events are added in Phase 2 (all were added in Phase 1).
Subid assignments are unchanged.

### NFR-6: Sealed blob resealing

After removing `http_enabled`/`grpc_enabled`/`http`/`grpc` from
`CognitiveMcpConfig`, run `op-blob seal cognitive_mcp` to produce a new sealed
blob. The projection publisher in `op-grpc-bridge` will emit the new shape on
next publish cycle.

---

## Out of Scope

- Adding per-tool capability granularity (deferred — single `cognitive_mcp.invoke`).
- Adding a UDS transport to `crates/op-mcp/src/transport/`.
- Protocol sniffer / auto-detect layer.
- Replacing `tcpfwd.py` with a Rust binary (not broken, not in scope).
- Normalizing method name casing across plugins.
- Fixing the `op-xray-daemon` bus name conflict.
- The `op-cognitive-mcp` process being removed entirely (it still serves
  `--stdio` mode; it may eventually become library-only but that is a future spec).
