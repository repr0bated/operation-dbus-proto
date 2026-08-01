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

**Capability granularity**: Single blanket `cognitive_mcp.invoke` for all tools. This means
one grant unlocks the entire agent tool set (including `agent_shell_executor_exec`,
`agent_python_executor_run`). This is acceptable because:
1. Per-tool capabilities would require dynamic schema mutations (tool count is 406 and
   growing at runtime) which violates the sealed-blob contract.
2. **Correction (verified post-implementation)**: the capability is **NOT** granted today.
   An earlier draft claimed it was, inferring that from `zcall` calls reaching arg
   validation. That inference was wrong — arg validation runs *before* the capability
   check, so the `{} is not of type "null"` failure was masking the real gate. With the
   args defect fixed, calls now return `Access denied`.
   `/dev/shm/opdbus/capability-grants.json` grants only
   `cap.software.zeroclaw.chat@v1` and `cap.software.zeroclaw.models.read@v1` to the `*`
   footprint. Matching is exact string membership
   (`interceptor.rs:339 load_capability_grants`), so `cognitive_mcp.invoke` and
   `cognitive_mcp.read` must be listed literally. **cognitive_mcp has therefore never
   been reachable through the bridge.** Granting it is an explicit operator decision,
   not a side effect of this spec.
3. If finer granularity is needed later, a `cognitive_mcp.invoke.dangerous` split-capability
   can be added without breaking the schema (just adding a second check inside the dispatch
   arm for a tool-name allowlist). Deferred to a future spec.

**Rejected alternatives**:

1. *One schema method per tool* — impossible; tool count is dynamic (406 registered at
   runtime). Schema is sealed at blob build time. Would require re-sealing on every tool
   registration.

2. *Reuse `list_tools` return value as an arg-validation schema for `invoke_tool`* — violates
   the static schema principle. The bridge validates args against the MethodDecl which is fixed
   at build time. The `arguments` field is typed as `object` (permissive) because tool schemas
   are dynamic. Tool-level arg validation happens inside the tool executor, not the bridge gate.

3. *Multiple invoke methods by category* (`invoke_memory_tool`, `invoke_code_tool`, etc.) —
   adds complexity without security benefit since capability is the same. One method, one gate.

---

### DQ-2: Dispatch path — no new bus name required

**Decision**: The bridge dispatches `cognitive_mcp` methods by calling the tool registry
**in-process via a Rust function call**, not via a second D-Bus hop.

**Why the original "executor bus name" design was wrong**:
- The only well-known name on the session bus is `org.opdbus.v1.plugins` (owned by
  `op-grpc-bridge`, PID 29396). Verified with `busctl list`.
- The design previously invented `org.opdbus.v1.executors.cognitive_mcp` — this name does
  not exist and would require `op-cognitive-mcp` to connect to the session bus and claim it.
- In D-Bus, method calls are routed to the name owner. Since `op-grpc-bridge` owns
  `org.opdbus.v1.plugins`, only it receives calls to any destination under that name.
  A separate process cannot serve objects under another process's name.

**Corrected architecture**: `op-grpc-bridge`'s `MutationEngine` dispatch arm for
`cognitive_mcp` calls the tool registry directly. The tool registry lives inside
`op-cognitive-mcp`, so the bridge must reach it via one of:

| Option | Mechanism | Verdict |
|--------|-----------|---------|
| A. D-Bus (new bus name) | `op-cognitive-mcp` claims a well-known name | Rejected — adds a name outside the `org.opdbus.v1.plugins` namespace |
| B. D-Bus (unique name rendezvous) | `op-cognitive-mcp` connects to the bus, bridge discovers its unique name (`:1.N`) via a property or signal | Viable but fragile — requires discovery protocol |
| C. gRPC loopback | Bridge calls `op-cognitive-mcp`'s existing `:50052` gRPC | Works today but creates circular dependency on the port we're deprecating |
| **D. HTTP loopback** | Bridge calls `op-cognitive-mcp`'s existing `:3003` MCP endpoint | **Selected for Phase 1** — already works, zero new code in `op-cognitive-mcp` |
| E. Shared library / in-process | Link `ToolRegistry` into `op-grpc-bridge` | Best long-term but large refactor — Phase 2 |

