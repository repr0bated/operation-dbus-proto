# Tasks: cognitive-mcp-bridge-only-door

Each task is independently verifiable. Complete them in order — each step's output is the
next step's input. No implementation code is written in this spec; tasks reference the
design decisions from `design.md`.

---

## Task 1 — Add `invoke_tool` method to cognitive_mcp schema

**Crate:** `op-plugins`
**File:** `crates/op-plugins/src/state_plugins/cognitive_mcp.rs`

### What to add

1. Two new structs with schemars derives:

```rust
/// OSCAL subid: sch.software.cognitive-mcp.invoke-tool-input@v1
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InvokeToolInput {
    /// Name of the tool to invoke (must exist in the cognitive_mcp ToolRegistry).
    pub tool_name: String,
    /// Tool-specific arguments (opaque object passed verbatim to the tool executor).
    pub arguments: serde_json::Value,
}

/// OSCAL subid: sch.software.cognitive-mcp.invoke-tool-output@v1
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InvokeToolOutput {
    pub success: bool,
    pub tool_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
```

2. One new `methods.insert` in `cognitive_mcp_schema()` (after the existing 15):

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

3. Update the handler audit comment at line 910 to include:
```
// invoke_tool           → op-grpc-bridge/mutation_engine.rs → D-Bus → executor CallTool
```

### Acceptance criteria

```bash
cargo check -p op-plugins
cargo clippy -p op-plugins --all-targets -- -D warnings
# After rebuild + restart of op-grpc-bridge:
./bin/zcall methods cognitive_mcp | grep invoke_tool
# Expected: invoke_tool  mutation  cognitive_mcp.invoke  mut.service.cognitive-mcp.tool.invoke@v1
```

---

## Task 2 — Register CognitiveMcpInterface on the session bus

**Crate:** `op-cognitive-mcp`
**Files:** `crates/op-cognitive-mcp/src/main.rs`, `crates/op-cognitive-mcp/Cargo.toml` (if
zbus is not already a dependency — verify first)

### What to change

1. After `CognitiveMcpServer::new()` returns (line ~175 in `main.rs`), add session bus
   registration:

```rust
// Register the tool executor interface on the session bus so op-grpc-bridge's
// MutationEngine can dispatch invoke_tool calls to our ToolRegistry.
use crate::dbus_interface::CognitiveMcpInterface;

let executor_iface = CognitiveMcpInterface::new(server.tool_registry());
let dbus_conn = zbus::ConnectionBuilder::address(
    &std::env::var("DBUS_SESSION_BUS_ADDRESS")
        .unwrap_or_else(|_| "unix:path=/run/opdbus/session-bus.sock".to_string()),
)?
.name("org.opdbus.v1.executors.cognitive_mcp")?
.serve_at("/executor", executor_iface)?
.build()
.await?;

info!("Registered CognitiveMcpInterface on session bus as org.opdbus.v1.executors.cognitive_mcp");
```

2. This registration happens BEFORE the `if cli.stdio { ... }` branch and BEFORE any
   listener bind. The D-Bus registration is always active regardless of transport flags.

3. Hold the `dbus_conn` alive for the process lifetime (store it in a variable that lives
   until `main()` returns, or move it into an `Arc` on the server struct).

### Acceptance criteria

```bash
cargo check -p op-cognitive-mcp
# After rebuild + restart:
busctl --address=unix:path=/run/opdbus/session-bus.sock introspect \
  org.opdbus.v1.executors.cognitive_mcp /executor
# Expected: shows CognitiveMcp interface with ListTools, GetToolSchema, CallTool methods
```

---

## Task 3 — Add cognitive_mcp dispatch arm in MutationEngine

**Crate:** `op-grpc-bridge`
**File:** `crates/op-grpc-bridge/src/mutation_engine.rs`

### What to add

1. A new match arm in `dispatch_method_call` (before the `_ =>` catch-all at line ~1010):

```rust
"cognitive_mcp" => {
    let args = serde_json::to_value(&parsed_value)?;
    dispatch_cognitive_mcp_method(method, &args).await?
}
```

2. A new free function `dispatch_cognitive_mcp_method`:

