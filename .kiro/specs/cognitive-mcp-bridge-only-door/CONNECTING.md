# Connecting to cognitive-mcp: The Bridge-Only Door

## How to connect (today)

**One entry, all tools.** Use `op-mcp-server --stdio -m cognitive`:

```json
{
  "op-web-compact": {
    "command": "/usr/local/bin/op-mcp-server",
    "args": ["--stdio", "-m", "cognitive"],
    "type": "stdio"
  }
}
```

This serves **561 tools** from a single stdio socket:
- 406 cognitive tools (memory, RAG, code search, agents, NotebookLM) → routed through the bridge enforcement chain
- 155 system tools (dbus, ovs, file ops, service management) → executed locally

The client sees 5 meta-tools (`list_tools`, `search_tools`, `get_tool_schema`,
`execute_tool`, `respond`) which lazily discover the full set.

### Alternative: HTTP

```json
{
  "op-dbus-compact": {
    "url": "http://127.0.0.1:8080/mcp/compact",
    "type": "http"
  }
}
```

Note: the HTTP path currently serves only the op-tools builtins (155 tools), not the
merged set. Use stdio `cognitive` mode for the full surface.

### Alternative: D-Bus direct (scripts/automation)

```bash
# List available cognitive tools
./bin/zcall cognitive_mcp list_tools -a '{}'

# Invoke a specific tool through the bridge
./bin/zcall cognitive_mcp invoke_tool -a '{"tool_name":"cognitive_memory","arguments":{"operation":"list_namespaces"}}'

# Use a schema method directly
./bin/zcall cognitive_mcp memory_query -a '{"namespace":"project:op-dbus","key_pattern":"*","limit":5}'
```

---

## Architecture (as implemented)

```
MCP Client (IDE / script)
    │
    └─ stdio ──► op-mcp-server --stdio -m cognitive
                     │
                     ├─ cognitive tools ─► D-Bus PluginV1.Call("invoke_tool", ...)
                     │                        │
                     │                        ▼
                     │   op-grpc-bridge (org.opdbus.v1.plugins)
                     │        │  method gate → arg validation → capability check → event chain
                     │        │  dispatch_cognitive_mcp_method() → HTTP loopback
                     │        ▼
                     │   op-cognitive-mcp :3003/mcp → ToolRegistry
                     │
                     └─ op-tools builtins ─► local execution (no bridge hop)
                              (dbus, ovs, file ops, service mgmt)
```

561 total tools. One process. One socket. Cognitive tools get full enforcement;
op-tools builtins run directly (they're system utilities that don't need event-chain).

---

## Bus names

| Bus name | Owner | Purpose |
|----------|-------|---------|
| `org.opdbus.v1.plugins` | op-grpc-bridge (PID 12348) | **The only legal well-known name.** All plugin dispatch. |

No `org.opdbus.v1.executors.*` names exist or are registered. The spec's design.md
mentions them but they are fictional.

---

## Capability grants

From `/dev/shm/opdbus/capability-grants.json`:

```json
{
  "*": {
    "capabilities": [
      "cap.software.zeroclaw.chat@v1",
      "cap.software.zeroclaw.models.read@v1",
      "cognitive_mcp.read",
      "cognitive_mcp.invoke"
    ]
  }
}
```

The wildcard `*` means **all footprints** pass the capability check for both read and
mutation methods on cognitive_mcp. This is the current state — no footprint is denied.

---

## Schema methods and tool mapping

`./bin/zcall methods cognitive_mcp` yields 16 methods. The dispatch table in
`mutation_engine.rs:map_schema_method_to_tool()` translates them:

