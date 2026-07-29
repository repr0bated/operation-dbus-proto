# Tasks: cognitive-mcp-only-door-phase2

## Prerequisites

- Phase 1 (`.kiro/specs/cognitive-mcp-bridge-only-door/`) is fully implemented
  and merged.
- `bin/verify-bridge-equivalence.sh` has run successfully and its output is
  captured as the before/after oracle baseline.
- Phase 2 cannot merge until both conditions above are met.

Tasks are ordered so that the irreversible step (listener deletion, Task 6) comes
last. Each task has an explicit rollback.

**Tasks 0 and 0b were added after a live audit (2026-07-29) found two blockers the
original task list did not cover. Both gate Task 6.**

---

## Task 0 — Authenticate the MCP ingress (PREREQUISITE, gates Task 6)

**Crate:** `op-web`
**Files:** `crates/op-web/src/routes/mod.rs`, `crates/op-web/src/middleware/security.rs`
**Requirement:** FR-2a

### Why

This spec makes `:8080/mcp/compact` the sole MCP ingress for mesh clients, and it
is currently **unauthenticated**. Verified:

- `routes/mod.rs:324` applies `security::ip_security_middleware` globally, but that
  middleware only resolves an `AccessZone`, inserts it as a request extension, and
  calls `next.run(request)` **unconditionally** — it denies nothing
  (`middleware/security.rs:202-222`).
- `AccessZone` is read only by `groups_admin.rs` and `handlers/pair.rs`; the MCP
  handlers never read it.
- Confirmed live: `curl http://100.69.0.254:8080/mcp/compact` with no auth and no
  Ghostbridge headers returned the full tool list.
- `op-web` binds `0.0.0.0`, but nftables restricts the port to the mesh
  (`ip saddr 100.69.0.0/16 ... dport { ... 8080 ... } accept`), so it is **not**
  internet-exposed.

`execute_tool` reaches `agent_shell_executor_exec` and
`agent_python_executor_run`. Precisely stated: any peer on the netmaker mesh can
execute shell commands without a credential, and no per-client identity is
recorded at the ingress. Lateral movement and attribution, not a perimeter hole.
The bridge capability gate does not substitute: `cognitive_mcp.invoke` is a single
blanket grant.

### What to add

An enforcing middleware on the `/mcp` nest (not global, to avoid changing the
posture of unrelated routes) that rejects a request unless it either resolves to an
allowed `AccessZone` (netmaker/loopback) or presents a valid Ghostbridge footprint
header, matching what `:8090` enforces
(`op-grpc-bridge/src/interceptor.rs:98`).

### Acceptance criteria

```bash
cargo check -p op-web
cargo clippy -p op-web --all-targets -- -D warnings

# Unauthenticated request from a non-allowed zone is rejected:
curl -s -o /dev/null -w '%{http_code}\n' http://<public-ip>:8080/mcp/compact \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
# Expected: 401 or 403   (currently returns 200)

# Netmaker-zone request still succeeds:
curl -s http://100.69.0.254:8080/mcp/compact -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | jq -e '.result.tools | length == 4'
```

### Rollback

Revert the middleware layer. No data or service change.

---

## Task 0b — Relocate CognitiveToolService (PREREQUISITE, gates Task 6)

**Crates:** `op-grpc-bridge`, `op-identity`
**Requirement:** FR-2b

### Why

`:50052` hosts **two** services, not one
(`crates/op-cognitive-mcp/src/server.rs:287-298`):
`CognitiveToolServiceServer` (wrapped in `ghostbridge_interceptor`) **and**
`waypipe`. Task 4 / FR-5 relocates only `waypipe` into `op-waypipe-grpc`, which
would strand `CognitiveToolService`.

It has a live consumer: Xray routes `mcp.internal` to `10.200.0.2:50052` targeting
`operation.cognitive.v1.CognitiveToolService`
(`crates/op-identity/src/schema_bridge.rs:1454-1458`). Task 6 also deletes
`interceptor.rs`, which is its enforcement wrapper.

### What to change

Per FR-2b option (a) — the preferred resolution:

1. Mount `CognitiveGrpcService` on the bridge's existing tonic server that binds
   `:50051` (`op-grpc-bridge/src/server.rs:46`), behind the bridge's Ghostbridge
   interceptor. Register the cognitive file descriptor set for reflection.
2. Keep the gRPC service name, package, and method signatures identical so
   `mcp.internal` clients need only a new address.
