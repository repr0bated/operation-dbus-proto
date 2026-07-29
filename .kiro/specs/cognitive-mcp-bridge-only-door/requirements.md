# Requirements: cognitive-mcp-bridge-only-door

## Purpose

Make `op-grpc-bridge` the only door to the cognitive MCP tool surface. Every MCP
client — host-side stdio, host-side HTTP, container gateway — reaches cognitive
tools exclusively through `org.opdbus.v1.PluginV1.Call` on
`/org/opdbus/v1/plugins/cognitive_mcp`, with schema validation, capability-vs-footprint
enforcement, and event-chain recording on every invocation.

`op-cognitive-mcp` stops opening its own `:3003`/`:50052` listeners. The
transport-selection flags (`--no-http`, `--no-grpc`, and the stdio special case)
stop being architecture-level concerns for the control plane.

## Context and Verified Baseline

### What already exists and is correct

- **Session bus**: `unix:path=/run/opdbus/session-bus.sock`. Bus name `org.opdbus.v1.plugins`
  owned by the PID of `op-grpc-bridge`. Verified with `./bin/zcall tree` (65 plugin objects).

- **Plugin interface**: every plugin object exposes exactly one non-freedesktop interface,
  `org.opdbus.v1.PluginV1`, with `Call(ss) -> s`, `GetAllProperties() -> s`,
  `GetProperty(s) -> s`, `SetProperty(ss) -> ()`. Verified via
  `./bin/zcall introspect cognitive_mcp`.

- **Enforcement chain** (`crates/op-grpc-bridge/src/schema_router.rs:985-1075`):
  1. Method-existence gate → `UnknownMethod` if method not in schema's `methods` map (:1001).
  2. `validate_json_args` against MethodDecl's arg schema (:1008).
  3. Capability check via `op_identity::read_sled` + `load_capability_grants` (:1020-1039).
  4. `MutationEngine::dispatch_method_call` → event-chain recording (:1064).

- **cognitive_mcp plugin schema** (`crates/op-plugins/src/state_plugins/cognitive_mcp.rs:926`):
  15 declared methods with capabilities and subids. Confirmed via `./bin/zcall methods cognitive_mcp`.

- **Tool registry**: `crates/op-cognitive-mcp/src/server.rs:44` creates a `ToolRegistry`,
  registers tools via `CognitiveToolRegistry::register_all` (:61), `typed_tools` (:68),
  and `register_code_tools` (:103). HTTP and stdio transports create `RegistryExecutor::new(tool_registry)`
  giving full tool access (:137, :156).

- **Existing D-Bus interface in op-cognitive-mcp**: `crates/op-cognitive-mcp/src/dbus_interface.rs:27`
  defines `CognitiveMcpInterface` with `list_tools`, `get_tool_schema`, `call_tool` on interface
  `org.opdbus.v1.plugins.CognitiveMcp`. This is a **separate**, unwired interface (module is
  `pub mod` in `lib.rs:24` but not registered on any bus connection).

- **Projection read**: `crates/op-cognitive-mcp/src/main.rs:90-105` already reads
  `cognitive_mcp` projection for bind config via `op_core::projection_shm::read_projection_bytes`.

- **CognitiveMcpConfig** (`crates/op-plugins/src/state_plugins/cognitive_mcp.rs:44-58`): struct
  with `http`, `grpc`, `wg_interface`, `http_enabled`, `grpc_enabled`, `dbus_enabled`.

### What is broken

1. **No backend dispatch for `cognitive_mcp` methods**: `crates/op-grpc-bridge/src/mutation_engine.rs:1010`
   — the `cognitive_mcp` plugin falls through to the `_ =>` catch-all arm, which returns
   `serde_json::to_value(&parsed_value)` (just echoes args back). None of the 15 schema methods
   actually execute anything via D-Bus `Call`.

2. **No generic tool-invoke method on the schema**: `list_tools` can enumerate the registry, but
   an MCP `tools/call` for e.g. `agent_rust_pro_clippy`, `search_blob_vectors`, or `ask_question`
   has no path through `PluginV1.Call`. The schema's 15 methods cover specific
   memory/code/config operations, not arbitrary tool dispatch.

3. **Capability welded to transport** (`crates/op-cognitive-mcp/src/server.rs:253-303`):
   `start_grpc_server` constructs `CognitiveGrpcService` (memory/session/quota/gemini only),
   while `start_http_server` (:156) constructs `RegistryExecutor::new(tool_registry)` (full
   tool set). Which tools a client can reach depends on which port it hit.