**Phase 1 dispatch path (HTTP loopback)**:
```
PluginV1.Call("invoke_tool", '{"tool_name":"X","arguments":{...}}')
  → SchemaBackedInterface.call() [validates, records event]
    → MutationEngine.dispatch_method_call("cognitive_mcp", "invoke_tool", ...)
      → dispatch_cognitive_mcp_method()
        → HTTP POST http://10.200.0.2:3003/mcp
           body: {"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"X","arguments":{...}}}
        ← JSON result extracted from MCP response envelope
```

This keeps `op-cognitive-mcp` unchanged. The HTTP loopback is local (10.200.0.2 is the
WireGuard svc0 address on the same host), sub-millisecond, and already proven working.
Note: `tcpfwd.py` on `100.69.0.254:3003` forwards to `10.200.0.1:3003`, which is the
xray server (incus container) acting as the intended relay hop to this host. That
chain currently dead-ends because xray exposes no port-3003 listener — see the
Phase 2 spec FR-2. Phase 1 changes nothing here.

**Phase 2 dispatch path (in-process)**: `op-grpc-bridge` constructs `CognitiveMcpServer`
directly and calls `ToolRegistry::execute` in-process, eliminating the HTTP hop. No crate
extraction is required — `ToolRegistry` already lives in the shared `op-mcp` crate and
`op-grpc-bridge` already depends on `op-cognitive-mcp`. At that point `:3003` can be killed.
See `../cognitive-mcp-only-door-phase2/`.

**Context-awareness SSE surface**: Relocated to `op-web`'s HTTP server under
`/cognitive/context/` prefix. `op-web` reaches the context engine via the same HTTP
loopback to `op-cognitive-mcp:3003` (endpoint: `/context/subscribe`). This is consistent
with op-web's role as the unified HTTP front-end.

**Interface used for all external calls**: `org.opdbus.v1.PluginV1` — the single canonical
interface. All 16 methods (15 existing + `invoke_tool`) are dispatched through
`PluginV1.Call(method_name, json_args)` on object path
`/org/opdbus/v1/plugins/cognitive_mcp`. No other interface is involved in the control plane.

**The existing `CognitiveMcpInterface` in `dbus_interface.rs`**: This interface
(`org.opdbus.v1.plugins.CognitiveMcp`) is defined but NOT registered on the session bus
today. It is dead code from a prior design iteration. It is NOT needed for this spec — the
bridge dispatches via HTTP loopback, not via D-Bus to an executor. This interface MAY be
useful in Phase 2 if we choose option B (unique-name rendezvous) instead of option E
(in-process), but that decision is deferred.

**Rejected alternatives**:

1. *Invent `org.opdbus.v1.executors.cognitive_mcp`* — adds a bus name outside the established
   namespace. Violates the constraint that `org.opdbus.v1.plugins` is the only legal name.

2. *Add ToolRegistry to `op-grpc-bridge`'s process* — correct end-state but large refactor.
   Phase 1 uses the HTTP loopback; Phase 2 does the extraction.

3. *Unix domain socket IPC* — new transport layer not needed when HTTP already works.

---

### DQ-3: Projection stops acting as bind directive

**Decision**: Remove the `cognitive_mcp_bind_config()` function from
`crates/op-cognitive-mcp/src/main.rs:90-105`. Replace with direct CLI/env-var reading
(which clap already provides via `#[arg(long, env = "...")]` on the `Cli` struct).

Bind configuration legitimately lives in:
1. **runit run script** (`/etc/runit/sv/op-cognitive-mcp/run`) — sets env vars inline:
   `COGNITIVE_MCP_BIND=10.200.0.2:3003`, `COGNITIVE_MCP_GRPC_BIND=10.200.0.2:50052`.
   There is NO env-dir; env vars are exported directly in the script.
