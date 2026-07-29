# Design: cognitive-mcp-bridge-only-door

## Architecture Decisions

Five design questions resolved below with decisions, rationale, and rejected alternatives.

---

### DQ-1: Generic tool invocation — method design

**Decision**: Add a single new schema method `invoke_tool` to the `cognitive_mcp` plugin.

```
Method name:     invoke_tool
Effect:          Mutation
Capability:      cognitive_mcp.invoke
Subid:           mut.service.cognitive-mcp.tool.invoke@v1
```

**Arg schema** (validated by the bridge's `validate_json_args` gate):

```json
{
  "type": "object",
  "required": ["tool_name", "arguments"],
  "properties": {
    "tool_name": { "type": "string", "minLength": 1 },
    "arguments": { "type": "object" }
  },
  "additionalProperties": false
}
```

**Response envelope** (returned as the single string from `Call`):

```json
{
  "success": true,
  "tool_name": "memory_store",
  "result": { /* tool-specific output */ },
  "error": null,
  "event_id": 42,
  "event_hash": "b3abc..."
}
```

On tool-not-found:
```json
{
  "success": false,
  "tool_name": "nonexistent_tool",
  "result": null,
  "error": "tool not found: nonexistent_tool",
  "event_id": 42,
  "event_hash": "b3abc..."
}
```

**How `list_tools` and `invoke_tool` stay in sync**: `list_tools` already queries
`ToolRegistry::list()` which returns live `ToolDefinition` entries. `invoke_tool` calls
`ToolRegistry::execute()` by name. They share the same registry instance. No separate
synchronization is needed — the registry IS the source of truth for available tools.

**Error distinction**:
- `UnknownMethod` D-Bus error → method `invoke_tool` does not exist in schema (impossible after
  this change).
- `InvalidArgs` D-Bus error → JSON doesn't match the arg schema (e.g. missing `tool_name`).
- Envelope `success: false` → method dispatched correctly but the tool itself failed or was
  not found. This is the tool-layer error, not a protocol error.

**Capability granularity**: single blanket `cognitive_mcp.invoke` for all tools. Per-tool
capabilities would require dynamic schema mutations (schema changes on tool registration)
which violates the sealed-blob contract. Deferred to a future spec if needed.

**Rejected alternatives**:

1. *One schema method per tool* — impossible; tool count is dynamic (40+ registered at runtime).
   Schema is sealed at blob build time. Would require re-sealing on every tool registration.

2. *Reuse `list_tools` return value as an arg-validation schema for `invoke_tool`* — violates
   the static schema principle. The bridge validates args against the MethodDecl which is fixed
   at build time. The `arguments` field is typed as `object` (permissive) because tool schemas
   are dynamic. Tool-level arg validation happens inside the tool executor, not the bridge gate.

3. *Multiple invoke methods by category* (`invoke_memory_tool`, `invoke_code_tool`, etc.) —
   adds complexity without security benefit since capability is the same. One method, one gate.

---

### DQ-2: Capability decoupled from transport

**Decision**: `CognitiveGrpcService` stays as internal plumbing (NotebookLM bridge RPCs) but
is NOT the control-plane door. The bridge dispatch arm for `cognitive_mcp` reaches the tool
registry via a D-Bus call to the `CognitiveMcpInterface` already defined in
`crates/op-cognitive-mcp/src/dbus_interface.rs:27`.

Concretely:
- `op-cognitive-mcp` registers `CognitiveMcpInterface` on the session bus at object path
  `/org/opdbus/v1/plugins/cognitive_mcp/executor` (a child of the plugin's object) under
  interface `org.opdbus.v1.plugins.CognitiveMcp`.
- The bridge's `MutationEngine` dispatch arm for `cognitive_mcp` + `invoke_tool` makes a
  D-Bus proxy call to that interface's `CallTool(tool_name, args_json)`.
- For the existing 15 schema methods (`memory_store`, `code_search`, etc.): dispatch
  translates the method name to a tool-registry call via the same `CallTool` path. Methods
  that are plugin-local (`get_config`, `set_config`, `restart_service`) dispatch to the
  plugin's own `apply_state`/`current_config` without crossing to the executor.

**Context-awareness SSE surface**: relocated to `op-web`'s HTTP server under
`/cognitive/context/` prefix. `op-web` already has the MCP compact endpoint; adding an SSE
proxy route is consistent with its role as the unified HTTP front-end.

**Rejected alternatives**:

1. *Delete `CognitiveGrpcService` entirely* — premature. The NotebookLM bridge gRPC surface
   (`ask_question`, `query_notebook`, etc.) is consumed by the container gateway. These map
   to the `notebooklm` plugin's 106 methods, not `cognitive_mcp`. Killing the gRPC server
   would strand NotebookLM consumers. Separate concern, separate spec.

2. *Add ToolRegistry to the gRPC service* — wrong direction. Would create two doors (gRPC
   with full tools + bridge with full tools) and two auth chains. The goal is one door.

3. *Unix domain socket IPC instead of D-Bus* — D-Bus is already running, `op-cognitive-mcp`
   already has zbus in its deps via `dbus_interface.rs`, and the existing `CognitiveMcpInterface`
   is already written. UDS would require a new transport layer. Unnecessary new mechanism.

---

### DQ-3: Projection stops acting as bind directive

**Decision**: Remove the `cognitive_mcp_bind_config()` function from
`crates/op-cognitive-mcp/src/main.rs:90-105`. Replace with direct CLI/env-var reading
(which clap already provides via `#[arg(long, env = "...")]` on the `Cli` struct).

Bind configuration legitimately lives in:
1. **s6 env-dir** (`/etc/s6/sv/op-cognitive-mcp/env/COGNITIVE_MCP_BIND`) — written by
   `cognitive_mcp.rs::apply_state()` when `set_config` is called via D-Bus.
2. **CLI flags** (`--http`, `--grpc`) — for manual/debugging overrides.
3. **WireGuard IP detection** — runtime address promotion from `0.0.0.0`.

The projection at `/dev/shm/opdbus/projections/cognitive_mcp.json` is **published state**:
it tells consumers "here is what the cognitive_mcp service looks like right now" (health,
tool count, addresses it claims to be on). It is NEVER read back as config input by the
service itself. This is consistent with the principle "the sealed blob IS the plugin" —
projections are outbound state publications, not inbound configuration.

**What changes in the projection**: the `http_enabled`/`grpc_enabled` fields remain as
status indicators (they reflect whether the service has those transports active). But
they no longer influence the service's own startup behavior.

**Rejected alternatives**:

1. *Keep projection-read but make CLI override it* — violates "projections are published state".
   The projection should reflect reality, not prescribe it. Creates a confusing feedback loop
   where changing `set_config` via D-Bus writes a projection that the service reads on restart,
   but only if the CLI doesn't override. Too many precedence layers.

2. *Delete the projection fields entirely* — breaks consumers that read the projection to
   discover which endpoints are available. Keep as status, remove as input.

---

### DQ-4: Client transport detection replaces server flags

**Decision**: The server does NOT detect or sniff client transport. Instead:

- **`op-cognitive-mcp --stdio`** remains the one direct-attach mode. It is how
  `op-grpc-bridge`'s dispatch arm reaches the tool registry: `op-cognitive-mcp` is
  launched by the service supervisor, registers its `CognitiveMcpInterface` on the session
  bus, and the bridge calls into it. The stdio flag becomes irrelevant to external clients
  because they go through the bridge, not through `op-cognitive-mcp` directly.

- **External MCP clients** choose their transport at the CLIENT config level:
  - Stdio: `op-mcp-server --stdio` (the compact MCP forwarder, already exists).
  - HTTP/SSE: `http://127.0.0.1:8080/mcp/compact` (op-web, already exists).
  - D-Bus direct: `busctl --address=... call ...` (for scripts/automation).

- **`--no-http`/`--no-grpc` flags**: become no-ops in Phase 1. Removed in Phase 2.

**What is in scope for this spec**:
- Wiring `op-cognitive-mcp` to register `CognitiveMcpInterface` on the session bus at startup.
- Adding the `cognitive_mcp` dispatch arm in `MutationEngine`.
- Migrating MCP client configs to bridge-backed paths.
- Deprecating the direct listeners.

**What is deferred**:
- A protocol sniffer (auto-detect stdio vs HTTP vs WebSocket on a single socket).
- A UDS transport in `crates/op-mcp/src/transport/` (the directory has stdio/http/websocket
  but no UDS).
- Removing the listener code paths from `op-cognitive-mcp` (Phase 2).

**Rejected alternatives**:

1. *Protocol sniffer on a single socket* — elegant but complex, out of scope, and doesn't
   solve the "one door" problem (a sniffer is still a second door if it's not behind the
   bridge enforcement chain).

2. *Remove `--stdio` entirely* — can't. Stdio is needed for the process to function as a
   D-Bus service while also being able to register tools (the tool registration happens
   inside `CognitiveMcpServer::new()` which must run regardless of network listeners).

---

### DQ-5: Cutover sequencing

**Decision**: Five ordered steps, each independently verifiable and reversible.

| Step | Action | Verification | Rollback |
|------|--------|--------------|----------|
| 1 | Add `invoke_tool` to `cognitive_mcp_schema()` + `InvokeToolInput`/`InvokeToolOutput` structs | `cargo check -p op-plugins` + `./bin/zcall methods cognitive_mcp` shows 16 methods | Remove the `methods.insert` line |
| 2 | Add `"cognitive_mcp"` arm in `MutationEngine::dispatch_method_call` that calls `CognitiveMcpInterface.CallTool` on the session bus | `cargo check -p op-grpc-bridge` + `./bin/zcall call cognitive_mcp invoke_tool '{"tool_name":"list_tools","arguments":{}}'` returns tool list | Remove the match arm |
| 3 | Wire `op-cognitive-mcp` to register `CognitiveMcpInterface` on the session bus at startup (before any listener bind) | `busctl introspect ... /org/opdbus/v1/plugins/cognitive_mcp/executor` shows `CallTool` method | Remove the bus registration code |
| 4 | Remove `cognitive_mcp_bind_config()` from `main.rs`; use CLI/env only | `cargo check -p op-cognitive-mcp` + start with `--no-http --no-grpc --stdio` → `ss -lntp` shows zero listeners for PID | Restore the function |
| 5 | Migrate MCP client configs: `.mcp.json` → `op-web-compact`, `~/.factory/mcp.json` → remove HTTP `cognitive-mcp` entry | All MCP clients successfully call tools via bridge path | Revert config files to `:3003` entries |

**Explicitly migrated MCP client entries**:

| Config file | Entry name | Current | New |
|-------------|-----------|---------|-----|
| `.mcp.json` | `cognitive-mcp` | `http://10.200.0.2:3003/mcp` | Removed (use `op-web-compact` at `:8080`) |
| `~/.factory/mcp.json` | `cognitive-mcp` | `http://10.200.0.2:3003/mcp` | Removed (use `op-dbus-compact` at `:8080`) |
| `~/.factory/mcp.json` | `op-cognitive-mcp` | stdio, disabled | Removed entirely (redundant with bridge path) |

---

## Affected Files

| File | Change type |
|------|-------------|
| `crates/op-plugins/src/state_plugins/cognitive_mcp.rs` | Add `invoke_tool` method + `InvokeToolInput`/`InvokeToolOutput` structs |
| `crates/op-grpc-bridge/src/mutation_engine.rs` | Add `"cognitive_mcp"` dispatch arm |
| `crates/op-cognitive-mcp/src/main.rs` | Remove `cognitive_mcp_bind_config()`; add session bus registration for `CognitiveMcpInterface` |
| `crates/op-cognitive-mcp/src/dbus_interface.rs` | No structural change (already correct); confirm it is wired at startup |
| `crates/op-cognitive-mcp/src/lib.rs` | Ensure `dbus_interface` module is pub-exported |
| `.mcp.json` | Phase 1: add deprecation comment. Phase 2: remove `cognitive-mcp` entry. (NOT modified in this spec per instructions) |
| `~/.factory/mcp.json` | Same as above (NOT modified in this spec per instructions) |

---

## Exact Signatures and JSON Shapes

### New schema method registration (cognitive_mcp.rs)

```rust
// Input: validated by bridge arg gate
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InvokeToolInput {
    /// Name of the tool to invoke (must exist in ToolRegistry).
    pub tool_name: String,
    /// Tool-specific arguments (opaque object passed to the tool executor).
    pub arguments: serde_json::Value,
}

// Output: returned as the Call response string
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InvokeToolOutput {
    pub success: bool,
    pub tool_name: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}
```

Schema registration:
```rust
schema.methods.insert(
    "invoke_tool".to_string(),
    method_decl_from_schemars_with_output::<InvokeToolInput, InvokeToolOutput>(
        "invoke_tool",
        SideEffect::Mutation,
        false,
        "cognitive_mcp.invoke",
        "mut.service.cognitive-mcp.tool.invoke@v1",
    ),
);
```

### MutationEngine dispatch arm (mutation_engine.rs)

```rust
"cognitive_mcp" => {
    let args = serde_json::to_value(&parsed_value)?;
    dispatch_cognitive_mcp_method(method, &args).await?
}
```

Free function:
```rust
async fn dispatch_cognitive_mcp_method(
    method: &str,
    args: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    // Plugin-local methods (no executor needed)
    match method {
        "get_config" => {
            let cfg = op_plugins::state_plugins::cognitive_mcp::CognitiveMcpConfig::default();
            // Read current config from s6 env-dir
            return Ok(serde_json::to_value(
                op_plugins::state_plugins::cognitive_mcp::CognitiveMcpPlugin::current_config_json()
            )?);
        }
        "restart_service" => {
            return op_plugins::state_plugins::cognitive_mcp::reload_service_dbus().await;
        }
        _ => {}
    }

    // Tool-registry methods: forward to CognitiveMcpInterface on session bus
    let tool_name = match method {
        "invoke_tool" => args.get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing tool_name"))?
            .to_string(),
        // Existing 15 schema methods map to tool names
        other => other.to_string(),
    };

    let tool_args = match method {
        "invoke_tool" => args.get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({})),
        _ => args.clone(),
    };

    // D-Bus call to the executor interface
    let conn = zbus::Connection::session().await?;
    let proxy = zbus::Proxy::builder(&conn)
        .destination("org.opdbus.v1.plugins")?
        .path("/org/opdbus/v1/plugins/cognitive_mcp/executor")?
        .interface("org.opdbus.v1.plugins.CognitiveMcp")?
        .build()
        .await?;

    let result_json: String = proxy
        .call("CallTool", &(tool_name.as_str(), serde_json::to_string(&tool_args)?.as_str()))
        .await?;

    serde_json::from_str(&result_json)
        .map_err(|e| anyhow::anyhow!("executor returned invalid JSON: {e}"))
}
```

### Session bus registration (op-cognitive-mcp main.rs)

```rust
// After server construction, before listener bind:
let conn = zbus::ConnectionBuilder::session()?
    .serve_at(
        "/org/opdbus/v1/plugins/cognitive_mcp/executor",
        CognitiveMcpInterface::new(server.tool_registry()),
    )?
    .build()
    .await?;
info!("Registered CognitiveMcpInterface on session bus");
```

Note: `op-cognitive-mcp` does NOT `request_name` — it does not own the bus name. It only
registers an object under the path owned by `op-grpc-bridge`. This works because both
processes connect to the same session bus and zbus allows any connection to serve objects
at any path (the name owner controls routing for method calls to that destination, but
`op-grpc-bridge` can route calls to child paths to the appropriate connection).

**Correction**: Actually, in D-Bus, only the name owner receives method calls destined for
that name. Since `op-grpc-bridge` owns `org.opdbus.v1.plugins`, calls to child paths under
that name go to `op-grpc-bridge`, not `op-cognitive-mcp`. Therefore:

**Revised approach**: `op-cognitive-mcp` requests its own well-known name
`org.opdbus.v1.executors.cognitive_mcp` on the session bus. The dispatch arm in
`mutation_engine.rs` calls that destination:

```rust
let proxy = zbus::Proxy::builder(&conn)
    .destination("org.opdbus.v1.executors.cognitive_mcp")?
    .path("/executor")?
    .interface("org.opdbus.v1.plugins.CognitiveMcp")?
    .build()
    .await?;
```

This is clean: `org.opdbus.v1.plugins` = schema authority (bridge),
`org.opdbus.v1.executors.*` = tool executors (backends).

---

## Communication Flow

```
MCP Client
    │  (stdio / HTTP / D-Bus — client's choice)
    ▼
op-web :8080 / op-mcp-server --stdio / busctl
    │  PluginV1.Call("invoke_tool", '{"tool_name":"X","arguments":{...}}')
    ▼
op-grpc-bridge (session bus owner: org.opdbus.v1.plugins)
    │  1. Method-existence gate: "invoke_tool" in schema ✓
    │  2. Arg validation: {tool_name: string, arguments: object} ✓
    │  3. Capability check: cognitive_mcp.invoke granted? ✓
    │  4. Event chain: record actor_id, capability_id, event_hash
    │  5. Dispatch: "cognitive_mcp" match arm
    ▼
D-Bus call → org.opdbus.v1.executors.cognitive_mcp /executor CallTool("X", "{...}")
    │
    ▼
op-cognitive-mcp (CognitiveMcpInterface)
    │  ToolRegistry::execute("X", parsed_args)
    ▼
Tool handler (e.g. MemoryTool, CodeSearchTool, ...)
    │
    ▼
Result JSON → back up the chain → Call return string
```

---

## What Does NOT Change

| Item | Reason |
|------|--------|
| `ToolRegistry` / `Tool` trait in `op-mcp` | Execution substrate; stays as-is |
| `CognitiveGrpcService` in `op-cognitive-mcp` | NotebookLM bridge; orthogonal |
| Tool registration in `server.rs::new()` | Tools registered correctly; only ingress changes |
| Sealed blob catalog | Read-only from `op-cognitive-mcp`'s perspective |
| The 15 existing schema methods | They stay; `invoke_tool` is additive |
| `notebooklm.rs` | Separate concern |
| `op-state-store` legacy catalog | Not involved |
| `interceptor.rs` Ghostbridge logic | Stays for deprecated listeners during Phase 1 |

---

## OSCAL Subid Assignments

| Item | Subid |
|------|-------|
| `invoke_tool` schema method | `mut.service.cognitive-mcp.tool.invoke@v1` |
| `InvokeToolInput` struct | `sch.software.cognitive-mcp.invoke-tool-input@v1` |
| `InvokeToolOutput` struct | `sch.software.cognitive-mcp.invoke-tool-output@v1` |
| `dispatch_cognitive_mcp_method` function | `mut.service.cognitive-mcp.dispatch@v1` |
| `org.opdbus.v1.executors.cognitive_mcp` bus name | `cfg.service.cognitive-mcp.executor-bus@v1` |