4. **Projection used as bind directive**: `/dev/shm/opdbus/projections/cognitive_mcp.json`
   currently contains `http_enabled: true` and `grpc_enabled: true`. Since
   `cognitive_mcp_bind_config` (:90) reads the projection FIRST, `--no-http`/`--no-grpc` CLI
   flags are silently overridden on any non-stdio launch.

5. **Direct :3003 path skips enforcement**: HTTP endpoint gated only by Ghostbridge header
   presence check (`crates/op-cognitive-mcp/src/interceptor.rs:20-30`). Does not pass through
   schema validation, capability-vs-footprint grants from sled, or the mutation/event chain.

### What is NOT broken and must not be touched

- The `CognitiveMcpServer::new()` constructor and tool registration pipeline
  (`crates/op-cognitive-mcp/src/server.rs:38-116`). Tools are registered correctly; only
  the ingress path changes.

- The `ToolRegistry` / `Tool` trait / `RegistryExecutor` in `crates/op-mcp/src/tool_registry.rs`.
  These are the correct execution substrate.

- The `CognitiveGrpcService` RPC implementations (`crates/op-cognitive-mcp/src/grpc_service.rs`).
  These cover NotebookLM bridge operations which are orthogonal (they have their own schema in
  `notebooklm.rs`). They may remain as internal plumbing but are not the control-plane door.

- The sealed blob catalog (`/dev/shm/opdbus/plugin-blobs/`). Must not be written by
  `op-cognitive-mcp`.

- `crates/op-cognitive-mcp/src/notebooklm.rs` — MCP sidecar bridge; not involved.

- `crates/op-plugins/src/state_plugins/cognitive_mcp.rs` schema shape (15 methods).
  The existing methods stay; we ADD a generic tool-invoke method.

- `op-state-store` legacy catalog. Not touched.

- Method name casing across plugins (e.g. `unix_socket` lowercase, `zeroclaw` PascalCase).
  Do not normalize.

---

## Functional Requirements

### FR-1: Schema-declared generic tool invocation method

A new method `invoke_tool` is added to the `cognitive_mcp` plugin schema
(`crates/op-plugins/src/state_plugins/cognitive_mcp.rs`) with:

- **Effect**: `Mutation` (tools may have side effects; all invocations must be recorded).
- **Capability**: `cognitive_mcp.invoke`.
- **Subid**: `mut.service.cognitive-mcp.tool.invoke@v1`.
- **Args schema**: JSON object with required fields `tool_name: string` and
  `arguments: object` (the tool's input payload, schema-free at the bridge layer since
  tool schemas are dynamic and enumerated by `list_tools`).
- **Response envelope**: the `Call` D-Bus method returns a single string. The response is
  JSON: `{"success": bool, "result": <tool_output>, "error": string|null,
  "tool_name": string, "event_id": u64, "event_hash": string}`.
- **Error semantics**:
  - Tool not found in registry → response envelope with `success: false`,
    `error: "tool not found: <name>"`. NOT an `UnknownMethod` D-Bus error (the *method*
    `invoke_tool` exists; the *tool* argument was invalid).
  - Tool execution failure → response envelope with `success: false`, `error: <message>`.
  - Schema validation failure (missing `tool_name` or `arguments`) → `InvalidArgs` D-Bus error
    (handled by the existing bridge arg-validation gate).

**Acceptance criteria**: `./bin/zcall methods cognitive_mcp` includes `invoke_tool` with
effect=mutation, capability=`cognitive_mcp.invoke`,
subid=`mut.service.cognitive-mcp.tool.invoke@v1`. A `./bin/zcall cognitive_mcp invoke_tool -a '{"tool_name":"cognitive_memory","arguments":{"operation":"list_namespaces"}}'`
returns a JSON envelope (success or error), never an `UnknownMethod` error.

`cognitive_memory` is used here because it is a real registry tool name; schema method
names such as `memory_store` are **not** valid `tool_name` values (see FR-2).

### FR-2: MutationEngine dispatch arm for cognitive_mcp

A new match arm in `crates/op-grpc-bridge/src/mutation_engine.rs` for `"cognitive_mcp"`.
The current `_ =>` catch-all echoes `parsed_value` back unchanged; that is replaced with
real execution.

**Transport**: HTTP loopback to `http://10.200.0.2:3003/mcp` (MCP `tools/call`) in Phase 1.
No new D-Bus name is created and `op-cognitive-mcp` gains no session-bus registration —
see design.md DQ-2. All external calls continue to enter through
`org.opdbus.v1.PluginV1.Call` on `/org/opdbus/v1/plugins/cognitive_mcp`.