2. **CLI flags** (`--http`, `--grpc`) — for manual/debugging overrides.
3. **WireGuard IP detection** — runtime address promotion from `0.0.0.0`.

**Correction from original design**: The original spec claimed env vars come from "s6
env-dir written by the plugin's `apply_state`". This is wrong:
- The host runs **runit**, not s6. Verified: `sv status op-cognitive-mcp` works;
  `/etc/runit/sv/op-cognitive-mcp/run` exists.
- There is **no env-dir** at `/etc/runit/sv/op-cognitive-mcp/env/`. Env vars are set
  inline in the run script.
- `apply_state` does NOT write to any env-dir today. If it needs to change bind addresses
  in the future, it would rewrite the run script and `sv restart op-cognitive-mcp`.

The projection at `/dev/shm/opdbus/projections/cognitive_mcp.json` is **published state**:
it tells consumers "here is what the cognitive_mcp service looks like right now" (health,
tool count, addresses it claims to be on). It is NEVER read back as config input by the
service itself.

**What changes in the projection**: the `http_enabled`/`grpc_enabled` fields remain as
status indicators (they reflect whether the service has those transports active). But
they no longer influence the service's own startup behavior.

**Rejected alternatives**:

1. *Keep projection-read but make CLI override it* — violates "projections are published state".
   Creates a confusing feedback loop.

2. *Delete the projection fields entirely* — breaks consumers that read the projection to
   discover which endpoints are available. Keep as status, remove as input.

---

### DQ-4: Client transport detection replaces server flags

**Decision**: The server does NOT detect or sniff client transport. Instead:

- **`op-cognitive-mcp --stdio`** remains available for local debugging/attach. It is NOT
  required for D-Bus functionality — `op-cognitive-mcp` does not register on the session
  bus today and this spec does not add such registration (see DQ-2). The `--stdio` flag
  simply starts an MCP stdio transport alongside (or instead of) HTTP/gRPC listeners.

- **External MCP clients** choose their transport at the CLIENT config level:
  - Stdio: `op-mcp-server --stdio` (the compact MCP forwarder via `op-web-compact`).
  - HTTP/SSE: `http://127.0.0.1:8080/mcp/compact` (op-web, already exists).
  - D-Bus direct: `busctl call org.opdbus.v1.plugins /org/opdbus/v1/plugins/cognitive_mcp
    org.opdbus.v1.PluginV1 Call ss "invoke_tool" '{"tool_name":"X","arguments":{}}'`.

- **`--no-http`/`--no-grpc` flags**: Become no-ops in Phase 1. Removed in Phase 2.

**Correction from original DQ-4**: The original spec claimed "stdio is required for the
process to work as a D-Bus service." This is wrong — `--stdio` and D-Bus registration are
independent concerns:
- `--stdio` controls whether an MCP stdio transport is spawned on stdin/stdout.
- D-Bus registration (if added) happens via `zbus::ConnectionBuilder` regardless of
  `--stdio`. The tool registry initializes in `CognitiveMcpServer::new()` unconditionally.
- The real reason `--stdio` is kept: it allows `op-cognitive-mcp` to be used as an MCP
  server in stdio mode by config entries like the `op-cognitive-mcp` entry in `.mcp.json`.

**Rejected alternatives**:

1. *Protocol sniffer on a single socket* — complex, out of scope, and doesn't solve the
   "one door" problem.

2. *Remove `--stdio` entirely* — breaks the `.mcp.json` `op-cognitive-mcp` stdio entry
   and removes a useful debugging mode.

---

### DQ-5: Cutover sequencing and Phase 1 scope

**Decision**: Phase 1 explicitly keeps `:3003` and `:50052` alive. The "only door" goal
is NOT achieved until Phase 2.

**Why this is acceptable**:
1. Phase 1 proves the bridge path works end-to-end with equivalence testing.
2. The HTTP loopback dispatch (DQ-2) actually DEPENDS on `:3003` being alive — it's the
   mechanism by which the bridge reaches the tool registry.
