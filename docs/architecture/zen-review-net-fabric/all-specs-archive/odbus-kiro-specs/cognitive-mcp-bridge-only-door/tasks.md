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

3. Fix null-arg schema for `list_tools`, `get_health`, `get_config`: change their arg
   type from `"null"` to accept either null or empty object `{}`. This fixes the current
   `InvalidArgs: {} is not of type "null"` error when calling via `zcall`.

### Acceptance criteria

```bash
cargo check -p op-plugins
cargo clippy -p op-plugins --all-targets -- -D warnings
# After rebuild + restart of op-grpc-bridge:
./bin/zcall methods cognitive_mcp | grep invoke_tool
# Expected: invoke_tool  mutation  cognitive_mcp.invoke  mut.service.cognitive-mcp.tool.invoke@v1
./bin/zcall cognitive_mcp get_health
# Expected: returns projection JSON (no longer "not of type null" error)
```

---

## Task 2 — Add cognitive_mcp dispatch arm in MutationEngine

**Crate:** `op-grpc-bridge`
**File:** `crates/op-grpc-bridge/src/mutation_engine.rs`

### What to add

1. A new match arm in `dispatch_method_call` (before the `_ =>` catch-all at ~line 940):

```rust
"cognitive_mcp" => {
    dispatch_cognitive_mcp_method(method, json_args).await?
}
```

2. The `dispatch_cognitive_mcp_method` function and `map_schema_method_to_tool` helper
   as specified in design.md "Exact Signatures" section. This uses HTTP loopback to
   `http://10.200.0.2:3003/mcp` for tool-registry methods, and reads projections
   in-process for `get_config`/`get_health`.

3. Add `reqwest` dependency to `op-grpc-bridge/Cargo.toml` if not already present
   (check first — it may already be there for other HTTP calls).

### Key design points

- NO new D-Bus bus name is created.
- NO session bus registration in `op-cognitive-mcp`.
- The dispatch uses HTTP loopback to the existing `:3003` MCP endpoint.
- Memory methods (`memory_store`, `memory_query`, etc.) translate to `cognitive_memory`
  tool with an `operation` field injected into args.
- `invoke_tool` extracts `tool_name` and `arguments` and calls `tools/call` directly.
- Interface for all calls remains `org.opdbus.v1.PluginV1`.

### Acceptance criteria

```bash
cargo check -p op-grpc-bridge
cargo clippy -p op-grpc-bridge --all-targets -- -D warnings
# After rebuild + restart:
./bin/zcall cognitive_mcp invoke_tool -a '{"tool_name":"cognitive_memory","arguments":{"operation":"list_namespaces"}}'
# Expected: JSON result with namespace list (not an echo of the input args)
./bin/zcall cognitive_mcp invoke_tool -a '{"tool_name":"get_health","arguments":{}}'
# Expected: health status JSON from the tool registry
```

---

## Task 3 — Remove projection-as-bind-directive from main.rs

**Crate:** `op-cognitive-mcp`
**File:** `crates/op-cognitive-mcp/src/main.rs`

### What to change

1. **Delete** the `cognitive_mcp_bind_config()` function (lines 90-105) and its doc comment.

2. **Replace** the `let bind_config = cognitive_mcp_bind_config(&cli);` call with
   direct reads from the `Cli` struct:

```rust
let http_enabled = !cli.no_http;
let grpc_enabled = !cli.no_grpc;
let wg_interface = &cli.wg_interface;
```

3. Update subsequent code that reads `bind_config.*` to read from `cli.*` / local vars.

4. Remove unused imports (`CognitiveMcpConfig`, `read_projection_bytes`, etc.).

### Acceptance criteria

```bash
cargo check -p op-cognitive-mcp
cargo clippy -p op-cognitive-mcp --all-targets -- -D warnings
# Verify: start with explicit --no-http --no-grpc --stdio
# The process should NOT open any TCP listeners regardless of what the projection says.
```

---

## Task 4 — Deprecate direct listeners in documentation

**Crate:** `op-cognitive-mcp`
**Files:** `crates/op-cognitive-mcp/src/main.rs`, `crates/op-cognitive-mcp/src/server.rs`

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

## Task 5 — Prove equivalence: bridge path vs direct HTTP path

**Crate:** (no code changes — verification script)
**File:** `bin/verify-bridge-equivalence.sh` (new script, NOT under crates/)

### What to create

A shell script that:

1. Calls a sample of tools via the bridge path:
   ```bash
   ./bin/zcall cognitive_mcp invoke_tool -a '{"tool_name":"cognitive_memory","arguments":{"operation":"list_namespaces"}}'
   ```

2. Calls the same tools via the old HTTP path:
   ```bash
   curl -s -H "X-Ghostbridge-Footprint: verify" -H "X-Ghostbridge-Trace-ID: verify" \
     -H "Content-Type: application/json" \
     http://10.200.0.2:3003/mcp \
     -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"cognitive_memory","arguments":{"operation":"list_namespaces"}}}'
   ```

3. Compares the JSON shape (keys present, types) between bridge and direct responses.