**Per-method dispatch mapping** (resolves the earlier unresolved fork). The schema method
names do **not** map 1:1 to tool-registry names: the registry exposes `cognitive_memory`
with an `operation` argument, not `memory_store` / `memory_query` as separate tools.
Verified against the live registry (406 tools).

| Schema method | Dispatch target | Tool name | Arg translation |
|---|---|---|---|
| `memory_store` | tool registry | `cognitive_memory` | inject `operation: "store"` |
| `memory_query` | tool registry | `cognitive_memory` | inject `operation: "query"` |
| `memory_retrieve` | tool registry | `cognitive_memory` | inject `operation: "retrieve"` |
| `memory_delete` | tool registry | `cognitive_memory` | inject `operation: "delete"` |
| `memory_list_namespaces` | tool registry | `cognitive_memory` | inject `operation: "list_namespaces"` |
| `code_search` | tool registry | `search_blob_vectors` | pass-through |
| `code_index` | tool registry | `refresh_blob_vectors` | pass-through |
| `code_context` | tool registry | `search_blob_vectors` | inject `activity_type: "query"` |
| `gemini_query` | tool registry | `ask_question` | map `query` → `question` |
| `list_tools` | MCP `tools/list` | — | no args |
| `register_tool` | tool registry | `register_tool` | pass-through |
| `get_health` | **in-process** | — | read projection |
| `get_config` | **in-process** | — | read projection |
| `set_config` | **in-process** | — | `apply_state` path |
| `restart_service` | **in-process** | — | `sv restart` |
| `invoke_tool` (new) | tool registry | from `tool_name` arg | pass `arguments` verbatim |

The four `in-process` methods never cross to the executor. `invoke_tool` bypasses the
mapping entirely and is the recommended path for new consumers; the 15 existing methods
are retained for backward compatibility as sugar over it.

**Known schema defect to fix in the same change**: `list_tools`, `get_health` and
`get_config` declare their args as type `"null"`, so `zcall` sending `{}` fails with
`InvalidArgs: {} is not of type "null"`. They must accept `null` or `{}`.

**Acceptance criteria**: `./bin/zcall cognitive_mcp invoke_tool -a '{"tool_name":"cognitive_memory","arguments":{"operation":"list_namespaces"}}'`
returns a result from the actual `CognitiveMemoryStore`, not an echo of the input args.
`./bin/zcall cognitive_mcp get_health` returns projection data rather than an
`InvalidArgs` error. Event-chain recording is asserted from the response envelope itself,
which `MutationEngine::dispatch_method_call` builds at
`crates/op-grpc-bridge/src/mutation_engine.rs:1015-1021` (`success`, `event_id`, `event_hash`,
`plugin_id`, `method`): the returned JSON must carry a non-zero `event_id` and a non-empty
`event_hash`. There is no `zcall events` subcommand; do not invent one.

### FR-3: Projection stops acting as bind directive

The `cognitive_mcp_bind_config()` function (`crates/op-cognitive-mcp/src/main.rs:90-105`)
is removed or refactored so that `/dev/shm/opdbus/projections/cognitive_mcp.json` is NEVER
read as a bind-address source. Bind configuration precedence becomes:

1. `COGNITIVE_MCP_BIND` / `COGNITIVE_MCP_GRPC_BIND` env vars, exported **inline in the
   runit run script** at `/etc/runit/sv/op-cognitive-mcp/run`.
2. `--http` / `--grpc` CLI flags.
3. WireGuard interface IP detection.
4. `0.0.0.0` fallback.

**Correction to an earlier draft**: this section previously claimed the env vars come from
an "s6 env-dir written by the plugin's `apply_state`". That was wrong on three counts,
verified 2026-07-29:
- The host runs **runit**, not s6 (`runsv op-cognitive-mcp` PID 1108;
  `/etc/runit/sv/op-cognitive-mcp/run` exists).
- There is **no env-dir** — `/etc/runit/sv/op-cognitive-mcp/env/` does not exist. The two
  variables are `export`ed directly in the run script.
- `apply_state` does **not** write them today. Changing bind addresses would require
  rewriting the run script and `sv restart op-cognitive-mcp`.

The projection remains published state (consumers read it for status); it is no longer
config input.

**Acceptance criteria**: After `op-cognitive-mcp` starts with `--no-http --no-grpc --stdio`,
the projection file still says `http_enabled: true` but the process owns zero listeners
(verified with `ss -lntp | grep -c <PID>` = 0). The projection does not override CLI flags.

### FR-4: op-cognitive-mcp listeners become opt-in legacy, then removed