3. Drift-back is prevented by: (a) deprecation warnings on the listener functions,
   (b) MCP client configs migrated to bridge-backed paths, (c) the verification script
   that proves equivalence, (d) Phase 2 has a concrete prerequisite: ToolRegistry extraction
   into a shared crate eliminates the HTTP dependency.

**Phase 2 gate** (superseded — see `../cognitive-mcp-only-door-phase2/`):
The `op-mcp-registry` extraction is unnecessary; `op-grpc-bridge` already depends on
`op-cognitive-mcp` and can construct `CognitiveMcpServer` in-process. Phase 2 replaces
the HTTP loopback with in-process `ToolRegistry::execute` directly.

Phase 2's real constraint is different: `CognitiveMcpServer::new` opens a **persistent
CozoDB** (`server.rs:41`), which is single-writer. The bridge cannot take registry
ownership while the `op-cognitive-mcp` service is still running, so Phase 2 retires that
service.

**Netmaker client access**: `tcpfwd.py` on `100.69.0.254:3003` forwards to
`10.200.0.1:3003`. `10.200.0.1` is the **xray server** (incus container
`incus-ct-xray`), the intended relay hop that dokodemo-doors mesh traffic on to this
host at `10.200.0.2`. The forwarder is correctly targeted — but xray exposes no
listener for port 3003 (its inbounds cover 8443, 8081, 6334, 8090 and the xhttp
vless port 8444), so the chain dead-ends: the probe of `10.200.0.1:3003` returns
REFUSED. `curl http://100.69.0.254:3003/mcp` returns an empty body, whereas
`curl http://10.200.0.2:3003/mcp` returns a valid MCP `initialize` response.

Consequences:
- No working netmaker traffic currently reaches `:3003`, so Phase 2 can delete
  `fwd-3003` without breaking a live consumer — and should delete rather than
  complete it, since adding an xray port-3003 inbound would finish building the door
  Phase 2 closes.
- Any mesh client configured directly against `10.200.0.2:3003` (bypassing both
  hops) *is* live and must be retargeted.
- The replacement path already works: `op-web` binds `0.0.0.0:8080`, so
  `http://100.69.0.254:8080/mcp/compact` is reachable from netmaker today with no forwarder
  — verified, it returned the tool list. `op-web` provides the HTTP→gRPC translation the
  bridge door requires.

Phase 2 FR-4/FR-6 handle the migration and delete `fwd-3003`.

**Cutover steps** (Phase 1):

| Step | Action | Verification | Rollback |
|------|--------|--------------|----------|
| 1 | Add `invoke_tool` to `cognitive_mcp_schema()` | `cargo check -p op-plugins` + `zcall methods` shows 16 methods | Remove the `methods.insert` line |
| 2 | Add `"cognitive_mcp"` dispatch arm in MutationEngine (HTTP loopback) | `zcall cognitive_mcp invoke_tool -a '{"tool_name":"cognitive_memory","arguments":{"operation":"list_namespaces"}}'` returns data | Remove the match arm |
| 3 | Remove `cognitive_mcp_bind_config()` from `main.rs`; use CLI/env only | Start with `--no-http --no-grpc --stdio` → no TCP listeners | Restore the function |
| 4 | Migrate MCP client configs to bridge-backed paths | All MCP clients call tools via bridge path | Revert config files |
| 5 | Run equivalence verification script | All tools return equivalent shapes via bridge vs direct | N/A (read-only check) |

**Explicitly migrated MCP client entries**:

| Config file | Entry name | Current | New |
|-------------|-----------|---------|-----|
| `.mcp.json` | `cognitive-mcp` | `http://10.200.0.2:3003/mcp` (HTTP direct) | Removed (use `op-dbus-compact` at `:8080` or `op-web-compact` stdio) |
| `~/.factory/mcp.json` | `cognitive-mcp` | `http://10.200.0.2:3003/mcp` (HTTP direct) | Removed (use `op-dbus-compact` at `:8080`) |
| `~/.factory/mcp.json` | `op-cognitive-mcp` | stdio, disabled | Removed entirely |