4. Reports pass/fail per tool. Expected: structure matches (values may differ for
   time-dependent results).

### Acceptance criteria

```bash
chmod +x bin/verify-bridge-equivalence.sh
./bin/verify-bridge-equivalence.sh
# Expected: all tested tools report PASS or SKIP. Zero FAIL results.
```

---

## Task 6 — Migrate MCP client configs

**Files:** `.mcp.json`, `~/.factory/mcp.json`

### What to change

1. In `.mcp.json`: Remove `cognitive-mcp` entry (HTTP to :3003). Clients should use
   `op-dbus-compact` (HTTP :8080) or `op-web-compact` (stdio) instead.

2. In `~/.factory/mcp.json`: Remove `cognitive-mcp` entry (HTTP to :3003). Remove
   `op-cognitive-mcp` disabled stdio entry (redundant with bridge path).

3. Add a comment or README note explaining the migration path.

### Acceptance criteria

```bash
# Verify cognitive-mcp entry removed from both files:
! grep -q '"cognitive-mcp"' .mcp.json
! grep -q '"cognitive-mcp"' ~/.factory/mcp.json
# Verify op-dbus-compact still works:
curl -s http://127.0.0.1:8080/mcp/compact -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | jq '.result.tools | length'
# Expected: 4 (list_tools, search_tools, get_tool_schema, execute_tool)
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
./bin/zcall cognitive_mcp invoke_tool -a '{"tool_name":"cognitive_memory","arguments":{"operation":"list_namespaces"}}'

# 5. End-to-end: existing method via bridge (no longer echoes args)
./bin/zcall cognitive_mcp get_health

# 6. Verify event chain records the call
./bin/zcall cognitive_mcp invoke_tool -a '{"tool_name":"cognitive_memory","arguments":{"operation":"list_namespaces"}}' \
  | jq -e '.success == true and .event_id > 0 and (.event_hash | length) > 0'

# 7. Run equivalence verification
./bin/verify-bridge-equivalence.sh
```

### Acceptance criteria

- `cargo build --workspace` exits 0.
- `cargo clippy` produces no new `-D warnings` failures in the three affected crates
  (deprecation warnings on `start_http_server` etc. are expected and intentional).
- `./bin/zcall cognitive_mcp invoke_tool ...` returns a valid JSON envelope with
  `success: true`, a non-null `result`, a non-zero `event_id`, and a non-empty `event_hash`.
- `./bin/zcall cognitive_mcp get_health` returns projection data (not an echo, not an error).
- The projection at `/dev/shm/opdbus/projections/cognitive_mcp.json` still exists and is
  valid JSON.
- The equivalence script reports zero FAIL results.

---

## Summary Table

| Task | Crate(s) | File(s) | Type |
|------|----------|---------|------|
| 1 — Add `invoke_tool` to schema | op-plugins | cognitive_mcp.rs | Add method + structs + fix null args |
| 2 — Dispatch arm in MutationEngine | op-grpc-bridge | mutation_engine.rs | Add match arm + HTTP loopback |
| 3 — Remove projection-as-bind-directive | op-cognitive-mcp | main.rs | Delete + simplify |
| 4 — Deprecate direct listeners | op-cognitive-mcp | main.rs, server.rs | Doc + attributes |
| 5 — Prove equivalence script | (none) | bin/verify-bridge-equivalence.sh | New script |
| 6 — Migrate MCP client configs | (none) | .mcp.json, ~/.factory/mcp.json | Config change |
| 7 — Full build + smoke-test | all three | — | Verify |

---

## Phase 2 (deferred — separate spec)

After Phase 1 is stable and all consumers are migrated. Full spec:
`../cognitive-mcp-only-door-phase2/`.

1. Bridge constructs `CognitiveMcpServer` and owns the `ToolRegistry` in-process
   (no crate extraction needed — `ToolRegistry` is already in the shared `op-mcp` crate).
2. Replace HTTP loopback dispatch with in-process `ToolRegistry::execute()` calls.
3. Delete `start_http_server`, `start_grpc_server`, `start_dual` from `server.rs`.
4. Delete `--no-http`, `--no-grpc` CLI flags from `main.rs`.
5. Delete `interceptor.rs` (Ghostbridge for direct listeners).
6. Relocate `context_server.rs` (SSE routes) to op-web.
7. Move `CognitiveToolService` to the bridge's `:50051` and re-point the Xray
   `mcp.internal` route (it currently targets `10.200.0.2:50052`).
8. Delete the `cognitive-mcp` HTTP entries from any remaining MCP config files.
9. Delete the `fwd-3003` and `fwd-28082` forwarder service dirs.

**Phase 2 gate**: Not the `op-mcp-registry` crate — that is unnecessary. The real
constraint is the single-writer CozoDB: `op-cognitive-mcp` must be stopped before the
bridge takes registry ownership. Additionally, `op-web:8080` is currently an
**unauthenticated** MCP ingress, which must be fixed before it becomes the sole door.
Netmaker mesh clients (`100.69.0.0/16` via `3tched`) migrate to
`http://100.69.0.254:8080/mcp/compact`, which already works without a forwarder.