Phase 1 (this spec): `op-cognitive-mcp` retains its listeners behind the existing flags but
they are no longer the authoritative path. The `--stdio` mode remains as the only supported
transport for direct MCP clients; HTTP/gRPC listeners are deprecated.

Phase 2 (follow-up, explicitly deferred): listeners are deleted, `--no-http`/`--no-grpc`
flags removed, `start_http_server`/`start_grpc_server`/`start_dual` methods deleted.

**Acceptance criteria (Phase 1)**: MCP clients reconfigured to use the bridge path
(`./bin/zcall cognitive_mcp invoke_tool ...`) reach all tools. The `:3003`/`:50052`
listeners still function but are documented as deprecated.

### FR-5: Client transport is a client-side concern

The bridge (op-grpc-bridge) already serves gRPC on the session bus socket and HTTP on
`op-web`'s `:8080`. MCP clients choose their transport:

- **Host-side stdio clients** (e.g. the `op-cognitive-mcp --stdio` entry in `~/.factory/mcp.json`):
  switch to `op-mcp-server --stdio` pointing at the `cognitive_mcp` plugin, OR use
  `./bin/zcall` as a JSON-RPC forwarder.
- **Host-side HTTP clients** (e.g. `.mcp.json` `cognitive-mcp` entry at `http://10.200.0.2:3003/mcp`):
  switch to `http://127.0.0.1:8080/mcp/compact` (op-web's existing MCP endpoint, which
  already routes through the D-Bus plugin tree). Verified: `crates/op-web/src/mcp.rs:111`
  `create_mcp_router()` declares `/compact` (GET `mcp_compact_sse_handler`, POST
  `mcp_compact_message_handler`) and `/compact/message`; `crates/op-web/src/routes/mod.rs:271`
  binds that router and `:303` nests it with `.nest("/mcp", mcp_route)`, yielding
  `/mcp/compact`.
- **Container gateways**: use the session bus socket directly via `busctl` or a thin
  forwarding process.

What replaces `--no-http`/`--no-grpc`/`--stdio`:
- `--stdio` is preserved as the only direct-attach transport for `op-cognitive-mcp` (it is
  the tool-registry executor speaking MCP JSON-RPC, useful for debugging).
- `--no-http`/`--no-grpc` become no-ops once the bridge is the door, then are deleted.

**Tool-surface caveat**: `op-web`'s `/mcp/compact` exposes **4 meta-tools** —
`list_tools`, `search_tools`, `get_tool_schema`, `execute_tool` — which wrap the registry,
not a flat list of the 406 tools that `:3003` exposes directly. Verified live. Clients that
enumerated tools directly must switch to `search_tools` + `execute_tool`. This is an
intentional surface change, not a regression.

**Acceptance criteria**: `.mcp.json` and `~/.factory/mcp.json` `cognitive-mcp` entries are
reconfigured. `./bin/zcall cognitive_mcp invoke_tool -a '{"tool_name":"cognitive_memory","arguments":{"operation":"query","namespace":"project:op-dbus"}}'`
returns the same result as the old `http://10.200.0.2:3003/mcp` path.