**Note on tool surface equivalence**: `op-web-compact` (via `op-mcp-server --stdio` or
`:8080/mcp/compact`) exposes 4 meta-tools: `list_tools`, `search_tools`, `get_tool_schema`,
`execute_tool`. These wrap the full 332-tool registry (builtin + dbus-projected + filesystem
tools). This is NOT the same tool surface as `cognitive-mcp`'s direct 406-tool list.
Clients migrating from `cognitive-mcp` to `op-dbus-compact` get the meta-tool interface
(search + execute) rather than 406 flat tools. This is intentional — the meta-tool
interface is the bridge-enforced path.

---

## FR-2: Per-Method Dispatch Mapping (15 existing methods)

The schema declares 15 methods for `cognitive_mcp`. The MutationEngine `_ =>` catch-all
currently echoes `parsed_value` as-is (no real dispatch). This table defines where each
method SHOULD route when the dispatch arm is implemented:

**Critical finding**: The tool registry does NOT expose tools named `memory_store`,
`memory_query`, etc. It exposes `cognitive_memory` (single tool with an `operation`
argument). The schema method names do not map 1:1 to tool registry names.

| Schema method | Effect | Dispatch target | Tool registry name | Arg translation |
|---------------|--------|----------------|-------------------|-----------------|
| `memory_store` | Mutation | HTTP loopback → tool registry | `cognitive_memory` | `{"operation":"store", ...rest}` |
| `memory_query` | Read | HTTP loopback → tool registry | `cognitive_memory` | `{"operation":"query", ...rest}` |
| `memory_retrieve` | Read | HTTP loopback → tool registry | `cognitive_memory` | `{"operation":"retrieve", ...rest}` |
| `memory_delete` | Mutation | HTTP loopback → tool registry | `cognitive_memory` | `{"operation":"delete", ...rest}` |
| `memory_list_namespaces` | Read | HTTP loopback → tool registry | `cognitive_memory` | `{"operation":"list_namespaces"}` |
| `code_search` | Read | HTTP loopback → tool registry | `search_blob_vectors` | Pass-through |
| `code_index` | Mutation | HTTP loopback → tool registry | `refresh_blob_vectors` | Pass-through |
| `code_context` | Read | HTTP loopback → tool registry | `search_blob_vectors` | `{"activity_type":"query", ...rest}` |
| `gemini_query` | Mutation | HTTP loopback → tool registry | `ask_question` | Map `query` → `question` field |
| `list_tools` | Read | HTTP loopback → MCP `tools/list` | N/A (MCP protocol method) | No args |
| `get_health` | Read | In-process (read projection) | N/A | No args, return projection JSON |
| `get_config` | Read | In-process (read projection) | N/A | No args, return config subset |
| `set_config` | Mutation | In-process (apply_state) | N/A | Config fields |
| `restart_service` | Mutation | In-process (sv restart) | N/A | No args |
| `register_tool` | Mutation | HTTP loopback → tool registry | `register_tool` | Pass-through |

**Arg validation bug**: The current schema declares `list_tools`, `get_health`, and
`get_config` with arg type `"null"` (meaning no arguments allowed). But `zcall` sends `{}`
by default, causing `InvalidArgs: {} is not of type "null"`. This needs fixing in the
schema — these methods should accept either `null` or `{}` (empty object).

**The `invoke_tool` method** (new, DQ-1) bypasses this mapping entirely — it takes an
explicit `tool_name` and `arguments` and calls `tools/call` on the MCP endpoint directly.
This is the recommended path for new consumers. The 15 existing methods are kept for
backward compatibility but are effectively sugar over `invoke_tool`.

---

## FR-7: Context-awareness SSE surface

**Decision**: Relocate to `op-web` under `/cognitive/context/`.

**How op-web reaches the context engine**: HTTP proxy to `http://10.200.0.2:3003/context/subscribe`.
This is the same HTTP loopback pattern as DQ-2's dispatch. `op-web` already proxies other
services (it has the MCP compact endpoint). Adding an SSE proxy route is consistent.