3. Re-point the Xray route in `crates/op-identity/src/schema_bridge.rs:1454-1458`
   from `10.200.0.2:50052` to the bridge's `:50051`, leaving `service_name`
   unchanged.

If option (b) is chosen instead (both services hosted by `op-waypipe-grpc` on
`:50052`), update Task 4's scope and the binary's name, and re-implement the
ghostbridge interceptor there — do not silently drop enforcement.

### Acceptance criteria

```bash
cargo build -p op-grpc-bridge -p op-identity
cargo clippy -p op-grpc-bridge -p op-identity --all-targets -- -D warnings

# Service reachable at its new location:
grpcurl -plaintext 127.0.0.1:50051 list \
  | grep operation.cognitive.v1.CognitiveToolService

# A NotebookLM RPC answers there:
grpcurl -plaintext -d '{}' 127.0.0.1:50051 \
  operation.cognitive.v1.CognitiveToolService/ListNotebooks

# Xray route no longer targets the old port (option (a)):
grep -n '50052' crates/op-identity/src/schema_bridge.rs   # expect: no match
```

If `grpcurl` is unavailable, install it or use an equivalent reflection client —
do not skip this verification.

### Rollback

`git revert` both changes; the Xray route returns to `10.200.0.2:50052`, which is
still served until Task 6.

---

## Task 1 — Construct CognitiveMcpServer in-process in op-grpc-bridge

**Crate:** `op-grpc-bridge`
**File:** `crates/op-grpc-bridge/src/grpc_server.rs`

### What to add

1. Adjacent to the existing `QdrantSemanticShuttle` construction at `:758`,
   add `CognitiveMcpServer::new()`:

```rust
use op_cognitive_mcp::server::CognitiveMcpServer;
use op_mcp::tool_registry::ToolRegistry;

let db_path = std::env::var("COGNITIVE_MCP_DB_PATH")
    .unwrap_or_else(|_| "/var/lib/op-cognitive-mcp/memory.db".into());

let (cognitive_tool_registry, cognitive_context_engine) = match CognitiveMcpServer::new(&db_path).await {
    Ok(server) => {
        let reg = server.tool_registry();
        let ctx = server.context_engine();
        let mem = server.memory_store();
        let sess = server.session_manager();
        tracing::info!(tools = reg.count().await, "Cognitive tool registry loaded in-process");
        (Some(reg), Some((ctx, mem, sess)))
    }
    Err(e) => {
        tracing::error!(error = %e, "CognitiveMcpServer init failed; cognitive_mcp tools unavailable but bridge continues");
        (None, None)
    }
};
```

2. Store `cognitive_tool_registry: Option<Arc<ToolRegistry>>` in `MutationEngine`
   (or its enclosing server struct). Pass it through the constructor.

3. Store the context engine tuple for Task 3 (context-awareness routes).

### Verification

```bash
cargo check -p op-grpc-bridge
cargo clippy -p op-grpc-bridge --all-targets -- -D warnings
```

### Rollback

Remove the `CognitiveMcpServer::new()` block and the stored field. Revert to
Phase 1 HTTP loopback (which still works as long as `:3003` is alive).

---

## Task 2 — Replace HTTP loopback with in-process ToolRegistry::execute

**Crate:** `op-grpc-bridge`
**File:** `crates/op-grpc-bridge/src/mutation_engine.rs`

### What to change

1. Replace Phase 1's `dispatch_cognitive_mcp_method` function body. Instead of
   HTTP POST to `http://10.200.0.2:3003/mcp`, call:

```rust
async fn dispatch_cognitive_mcp_method(
    tool_registry: &Option<Arc<ToolRegistry>>,
    method: &str,
    json_args: &str,
) -> anyhow::Result<serde_json::Value> {
    // Plugin-local methods: unchanged from Phase 1
    match method {
        "get_config" | "get_health" => {
            let bytes = op_core::projection_shm::read_projection_bytes("cognitive_mcp")
                .ok_or_else(|| anyhow::anyhow!("cognitive_mcp projection not available"))?;
            let val: serde_json::Value = serde_json::from_slice(&bytes)?;
            return Ok(val);
        }
        "set_config" | "restart_service" => {
            return Ok(serde_json::json!({"acknowledged": true, "method": method}));
        }
        _ => {}
    }

    let registry = tool_registry.as_ref()
        .ok_or_else(|| anyhow::anyhow!("cognitive_mcp tool registry unavailable (init failed)"))?;

    let (tool_name, tool_args) = map_schema_method_to_tool(method, json_args)?;

    // Execute with timeout on a spawned task to avoid blocking D-Bus loop
    let reg = registry.clone();
    let name = tool_name.clone();
    let timeout = Duration::from_secs(
        std::env::var("COGNITIVE_TOOL_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30)
    );

    let result = tokio::time::timeout(timeout, async move {
        if name == "list_tools" {
            // Special case: list returns definitions, not a tool execution
            let defs = reg.list(0, usize::MAX, None).await;
            Ok(serde_json::to_value(defs)?)
        } else {
            reg.execute(&name, tool_args).await
                .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
        }
    }).await
    .map_err(|_| anyhow::anyhow!("tool execution timed out after {}s: {}", timeout.as_secs(), tool_name))?
    .map_err(|e| anyhow::anyhow!("tool execution failed: {}", e))?;

    Ok(result)
}
```

2. The `map_schema_method_to_tool` helper and `inject_op` remain unchanged from
   Phase 1.

3. Update the match arm to pass the registry reference:
```rust
"cognitive_mcp" => {
    dispatch_cognitive_mcp_method(&self.cognitive_tool_registry, method, json_args).await?
}
```

4. Remove the `reqwest` HTTP client code (the HTTP loopback). If `reqwest` was
   added to `op-grpc-bridge/Cargo.toml` solely for this dispatch, remove it.
   (Check: `reqwest` is NOT in `op-grpc-bridge/Cargo.toml` as a direct dep —
   verified. Phase 1 may have added it; if so, remove.)

### Verification

```bash
cargo check -p op-grpc-bridge
cargo clippy -p op-grpc-bridge --all-targets -- -D warnings
# After rebuild + restart:
./bin/zcall cognitive_mcp invoke_tool -a '{"tool_name":"cognitive_memory","arguments":{"operation":"list_namespaces"}}'
# Must return valid result WITHOUT op-cognitive-mcp running on :3003
# (Stop the service: sudo sv stop op-cognitive-mcp; re-test; then restart)
```

### Rollback

Revert to Phase 1's HTTP loopback implementation. Restart `op-cognitive-mcp`
with listeners enabled.

---

## Task 3 — Host context-awareness SSE routes in op-web

**Crate:** `op-web`
**File:** `crates/op-web/src/routes/mod.rs` (or equivalent route-building file)

### What to add

1. Import and use the context router from the cognitive server instance:

```rust
use op_cognitive_mcp::context_server::build_context_router;
```

2. In the route tree construction, nest the context router:

```rust
if let Some((context_engine, memory_store, session_manager)) = cognitive_context_state {
    let context_router = build_context_router(context_engine, memory_store, session_manager);
    app = app.nest("/cognitive/context", context_router);
}
```

3. Remove Phase 1's proxy route at `/cognitive/context/` that forwarded to
   `:3003` (if it exists at this point).

### Verification

```bash
cargo check -p op-web
cargo clippy -p op-web --all-targets -- -D warnings
# After rebuild + restart:
curl -s http://127.0.0.1:8080/cognitive/context/health
# Expected: 200 OK with health JSON
```

### Rollback

Remove the nest. The context routes become unavailable (no proxy fallback since
`:3003` is being killed — this task must succeed before Task 6).

---

## Task 4 — Create op-waypipe-grpc runit service

**File:** `/etc/runit/sv/op-waypipe-grpc/run` (new)
**File:** `/etc/runit/sv/op-waypipe-grpc/log/run` (new)

### What to create

1. Service run script:
```sh
#!/bin/sh
exec 2>&1
# Standalone WaypipeTunnel gRPC service (relocated from op-cognitive-mcp :50052).
# Listens on svc0 interface for laptop tunnel clients.
exec /usr/local/bin/op-waypipe-grpc serve --listen 10.200.0.2:50052
```

2. Log run script:
```sh
#!/bin/sh
exec svlogd -tt /var/log/op-waypipe-grpc
```

3. Create log directory: `mkdir -p /var/log/op-waypipe-grpc`

4. Enable and start: `ln -s /etc/runit/sv/op-waypipe-grpc /var/service/op-waypipe-grpc`