Note the tool name: the registry exposes `cognitive_memory` with an `operation` argument.
There is no `memory_query` *tool* — `memory_query` is a schema *method* that maps onto
`cognitive_memory` (see FR-2's mapping table). Do not pass schema method names as
`tool_name`.

### FR-6: Cutover sequencing with rollback

The migration is sequenced so no consumer is stranded:

1. **Add `invoke_tool` to schema + dispatch arm** — additive, no breakage.
2. **Wire `op-cognitive-mcp` to register on the session bus** (expose its `CognitiveMcpInterface`
   or a UDS IPC channel) so the bridge dispatch arm can reach the tool registry.
3. **Prove equivalence** — run `./bin/zcall cognitive_mcp invoke_tool ...` for every tool
   in `list_tools` output and compare with the direct HTTP result.
4. **Migrate client configs** — update `.mcp.json`, `~/.factory/mcp.json`, container gateway.
5. **Deprecate listeners** — document that `:3003`/`:50052` are deprecated.
6. **Delete listeners** (Phase 2, deferred).

Rollback at each step: revert the client config to the HTTP entry; the old listeners still
work until Phase 2.

**Acceptance criteria**: At step 3, a script exercises every tool via both paths and asserts
identical JSON shapes. At step 4, no MCP client entry points at `:3003` or `:50052`.

### FR-7: Context-awareness SSE surface relocated

The context-awareness SSE endpoints (currently mounted on the `:3003` HTTP server via
`build_context_router` in `crates/op-cognitive-mcp/src/server.rs:167-174`) must be
accessible after `:3003` is deprecated. They are relocated to `op-web`'s `:8080` server
under a `/cognitive/context` prefix.

**How `op-web` reaches the context engine** (resolves the earlier "via D-Bus or an internal
channel" ambiguity):

`build_context_router` (`context_server.rs:75`) mounts five routes —
`GET /stream/:session_id` (SSE), `GET /status/:session_id`, `POST /record`,
`POST /request_push`, `GET /health` — and takes the engine, memory store and session
manager as arguments.

- **Phase 1**: no relocation happens. `:3003` stays up and the routes remain where they
  are. This requirement is stated here for continuity but is satisfied by Phase 2.
- **Phase 2**: `op-web` does **not** construct its own `ContextAwarenessEngine`. Doing so
  would open a second writer against the same persistent CozoDB that
  `CognitiveMcpServer::new` opens (`server.rs:41`), which is single-writer. Instead
  `op-web` implements the five routes as **translation shims that call the bridge**,
  consistent with its existing role as the HTTP→gRPC translator for `/mcp/compact`.
  `op-web` already depends on `op-cognitive-mcp` (`op-web/Cargo.toml:30`), so the types
  are nameable without a new dependency.

**Open risk**: the SSE stream is the one genuinely unvalidated piece — streaming through
the bridge without buffering the full response has not been prototyped. It must be proven
before the Phase 2 cutover is scheduled; if it proves impractical, the design is revisited
rather than falling back to a second engine instance. Tracked in the Phase 2 spec.

**Acceptance criteria**: After `:3003` is removed, the SSE endpoints are reachable at
`http://127.0.0.1:8080/cognitive/context/...` with equivalent functionality, and
`op-web` holds no second CozoDB handle.

---

## Non-Functional Requirements

### NFR-1: No new crate dependencies

The bridge dispatch arm uses existing IPC (D-Bus session bus or UDS) to reach the tool
registry. No new proto packages, no new HTTP clients.

### NFR-2: Event chain integrity

Every `invoke_tool` call through the bridge produces an event in the chain with `actor_id`,
`capability_id`, `event_hash`. The same accountability surface that `memory_store` gets via
the schema-backed interface applies to arbitrary tool invocations.

### NFR-3: No polling, no watchers

The bridge dispatch calls the tool registry synchronously (request-reply over D-Bus or UDS).
No subscription loops, no file watchers.

### NFR-4: Backward compatibility during Phase 1

`:3003` and `:50052` continue functioning during Phase 1. Existing MCP client configs work
until explicitly migrated. The bridge path is additive.

### NFR-5: OSCAL subid coverage

New method `invoke_tool` carries subid `mut.service.cognitive-mcp.tool.invoke@v1`, registered
in `crates/op-plugins/src/state_plugins/oscal_subid_registry.rs`.

---

## Out of Scope

- Deleting the listeners (Phase 2 — separate spec after equivalence is proven).
- Adding a UDS transport to `crates/op-mcp/src/transport/` (useful but not blocking).
- A protocol sniffer / auto-detect layer (the user's vision but not this spec's scope).
- Per-tool capability granularity (deferred — all tools use `cognitive_mcp.invoke` for now;
  fine-grained caps are a follow-up once the invoke path is stable).
- Fixing illegal D-Bus name hard-codes elsewhere in the tree (see Adjacent Issues below).
- Normalizing method name casing across plugins.
- The NotebookLM gRPC bridge (106 methods) — orthogonal concern.

## Adjacent Issues (NOT in scope, documented for awareness)

These are real defects found during verification that are adjacent but must NOT be bundled:

| Location | Issue |
|----------|-------|
| `crates/op-xray-daemon/src/main.rs:116` | `request_name("org.opdbus.v1.plugins")` — conflicts with op-grpc-bridge's bus name |
| `crates/op-xray-daemon/README.md`, `deploy/install-op-xray-daemon.sh` | References to the above |
| `crates/op-network/src/ovsdb.rs:18` | `DBUS_BUS_NAME: &str = "org.opdbus.v1.plugins"` hard-coded instead of discovered |
| `crates/op-grpc-bridge/src/schema_router.rs:53` | `dbus_destination` field + 6 test fixtures hard-coding the name |
| `crates/op-state/src/dbus_server.rs:22` | Interface `org.opdbus.v1.PluginV1` — actually correct (matches live bus), contrary to prompt claim of "missing `.v1`" |

Note: The prompt claimed `op-state/src/dbus_server.rs:22` uses `org.opdbus.PluginV1` (missing `.v1`).
Verified against the tree: the actual string is `org.opdbus.v1.PluginV1`, which matches the live
introspection output. The tree wins; this is NOT a defect.