**This gates removing `:3003`**: Yes. Until Phase 2 extracts the context engine into a
shared crate or moves it to `op-web` natively, the SSE proxy depends on `:3003`. This is
explicitly acknowledged as a Phase 2 prerequisite.

---

## Communication Flow

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
    │  5. Dispatch: "cognitive_mcp" match arm
    ▼
HTTP POST http://10.200.0.2:3003/mcp
    body: {"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"X","arguments":{...}}}
    │
    ▼
op-cognitive-mcp (ToolRegistry::execute)
    │
    ▼
Tool handler (e.g. MemoryTool, CodeSearchTool, ...)
    │
    ▼
Result JSON → back up the chain → PluginV1.Call return string
```

---

## Design Decision Notes (Issues 8–11)

### Issue 8: Capability granularity

**Resolution**: Single `cognitive_mcp.invoke` is intentional for Phase 1. See DQ-1
"Capability granularity" section. The capability is **NOT** granted today — corrected
after implementation: `zcall` calls now return `Access denied` once the arg-validation
defect that was masking the capability gate is fixed. Only the two
`cap.software.zeroclaw.*` entries are present in
`/dev/shm/opdbus/capability-grants.json`. Granting `cognitive_mcp.invoke` is an operator
decision and unlocks the entire 406-tool registry, including
`agent_shell_executor_exec` and `agent_python_executor_run`. A future spec
can add a tool-name allowlist inside the dispatch arm without changing the schema.

### Issue 9: invoke_tool declared as Mutation

**Resolution**: Intentional. Every `invoke_tool` call writes an event-chain entry because:
1. The event chain is the accountability audit trail. Even read-only tool calls must be
   recorded for forensic purposes (who called what, when).
2. The event chain append is the mechanism that generates `event_id` and `event_hash` in
   the response envelope.
3. Read-only tools (`search_blob_vectors`, `list_tools`, `code_search`) still have
   side-effects from an accountability perspective — they consume quota, they prove
   capability was exercised.

The existing 15 methods already split: `code_search`, `memory_query`, etc. are declared
`Read` (they still get event chain entries via the same `dispatch_method_call` path, but
the `SideEffect` annotation controls whether the call is eligible for caching/replay).
`invoke_tool` is `Mutation` because the tool it dispatches to may mutate — the bridge
cannot know at schema time whether the tool is read-only.

### Issue 10: DQ-4 rejected-alternative reasoning (--stdio)

**Resolution**: Fixed. The original claim "stdio is required for the process to work as a
D-Bus service" was incorrect. See corrected DQ-4 above. The real justification for keeping
`--stdio` is: (a) the `.mcp.json` `op-cognitive-mcp` stdio entry uses it, (b) it's a
useful debugging mode for local attach.

### Issue 11: Phase 1 keeps :3003/:50052 alive

**Resolution**: Explicitly acknowledged in DQ-5. "Only door" is a Phase 2 outcome. Phase 1
proves equivalence and migrates consumers. Drift-back prevention mechanisms are documented.
A tracking issue is filed when Phase 1 merges.

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
| `op-cognitive-mcp` session bus registration | NOT added — bridge uses HTTP loopback |

---

## Affected Files

| File | Change type |
|------|-------------|
| `crates/op-plugins/src/state_plugins/cognitive_mcp.rs` | Add `invoke_tool` method + structs; fix null-arg schema for `get_health`/`list_tools`/`get_config` |
| `crates/op-grpc-bridge/src/mutation_engine.rs` | Add `"cognitive_mcp"` dispatch arm (HTTP loopback) |
| `crates/op-cognitive-mcp/src/main.rs` | Remove `cognitive_mcp_bind_config()`; add deprecation docs |
| `crates/op-cognitive-mcp/src/server.rs` | Add `#[deprecated]` on `start_http_server`/`start_grpc_server`/`start_dual` |

---

## Exact Signatures and JSON Shapes

### New schema method registration (cognitive_mcp.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InvokeToolInput {
    /// Name of the tool to invoke (must exist in ToolRegistry).
    pub tool_name: String,
    /// Tool-specific arguments (opaque object passed to the tool executor).
    pub arguments: serde_json::Value,
}

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
    dispatch_cognitive_mcp_method(method, json_args).await?
}
```

Free function:
```rust
/// Dispatches cognitive_mcp schema methods to the tool registry via HTTP loopback.
///
/// Plugin-local methods (get_config, set_config, restart_service, get_health) are
/// handled without crossing to the executor. All others go via HTTP to :3003.
async fn dispatch_cognitive_mcp_method(
    method: &str,
    json_args: &str,
) -> anyhow::Result<serde_json::Value> {
    // Plugin-local methods: no HTTP hop needed
    match method {
        "get_config" | "get_health" => {
            let bytes = op_core::projection_shm::read_projection_bytes("cognitive_mcp")
                .ok_or_else(|| anyhow::anyhow!("cognitive_mcp projection not available"))?;
            let mut buf = bytes;
            let val = simd_json::to_owned_value(&mut buf)
                .map_err(|e| anyhow::anyhow!("invalid projection JSON: {e}"))?;
            return Ok(serde_json::to_value(&val)?);
        }
        "set_config" | "restart_service" => {
            // Handled by plugin's apply_state path.
            // TODO: Wire to actual sv restart / config write.
            return Ok(serde_json::json!({"acknowledged": true, "method": method}));
        }
        _ => {}
    }

    // Map schema method names to MCP tool calls
    let (tool_name, tool_args) = map_schema_method_to_tool(method, json_args)?;

    // HTTP loopback to op-cognitive-mcp's MCP endpoint
    let client = reqwest::Client::new();
    let mcp_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": tool_args
        }
    });

    let resp = client
        .post("http://10.200.0.2:3003/mcp")
        .header("Content-Type", "application/json")
        .header("X-Ghostbridge-Footprint", "bridge-dispatch")
        .header("X-Ghostbridge-Trace-ID", "bridge-internal")
        .json(&mcp_request)
        .send()
        .await
        .context("HTTP loopback to cognitive_mcp :3003")?;

    let mcp_response: serde_json::Value = resp.json().await
        .context("parsing MCP response from cognitive_mcp")?;

    // Extract result from MCP response envelope
    mcp_response
        .get("result")
        .cloned()
        .ok_or_else(|| {
            let err = mcp_response.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown MCP error");
            anyhow::anyhow!("MCP call failed: {}", err)
        })
}