**Important**: Start this service BEFORE Task 6 (listener deletion). Verify it
binds `:50052` successfully while `op-cognitive-mcp` still owns that port — this
will fail until Task 6 frees the port. Therefore:
- Create the service definition now (Task 4).
- Do NOT link to `/var/service/` until Task 6 stops the old listener.
- Task 6 includes the activation step.

### Verification

```bash
# After Task 6 completes and the service is activated:
sudo sv status op-waypipe-grpc
# Expected: running
ss -lntp sport = :50052
# Expected: op-waypipe-grpc PID, NOT op-cognitive-mcp PID
```

### Rollback

Remove the runit service directory. WaypipeTunnel becomes unavailable until
`op-cognitive-mcp` is restarted with its gRPC listener (reverting Task 6).

---

## Task 5 — Remove schema fields and reseal blob

**Crate:** `op-plugins`
**File:** `crates/op-plugins/src/state_plugins/cognitive_mcp.rs`

### What to change

1. Remove from `CognitiveMcpConfig` struct:
   - `pub http: String` (`:46`)
   - `pub grpc: String` (`:48`)
   - `pub http_enabled: bool` (`:52`)
   - `pub grpc_enabled: bool` (`:54`)

2. Remove helper functions: `default_http()`, `default_grpc()`.

3. Remove constants: `DEFAULT_HTTP`, `DEFAULT_GRPC` (if they exist and are
   unused after removal).

4. Update `Default for CognitiveMcpConfig` impl (`:72-85`): remove the four fields.

5. Update `CognitiveMcpPlugin::current_config()` (`:110-116`): remove the four
   fields and the `read_env("COGNITIVE_MCP_HTTP_DISABLED")` /
   `read_env("COGNITIVE_MCP_GRPC_DISABLED")` calls.

6. Remove `apply_state` diff arms for `http_enabled` (`:271`) and
   `grpc_enabled` (`:283`).

7. Remove `GetConfigOutput` fields `http`, `grpc`, `http_enabled`, `grpc_enabled`
   (`:744-745`).

8. After code changes compile, reseal: `op-blob seal cognitive_mcp`.

### Verification

```bash
cargo check -p op-plugins
cargo clippy -p op-plugins --all-targets -- -D warnings
# After blob reseal + bridge restart:
./bin/zcall cognitive_mcp get_config
# Expected: JSON with wg_interface and dbus_enabled only (no http/grpc fields)
cat /dev/shm/opdbus/projections/cognitive_mcp.json \
  | jq -e 'has("http_enabled") == false and has("grpc_enabled") == false' \
  && echo "OK: schema fields removed from projection"
```

### Rollback

Restore the four fields to `CognitiveMcpConfig`. Re-seal. The projection returns
to its previous shape.

---

## Task 6 — Delete listeners and dead code (IRREVERSIBLE)

**Crate:** `op-cognitive-mcp`
**Files:** `src/main.rs`, `src/server.rs`, `src/lib.rs`, `src/interceptor.rs`
(delete), `src/dbus_interface.rs` (delete), `src/client_config.rs` (delete),
`src/grpc_service.rs` (delete), `examples/external_client.rs` (delete)

⚠️ **This task is irreversible in production.** Once listeners are removed and
the service restarted, any client still pointing at `:3003`/`:50052` on
`10.200.0.2` will fail. Ensure Tasks 1-5 are verified and the equivalence
baseline passes before proceeding.

⚠️ **Task 0 and Task 0b are hard prerequisites.** Do not start this task until:
- **Task 0** passes — otherwise `:8080` becomes the sole MCP ingress while still
  unauthenticated, which *widens* the exposed surface instead of narrowing it
  (today's `:3003` path at least runs the cognitive `interceptor.rs`, which this
  task deletes).
- **Task 0b** passes — otherwise deleting `serve_cognitive_grpc` strands
  `CognitiveToolService` and breaks the Xray `mcp.internal` route. Task 4 covers
  only `waypipe`, not both services on `:50052`.

### What to change

**`src/main.rs`:**
1. Delete `--no-http` and `--no-grpc` fields from `Cli` struct.
2. Delete the `resolve_bind()` function (no bind addresses to resolve).
3. Delete the transport match block (`:181-200`). Replace with:
```rust
// Post-Phase 2: stdio is the only transport.
// Tool execution happens in-process in op-grpc-bridge.
info!("Running stdio only (in-process registry is in op-grpc-bridge)");
server.start_stdio().await?;
```
4. Remove unused imports (env vars for `COGNITIVE_MCP_BIND`,
   `COGNITIVE_MCP_GRPC_BIND`, WireGuard identity code if only used for bind
   resolution).