| Schema method | Effect | Tool registry name | Notes |
|---------------|--------|--------------------|-------|
| `memory_store` | mutation | `cognitive_memory` | `operation: "store"` injected |
| `memory_query` | read | `cognitive_memory` | `operation: "query"` injected |
| `memory_retrieve` | read | `cognitive_memory` | `operation: "retrieve"` injected |
| `memory_delete` | mutation | `cognitive_memory` | `operation: "delete"` injected |
| `memory_list_namespaces` | read | `cognitive_memory` | `operation: "list_namespaces"` injected |
| `code_search` | read | `search_blob_vectors` | direct pass-through |
| `code_index` | mutation | `refresh_blob_vectors` | direct pass-through |
| `code_context` | read | `search_blob_vectors` | `activity_type: "query"` injected |
| `gemini_query` | mutation | `ask_question` | `query` → `question` field rename |
| `register_tool` | mutation | `register_tool` | direct pass-through |
| `invoke_tool` | mutation | *(caller names it)* | `tool_name` + `arguments` from args |
| `list_tools` | read | *(MCP tools/list)* | Protocol method, not a registry tool |
| `get_config` | read | *(projection read)* | Answered from shared-memory projection |
| `get_health` | read | *(projection read)* | Answered from shared-memory projection |
| `set_config` | mutation | *(apply_state)* | Acknowledged, handled by plugin |
| `restart_service` | mutation | *(apply_state)* | Acknowledged, handled by plugin |

---

## Port ownership (live)

| Address | Port | Process | PID | Role |
|---------|------|---------|-----|------|
| 0.0.0.0 | 8080 | op-web-server | 27787 | HTTP front-end (bridge-backed) |
| 10.200.0.2 | 3003 | op-cognitive-mcp | 24389 | Direct MCP HTTP (deprecated, Phase 1 legacy) |
| 10.200.0.2 | 50052 | op-cognitive-mcp | 24389 | Direct gRPC (deprecated, Phase 1 legacy) |
| 100.69.0.254 | 3003 | python3 | 1250 | Container gateway proxy |

---

## Service supervision

`op-cognitive-mcp` is a **runit** service at `/etc/runit/sv/op-cognitive-mcp/run`:

```sh
#!/bin/sh
exec 2>&1
# ... wait_dep ovsbr0-addr ...
export COGNITIVE_MCP_BIND=10.200.0.2:3003
export COGNITIVE_MCP_GRPC_BIND=10.200.0.2:50052
export COGNITIVE_MCP_DB_PATH=/var/lib/op-cognitive-mcp/memory.db
exec /usr/local/bin/op-cognitive-mcp
```

Managed by `runsv` (PID 1108). **No s6 env-dir exists on this host.** The spec's FR-3
reference to "s6 env-dir written by apply_state" is incorrect — env vars are hardcoded
in the runit `run` script.

---

## Unresolved items (spec vs reality)

These are known gaps between `design.md` and the implementation:

1. **design.md self-correction paragraph** (lines 344–402): Proposes `serve_at` under the
   bridge's path, then immediately contradicts itself with "Correction: Actually in D-Bus..."
   and introduces `org.opdbus.v1.executors.cognitive_mcp`. Neither was implemented. The
   actual transport is HTTP loopback. The self-correction should be deleted and replaced
   with: "Phase 1 uses HTTP loopback to :3003; Phase 2 may use in-process registry."

2. **FR-7 context_engine in op-web**: Not wired. The SSE endpoints (`/context/stream`,
   `/context/status`, `/context/request_push`) exist only on the deprecated `:3003` listener.
   No `/cognitive/context` route exists in op-web. This blocks full `:3003` retirement.

3. **Phase 1 ports alive = "only door" not yet achieved**: Correct. `:3003` and `:50052`
   remain functional. The bridge is the *authoritative* door (enforcement chain), but not
   yet the *only* door. Phase 2 is explicitly deferred.

4. **DQ-4 rejected-alternative reasoning**: "Remove --stdio entirely — can't. Stdio is
   needed for the process to function as a D-Bus service." This is wrong. Stdio transport
   and D-Bus service registration are independent. The real reason to keep `--stdio` is:
   it's useful for local debugging and testing (direct MCP client attach without network).