/// Maps cognitive_mcp schema method names to tool registry names + translated args.
fn map_schema_method_to_tool(
    method: &str,
    json_args: &str,
) -> anyhow::Result<(String, serde_json::Value)> {
    let args: serde_json::Value = if json_args == "null" || json_args.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(json_args)?
    };

    match method {
        // Memory methods → cognitive_memory tool with operation arg
        "memory_store" => Ok(("cognitive_memory".into(), inject_op(args, "store"))),
        "memory_query" => Ok(("cognitive_memory".into(), inject_op(args, "query"))),
        "memory_retrieve" => Ok(("cognitive_memory".into(), inject_op(args, "retrieve"))),
        "memory_delete" => Ok(("cognitive_memory".into(), inject_op(args, "delete"))),
        "memory_list_namespaces" => Ok(("cognitive_memory".into(), inject_op(args, "list_namespaces"))),
        // Code methods
        "code_search" => Ok(("search_blob_vectors".into(), args)),
        "code_index" => Ok(("refresh_blob_vectors".into(), args)),
        "code_context" => Ok(("search_blob_vectors".into(), args)),
        // Gemini
        "gemini_query" => Ok(("ask_question".into(), args)),
        // Tool management
        "list_tools" => Ok(("list_tools".into(), args)),  // MCP tools/list
        "register_tool" => Ok(("register_tool".into(), args)),
        // invoke_tool: extract tool_name and arguments
        "invoke_tool" => {
            let tool_name = args.get("tool_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("invoke_tool: missing tool_name"))?
                .to_string();
            let tool_args = args.get("arguments")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            Ok((tool_name, tool_args))
        }
        other => Err(anyhow::anyhow!("unmapped cognitive_mcp method: {other}")),
    }
}