```rust
/// OSCAL subid: mut.service.cognitive-mcp.dispatch@v1
async fn dispatch_cognitive_mcp_method(
    method: &str,
    args: &serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    use anyhow::Context;

    // Plugin-local methods handled without crossing to the executor:
    match method {
        "get_config" => {
            return op_plugins::state_plugins::cognitive_mcp::CognitiveMcpPlugin::current_config_json();
        }
        "set_config" | "restart_service" => {
            // These are handled by the plugin's apply_state path.
            // For now, return the args as acknowledgment (same as current catch-all).
            // Full apply_state wiring is a follow-up once the executor path is proven.
            return Ok(args.clone());
        }
        _ => {}
    }

    // All other methods (memory_*, code_*, gemini_query, list_tools, invoke_tool):
    // Forward to the cognitive_mcp executor via D-Bus.
    let tool_name = match method {
        "invoke_tool" => args
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("invoke_tool: missing tool_name field"))?
            .to_string(),
        other => other.to_string(),
    };

    let tool_args = match method {
        "invoke_tool" => args
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
        _ => args.clone(),
    };

    let tool_args_str = serde_json::to_string(&tool_args)?;

    let conn = zbus::Connection::from_str(
        &std::env::var("DBUS_SESSION_BUS_ADDRESS")
            .unwrap_or_else(|_| "unix:path=/run/opdbus/session-bus.sock".to_string()),
    )
    .await
    .context("connecting to session bus for cognitive_mcp dispatch")?;

    let reply: zbus::Message = conn
        .call_method(
            Some("org.opdbus.v1.executors.cognitive_mcp"),
            "/executor",
            Some("org.opdbus.v1.plugins.CognitiveMcp"),
            "CallTool",
            &(tool_name.as_str(), tool_args_str.as_str()),
        )
        .await
        .context("D-Bus CallTool to cognitive_mcp executor")?;

    let result_json: String = reply.body().deserialize()?;
    let result: serde_json::Value = serde_json::from_str(&result_json)
        .context("executor returned invalid JSON")?;

    Ok(result)
}
```

3. The `zbus::Connection` should be cached on the `MutationEngine` struct to avoid
   reconnecting on every call. If that requires larger refactoring, a `tokio::sync::OnceCell`
   at module level is acceptable for Phase 1.

### Acceptance criteria

```bash
cargo check -p op-grpc-bridge
cargo clippy -p op-grpc-bridge --all-targets -- -D warnings
# After rebuild + restart (with op-cognitive-mcp also running):
./bin/zcall call cognitive_mcp invoke_tool '{"tool_name":"list_tools","arguments":{}}'
# Expected: JSON array of tool definitions (not an echo of the input args)
./bin/zcall call cognitive_mcp memory_query '{"namespace":"project:op-dbus","key_pattern":"*","limit":1}'
# Expected: actual memory entries (not an echo)
```

---

## Task 4 — Remove projection-as-bind-directive from main.rs

**Crate:** `op-cognitive-mcp`
**File:** `crates/op-cognitive-mcp/src/main.rs`

### What to change

1. **Delete** the `cognitive_mcp_bind_config()` function (lines 90-105) and its doc comment.

2. **Replace** the `let bind_config = cognitive_mcp_bind_config(&cli);` call (line ~129) with
   direct reads from the `Cli` struct:

```rust
let http_enabled = !cli.no_http;
let grpc_enabled = !cli.no_grpc;
let wg_interface = &cli.wg_interface;
```

3. Update subsequent code that reads `bind_config.http`, `bind_config.grpc`,
   `bind_config.wg_interface`, `bind_config.http_enabled`, `bind_config.grpc_enabled` to read
   from `cli.http`, `cli.grpc`, `cli.wg_interface`, `http_enabled`, `grpc_enabled` directly.

4. Remove the `use op_plugins::state_plugins::cognitive_mcp::CognitiveMcpConfig;` import if
   it becomes unused.

5. Remove the `use op_core::projection_shm::read_projection_bytes;` import if unused.

### Acceptance criteria

```bash
cargo check -p op-cognitive-mcp
cargo clippy -p op-cognitive-mcp --all-targets -- -D warnings
# Verify: start with explicit --no-http --no-grpc --stdio
# The process should NOT open any TCP listeners regardless of what the projection says.
# (Previously the projection's http_enabled:true would override --no-http.)
```

---

## Task 5 — Prove equivalence: bridge path vs direct HTTP path

**Crate:** (no code changes — verification script)
**File:** `bin/verify-bridge-equivalence.sh` (new script, NOT under crates/)

### What to create

A shell script that:

1. Reads the tool list from the bridge path:
   ```bash
   BRIDGE_TOOLS=$(./bin/zcall call cognitive_mcp list_tools '{}')
   ```

2. For each tool name in the list, invokes via the bridge:
   ```bash
   ./bin/zcall call cognitive_mcp invoke_tool "{\"tool_name\":\"$TOOL\",\"arguments\":{}}"
   ```

3. Compares the JSON shape (keys present, types) against the same call via the old HTTP path:
   ```bash
   curl -s -H "X-Ghostbridge-Footprint: verify" -H "X-Ghostbridge-Trace-ID: verify" \
     http://10.200.0.2:3003/mcp -d '{"method":"tools/call","params":{"name":"$TOOL","arguments":{}}}'
   ```