**`src/server.rs`:**
1. Delete `start_http_server` (`:146-177`).
2. Delete `start_grpc_server` (`:179-190`).
3. Delete `start_dual` (`:192-217`).
4. Delete `serve_cognitive_grpc` (`:253-305`).
5. Remove unused imports: `CognitiveGrpcService`, `CognitiveToolServiceServer`,
   `build_context_router` (if only used by `start_http_server`), tonic/tower
   types used only by the gRPC server.

**`src/lib.rs`:**
1. Remove: `pub mod interceptor;`
2. Remove: `pub mod dbus_interface;`
3. Remove: `pub mod client_config;`
4. Remove: `pub mod grpc_service;`

**Delete files:**
- `src/interceptor.rs`
- `src/dbus_interface.rs`
- `src/client_config.rs`
- `src/grpc_service.rs`
- `examples/external_client.rs`

**`Cargo.toml` (op-cognitive-mcp):**
- Remove deps only used by deleted code: check if `tonic`, `tonic-reflection`,
  `tonic-health`, `tonic-web`, `tower-http` are still needed (they may be used
  by `op-waypipe-grpc` dependency or other remaining code — verify before
  removing).

**Runit service activation:**
1. `sudo sv stop op-cognitive-mcp` — frees `:3003` and `:50052`.
2. Activate WaypipeTunnel: `sudo ln -s /etc/runit/sv/op-waypipe-grpc /var/service/op-waypipe-grpc`
3. Wait for `op-waypipe-grpc` to bind `:50052`.
4. Update `/etc/runit/sv/op-cognitive-mcp/run`:
```sh
#!/bin/sh
exec 2>&1
# Phase 2: no network listeners. Stdio mode only.
# Tool execution is now in-process in op-grpc-bridge.
# NOTE: CozoDB concurrent access — bridge is primary writer; stdio is read-mostly.
export COGNITIVE_MCP_DB_PATH=/var/lib/op-cognitive-mcp/memory.db
exec /usr/local/bin/op-cognitive-mcp --stdio
```
5. `sudo sv start op-cognitive-mcp` — restarts in stdio-only mode.

**Update `deploy/config/cognitive-mcp-clients.json`:**
- Remove `cognitive_mcp` endpoint (`:3003`).
- Remove `grpc_cognitive` endpoint (`:50052`).
- Add `bridge` endpoint: `unix:path=/run/opdbus/session-bus.sock`
  (org.opdbus.v1.PluginV1.Call).
- Add `http_compact` endpoint: `http://127.0.0.1:8080/mcp/compact`.

### Verification

```bash
cargo check -p op-cognitive-mcp
cargo clippy -p op-cognitive-mcp --all-targets -- -D warnings

# Verify no listeners from op-cognitive-mcp:
ss -lntp | grep -E '3003|50052'
# Expected: 100.69.0.254:3003 (fwd-3003, PID 1250) and 10.200.0.2:50052 (op-waypipe-grpc)
# NOT: 10.200.0.2:3003

# Verify WaypipeTunnel works:
sudo sv status op-waypipe-grpc
# Expected: running

# Verify tools still work through bridge:
./bin/zcall cognitive_mcp invoke_tool -a '{"tool_name":"cognitive_memory","arguments":{"operation":"list_namespaces"}}'
# Expected: success envelope with event_id > 0

# Verify context routes:
curl -s http://127.0.0.1:8080/cognitive/context/health
# Expected: 200 OK

# Run full equivalence check:
./bin/verify-bridge-equivalence.sh
# Expected: all PASS, zero FAIL
```

### Rollback note

**This step is difficult to reverse in production.** To rollback:
1. Restore deleted source files from git.
2. Rebuild `op-cognitive-mcp` with listeners.
3. Restore the old `/etc/runit/sv/op-cognitive-mcp/run` with bind env vars.
4. `sudo sv restart op-cognitive-mcp`.
5. `sudo sv stop op-waypipe-grpc && sudo rm /var/service/op-waypipe-grpc`.
6. Revert Phase 1's dispatch to HTTP loopback (or leave in-process dispatch —
   both work as long as the registry is constructed).

---

## Task 7 — Full workspace build and final verification