fn inject_op(mut args: serde_json::Value, operation: &str) -> serde_json::Value {
    if let Some(obj) = args.as_object_mut() {
        obj.insert("operation".to_string(), serde_json::Value::String(operation.to_string()));
    }
    args
}
```

---

## OSCAL Subid Assignments

| Item | Subid |
|------|-------|
| `invoke_tool` schema method | `mut.service.cognitive-mcp.tool.invoke@v1` |
| `InvokeToolInput` struct | `sch.software.cognitive-mcp.invoke-tool-input@v1` |
| `InvokeToolOutput` struct | `sch.software.cognitive-mcp.invoke-tool-output@v1` |
| `dispatch_cognitive_mcp_method` function | `mut.service.cognitive-mcp.dispatch@v1` |

---

## Verified System Facts (as of 2026-07-29)

These were confirmed by live inspection and inform the design:

| Claim | Status | Evidence |
|-------|--------|----------|
| Only bus name is `org.opdbus.v1.plugins` | ✅ Verified | `busctl list` shows only this + freedesktop names |
| Interface is `org.opdbus.v1.PluginV1` | ✅ Verified | `busctl introspect` on cognitive_mcp object |
| op-cognitive-mcp is runit-supervised | ✅ Verified | `/etc/runit/sv/op-cognitive-mcp/run` exists, `runsv` PID 1108 |
| No s6 env-dir exists | ✅ Verified | `/etc/runit/sv/op-cognitive-mcp/env/` does not exist |
| Port :3003 owned by op-cognitive-mcp | ✅ Verified | `ss -lntp`: PID 1195 on 10.200.0.2:3003 |
| Port :50052 owned by op-cognitive-mcp | ✅ Verified | `ss -lntp`: PID 1195 on 10.200.0.2:50052 |
| 100.69.0.254:3003 is netmaker (`3tched`) mesh ingress | ✅ Verified | PID 1250, `tcpfwd.py` → `10.200.0.1:3003` (the xray incus container, intended relay hop to `10.200.0.2`) |
| xray relays mesh traffic `10.200.0.1:<p>` → `10.200.0.2:<p>` | ✅ Verified | `/etc/xray/xray_config.json` dokodemo-door listen `10.200.0.1:{8443,8081,6334,8090}` |
| xray has no port-3003 listener, so the `:3003` chain dead-ends | ✅ Verified | probe `10.200.0.1:3003` REFUSED; xray inbounds are 8443/8081/6334/8090/8444 |
| cognitive_mcp.invoke capability is **NOT** granted | ✅ Verified post-impl | `/dev/shm/opdbus/capability-grants.json` `*` grants only `cap.software.zeroclaw.chat@v1` + `cap.software.zeroclaw.models.read@v1`; live `zcall` returns `Access denied` |
| Capability check runs AFTER arg validation | ✅ Verified | `schema_router.rs` `call()` order: method gate → `validate_json_args` → capability → dispatch. The arg defect masked the capability denial |
| Grant matching is exact string membership | ✅ Verified | `interceptor.rs:339 load_capability_grants`, exact footprint hex with `"*"` fallback |
| Tool registry has 406 tools | ✅ Verified | MCP `tools/list` returns 406 entries |
| `cognitive_memory` is single tool with op arg | ✅ Verified | Registry name is `cognitive_memory`, not `memory_store` |
| op-web-compact exposes 4 meta-tools | ✅ Verified | `list_tools`, `search_tools`, `get_tool_schema`, `execute_tool` |
| MutationEngine catch-all echoes args | ✅ Verified | `cognitive_mcp` has no match arm, falls to `_ =>` |
| `.mcp.json` `cognitive-mcp` → :3003 HTTP | ✅ Verified | Direct HTTP, NOT stdio, NOT op-web-compact |
| `~/.factory/mcp.json` has disabled stdio entry | ✅ Verified | `op-cognitive-mcp` entry with `"disabled": true` |