4. Reports pass/fail per tool. Expected: all tools return equivalent shapes (values may differ
   for time-dependent tools but structure must match).

### Acceptance criteria

```bash
chmod +x bin/verify-bridge-equivalence.sh
./bin/verify-bridge-equivalence.sh
# Expected: all tools report PASS or SKIP (for tools that require specific state).
# Zero FAIL results.
```

---

## Task 6 — Deprecate direct listeners in documentation

**Crate:** `op-cognitive-mcp`
**File:** `crates/op-cognitive-mcp/src/main.rs` (doc comments only)

### What to change

1. Add a deprecation notice to the module doc comment at the top of `main.rs`:

```rust
//! ## DEPRECATED TRANSPORTS
//!
//! The HTTP/SSE (:3003) and gRPC (:50052) listeners are deprecated as of this
//! spec. All MCP clients should use the bridge path:
//!   org.opdbus.v1.PluginV1.Call on /org/opdbus/v1/plugins/cognitive_mcp
//!
//! The `--no-http` and `--no-grpc` flags will be removed in a future release.
//! The `--stdio` flag remains for debugging/local-attach only.
```

2. Add `#[deprecated(note = "Use bridge path: PluginV1.Call on cognitive_mcp plugin")]`
   attributes to `start_http_server`, `start_grpc_server`, and `start_dual` in `server.rs`.

### Acceptance criteria

```bash
cargo check -p op-cognitive-mcp
# Deprecation warnings appear for start_http_server/start_grpc_server/start_dual
# (this is intentional — the warnings remind that these need removal in Phase 2)
```

---

## Task 7 — Full workspace build and integration smoke-test

**Verify the complete change set compiles and the bridge path works end-to-end.**

```bash
# 1. Full workspace build
cargo build --workspace

# 2. Clippy on affected crates
cargo clippy -p op-plugins -p op-grpc-bridge -p op-cognitive-mcp --all-targets -- -D warnings

# 3. Verify new method appears in live schema
./bin/zcall methods cognitive_mcp | grep invoke_tool

# 4. End-to-end: invoke a tool via the bridge
./bin/zcall call cognitive_mcp invoke_tool '{"tool_name":"memory_list_namespaces","arguments":{}}'

# 5. End-to-end: existing method via bridge (no longer echoes args)
./bin/zcall call cognitive_mcp get_health '{}'

# 6. Verify event chain records the call
./bin/zcall events --last 1 | grep invoke_tool

# 7. Verify executor bus name is registered
busctl --address=unix:path=/run/opdbus/session-bus.sock list | grep executors
```

### Acceptance criteria

- `cargo build --workspace` exits 0.
- `cargo clippy` produces no new `-D warnings` failures in the three affected crates
  (deprecation warnings on `start_http_server` etc. are expected and intentional).
- `./bin/zcall call cognitive_mcp invoke_tool ...` returns a valid JSON envelope with
  `success: true` and a non-null `result`.
- `./bin/zcall call cognitive_mcp memory_query ...` returns actual data (not an echo).
- The executor bus name `org.opdbus.v1.executors.cognitive_mcp` appears in `busctl list`.
- The projection at `/dev/shm/opdbus/projections/cognitive_mcp.json` still exists and is
  valid JSON (it is published state, untouched by this change).

---

## Summary Table

| Task | Crate(s) | File(s) | Type |
|------|----------|---------|------|
| 1 — Add `invoke_tool` to schema | op-plugins | cognitive_mcp.rs | Add method + structs |
| 2 — Register executor on session bus | op-cognitive-mcp | main.rs | Wire D-Bus registration |
| 3 — Dispatch arm in MutationEngine | op-grpc-bridge | mutation_engine.rs | Add match arm + function |
| 4 — Remove projection-as-bind-directive | op-cognitive-mcp | main.rs | Delete + simplify |
| 5 — Prove equivalence script | (none) | bin/verify-bridge-equivalence.sh | New script |
| 6 — Deprecate direct listeners | op-cognitive-mcp | main.rs, server.rs | Doc + attributes |
| 7 — Full build + smoke-test | all three | — | Verify |

---

## Phase 2 (deferred — separate spec)

After Phase 1 is stable and all consumers are migrated:

1. Delete `start_http_server`, `start_grpc_server`, `start_dual` from `server.rs`.
2. Delete `--no-http`, `--no-grpc` CLI flags from `main.rs`.
3. Delete `interceptor.rs` (Ghostbridge for direct listeners).
4. Delete `context_server.rs` (SSE routes — must be relocated to op-web first).
5. Remove `:3003`/`:50052` from firewall rules and WireGuard allowed-IPs.
6. Delete the `cognitive-mcp` HTTP entries from all MCP config files.
7. Optionally: remove the `CognitiveGrpcService` if NotebookLM bridge is migrated to its own
   plugin dispatch.