**Verify the complete Phase 2 change set compiles and the only-door property
holds.**

```bash
# 1. Full workspace build
cargo build --workspace

# 2. Clippy on all affected crates
cargo clippy -p op-plugins -p op-grpc-bridge -p op-cognitive-mcp -p op-web \
  --all-targets -- -D warnings

# 3. Verify the door is closed — no cognitive-mcp TCP listeners
ss -lntp | grep -E '3003|50052'
# Expected output (2 lines only):
#   LISTEN ... 100.69.0.254:3003 ... (fwd-3003)
#   LISTEN ... 10.200.0.2:50052 ... (op-waypipe-grpc)

# 4. Verify in-process dispatch works without HTTP:
sudo sv stop op-cognitive-mcp   # ensure no :3003 at all
./bin/zcall cognitive_mcp invoke_tool -a '{"tool_name":"cognitive_memory","arguments":{"operation":"list_namespaces"}}'
# Expected: success envelope (tool registry is in-process in bridge)
sudo sv start op-cognitive-mcp  # restore stdio mode

# 5. Verify event chain integrity:
./bin/zcall cognitive_mcp invoke_tool -a '{"tool_name":"cognitive_memory","arguments":{"operation":"list_namespaces"}}' \
  | jq -e '.success == true and .event_id > 0 and (.event_hash | length) > 0' \
  && echo "OK: event chain recorded"

# 6. Verify context-awareness SSE:
curl -sf http://127.0.0.1:8080/cognitive/context/health | jq -e '.status'

# 7. Verify WaypipeTunnel:
sudo sv status op-waypipe-grpc
# Expected: running

# 8. Verify projection shape:
jq -e '[has("http_enabled"), has("grpc_enabled"), has("http"), has("grpc")]
       | all(. == false)' \
  /dev/shm/opdbus/projections/cognitive_mcp.json \
  && echo "OK: dead fields removed from projection"

# 9. Confirm no Python remains in the cognitive ingress path (NFR-4):
pgrep -af 'tcpfwd.py .* 3003'   # expect: no output

# 9. Run equivalence baseline comparison:
./bin/verify-bridge-equivalence.sh
# Expected: all PASS
```

### Acceptance criteria

- `cargo build --workspace` exits 0.
- `cargo clippy` produces zero warnings in the four affected crates.
- `ss -lntp` shows NO `op-cognitive-mcp` TCP listeners on any address.
- `./bin/zcall cognitive_mcp invoke_tool` succeeds even when `op-cognitive-mcp`
  service is stopped (proving in-process dispatch).
- Event envelope carries `event_id > 0` and non-empty `event_hash`.
- Context-awareness health endpoint returns 200.
- WaypipeTunnel service is running on `:50052`.
- Projection no longer contains `http_enabled`/`grpc_enabled`/`http`/`grpc`.
- Equivalence script reports zero FAIL.

---

## Summary Table

| Task | Crate(s) | Type | Reversible |
|------|----------|------|-----------|
| 1 — Construct CognitiveMcpServer in bridge | op-grpc-bridge | Add construction | ✅ Yes |
| 2 — Replace HTTP loopback with in-process | op-grpc-bridge | Replace dispatch | ✅ Yes (revert to HTTP) |
| 3 — Context SSE routes in op-web | op-web | Add routes | ✅ Yes |
| 4 — Create op-waypipe-grpc runit service | deploy | New service def | ✅ Yes |
| 5 — Remove schema fields, reseal blob | op-plugins | Schema change | ✅ Yes (re-add + reseal) |
| 6 — Delete listeners and dead code | op-cognitive-mcp | **Delete** | ⚠️ Hard to reverse |
| 7 — Full build + final verification | all | Verify | N/A |

---

## Sequencing constraints

```
Phase 1 implemented + equivalence baseline captured
    │
    ▼
Task 1 (construct registry in bridge)
    │
    ▼
Task 2 (replace HTTP loopback) ←── verify: tools work with op-cognitive-mcp stopped
    │
    ├──▶ Task 3 (context SSE in op-web) ←── can be parallel with Task 4
    │
    ├──▶ Task 4 (waypipe runit service definition) ←── parallel with Task 3
    │
    ▼
Task 5 (schema field removal + reseal)
    │
    ▼
Task 6 (delete listeners, activate waypipe, rewrite run script) ←── IRREVERSIBLE
    │
    ▼
Task 7 (final verification)
```
