# Requirements: unified-authenticated-mcp-cognitive-control-plane

## Purpose

Establish `10.0.0.3:8090` as the authenticated unified fabric surface itself, owned by
`op-grpc-bridge`: MCP at `/mcp`, native gRPC/gRPC-Web, generated plugin methods, and
mutation/control capabilities all enter through that one surface. Every MCP client —
agents, cognitive, code, memory, blob-schema/catalog, context-awareness, Waypipe, and
generated plugin methods — reaches the same
in-process registry (and the same sealed-plugin route surface) through the same
authentication, route-building, capability, validation, dispatch, and event-chain
stack, whether the transport is TCP, the host Unix domain socket, or a container
Unix domain socket.

This spec supersedes and fully incorporates the relevant content of:

- `.kiro/specs/cognitive-mcp-bridge-only-door/` (Phase 1)
- `.kiro/specs/cognitive-mcp-only-door-phase2/` (Phase 2 + `fanin-proxy.md`)
- `.kiro/specs/unified-blob-catalog-mcp/`
- `.kiro/specs/voyage-plugin-cognitive-mcp-boundaries/`

It integrates (does not replace) the identity, reflection-pipeline, UI, and audit
concerns owned by:

- `.kiro/specs/netmaker-xray-identity-handoff/` — the authoritative Oracle-signed
  assertion identity model. This spec depends on it and does not redefine it.
- `.kiro/specs/schemars-to-reflection-plugin-pipeline/` — the PluginSchema →
  build.rs proto → reflection pipeline. This spec consumes it.
- `.kiro/specs/netmaker-custom-json-render-ui/` — the operator UI that reads the
  sealed blob catalog and drives gRPC. This spec fixes its endpoint references.
- `.kiro/specs/accountability-audit-trail/` — the event-chain query surface. This
  spec relies on the event-chain append it describes.

### Canonical spec tree

The authoritative Kiro spec tree in this repository is `.kiro/specs/` (it carries
`README.md` and the full spec set). A second, duplicate `kiro/specs/` tree exists
and is byte-identical for the four subsumed specs. This canonical spec lives ONLY
under `.kiro/specs/`. The consolidation tasks delete the subsumed directories and
de-conflict the orthogonal ones in **both** trees, and record the duplicate tree as
an issue to reconcile (see CR-6).

This document is written against verified live and source state as of 2026-08-31.
Old spec claims that disagree with that state are corrected here; corrections are
called out inline and in the Verified Baseline section.

---

## Terminology

| Term | Meaning |
|---|---|
| **Bridge** | `op-grpc-bridge`, the sole owner of the `:8090` TLS ingress, of the D-Bus session-bus name `org.opdbus.v1.plugins`, of the canonical `ToolRegistry`, of the sealed-plugin route surface, and of the single authoritative durable Cozo writer. |
| **Unified fabric surface** | The one bridge-owned mesh-private external surface, `10.0.0.3:8090`, carrying MCP `/mcp`, native gRPC/gRPC-Web, generated plugin methods, and mutation/control capabilities through one authority. It is not merely an endpoint on another fabric. |
| **Ingress** | The listener set implementing the unified fabric surface on TLS `:8090`. |
| **UDS** | Unix domain socket. Host UDS = `/run/opdbus/grpc.sock`; container UDS = the bind-mounted `/run/ghostbridge/container.sock`. |
| **Registry** | The single in-process `ToolRegistry` (`crates/op-mcp/src/tool_registry.rs`) owned by the bridge. |
| **Registry tool** | A typed `Tool` implementation registered in the bridge-owned `ToolRegistry` (cognitive, code, memory, agent, blob, HOT tools, or WARM/COLD set members). The registry is an in-process implementation detail behind the canonical D-Bus execution path; no network adapter may execute it directly. |
| **Generated plugin method** | A sealed-PluginSchema-derived gRPC method dispatched through `PluginService.CallMethod` → D-Bus → `MutationEngine`. These are **routes**, not registry tools (see FR-3a). |
| **Tool descriptor** | A read-only runtime projection of the tool's authoritative sealed `PluginSchema.methods[method]` / `MethodDecl`: `required_capability`, `subid`, `side_effect`, `idempotent`, `input_schema`, `output_schema`, `approval_required` (see FR-3b). It is not a second independently authored contract. |
| **Capability-filtered view** | A per-client subset of the tool/route surface computed from the caller's resolved identity and granted capabilities — never a separate network service. |
| **Sealed blob** | An immutable `OPBLOB01` object at `/dev/shm/opdbus/plugin-blobs/<plugin_id>.<schema_hash16>.blob`, written only by `op-blob`. A blob is active only when it is manifest-selected, hash/compatibility validated, and its declared executable shapes are mounted; an uncompiled or incompatible blob remains staged/inactive (FR-9). |
| **PluginSchema** | The published contract for a plugin, derived from schemars; single source of truth for method shapes, capabilities, subids. |
| **OIA1** | The versioned Oracle Identity Assertion wire envelope defined by `netmaker-xray-identity-handoff`. |
| **SID1** | The bounded MutationEngine-authored immutable session identity envelope stored inline in exactly one selected identity sled and forwarded unchanged by the local header helper. It is not an OPBLOB01 object, footprint, grant key, or Snowball input. |
| **Header helper** | Short-lived `op-identity-headers`, invoked natively by Codex through per-request `http_headers_helper`; it emits one JSON header object and owns no listener, MCP transport, proxy, or identity-authoring logic. |
| **identity_sled** | The per-session identity projection resolved by the bridge for capability lookup. |
| **Event chain** | The per-mutation immutable chain (`op-snowball` / `EventChain`) appended on every dispatched call. |
| **Memory domain** | A trust/scope class of memory: system-curated, chatbot-soul, user/container, workspace/project, or shared-semantic (see DR-6). |

---

## Non-negotiable target architecture (normative)

1. `10.0.0.3:8090` is exactly one external **unified fabric surface**, owned by the
   bridge, covering MCP, gRPC/gRPC-Web, plugins, mutation, and control capabilities.
2. All MCP clients authenticate at `:8090` before discovery or execution with exactly
   one accepted identity credential: exact active-sled SID1 for the protected local
   client or fresh OIA1 for another caller.
3. The `:8090` listener multiplexes, on **one port**, MCP Streamable HTTP/JSON-RPC,
   optional MCP SSE, and native gRPC (with server reflection). "One port, two
   protocols" means the same listener handles each protocol over its own valid
   connection; it does not mean a single connection carries both protocols
   simultaneously.
4. Host UDS and container UDS are **alternate transports** through the exact same
   authorization → route-building → capability → validation → dispatch →
   event-chain stack. They are not separate authorities and grant no implicit trust.
   Transport-specific *binding* differs (TCP source binding vs UDS peer-credential
   binding, TR-2); *authorization semantics* are identical.
5. `op-web :8080` remains available for dashboard/UI and ordinary REST only. It
   exposes **zero** MCP execution endpoints.
6. The following are **forbidden** in the final architecture:
   - MCP execution through `:8080`;
   - `:3003`; MCP-related `:50051`; `:50052`; `:11438` (and `:11437`);
   - standalone cognitive, compact, agents, blob-schema, or Waypipe MCP listeners;
   - bridge-to-cognitive HTTP loopbacks;
   - direct web-owned tool execution;
   - reflection entries without mounted callable routes;
   - authentication based on sentinel footprints, wildcard identities,
     `plugin_schema.dat`, raw self-asserted identity headers, or an implicit
     "trusted local" bypass.
7. `plugin_schema.dat` must not be read, created, restored, documented as active,
   used as a fallback, or treated as a system component.
8. Every executable MCP/tool/generated-method request MUST enter the single canonical
   D-Bus path (`PluginService.CallMethod` → `MutationEngine`) after bridge
   authentication. The in-process `ToolRegistry` is an executor selected by the
   MutationEngine, not a parallel control plane. Tool contracts are derived from the
   manifest-selected sealed `PluginSchema`; MCP, gRPC, typed HOT/WARM/COLD tools, and direct
   generated calls MUST NOT author or execute a second contract.
9. EMQX is a standalone internal broker attached behind the fabric through loopback
   MQTT and a dedicated authenticated ExHook listener. It never exposes or forwards
   MCP and never becomes a second external endpoint or identity authority.

---

## Verified Baseline (evidence, 2026-08-31)

Confirmed by direct live inspection and source reads. Where a line moves, the symbol
name is authoritative.

### Live host (`sv status`, `ss -lntp`, `/dev/shm`)

| Fact | Status | Evidence |
|---|---|---|
| `op-grpc-bridge` up; **directly** listens `127.0.0.1:8090` (TLS) only. `10.0.0.3:8090` is a **`python3` socket-relay** (`fwd-8090`, PID 1496) forwarding to `127.0.0.1:8090` — NOT a bridge bind | ✅ (corrected) | privileged `sudo ss -lntp`: `127.0.0.1:8090 → op-grpc-bridge (pid 7540)`, `10.0.0.3:8090 → python3 (pid 1496)`; `deploy/runit/fwd-8090/run` execs `socket-relay tcp-listen 10.0.0.3:8090 tcp-connect 127.0.0.1:8090` |
| The mesh-facing bind is therefore a Python relay, and the bridge does **not** directly bind any non-loopback `:8090`; `10.200.0.2:8090` is not bound at all | ⚠️ gap + policy violation | the relay also violates the no-Python policy (NFR-6); FR-1 resolves the intended direct-bind topology |
| **`op-web :8080` proxies ALL `application/grpc*` to `:8090`** via `crates/op-web/src/grpc_proxy.rs` (`dispatch` middleware, upstream `https://127.0.0.1:8090`), plus `/jsonrpc`, `/rpc`, `/.well-known/mcp.json`, `mcp_smart_router.rs`, `mcp_discovery.rs` | ✅ (defect) | this is a full alternate gRPC ingress; removing only `/mcp*` does not close it (TR-4) |
| The bridge serves **gRPC-Web directly on `:8090`** (TLS) via `crate::grpc_web::enable` (wraps `tonic_web::GrpcWebLayer` + CORS) — the browser dashboard does not strictly require the `:8080` proxy for framing | ✅ | `crates/op-grpc-bridge/src/grpc_web.rs`, `grpc_server.rs` service registration, `.accept_http1(true)` |
| `op-web` up; `0.0.0.0:8080` | ✅ | `ss -lntp` |
| No `:3003`, `:50051`, `:50052`, `:11438`, `:11437` listeners | ✅ | `ss -lntp` |
| `op-cognitive-mcp` DOWN; `op-mcp-agents`/`op-mcp-compact` DOWN with `exec pause` run scripts | ✅ | `sv status`; `deploy/runit/op-mcp-agents/run`, `op-mcp-compact/run` |
| `op-mcp-blob-schema`, `op-mcp-cognitive`, `op-waypipe-grpc` service dirs absent on live host but present in repo tree | ⚠️ | `sv status` "file does not exist"; `deploy/runit/*` still present |
| **`op-web` `/mcp/*` currently executes tools unauthenticated** (pre-consolidation defect) | ✅ (defect) | `op-web/src/routes/mod.rs` mounts `mcp::create_mcp_router` + agents SSE; `ip_security_middleware` resolves a zone but denies nothing |
| **Live `:8090` reflection advertises cognitive methods that dispatch via the HTTP loopback** — and the source loopback target `:3003` is not running, so those methods effectively fail (UNIMPLEMENTED-class) at runtime | ✅ (defect) | source `mutation_engine.rs` loopback + no live `:3003` |
| Deployed bridge/web binaries lag current source | ⚠️ | run script says "Phase 2 in-process"; source still has loopback (see below) |
| `plugin_schema.dat` absent from `/dev/shm/opdbus` | ✅ | `find` → none |
| 66 sealed blobs in `/dev/shm/opdbus/plugin-blobs/` | ✅ | `ls` |
| `/dev/shm/opdbus/capability-grants.json` grants wildcard `*` identity `cognitive_mcp.invoke`, `cognitive_mcp.read`, `mcp.read/write`, `agent.invoke`, `compact_mcp.*` … | ✅ (defect) | file — the forbidden wildcard-identity bypass |
| A 64-hex derived-footprint grant key also exists | ✅ (defect) | same file — grants must migrate to registered `principal_id`, never a digest |
| Working typed capability inventory plus the four retired compact names | ✅ baseline | capture typed capabilities as the preservation oracle and compact names as the removal oracle in Phase 0 (DEP/CR-1) |

### Source (repo) — the `Tool`/`ToolRegistry` contract (the authorization gap)

Read from `crates/op-mcp/src/tool_registry.rs`:

- `trait Tool` (line 28) exposes only `name`, `description`, `input_schema`,
  `category`, `namespace`, `tags`, `execute`. It has **no** `required_capability`,
  `subid`, `side_effect`, `idempotent`, `output_schema`, or `approval_required`.
- `ToolDefinition` (from `op_core`) mirrors the trait; no capability field.
- `register` (line 61) does `HashMap::insert` — it **silently overwrites**
  duplicate names.
- `execute` (line 89) does `get(name)` then `tool.execute(input)` immediately —
  **no target-tool capability check and no validation of `input` against the
  tool's `input_schema`**.

Consequences that the requirements below MUST fix:

- Possessing the broad `cognitive_mcp.invoke` (or `execute_tool`) capability
  currently authorizes **every** registered tool, including
  `agent_shell_executor_exec` and `agent_python_executor_run`. This is a broad
  authorization bypass (FR-3b, SEC-3).
- `invoke_tool` / `execute_tool` validate only the outer `{tool_name, arguments}`
  envelope; the nested `arguments` are never validated against the target tool's
  schema (FR-5a).

### Source (repo) — dispatch, routing, listeners

| Fact | Classification | Evidence |
|---|---|---|
| Bridge ingress binds UDS `/run/opdbus/grpc.sock`, `container.sock`, TLS TCP `:8090`; TLS mandatory | (A) correct | `server.rs` `DEFAULT_BIND_ADDR`, `load_tls_identity` |
| Reflection hydrated from sealed catalog, arrival-triggered, then frozen; `operation.method.*` advertised only for sealed plugins | (A) correct | `dynamic_reflection.rs` `hydrate_reflection_from_shm`, `rebuild_index`; `grpc_server.rs` `freeze_plugin_method_reflection` |
| Dispatch pipeline: method gate → `validate_json_args` (draft-07) → capability check → `dispatch_method_call`; `Updated` signal after `write_projection` | (A) correct (for D-Bus method routes; not for registry tools) | `schema_router.rs` |
| OIA1 assertion validation present: parse → trusted decoy key → signature → expiry → replay nonce → source binding → HumanPrincipal resolution; legacy footprint path fails closed on unknown genesis | (A) correct | `interceptor.rs` `make_ghostbridge_interceptor`, `AssertionValidator.validate_with_bootstrap` |
| `load_capability_grants` honors the `"*"` wildcard | (B) defect | `interceptor.rs`; `capability-grants.json` |
| `cognitive_mcp` dispatch is an HTTP loopback to `http://10.200.0.2:3003/mcp` | (B) incorrectly integrated | `mutation_engine.rs` `dispatch_cognitive_mcp_method`, `cognitive_mcp_endpoint`, `cognitive_mcp_http` |
| `code_search → search_blob_vectors`, `code_index → refresh_blob_vectors`, `code_context → search_blob_vectors` | (B) code-tool mis-routing bug | `mutation_engine.rs` `map_schema_method_to_tool` |
| Real code tools exist under names `code_search`/`code_context`/`code_index` | (A) but unreachable via bridge | `code_tools.rs` `CodeSearchTool`/`CodeContextTool`/`CodeIndexTool`, `register_code_tools` |
| `CognitiveMcpServer` owns registry/context/memory/session; exposes `tool_registry()/context_engine()/memory_store()/session_manager()`; opens Cozo single-writer **with in-memory fallback if locked** | (A) substrate; (B) the silent in-memory fallback (DR-1a) | `server.rs` |
| `start_http_server(:3003)`, `start_grpc_server(:50052)`, `serve_cognitive_grpc` (co-hosts WaypipeTunnel) still exist, `#[deprecated]`, still invoked | (C) implemented, should not be deployed | `server.rs`, `main.rs` |
| `op-mcp-server` opens a socket per `--mode`; cognitive mode `MergedToolExecutor` reaches the bridge via D-Bus `PluginV1.Call invoke_tool` + local `op-tools` builtins | (B)/(E) | `op-mcp/src/main.rs`, `cognitive_bridge.rs` |
| `op-mcp` `blob_schema.rs` implements `blob_catalog`/`blob_schema`/`blob_manifest`/`blob_methods`/`blob_search`; `blob://<plugin_id>` resources in `resources.rs`; a **second** `blob_catalog` in `op-cognitive-mcp/blob_catalog_tool.rs` | (A) with duplicate to resolve | files present |
| `op-web/src/routes/mod.rs` mounts MCP execution routes on `:8080` | (B)/(E) forbidden | `routes/mod.rs`, `mcp.rs`, `mcp_agents.rs` |
| `op-chat` opens its own persistent Cozo `/var/lib/op-dbus/chat-memory.db` (independent writer) | (B) defect w.r.t. single-writer | `op-chat/src/main.rs` |
| `op-web` `UserStore` opens its own Cozo `/var/lib/op-dbus/users-cozo` (separate DB, users only) | (A) acceptable (not cognitive memory) | `op-web/src/state.rs` |
| Context signals complete; but `start_monitoring` runs a 5 s `tokio::time::interval` poll (`EVALUATION_INTERVAL_MS = 5000`) | (A) signals, (B) polling | `context_awareness.rs` |
| Post-turn memory uses a hardcoded regex; tool args/results not fully persisted; accepted/rejected outcomes not modeled | (B)/(D) | `memory_loop.rs` |
| `blob_vectors`: user-triggered wholesale refresh; deterministic UUIDv5 point IDs; one point per active plugin; `search_blob_vectors` semantic query; fail-closed when Qdrant absent | (A) — must be preserved (FR-8a) | `blob_vectors_tool.rs`, `qdrant_shuttle.rs` |
| `plugin_schema.dat` not referenced in live `crates/*/src`; `live-schema.json` monolith is test-only; blob catalog authoritative | (E) obsolete references | source search; `CLAUDE.md` |
| `Tool::execute(&self, input: Value)` receives **only caller-controlled JSON** — no `ExecutionContext`/identity/scope/deadline param | (B) defect (blocker 2) | `crates/op-mcp/src/tool_registry.rs:41`, `:89` |
| A sealed `PluginSchema.capability_grants` can carry a wildcard `"*"` grant; enforcement honors it (`grants.get(footprint).or_else(|| grants.get("*"))`) | (B) defect (blocker 4) | `crates/op-state-store/src/plugin_schema.rs:321`; `crates/op-plugins/src/state_plugins/tched_router.rs` inserts `"*"`; `grpc_server.rs` `enforce_bridge_capability` |
| `DECLARED_CAPABILITY_HEADER` (`x-opdbus-capability`) is **caller-supplied**; when a method declares no grants the check degrades to `identity.is_some() && capability_matches` | (B) defect (blocker 4) | `crates/op-grpc-bridge/src/grpc_server.rs:114`, `enforce_bridge_capability_with_schema` |
| Tools accept **caller-provided** `container_id`/`identity_id`/`namespace`/`session_id`/`collection`/`path` in their input schema (self-assertable scope) | (B) defect (blocker 3) | `crates/op-cognitive-mcp/src/cognitive_tools.rs` (memory input), `code_tools.rs` (`session_id`, `collections_from`) |
| No code validates a tool's **output** against `output_schema` before returning/persisting/vectorizing/prompting | (D) missing (blocker 4) | `output_schema`/`returns` used only for proto/reflection gen; no runtime output validator |
| Host UDS `/run/opdbus/grpc.sock` and container UDS are served **plaintext** (no `.tls_config`); shared socket chmod `0o666` (world read/write) | (B) defect (blocker 7) | `crates/op-grpc-bridge/src/server.rs` `serve_with_incoming` (no TLS); `shared_socket.rs` `set_permissions(0o666)` |
| `read_plugin_schema_shm` resolves `<plugin_id>.` by **first-prefix match**, not exact `<schema_hash16>` | (B) defect (blocker: additions) | `crates/op-blob/src/catalog.rs:352` |
| OIA1 replay-nonce cache is an **in-process `Mutex<HashMap>`** — lost on restart, not shared across transports/processes | (B) defect (blocker: additions) | `crates/op-grpc-bridge/src/oracle_assertion.rs:205` `AssertionReplayCache` |
| Per-method typed gRPC routes are generated **statically at build time**; a newly sealed blob whose method shape was not compiled in has no typed route (dynamic reflection can advertise it) — though `PluginService.CallMethod`/`schema_router` reads sealed schemas at runtime | (B)/(C) seal-vs-compiled gap (blocker 8) | `crates/op-grpc-bridge/build.rs:234` `generate_plugin_method_routes`; `dynamic_reflection.rs`; `schema_router.rs` |

### Governing docs

- `CLAUDE.md`'s "MCP gateways (settled — do not redesign)" paragraph
  (op-cognitive-mcp as universal `:50052` gateway; compact-mcp loopback) is **stale
  and contradicted** by both this spec's mandated architecture and the live host.
  This spec supersedes that paragraph (see design.md §Conflicts).
- `AGENTS.md`: host services via `sudo sv`; deployment via
  `deploy/runit/build-golden.sh` (btrfs golden image), never hand-copied; no
  `systemctl`/s6; Xray config only at `/etc/xray/xray_config.json` in the container.
  These are **governing repository policies**; the "no new deps"/"no Python"
  requirements below (NFR-1, NFR-6) derive from them, not from arbitrary preference.

---

## Requirement conventions

Each requirement has a unique ID, a normative statement, and testable acceptance
criteria. IDs are stable and referenced by `tasks.md`. Prefixes: **FR** functional ·
**SEC** security/identity · **DR** data · **TR** transport · **DEP** deployment ·
**CR** compatibility/migration · **NFR** non-functional. "MUST"/"MUST NOT"/"SHALL"
are normative. Criteria marked `[manual]` cannot be fully automated.

---

## Functional Requirements

### FR-1 — One unified fabric surface (resolved listener topology)

The bridge MUST own the only network listener that serves MCP tool discovery or
execution: TLS `:8090`. The current topology is defective: the bridge directly binds
only `127.0.0.1:8090`, and `10.0.0.3:8090` is a **Python `socket-relay`** (`fwd-8090`)
— an unmanaged L4 forwarding hop (TLS remains end-to-end), a second process owning
the canonical port, and a no-Python-policy violation. The canonical bind set is exact,
not an implementation-time choice:

- `127.0.0.1:8090` (host loopback);
- `10.0.0.3:8090` (the `svc0` unified fabric address).

There is no Netmaker bind in the target architecture. EMQX is also no longer a
Netmaker component; it stays behind this surface on loopback MQTT/ExHook.

`10.200.0.2:8090` is not an MCP ingress address and MUST NOT be bound unless a later,
reviewed change adds it to this canonical contract. The resolved implementation MUST:

- have the **bridge directly bind** every applicable canonical address with the same
  TLS identity, protocol demultiplexer, authentication, and route stack, so no relay
  terminates or forwards `:8090` traffic;
- state the firewall policy for the mesh-facing port and the TLS certificate SANs
  that cover every bound address;
- retire both historical `fwd-8090` (including its Python `socket-relay`) and
  `fwd-nm-mesh-8090`; neither is replaced by a Netmaker bind or relay;
- fail startup for a configured canonical address that exists on its interface but
  cannot be bound; it MUST NOT silently continue loopback-only.
No other process may open a network listener that serves MCP/gRPC, and no relay may
sit in front of `:8090`.

**Acceptance:**
- `sudo ss -lntp` shows only the **bridge PID** listening on `127.0.0.1:8090` and
  `10.0.0.3:8090`; no Netmaker/non-canonical `:8090` bind and no listener on
  `:3003`, `:50051`, `:50052`, `:11438`, `:11437` exists.
- `fwd-8090` and `fwd-nm-mesh-8090` are retired (`sudo sv status` → down/absent;
  service dirs removed) and no `socket-relay`/`tcpfwd`/`python3` process fronts
  `:8090`.
- Plaintext gRPC dial to `:8090` fails; a TLS-authenticated reflection dial succeeds
  against every bound address, and the presented cert's SANs cover them.
- The deployed firewall allows `10.0.0.3:8090` only on `svc0` from the enumerated
  trusted service CIDR and denies every other ingress; an unauthorized-interface
  reachability test fails while the authorized service path succeeds.

### FR-2 — Protocol multiplexing on `:8090` (one port, per-connection protocol)

The single `:8090` listener MUST serve, demultiplexed on the same port: (a) MCP
Streamable HTTP/JSON-RPC, (b) native gRPC with server reflection, and (c) MCP SSE
only if a live client still requires it. Each protocol is carried over its own
connection; adding a protocol MUST NOT add a port.

**Acceptance:**
- Over the same port, an independently authenticated canonical stateless MCP request
  completes `tools/list` without a legacy session, AND a separate gRPC connection
  completes reflection `list`; a real native Codex connection also completes
  `initialize`/`notifications/initialized` + `tools/list` in the same router with no
  proxy, listener, or authorization-bearing MCP session.
- The design decides SSE retain-or-remove; exactly one holds: (retained) an SSE
  subscription over `:8090` opens; or (removed) the acceptance test asserts no SSE
  route exists.

### FR-2a — MCP protocol conformance and canonical version

The MCP endpoint MUST declare a **single canonical MCP protocol version** and
implement its lifecycle; any legacy compatibility MUST be explicitly bounded. The
canonical version is the current MCP release (2026-07-28, stateless lifecycle;
https://blog.modelcontextprotocol.io/posts/2026-07-28/). The design MUST:

- adopt the 2026-07-28 **stateless** lifecycle and its required headers as canonical;
- accept native Codex `initialize` and `notifications/initialized` in the same bridge
  router as a bounded protocol-compatibility lifecycle, without a shim/proxy/listener
  or authorization-bearing server session;
- bind authentication to exactly one credential, never an MCP session id: the exact
  active-sled SID1 authenticates the protected local chatbot request, while a one-use
  OIA1 authenticates another caller's request. A stolen/replayed MCP session id is
  insufficient and every protected message still passes SEC-2.

The canonical stateless path MUST NOT create server-held authorization state and direct
requests remain independently valid. Native Codex may nevertheless send
`initialize`/`notifications/initialized`; the same router responds compatibly and then
continues stateless authorization. This compatibility is an in-router method behavior,
not a network shim, proxy, separate mode, second MCP endpoint, or identity authority.

Codex connects directly to `https://10.0.0.3:8090/mcp` and invokes
`op-identity-headers` through native per-request `http_headers_helper`. The helper reads
only the explicitly selected current sled's private `root:secrets` credential projection
(never public/legacy state) and emits only
`x-opdbus-sealed-id-bin` JSON containing its exact MutationEngine-authored SID1.
It owns no listener/transport and never rebuilds identity claims. OAuth is absent: no
authorization/token/callback/PKCE routes or same-endpoint OAuth-to-OIA exchange exist.

The full credential-bearing sled persists only in the root-only identity Cozo tree and
the `0640 root:secrets` private tmpfs projection. Public identity state and generic
D-Bus, StateSync get/subscribe, snapshot, method-result, Snowball/vector, schema, web,
log, and error surfaces recursively omit `sealed_id` and SID1 bytes.

The endpoint MUST also implement: protocol-version negotiation (reject unsupported
versions); JSON-RPC error objects; request cancellation; notifications; request
body-size limits and per-request timeouts; the browser policy of SEC-14;
SSE resumption (if SSE retained); pagination for `tools/list`; and `resources/list` +
`resources/read` for `blob://<plugin_id>` resources (FR-8). For ordinary MCP HTTP,
OIA1 and SID1 MUST each use canonical base64url-without-padding HTTP encoding with a
bounded decoded length; duplicates, non-canonical encodings, trailing bytes, dual
SID1+OIA1 credentials, and conflicting gRPC/HTTP representations are rejected. Native
gRPC retains its canonical `-bin` metadata encoding. The JSON-RPC body is authoritative
for method and target name; native Codex is not required to synthesize custom
`Mcp-Method` or `Mcp-Name` headers. `MCP-Protocol-Version` follows standard
initialize/subsequent-request behavior. No intermediary may translate a raw
self-asserted identity header into an accepted credential.

**Acceptance:**
- The server advertises exactly one canonical `protocolVersion` (2026-07-28) and
  rejects unsupported versions. There is no separate legacy stateful path or mode;
  native Codex lifecycle messages are handled in the same router.
- A request bearing a valid MCP session id but no exact active-sled SID1 or fresh OIA1
  is rejected; a replayed session id grants nothing.
- Direct stateless requests succeed with either accepted credential. Native Codex's
  initialize/initialized sequence succeeds in the same router and creates no standing
  authority; its helper is invoked per request and forwards the same immutable session
  SID1 until that sled is revoked, expired, inactive, or replaced.
- No OAuth support route, MCP shim/proxy process, alternate command, or second listener
  participates in the Codex path.
- `resources/list` includes `blob://` entries and `resources/read` returns a sealed
  plugin's resource.
- Duplicate/non-canonical/oversized SID1/OIA1 headers and dual credentials are rejected;
  SID1 is exact-matched to the active sled and OIA1 remains replay-protected.
- An oversized body and a slow request are bounded; cancellation follows the
  mutation-safe semantics of FR-3g/NFR-4; browser Origin/CORS behavior follows SEC-14.

### FR-3 — Unified surface; removing a service never removes its tools

All MCP capabilities MUST survive in the authoritative catalog spanning one bridge-owned in-process `ToolRegistry`
(registry tools) plus the shared sealed-plugin route surface (generated plugin
methods, FR-3a). Retiring any standalone service MUST NOT remove its typed underlying
capabilities. The combined surface MUST include: (1) the five HOT typed tools
`memory_recall`/`memory_store`/`workflow_query`/`workflow_run`/`toolsets`; (2) typed
agent tools; (3) cognitive tools; (4) code tools
`code_search`/`code_context`/`code_index`; (5) context-awareness services; (6) memory
tools; (7) blob-schema/catalog tools (FR-8); (8) Waypipe (FR-11); and (9) generated
plugin methods (FR-3a). The former generic compact meta-tools are an intentional
surface removal, not a lost capability: `toolsets` reaches authorized typed tools.

Presence in the authoritative catalog does not imply visibility to every caller. The
audience/temperature projection in
`standalone-emqx-identity-mcp` FR-12 through FR-15 is the newer authority for client-visible
`tools/list` and `tools/call`: exactly one configured chatbot `principal_id` begins
with exactly the five typed HOT tools, while other agents receive their capability-
authorized HOT subset. `toolsets` selects exactly one typed WARM/COLD set at a time.
No principal sees or calls `list_tools`, `search_tools`, `get_tool_schema`,
`execute_tool`, or `invoke_tool` on MCP.

**Acceptance:**
- With all retired standalone MCP services stopped, an internal catalog regression
  test enumerates the pre-consolidation tool/method names and asserts each remains in
  the one bridge-owned catalog.
- Audience-projected discovery never exposes every category at once; the exact five-
  tool singleton default and authorized one-set-at-a-time typed selection prove the
  preserved capabilities are reachable without a generic executor or another listener.

### FR-3a — Generated plugin methods are routes, not registry tools

Generated sealed-plugin methods MUST remain gRPC routes dispatched through
`PluginService.CallMethod` → `MutationEngine` (they are NOT `ToolRegistry` entries).
For MCP clients, the design MUST provide an explicit **read/execute MCP adapter**
over these sealed-plugin methods that presents them through the MCP surface while
dispatching to the existing route path — it MUST NOT copy them into the registry or
create a parallel executor. The adapter MUST enter `PluginService.CallMethod` and the
D-Bus `MutationEngine` path in-process; loopback gRPC/HTTP and direct adapter-to-tool
execution are forbidden.

**Acceptance:**
- No code registers generated plugin methods as `ToolRegistry` tools.
- An MCP client can discover and call a generated plugin method via the adapter, and
  the call flows through `PluginService.CallMethod` (same capability + validation +
  event-chain path). The MCP adapter enters the canonical
  D-Bus/`PluginService`/`MutationEngine` path and MUST NOT call the `ToolRegistry`
  directly as a parallel control plane (registry-tool dispatch also carries the
  ExecutionContext of FR-3d and the same auth pipeline of SEC-2).
- A route-spy test proves every direct typed MCP `tools/call`
  crosses the canonical `PluginService.CallMethod`/MutationEngine admission point
  exactly once before any tool implementation runs.

### FR-3b — Canonical tool descriptor and per-tool authorization

Every executable registry tool MUST have an authoritative method declaration in the
manifest-selected sealed `PluginSchema`. The runtime `Tool`/`ToolDefinition`
descriptor is derived from that `MethodDecl` and carries `required_capability`, OSCAL
`subid`, `side_effect` (Read/Mutation), `idempotent`, `input_schema`, `output_schema`,
and `approval_required`; tool implementation code MUST NOT independently redefine
those security fields. Registration MUST reject a missing declaration or any mismatch
between implementation projection and sealed declaration. The MutationEngine, before
selecting `ToolRegistry::execute`, MUST enforce the **target tool's**
`required_capability` against the caller's resolved identity. A broad category or
legacy invoke capability MUST NOT authorize every registered tool; each directly
called typed tool is authorized by its own `required_capability`.

**Acceptance:**
- Every registered tool exposes a descriptor with all fields populated (a test
  fails if any tool lacks `required_capability` or `subid`).
- Every descriptor is byte/semantically equal to its manifest-selected sealed
  `PluginSchema.methods` declaration; missing/mismatched declarations fail
  registration before the route is advertised.
- An identity granted a broad invoke capability but NOT `agent_shell_executor_exec`'s
  specific capability is denied when it targets that tool, before execution.
- A tool call's authorization decision is recorded in the event chain.

### FR-3d — Immutable bridge-created ExecutionContext

The bridge MUST construct an **immutable ExecutionContext** for every tool/method
invocation and pass it to execution; tools MUST NOT receive only caller-controlled
JSON (the current `Tool::execute(&self, input: Value)` at
`crates/op-mcp/src/tool_registry.rs:41` is insufficient). The ExecutionContext MUST
carry, all set by the bridge (never by the caller):

- actor and resolved identity (from `identity_sled`);
- container / workspace / session scope (derived, see FR-3f/SEC-6);
- the granted and the selected capability;
- trace / event correlation ids;
- parent invocation id and bounded delegation depth;
- deadline and cancellation token;
- verified approval (when the target tool is `approval_required`, FR-12/SEC-10);
- transport binding (TCP source binding or UDS peer credentials, TR-2).

Each direct typed `tools/call` receives a target-specific ExecutionContext constructed
by the bridge after projection and exact target-capability resolution. `toolsets`
changes only the selected view and never dispatches a nested target, constructs a
child context, adds a grant, or changes scope. The former generic MCP meta-dispatch
path is removed.

**Acceptance:**
- `Tool` execution receives an ExecutionContext the caller cannot forge or mutate; a
  test asserts a caller cannot inject/override any ExecutionContext field via
  `arguments`.
- A direct typed invocation carries the server-selected target capability and cannot
  widen actor, transport, scope, deadline, approval, or grants; `toolsets` selection
  does not execute a target or create another ExecutionContext.

### FR-3e — Output-schema validation before use

A tool's produced output MUST be validated against its `output_schema` (FR-3b)
**before** the result is returned to the caller, persisted to Cozo, vectorized into
Qdrant, or inserted into a prompt. Validation failure fails closed for the output (the
result is not returned/persisted/vectorized/injected), but it MUST append a redacted
`invalid_output` outcome to the already-created audit intent (SEC-9/DR-5); audit
evidence is never suppressed by output-validation failure. A mutation MUST use a
prepare/validate/commit contract or transactional adapter so externally visible state
is not committed before its typed output is validated. Where an external system cannot
participate transactionally, the intent record MUST declare that limitation and the
failure outcome MUST record `partial_side_effect` plus a reconciliation action.

**Acceptance:** a tool returning output that violates its `output_schema` yields an
error and produces no returned/persisted/vectorized/prompt-injected output artifact,
but does produce a redacted `invalid_output` audit outcome linked to its intent. A
mutation test proves malformed output cannot commit prepared state; a non-transactional
fixture proves a partial side effect is classified and queued for reconciliation.

### FR-3g — Durable idempotency and mutation-safe cancellation

Every mutation/tool call MUST carry a bridge-issued or validated operation id bound to
the actor, derived scope, target subid, canonical input hash, and policy/schema version.
Before dispatch, the bridge MUST atomically persist an audit/idempotency intent. For an
`idempotent=true` declaration, a retry with the same operation id and identical binding
returns the recorded result without repeating the side effect; reuse with different
input/scope/target is denied. For `idempotent=false`, a repeated operation id is denied
and never re-executed. This admission/deduplication state MUST survive restart.
Cancellation or deadline expiry MUST stop admission of uncommitted work, propagate to
the implementation, and record whether the operation was cancelled-before-commit,
committed, or requires reconciliation; cancellation MUST NOT claim rollback of an
already committed external effect.

**Acceptance:** transport retry and reconnect tests execute one side effect and produce
one intent with linked outcomes; a changed-input operation-id reuse is denied; both
idempotent and non-idempotent behavior survive bridge restart; cancellation races at
pre-dispatch, prepared, and committed stages produce the specified outcome without an
untracked side effect.

### FR-3f — Bridge-derived scope; arguments may narrow, never replace

Memory, context, coding, and semantic-query scope (container, identity, namespace,
workspace, collection, session id, path root) MUST be derived by the bridge from the
ExecutionContext (FR-3d), NOT accepted as authoritative from caller `arguments`.
Caller arguments MAY **narrow** within the authorized scope but MUST NOT **replace**
or widen it. Coding tools additionally MUST enforce a workspace root, canonicalize
paths, reject path traversal and symlink escape, authorize the target collection, and
bound archive/input sizes.

**Acceptance:**
- Forged-scope tests for reads, writes, deletes, context streams, semantic queries,
  and coding tools: a caller supplying another identity's `container_id`/`namespace`/
  `workspace`/`collection`/`session_id` is scoped to its OWN authorized scope (or
  denied), never the forged one.
- A coding tool given a traversal path (`../`), an absolute path outside the workspace
  root, or a symlink escaping the root is rejected; an oversized archive is rejected.

### FR-4a — Authoritative capability registry; no self-asserted authority

Tool/method `subid` MUST resolve against `oscal_subid_registry.rs`, and its
`required_capability` MUST resolve to a `CapabilityDecl` in the same
manifest-selected sealed `PluginSchema.capabilities`; cross-plugin capability reuse
must resolve to the identical canonical declaration. Those declarations define the
capability vocabulary, never identity grants. Registration of a tool/method/plugin
whose capability/subid is missing, conflicting, or unknown MUST be rejected. The
caller-supplied `x-opdbus-capability` header MUST NOT
be authoritative — it may express intent, but authorization derives from the resolved
identity's grants against the target's `required_capability`. The degraded path
"`identity.is_some() && capability_matches` when no grants are declared" MUST be
removed: a method/tool with no resolvable grant is denied, not allowed.

Wildcard `"*"` identity grants MUST be prohibited **without exception** in
`/dev/shm/opdbus/capability-grants.json`, inside sealed
`PluginSchema.capability_grants`, and in every fallback/materialized grant source.
Public liveness/onboarding is expressed by an exact route allowlist, never a wildcard
principal or capability grant. Moreover, `PluginSchema` declares capabilities that a
method **requires** but MUST NOT be an authority that **assigns** grants to identities:
all grants derive from the protected principal-grant projection keyed by an exact,
registered `principal_id`. `identity_sled` supplies the principal's current
session/genesis context but does not assign grants.
Legacy `PluginSchema.capability_grants` data MUST be ignored for authorization and
removed/resealed; encountering it during activation is a migration error, not a grant.
Hand-written services mounted by the shared route builder — context, Waypipe,
registration, health — MUST also carry capability, `side_effect`, `subid`, and
`approval_required` metadata and be authorized like any other method.

**Acceptance:**
- Registering a tool/method with an unknown capability or subid fails.
- A request whose only claim to a capability is the `x-opdbus-capability` header
  (without a resolved grant) is denied.
- A semantic CI/runtime gate fails on any `"*"` identity grant or any non-empty sealed
  `PluginSchema.capability_grants`, regardless of capability category; liveness and
  onboarding tests pass solely through the exact public-route allowlist.
- A per-principal or per-footprint grant embedded in a sealed schema does not authorize
  execution; only the protected principal-grant projection entry for the caller's exact
  registered `principal_id` does.
- Context/Waypipe/registration/health methods each expose capability + subid +
  side_effect + approval metadata.

### FR-4b — Principal identity is authority; footprint is sled payload only

The authorization namespace is the stable `principal_id` resolved from the
authoritative HumanPrincipal/service-principal registry. `session_id` is correlation;
`session_genesis` anchors one session term. Neither is a capability key. A
`PluginFootprint` is exclusively the canonical per-mutation payload envelope emitted
by the sled and delivered by the Shuttle to the Snowball hash-chain append path and
asynchronous vectorization.

The resolved `principal_id` MUST be injected into request/event context as
`actor_id`, then copied into footprint metadata alongside `session_id` and
`session_genesis`. No principal, grant, audience, SID1 claim, D-Bus binding,
tool-set handle, or cache scope may be derived from a footprint, genesis, chain head,
`data_hash`, `content_hash`, `event_hash`, or another digest. In particular, hashing a
footprint/hash to manufacture an identity is forbidden.

Snowball is the sole chain-hash author. It computes the current event receipt once
from the domain, previous link, and canonical payload bytes. The sled/Shuttle MUST NOT
prehash the current footprint/payload and ask Snowball to hash that digest again.
Vectorization consumes deterministic canonical payload text and records the returned
`event_id`/`event_hash` as provenance; a digest-only body is insufficient. The current
`ChainEvent.json_args_footprint` prehash is removed; bounded/redacted canonical
arguments are part of the footprint payload instead. Explicit digests of external
state/reference artifacts may remain clearly named provenance fields, but they are
not footprints and do not replace the current payload.

**Acceptance:**
- `HumanPrincipalIdentity.footprint`, `derive_human_footprint`, auth-facing
  `GhostbridgeIdentity.footprint`, footprint-keyed `AuthenticatedCaller`, and
  `capabilities_for_footprint` are removed or migrated to explicit principal/session/
  genesis fields.
- Production grant loading accepts registered `principal_id` keys only and rejects
  wildcard, 64-hex footprint, genesis, and other digest keys.
- Two sessions for one principal retain the same grants/audience while receiving
  distinct session/genesis/footprint records; identical payload digests owned by two
  principals do not transfer authority.
- Exactly one canonical footprint payload type feeds Snowball and vectorization;
  Snowball authors one current-event hash and no `json_args_footprint` or legacy
  `data_hash → content_hash` current-payload hash-of-hash generator remains.
- MutationEngine authors SID1 once from the immutable session identity/anchor and
  stores it in that sled; no footprint/hash is an SID1 input or authorization key.
- The A.N.N.A. Scribe persona and established email identity remain intact. Only its
  unsafe duplicate `/dev/shm/plugin_schema.dat` mmap reader and duplicate derivation
  are removed; any needed read uses the canonical selected-sled projection.

### FR-3c — Registration rejects all name collisions

`ToolRegistry::register` MUST reject any duplicate tool name (not silently
overwrite), for all tools — not only `blob_catalog`.

**Acceptance:** registering two tools with the same name returns an error and leaves
the first registration intact; a test asserts this for an arbitrary name.

### FR-4 — Client "modes" are capability-filtered views, not services

HOT/default and named WARM/COLD tool-set behavior MUST be projections of the same
surface, computed after resolving the caller's identity. The effective view is the
intersection of exact capabilities, server-side audience policy, HOT/default or one
selected set, and provider health. The former compact mode/meta-tool surface is
retired. No separate network service or execution engine per mode or tool set exists.

**Acceptance:**
- Two identities with different granted capabilities calling `tools/list` receive
  different filtered sets from the same registry instance.
- A tool-set selector can only narrow exact grants; a guessed call outside the active
  projection is denied.
- CI gate (scoped to production execution ownership, NFR-7): no production code path
  constructs a second authoritative `ToolRegistry` or a per-mode network listener.
  (Test-only and isolated-library registries are out of the gate's scope.)

### FR-4c — Provider-backed WARM sets are supervised, pinned, and truthfully gated

NotebookLM and `mongodb-mcp-server` MUST be independent runit-supervised local
providers, installed from version-and-SHA256-pinned release artifacts rather than
`npx @latest`. They MUST bind Streamable HTTP only on loopback and MUST be reachable
from clients only through the bridge's one authenticated `:8090/mcp` endpoint. The
bridge owns the typed `plugin.notebooklm.*` and `plugin.mongodb_mcp.*` adapters and
dispatches them through the same capability, schema-validation, identity-metadata,
and MutationEngine audit path as every other plugin method.

Service health and upstream authentication are separate facts:

- `notebooklm_auth` contains only `get_health` and `setup_auth` and requires the
  healthy provider marker; `notebooklm_research` requires a distinct marker produced
  only when NotebookLM reports `authenticated=true`. Interactive authentication runs
  as the desktop user on the protected CRD display and persists only the provider's
  Chrome profile, never a cookie copied into bridge configuration.
- `mongodb_knowledge` may use the healthy read-only provider without a database URI.
  `mongodb_data` requires a distinct authenticated marker created only after a real
  read-only `list-databases` probe succeeds against the configured `preconfigured`
  connection. A merely present URI MUST NOT satisfy this gate.
- MongoDB runs with read-only mode, server-side JavaScript disabled, request
  overrides disabled, and telemetry disabled. Its credential source is a protected
  root-owned environment file, never a tool argument, manifest value, or log field.
- Local `context_code` and `context_knowledge` sets remain in-process WARM sets.
  None of these providers or health checks participates in constructing or serving
  the five HOT tools.
- EMQX is not an MCP provider endpoint. It is a standalone internal broker attached
  through loopback MQTT and dedicated authenticated ExHook; it supplies asynchronous
  health/catalog/hook signals only and remains behind the `10.0.0.3:8090` fabric.

**Acceptance:** both services survive restart under `sudo sv`, expose only loopback
listeners, answer an MCP initialize/list/call probe, and remove readiness markers on
exit. With no MongoDB credential, `mongodb_data` is absent while
`mongodb_knowledge` remains selectable. With NotebookLM logged out,
`notebooklm_auth` remains selectable while `notebooklm_research` is absent. No
provider process, profile, credential, or additional URL appears in client MCP
configuration.

### FR-5 — `toolsets` replaces the compact lazy loader

The one configured singleton chatbot initially discovers exactly
`memory_recall`, `memory_store`, `workflow_query`, `workflow_run`, and `toolsets`.
`toolsets` lists authorized sets and selects exactly one typed WARM/COLD set; after
re-list, the view is exact grants ∩ audience ∩ (HOT ∪ selected set) ∩ provider health.
Selection grants nothing and cannot union multiple sets. The former
`list_tools`/`search_tools`/`get_tool_schema`/`execute_tool` surface and all
`invoke_tool` aliases are absent from active MCP discovery and denied by name for
every principal.

Drill-down requires explicit MCP client support. `toolsets(select)` returns the
principal/client/catalog-bound selector and `relist_required=true`; the client—not the
model—must capture it, issue a new `tools/list` with the selector in negotiated `_meta`,
and carry it on later calls. Schemas returned inside a tool result are informational
and do not register callable tools. A client that does not implement selector/re-list
behavior remains on its HOT default and MUST NOT be reported as supporting drill-down.

**Acceptance:**
- The singleton's initial `tools/list` is exactly the five HOT typed names.
- Selecting one authorized set and re-listing returns the HOT base plus only that
  set's authorized healthy typed tools; selecting another replaces it.
- A real-client capability test proves Codex/Grok selector/re-list support before
  claiming drill-down E2E; an unsupported client remains on HOT five with a typed
  `client_relist_required`/unsupported result and no hidden tool becomes callable.
- A guessed typed tool outside the active projection and every former compact/generic
  name are denied, even when client/model metadata is spoofed.

### FR-5a — Direct typed tool-argument validation (fail closed)

For every direct `tools/call`, the bridge resolves the selected typed tool's
`input_schema` and validates its `arguments` before execution. Invalid arguments MUST
NOT reach the tool. A missing or invalid target schema fails closed. `toolsets`
selection itself has a typed bounded schema and never carries nested tool arguments.

**Acceptance:**
- A direct typed call whose arguments violate its schema is rejected before the tool
  runs (no side effect; the event chain records the rejection).
- A tool whose schema cannot be resolved cannot be advertised or invoked.

### FR-6 — In-process cognitive registry ownership; no HTTP loopback

The bridge MUST construct and own one cognitive execution runtime and the one
canonical `ToolRegistry`. All registry-tool calls MUST first enter the canonical
D-Bus `PluginService.CallMethod` → `MutationEngine` path; only after that admission,
capability, schema, idempotency, and audit-intent processing may the MutationEngine
select the in-process `ToolRegistry::execute` implementation. The HTTP loopback to
`http://10.200.0.2:3003/mcp` MUST be removed. Direct MCP/gRPC-adapter-to-registry
execution is equally forbidden: in-process describes implementation locality, not a
second execution authority.

**Acceptance:**
- `rg -n '10.200.0.2:3003|cognitive_mcp_endpoint|reqwest' crates/op-grpc-bridge/src`
  returns no HTTP-dispatch match.
- An authenticated direct typed tool call succeeds while no process listens on `:3003`.
- A test fails if any production network adapter invokes `ToolRegistry::execute`
  without first crossing `PluginService.CallMethod`/MutationEngine, or if one logical
  call crosses that mutation admission point more than once.

### FR-7 — Correct code-tool routing

`code_search`, `code_context`, `code_index` MUST invoke the real code tools
(`code_tools.rs`), NOT be translated into `search_blob_vectors`/`refresh_blob_vectors`.

**Acceptance:**
- `rg` shows no mapping of `code_*` to `*_blob_vectors` in the bridge.
- A direct `tools/call` of `code_search` returns `CodeSearchTool`-shaped output,
  not blob-vector search output.

### FR-8 — Consolidated sealed blob catalog / blob-schema MCP

One canonical semantic contract MUST cover: `blob_catalog` (modes `list`,
`summary`, `full`; optional plugin-ID and category filters; `full` returns every
active **sanitized public schema projection**, FR-8b); `blob_schema(plugin_id)`;
`blob_manifest(plugin_id)`;
`blob_methods(plugin_id)`; `blob_search(query)`; MCP resources `blob://<plugin_id>`
(via `resources/list`/`resources/read`, FR-2a). The duplicate `blob_catalog`
implementations MUST be resolved into one. Source of truth is
`/dev/shm/opdbus/plugin-blobs/`; `op-blob` is the only writer; consumers are
read-only.

**Acceptance:**
- Exactly one `blob_catalog` tool is registered (registration test asserts
  singularity, enforced by FR-3c).
- `blob_catalog(mode="full")` covers every plugin id in the sealed manifest, with a
  bounded or streamed response (FR-8a).
- `blob_schema`/`blob_manifest`/`blob_methods`/`blob_search` and the `blob://`
  resource each return data for a known sealed plugin.
- Ordinary authenticated callers never receive embedded grants, private principal/session/footprint metadata, secrets,
  private examples/defaults, or tenant-private metadata through any blob/reflection
  surface (FR-8b).
- No consumer writes to the blob dir.

### FR-8a — Blob-vector semantics preserved

The consolidation MUST preserve: explicit user-triggered refresh only (no
background/automatic refresh); deterministic point IDs (UUIDv5 from `plugin_id`);
one point per active sealed plugin; defined handling for removed plugins
(wholesale replace-on-refresh, stale points overwritten/removed); manifest
generation / catalog-hash consistency between the vectors and the sealed catalog;
and bounded or streamed `blob_catalog(mode="full")` responses.

**Acceptance:**
- Running `refresh_blob_vectors` twice yields the same point count (no duplicates)
  and reflects the current manifest; there is no timer-driven refresh
  (`rg` finds none).
- `blob_catalog(mode="full")` does not return an unbounded single payload — it is
  paginated or streamed.
- Refresh is **atomic** (DR-8): an interrupted refresh leaves the previous vector
  generation active and queryable.

### FR-8b — Sanitized public schema projection; raw access is administrative

Reflection, `blob_catalog`, `blob_schema`, `blob_manifest`, `blob_methods`,
`blob_search`, and `blob://` resources MUST expose a single deterministic **public
schema projection**, not the raw sealed JSON. The projection includes callable method
shapes, public descriptions, side-effect/idempotency/approval metadata, subids, and
required capability names, but strips `capability_grants`, private principal/session/footprint metadata,
secret/sensitive defaults and examples, private tenant/org metadata, filesystem
secrets, and deployment-only fields. Sanitization runs before pagination, search,
vectorization, caching, logging, or response generation. Raw sealed-schema bytes, if
operationally required, are available only through a separate administrative method
with its own capability, mutation-free descriptor, explicit reason, and audit event;
they are never returned by `full` mode or a normal MCP resource.

**Acceptance:** fixtures embed a footprint grant, API-key sentinel, private org value,
and secret default/example; none appears in ordinary reflection, any blob tool,
`blob://`, vector payloads, logs, or caches. An ordinary schema-read identity is denied
raw access; a specifically authorized administrator can read the exact manifest-pinned
raw schema and the access is audited. Projection output is deterministic for the same
schema hash.

### FR-9 — Reflection / callability parity (both directions) and hot-seal bounds

No RPC/method may be advertised through reflection unless its implementation is
mounted and callable, **and** no mounted method may be absent from reflection —
parity holds in **both** directions (reflected ⇒ mounted, mounted ⇒ reflected).
Static reflection (build-time proto from PluginSchema methods) and dynamic reflection
(hydrated from the sealed catalog) MUST stay in parity with the mounted callable
routes (owned by `schemars-to-reflection-plugin-pipeline`).

Because per-method typed gRPC routes are generated **statically at build time**
(`build.rs`), the "sealed ⇒ callable" expectation MUST be bounded precisely:
- a newly sealed method **shape** that was not compiled in requires a
  **rebuild/redeploy** to become callable on the typed generated service;
- **compatibility / schema-hash validation** MUST run **before** a blob is activated;
  a mismatched or incompatible blob fails closed and is not activated;
- activation MUST atomically select one manifest generation and compare each method's
  canonical contract hash (input/output/capability/subid/side-effect/idempotency/
  approval) with the compiled mounted descriptor; a partially validated generation is
  never visible to routing, reflection, MCP listing, or schema reads;
- **compiled-but-unsealed** methods MUST remain **uncallable** (compilation alone
  does not activate a route; sealing is required);
- dynamic reflection MUST NOT advertise a method that has no mounted, callable route
  (no advertise-then-UNIMPLEMENTED).

**Acceptance:**
- For every RPC returned by reflection at `:8090`, an authenticated call reaches a
  mounted route (normal response or typed application/`InvalidArgs`/capability error
  — never "unimplemented"/route-not-found), and every mounted method appears in
  reflection.
- A sealed blob whose method shape was not compiled in is either not advertised by
  reflection or is flagged as requiring redeploy — it is never advertised as callable
  while returning UNIMPLEMENTED.
- Activating a blob with an incompatible/mismatched schema hash is rejected
  (fail-closed) before it can serve.
- A mixed/partial manifest generation and a contract whose shape matches but security
  metadata differs are rejected; all consumers observe either the prior fully active
  generation or the new fully validated generation.
- A compiled-but-unsealed method is not callable.

### FR-9a — Reflection visibility decision

Reflection/schema visibility is explicit: **authenticated callers may see the
complete sanitized public schema/reflection projection (FR-8b), while execution
remains capability-gated per target (FR-3b).** Public method shapes describe
services/capabilities, not identities; raw sealed schemas and grant/tenant/secret
metadata are not part of this visibility rule. Authorization is enforced at
execution, and any *listing* filtered by capability (FR-4) is a convenience view, not
the execution boundary.

**Acceptance:**
- An authenticated caller can retrieve the full **sanitized public** catalog via
  reflection;
  attempting to execute a method it lacks capability for is denied (FR-3b).
- The raw-schema sentinel tests of FR-8b pass for reflection as well as blob tools.
- There is no requirement or test asserting "identity A cannot read identity B's
  schemas" (that framing is retired as not meaningful).

### FR-10 — Context-awareness relocated onto `:8090`, event-driven

Context snapshot, activity recording, on-demand knowledge push, and streaming
notifications MUST be served through authenticated services at `:8090`. The
independent HTTP/SSE context server MUST be removed. Proactive evaluation MUST be
event-triggered (replacing the 5 s `EVALUATION_INTERVAL_MS` poll); the ban is on
periodic state-discovery/evaluation polling, not on legitimate timers (deadlines,
expiry, rate limits, SSE heartbeats). Context MUST be session- and identity-scoped
and bounded by an explicit prompt/token budget. All existing signals MUST remain:
file opened, edit applied, build error, test failure, diff viewed, symbol
navigation, tool call, query, context switch, stuck-session detection, error
assistance, topic changes, idle recovery.

**Acceptance:**
- No process other than the bridge serves a context stream; `build_context_router`
  is not mounted on an independent listener; context routes resolve under `:8090`.
- A recorded `build error` triggers a push produced by the event, not by a fixed
  interval (a test asserts event causation).
- A context snapshot never exceeds the configured token budget; a request scoped to
  identity A never returns identity B's context.

### FR-11 — Waypipe served on `:8090` (retained)

Waypipe functionality MUST survive and be reachable through the bridge's unified
route surface on `:8090` (mounted through the shared route builder), NOT through a
standalone `op-waypipe-grpc` listener on `:50052`. (Decision: retain, not retire.)

**Acceptance:**
- `ss -lntp` shows no `:50052` listener.
- Waypipe RPCs are reachable through `:8090` (reflection + authenticated call), with
  no orphaned Waypipe route lacking an implementation (FR-9).

### FR-12 — Coding-assistance workflow (suggestion vs application separation)

The workflow MUST: (1) gather workspace/session context; (2) retrieve relevant code,
sealed schemas, live projections, and prior outcomes; (3) produce a structured
coding suggestion with evidence and a proposed diff; (4) keep suggestion generation
separate from mutation/application; (5) require the apply capability **and** a signed
single-use approval bound to the exact change (SEC-10) to apply; (6) record accepted,
rejected, corrected, failed, and successful outcomes; (7) feed **independently
verified** outcomes (not chatbot self-labels) into future retrieval and ranking.
The suggestion contract is **mandatory** for any change-applying tool (a typed
input/output; workspace-root and path rules per FR-3f) and MUST NOT add a listener or
a second registry. Application-capable tools carry `approval_required` (FR-3b) and
enforce SEC-10.

**Acceptance:**
- A suggestion request returns a structured suggestion (evidence + proposed diff)
  WITHOUT applying a change; the suggestion is emitted via the mandatory contract.
- Applying requires the apply capability AND a valid single-use approval token bound
  to the suggestion/diff-hash/base-revision/actor/session/expiry/nonce (SEC-10); a
  suggestion-only identity, or a request with a mismatched/absent/expired/reused
  approval, is denied.
- Each of accepted/rejected/corrected/failed/successful is recordable and readable;
  promotion uses independently verified outcomes.

### FR-13 — Chatbot memory recall before a turn

Before a turn, retrieve identity-scoped memory, rank semantically against the
current prompt, inject only relevant top-ranked memories. Every injected memory MUST
carry provenance: source, identity/container, workspace, timestamp, confidence,
event ID/hash.

**Acceptance:**
- A turn for identity A retrieves only A's memories (SEC-6); the injected set is
  bounded (top-k) and each item carries all six provenance fields.
- A semantically related memory ranks above an unrelated one for the same prompt.

### FR-14 — Post-turn outcome storage (replaces regex heuristic)

After a turn, persist useful user facts, decisions, corrections, tool results, and
outcomes with structured extraction replacing the `memory_loop.rs` regex. Tool-call
arguments and results MUST be persisted only as the typed, minimized, secret-redacted,
size-bounded memory records of DR-6/SEC-11; raw payload persistence is forbidden.
Accepted/rejected/corrected suggestion feedback MUST be explicitly modeled, and trust,
domain, actor, and scope fields MUST be assigned by the bridge rather than extracted
from model/caller prose.

**Acceptance:**
- After a turn with a tool call, the persisted memory contains the tool name and typed
  redacted/minimized argument/result summaries (not a regex substring or raw secret).
- An accepted and a rejected suggestion each produce a distinct, queryable record.

### FR-15 — Memory lifecycle and consolidation (cross-store consistent)

Deduplication, correction, deletion, expiry, and confidence decay MUST be supported,
and repeatedly successful episodic lessons promoted to durable semantic memory. All
lifecycle operations MUST keep the durable store (Cozo) and the semantic index
(Qdrant) consistent: deletion/correction writes tombstones and reconciles the vector
index; derived/semantic memory is invalidated when its source changes; failures to
update one store are retried/reconciled (fail toward durable truth, DR-4).

**Acceptance:**
- Duplicate store does not create a second record; correction supersedes prior value
  and invalidates derived memory; expiry/decay reduces confidence; a repeatedly
  successful lesson is promoted.
- After a deletion, no caller ever receives the item or a tombstone. Qdrant removes
  the derived point when reconciliation succeeds; until then the durable Cozo
  tombstone suppresses every vector hit. A retry/reconciliation path exists for a
  transient Qdrant failure.

### FR-16 — Cognitive evolution loop, bounded

The loop MUST be observe → retrieve context+memory → suggest/act → verify → capture
feedback+tool outcome → consolidate lessons → improve retrieval/ranking. It is
memory/retrieval/policy-support evolution only. It MUST NOT modify model weights,
governing policy, capabilities, or authentication rules. Model fine-tuning requires
a separate reviewed dataset and approval process.

**Acceptance:**
- A closed-loop test shows a captured outcome changing a later retrieval/ranking
  (CR-2) while invoking **no** model training/fine-tuning/update API (assert no such
  API call is made — not a byte-comparison of remote weights) and making no change to
  capability grants or auth config.

### FR-17 — Shared route builder mounts everything

One shared route builder (`build_operation_routes`) MUST mount: generated plugin
RPCs, the cognitive/MCP protocol adapter (FR-3a), context streaming, Waypipe,
registration, health, and authenticated reflection. No surface may be mounted by a
second, independent builder. Every executable route from that builder delegates to the
single D-Bus `PluginService.CallMethod`/MutationEngine admission path; route presence
does not authorize a direct executor.

**Acceptance:** a test asserts each listed surface is present in the shared
builder's output and that no surface is mounted elsewhere.

### FR-18 — Only a minimal liveness endpoint (and defined onboarding) is public

The complete public allowlist is exactly: minimal `GET /healthz` and
`POST /genesis/complete`. Tool discovery, schemas, reflection, context streams,
memory, execution, and every other registration method require authentication.
`/genesis/complete` MUST accept only a canonical, versioned **OIG1 Oracle Identity
Genesis envelope** signed by a trusted Oracle-decoy key after the decoy has verified
the WireGuard peer and its human/key ownership. Its signed fields include the human
public key, WireGuard inner address, intended principal/session identifiers or their
derivation inputs, decoy key id, issued-at, expiry (maximum 15 minutes), one-use nonce,
and protocol purpose/domain. The bridge performs canonical parse, trust-key/signature,
time, transport binding, proof-purpose, and atomic durable nonce consumption before it
anchors identity/session genesis. Unsigned/self-asserted keys, raw email/user aliases,
and caller-selected principal/session ids are rejected. Responses are
anti-enumerating and reveal neither whether a key/email exists nor another principal's
state. The endpoint is body-bounded, exact-Origin/CSRF protected for browsers,
rate-limited (SEC-13), and emits non-actor pre-auth telemetry. It grants **no** tool
discovery/execution capability. Existing unauthenticated `SendMagicLink` /
`VerifyMagicLink` or bootstrap interceptors MUST be removed from the bridge public
surface or implemented entirely on the Oracle-decoy side that issues OIG1; they MUST
NOT remain additional bridge bypasses.

**Acceptance:**
- Unauthenticated liveness succeeds; unauthenticated `tools/list`, reflection,
  context stream, memory, and every typed tool call are rejected before dispatch (SEC-2).
- Route enumeration proves only `/healthz` and `/genesis/complete` are public.
  `/genesis/complete` is rate-limited and cannot reach tool discovery/execution; its
  pre-auth events are telemetry, not actor-attributed event-chain records.
- Negative tests cover unsigned/unknown-key/bad-signature/expired/replayed/wrong-source/
  wrong-purpose OIG1, caller-chosen identity ids, duplicate registration, Origin/CSRF,
  and enumeration attempts; no public `SendMagicLink`, `VerifyMagicLink`, admin
  registration, reflection, or schema route remains.

### FR-19 — Context idle recovery and stream resumption

Context idle recovery MUST use **per-session one-shot deadlines** (a single fired
recovery per idle episode, not a repeating timer — consistent with NFR-2), and
context streams MUST support **restart/resume cursors** backed by a bridge-owned
durable context journal so a dropped stream or bridge restart resumes from its last
delivered position without replaying the whole history or losing events. Journal keys
and monotonic sequence numbers are scoped by server-derived
`(identity,container,workspace,session)`; payloads are schema-validated,
secret-redacted, size-bounded, and retained under an explicit TTL/size policy. Resume
cursors are opaque, integrity-protected, expiry-bound, and bound to that exact scope;
a cursor is never authorization. If retention has removed the requested sequence, the
server returns an explicit `CursorExpired` plus a bounded sanitized checkpoint/summary
rather than silently skipping or replaying another scope's events.

**Acceptance:**
- An idle session fires exactly one recovery per idle episode (no repeated
  timer-driven pushes); a returning session re-arms.
- A context stream reconnect resumes from the last cursor (no gap, no full replay).
- After bridge restart, a retained cursor resumes from the durable journal; identity B
  cannot use identity A's cursor, a forged cursor is rejected, and an expired cursor
  returns the specified `CursorExpired` response.

---

## Security / Identity Requirements

### SEC-1 — TLS transport at the ingress

`:8090` MUST require TLS; plaintext refused.
**Acceptance:** plaintext dial fails; TLS dial succeeds.

### SEC-2 — Canonical identity pipeline on every request

Every MCP, registry-tool, and generated-plugin request MUST pass, before dispatch:
(1) transport authentication (TLS for TCP; peer-credential binding for UDS, TR-2);
(2) exactly one identity credential; (3a) for local SID1, canonical decode/integrity,
MCP-scope, derived-identifier, revocation, current active-sled exact-byte/claim match;
or (3b) for OIA1, signature/expiry/activity, non-mutating replay lookup, and transport-
appropriate binding; (4) exact registered principal and per-session `identity_sled`
resolution; (4b, OIA1 only) atomic check-and-consume in the independent durable replay
store of SEC-13; (5) **target** method/tool capability check (FR-3b); (6) direct typed
input-schema validation (FR-5a); (6b) durable idempotency/audit intent (FR-3g/DR-5); (7)
canonical D-Bus `PluginService.CallMethod`/MutationEngine dispatch; (8) typed output
validation, commit/reconciliation classification, and linked event outcome; (9)
response carrying trace/event identity where applicable. The assertion mechanics are owned by
`netmaker-xray-identity-handoff`.

**Acceptance:**
- Unauthenticated, fake, expired, inactive, replayed, and unauthorized requests each
  fail **before dispatch** (no tool executes, no memory read/write occurs).
- A successful call's response and event carry non-zero `event_id`/non-empty
  `event_hash`.
- Ordering tests prove SID1 canonical decode/seal → derived identifiers → active-sled
  exact match/revocation → exact principal grants, and separately prove OIA1 signature
  → expiry → replay lookup → binding → principal/sled resolution → atomic replay
  consume. Both then run capability → input validation → intent/idempotency admission
  → D-Bus dispatch → output validation/commit → audit outcome. A wrong-source OIA1
  cannot burn its nonce, and replay-store unavailability fails OIA-authenticated
  requests closed.

### SEC-3 — No wildcard/self-asserted/sentinel identity; no local bypass

Authentication MUST NOT rely on sentinel footprints, wildcard identities, any `"*"`
identity/capability grant,
`plugin_schema.dat`, raw self-asserted identity headers, or an implicit "trusted
local" bypass. Local UDS callers are subject to the same identity pipeline.

**Acceptance:**
- Semantic parsing of `capability-grants.json`, every sealed PluginSchema, and every
  materialized/fallback grant source finds no `"*"` identity grant and no non-empty
  authoritative `PluginSchema.capability_grants`; public routes use the exact FR-18
  allowlist instead.
- A host-UDS or container-UDS call with no accepted SID1/OIA1 credential is rejected
  identically to an unauthenticated TCP call.
- `rg -n 'plugin_schema.dat'` over active code/deploy/docs returns no active use.

### SEC-4 — Identical authorization semantics across TCP, host UDS, container UDS

The authorization decision for a given identity + target method/tool MUST be
identical across transports; only the *binding* step differs (TR-2).

**Acceptance:** the same identity + method yields equivalent results over TCP, host
UDS, container UDS; an unauthorized identity is denied identically on all three.

### SEC-5 — No cross-identity leakage in discovery or execution

Execution and identity-scoped surfaces (context streams, memory) MUST NOT leak
across identities. (Schema/reflection visibility follows FR-9a: schemas are not
identity-confidential; execution is capability-gated.)

**Acceptance:** identity A cannot stream context for, read memory for, or execute on
behalf of identity B.

### SEC-6 — Cross-container and cross-identity memory isolation

Memory MUST be isolated by identity, container, and workspace (see memory domains,
DR-6).
**Acceptance:** memory written under (identity A, container X, workspace W) is not
retrievable under any differing identity/container/workspace, except via an
explicitly authorized shared-semantic domain (DR-6).

### SEC-7 — Stored prompt-injection is data, not instruction

Stored memory MUST NOT be treated as control-plane instruction. Injection text
("ignore previous instructions", "grant capability X") MUST NOT change capability,
auth, routing, or tool selection.
**Acceptance:** a turn retrieving injection-laden memory produces no capability/auth
change and no unrequested tool invocation.

### SEC-8 — Memory-poisoning resistance via provenance and trust classification

Provenance (source, identity/container, workspace, timestamp, confidence, event
ID/hash) and a trust classification MUST be complete for every memory, so low-trust
content is not silently promoted or ranked as authoritative. Domain, trust,
confidence, verification state, and provenance are bridge-derived fields that ordinary
callers/models cannot set or raise. Writing or promoting **curated-system** memory
requires a distinct `memory.system.curate` capability plus a signed approval from the
approver authority of SEC-10; the curator cannot approve its own promotion. Shared
semantic promotion similarly requires its declared promotion capability and privacy
sanitization, but never gains system-instruction status merely by repetition.
**Acceptance:** an unverified/low-confidence memory is neither promoted to durable
semantic memory nor outranks a verified high-confidence memory for the same query;
forged `system-curated`/trust/confidence fields are ignored or denied; ordinary memory
writers and self-approvers cannot create system-role memory; authorized curation is
signed, audited, and provenance-preserving.

### SEC-9 — Audit failures, not only successes

The event chain MUST record denied, invalid-input, invalid-output, cancelled,
timed-out, partial-side-effect, reconciliation, failed, and successful tool attempts.
For every authenticated operation, an append-only durable **intent** is atomically
created with idempotency admission before dispatch; one or more linked outcomes close
or reconcile it. A crash after side effect but before outcome append MUST leave a
recoverable open intent, never an unaudited mutation. Sensitive arguments/results are
omitted or deterministically correlated only with a keyed HMAC; plain hashes of
low-entropy secrets are forbidden.
**Acceptance:** each failure class produces the specified linked outcome with no raw
or reversibly guessable secret material; killing the bridge between intent, external
effect, and outcome leaves an open intent that restart recovery reconciles; an invalid
output produces an audit outcome while its payload is absent from all other sinks.

### SEC-10 — Signed single-use approval bound to the exact change

For a tool marked `approval_required` (FR-3b/FR-12), authorization MUST require
**both** the apply capability **and** a signed, single-use approval token bound to:
the suggestion id, the exact proposed-diff hash, the workspace and base revision, the
applying actor, the session, target tool/subid, policy/schema version, issuance time,
expiry, nonce, **approver principal id**, and approver key id. The canonical envelope
is domain-separated (`OPA1`) from OIA1/OIG1 and is signed by a key in an authoritative
approver registry. At verification time the approver identity/key MUST be active and
granted the target's approval capability (e.g. `coding.approve`). The approver MUST be
distinct from the proposer and applying actor for change-applying/system-curation
operations. Approval is not a boolean flag on the request. After signature, approver,
capability, actor/scope, input, diff, base revision, and policy checks succeed, the
domain-separated approval nonce is atomically consumed immediately before mutation;
earlier invalid requests cannot burn it. A mismatch, untrusted/unauthorized signer,
self-approval, reused nonce, cross-purpose token, or expiry fails closed. The
suggestion contract is mandatory for any change-applying tool.

**Acceptance:**
- Applying a change without a valid approval token is denied even for an identity
  holding the apply capability.
- An approval token whose diff hash / base revision / actor / session does not match
  the request, or whose nonce was already used, or which has expired, is rejected.
- Untrusted-key, inactive/unauthorized approver, self-approval, wrong-target/purpose,
  wrong-policy-version, and OIA/OIG nonce-domain collision tests are rejected; an
  invalid pre-mutation request does not consume a valid approval.
- Promotion of a lesson/outcome uses **independently verified** outcomes, not
  chatbot self-labels (ties to FR-15/FR-16).

### SEC-11 — Redact secrets; bound argument/result size before persistence or prompts

Tool arguments and results MUST be secret-redacted and size-bounded **before** they
are written to Cozo, vectorized into Qdrant, written to logs or the event chain, or
inserted into a prompt.
**Acceptance:** a tool arg/result containing a credential pattern is redacted in
every sink (Cozo/Qdrant/log/event/prompt); an oversized arg/result is truncated or
rejected per policy, never stored raw.

### SEC-12 — Inject non-curated memory as untrusted, delimited data

Non-curated memory (user/container, workspace, and any unverified domain, DR-6) MUST
be injected into prompts as clearly delimited **untrusted data**, never as system
instructions (reinforces SEC-7). Only curated system memory may occupy an
instruction/system role, and only via the authorized curation path.
**Acceptance:** injected non-curated memory appears in a data section with untrusted
delimiters; a memory whose text mimics a system instruction does not change model
role assignment or tool selection.

### SEC-13 — Pre-auth telemetry separate; durable, cross-transport replay; rate limits

- Pre-authentication security telemetry (rejections before an actor is resolved) MUST
  be recorded **separately** from actor-attributed event-chain records (do not forge
  an actor for an unauthenticated rejection).
- OIA1/OIG1/OPA1 replay protection MUST use the bridge-owned dedicated Cozo database
  at `/var/lib/op-dbus/auth-replay.db`, separate from the cognitive-memory database
  and its locks. Keys are domain-separated `(envelope_kind, trusted_issuer_key_id,
  nonce)`; atomic insert-if-absent and expiry are the only mutation API. All TCP and
  UDS listeners in the process share this handle, and the database survives bridge
  restart. OIA1/OIG1 validation performs a non-mutating existence lookup, then
  transport binding and principal/session or genesis-proof resolution, and only then
  atomically consumes the nonce before authorization/dispatch or genesis commit; a
  binding/principal/proof failure cannot burn it. OPA1 performs the same early
  non-mutating lookup but consumes its nonce only at the SEC-10 commit boundary after
  approver, authority, target, actor/scope, policy, schema, input, and diff validation;
  an invalid approval request cannot burn a valid token. If the replay database is
  unavailable or its durability cannot be confirmed, the bridge may serve liveness
  but MUST reject every authenticated/onboarding/approval operation with `Unavailable`
  — no in-memory fallback and no replay bypass.
- The ingress MUST enforce **pre-auth rate limits** and **per-principal rate limits**.

**Acceptance:**
- An unauthenticated rejection produces a pre-auth telemetry record with no
  actor/event-chain attribution.
- A nonce accepted before a bridge restart is rejected after restart; a nonce used on
  TCP is rejected when replayed on UDS (and vice-versa).
- Replay-store lock/corruption/unavailability fails all protected paths closed while
  liveness remains available; a wrong-source assertion does not consume the nonce;
  OIA1, OIG1, and OPA1 equal nonce bytes do not collide across domains.
- A burst of unauthenticated requests is rate-limited; a single principal exceeding
  its rate is throttled.

### SEC-14 — Browser gRPC-Web/MCP Origin and CORS policy

Browser-facing MCP HTTP and gRPC-Web on `:8090` MUST use an exact configured allowlist
of dashboard origins including scheme, host, and port. Wildcard, suffix, regex,
reflected-origin, and `null` origins are forbidden. Browser requests with a missing,
duplicate, malformed, or unlisted `Origin` fail before assertion processing; native
gRPC and UDS are not required to manufacture an Origin. Preflight permits only the
required methods and headers (including the canonical OIA1 header), returns
`Vary: Origin`, and never returns credentials to an unlisted origin. Origin is a CSRF
control, not authentication: every allowed-origin request still needs fresh OIA1 and
normal capability checks. Redirects MUST NOT carry OIA1/approval headers to a different
origin, and the bridge MUST NOT log those headers.

**Acceptance:** real browser E2E from the configured dashboard origin completes
gRPC-Web discovery and a safe call with OIA1; exact wrong scheme/host/port, `null`,
missing/duplicate Origin, hostile preflight header, and cross-origin redirect cases
are rejected without dispatch or credential reflection. A native gRPC TLS client with
no Origin remains governed by SEC-2 rather than browser CORS.

---

## Data Requirements

### DR-1 — One authoritative durable Cozo writer

Exactly one process (the bridge) MUST be the authoritative durable writer for
cognitive memory. Others — including `op-chat` — MUST access memory through the
authenticated bridge, not by independently opening the persistent DB. (op-web's
distinct users DB is out of scope.)
**Acceptance:** only the bridge holds a durable write handle to the cognitive-memory
Cozo DB; starting `op-chat` while the bridge runs produces no Cozo lock conflict on
that DB.

### DR-1a — No silent ephemeral fallback for durable memory

If the persistent Cozo store is locked or unavailable, durable memory tools MUST
return an `Unavailable`-class error. The bridge MUST NOT silently substitute an
in-memory Cozo instance and accept production memory writes into it. (The bridge MAY
still start and serve non-memory plugins, NFR-3, but durable-memory operations fail
explicitly rather than writing to an ephemeral store.)
**Acceptance:** with the persistent Cozo path locked, a memory write returns
`Unavailable` (not success); no data is written to an ephemeral DB; a test asserts no
silent in-memory substitution occurs on the durable-write path.

### DR-2 — Sealed catalog is source of truth; readers read-only

`/dev/shm/opdbus/plugin-blobs/` is source of truth; `op-blob` is the only writer; a
manifest-selected sealed blob becomes active only after exact-hash, compatibility,
compiled-route, and mounted/reflected parity gates pass (FR-9); other blobs are
staged/inactive. Consumers are read-only; `plugin_schema.dat` is not a component.
**Acceptance:** no consumer writes the blob dir; `plugin_schema.dat` absent from
runtime/code/deploy/docs.

### DR-3 — Reflection/route derivation from PluginSchema + sealed catalog

Generated proto routes and static reflection derive from PluginSchema methods;
dynamic reflection hydrates from the sealed catalog; the two stay in parity (FR-9).
**Acceptance:** as FR-9. A newly sealed blob whose method shapes exactly match compiled
mounted routes becomes active and reflected after catalog pickup; a new/uncompiled or
incompatible shape remains inactive/unadvertised with `requires_redeploy` status until
a rebuild/redeploy. No sealed arrival alone creates a callable typed route.

### DR-4 — Durable memory survives semantic-search failure

Qdrant/Voyage failure MAY degrade semantic retrieval but MUST NOT disable durable
memory or bypass authentication.
**Acceptance:** with Qdrant/Voyage unreachable, durable memory reads/writes still
succeed through the authenticated bridge and auth is still enforced.

### DR-5 — Event-chain identity on every mutation/tool call

Every authenticated mutation/tool call uses the durable intent/outcome protocol of
SEC-9/FR-3g. The intent records `event_id`, `event_hash`, `actor_id`, `capability_id`,
operation id, target subid, schema/policy version, canonical redacted input digest, and
state; linked outcomes record success/failure/invalid-output/cancellation/partial effect
and prior hash. Intent admission is atomic with idempotency admission before D-Bus
dispatch; restart reconciles every open intent. Pre-auth telemetry is separate and has
no forged actor.
**Acceptance:** authenticated calls produce non-zero linked intent/outcome ids and
hashes with populated actor/capability; kill-at-each-boundary tests leave no untracked
effect and restart closes or explicitly reconciles every open intent.

### DR-6 — Memory domains and shared-promotion authorization

Memory MUST be modeled in distinct domains: (a) curated system memory; (b)
chatbot-soul memory; (c) user/container memory; (d) workspace/project memory;
(e) shared semantic lessons. Promotion into a shared/global domain MUST require
explicit authorization and MUST NOT expose one user's content to another; shared
lessons carry no raw per-user private content. Every durable memory row MUST conform
to a versioned typed `MemoryRecord` containing: server-derived domain and
`identity/container/workspace` scope; stable record key; monotonic revision; redacted
content or typed outcome summary; provenance/event link; server-derived trust and
verification state; created/updated/expiry timestamps; source-record links; and a
durable tombstone state. Caller/model text cannot set domain, scope, trust,
verification, provenance, revision, or tombstone ownership. Curated-system memory
obeys SEC-8/SEC-10.
**Acceptance:** a retrieval names the domain of each returned memory; promotion into
the shared domain is denied without the promotion capability; a shared lesson
contains no other user's private content.

### DR-7 — Durable, idempotent memory reconciliation (Cozo↔Qdrant)

Cross-store reconciliation (FR-15) MUST be durable, not best-effort. The design MUST
use a **Cozo-transactional outbox**: memory writes and their pending vector-index
operations commit atomically in Cozo; a reconciler drains the outbox to Qdrant with
**idempotent replay** keyed by stable memory point IDs and **monotonic revisions**,
suppressing stale revisions. It MUST recover on restart (undrained outbox entries are
replayed), and durable-truth (Cozo) MUST be checked before vector results are returned
so a **deleted memory is never returned to a caller as a tombstone**.

**Acceptance:**
- Killing the bridge mid-reconcile and restarting drains the outbox and converges
  Cozo and Qdrant (no lost or duplicated points).
- A stale-revision Qdrant entry is suppressed; a deleted memory is not returned by a
  semantic query (no tombstone leakage to callers).
- Point IDs are stable across re-embeds; replay is idempotent (re-running does not
  duplicate).
- Qdrant point IDs MUST be UUIDv5 over a canonical tuple containing domain, identity,
  container, workspace, and stable record key; payload filters MUST include the same
  complete server-derived scope. Identical keys/content in different scopes remain
  distinct. Tombstones are durable Cozo truth and are never returned to callers;
  Qdrant deletion is an idempotent derived operation, and stale points are suppressed
  by revision plus the durable-truth check.
- Tests write identical keys/content into two identities/containers/workspaces and
  prove no collision, overwrite, search, correction, or deletion crosses scope.

### DR-8 — Exact schema-hash blob resolution; atomic vector refresh

Sealed-blob reads MUST resolve a plugin by the manifest's **exact schema hash**
(`<plugin_id>.<schema_hash16>.blob`), not by first-prefix match (the current
`read_plugin_schema_shm` first-prefix behavior at `op-blob/src/catalog.rs` is a
defect when two hashes for one plugin coexist). Blob-vector refresh (FR-8a) MUST be
**atomic**: a partial/failed refresh leaves the previous generation active (no
half-built vector collection is ever served).

**Acceptance:**
- With two blobs for one plugin id present, the reader returns the manifest-pinned
  hash, not an arbitrary first entry.
- A refresh interrupted midway leaves the prior vector generation queryable; callers
  never observe a partially-rebuilt collection.

### DR-9 — Durable scoped context journal

The bridge MUST persist context events and resume checkpoints in a versioned Cozo
journal owned by the same bridge process but logically separate from user memory.
Primary identity is `(identity,container,workspace,session,sequence)`; sequence
allocation and event append are atomic and monotonic per scope. Events are typed,
redacted, bounded, and immutable; correction is a linked superseding event. Retention
creates a sanitized checkpoint before pruning and never permits a cursor to resolve
into another scope. The journal is restored before context subscriptions are accepted
after restart.

**Acceptance:** concurrent append preserves a gap-free per-scope order; restart resumes
from the next retained sequence; forged/cross-scope cursors fail; retention yields the
explicit FR-19 `CursorExpired` checkpoint behavior; locked/unavailable journal returns
`Unavailable` for context persistence/subscription without falling back to an
in-memory authoritative history.

---

## Transport Requirements

### TR-1 — TCP ingress

TLS `:8090`, bridge-owned and directly bound only on `127.0.0.1` and the unified
fabric address `10.0.0.3`, multiplexing MCP HTTP/JSON-RPC, optional SSE, gRPC-Web,
native gRPC, plugins, mutation, and control through one route/auth stack (FR-1, FR-2,
SEC-1/SEC-14). No relay, Netmaker bind, or `10.200.0.2:8090` bind.

### TR-2 — Host and container UDS are equivalent alternate transports with transport-specific binding

Host UDS (`/run/opdbus/grpc.sock`) and container UDS
(`/run/ghostbridge/container.sock`) MUST route through the identical
authorization/route/capability/validation/dispatch/event stack and return equivalent
results to TCP for the same identity + method. Binding is transport-specific:
- **TCP:** exact selected-sled SID1 for the configured protected local path, or OIA1
  plus authenticated network/source binding for another caller.
- **UDS:** accepted SID1/OIA1 credential + `SO_PEERCRED` (peer uid/gid/pid), socket ownership, and
  session/container binding.
Neither transport receives implicit trust; a UDS peer with no accepted credential is
rejected exactly like an unauthenticated TCP peer.

**Acceptance:** TCP, host UDS, container UDS return equivalent results for the same
authenticated request; an unauthenticated request is rejected identically on all
three; a UDS request's peer credentials are validated (a forged/absent credential over
UDS is rejected).

### TR-3 — No alternate MCP transports/listeners

No standalone cognitive/compact/agents/blob-schema/Waypipe listener, no
bridge-to-cognitive HTTP loopback, and no MCP execution on `:8080`.
**Acceptance:** `ss -lntp` + CI grep gates (NFR-7) confirm none exist.

### TR-4 — Close the op-web alternate gRPC ingress

`op-web :8080` MUST NOT forward gRPC/MCP traffic to `:8090`. The
`crates/op-web/src/grpc_proxy.rs` middleware (forwards ALL `application/grpc*` to
`https://127.0.0.1:8090`, reaching cognitive/generated RPCs) MUST be **deleted**, or
replaced by an explicit **non-MCP allowlist** that cannot reach cognitive, generated
plugin, memory, context, or tool RPCs. The related aliases MUST also be removed and
tested: `/jsonrpc`, `/rpc`, `/.well-known/mcp.json` (`mcp_discovery.rs`), the MCP
discovery aliases, and `mcp_smart_router.rs`. Because the browser dashboard uses
gRPC-Web, and the bridge already serves authenticated gRPC-Web directly on `:8090`
(`crate::grpc_web::enable`), the dashboard MUST be pointed at `:8090` with a defined
CORS/`Origin` policy and pass a browser E2E **before** the `:8080` proxy is removed.

**Acceptance:**
- Sending `application/grpc` to `:8080` no longer reaches `:8090` (proxy deleted, or
  a non-MCP allowlist rejects cognitive/generated/memory/context/tool RPCs).
- `/jsonrpc`, `/rpc`, `/.well-known/mcp.json`, and MCP discovery aliases on `:8080`
  return 404/410 (tested).
- The dashboard reaches the bridge's gRPC-Web on `:8090` directly (authenticated,
  CORS/`Origin` enforced) — verified by a browser E2E — before `:8080` proxy removal.

### TR-5 — UDS security model (explicit)

The UDS transports MUST have a fully defined security model, not merely
`SO_PEERCRED`:
- **TLS-over-UDS is mandatory** using the bridge trust root and an explicit validated
  server name present in the certificate SAN; there is no plaintext policy exception;
- which **wire protocols** each UDS accepts (native gRPC, gRPC-Web, MCP HTTP);
- **trusted peer-credential extraction** and **UID/GID/PID mapping through user
  namespaces** (a container's namespaced uid maps to the correct principal);
- **restrictive socket ownership/mode** — the current world-writable `0o666`
  (`shared_socket.rs`) MUST be tightened to the minimal owner/group needed;
- how **OIA source binding** is adapted for UDS (peer-credential/session binding in
  place of network source IP, consistent with TR-2).

**Acceptance:**
- Plaintext UDS connections fail; TLS-over-UDS succeeds only with the configured trust
  root and server name, and fails for an untrusted root/wrong name. Socket mode is
  `0660` or stricter with the intended owner/group and is not world-writable.
- A container peer's namespaced uid/gid maps to the intended principal; a peer that
  cannot be mapped is denied.
- Each UDS's accepted wire protocols are enumerated and enforced.

---

## Deployment Requirements

### DEP-1 — runit-managed services via `sudo sv`

Host lifecycle actions MUST use `sudo sv`. `systemctl`/s6 MUST NOT be used. Retired
MCP service definitions MUST be removed from `deploy/runit/` to match the live host.
No retired MCP shim/proxy service survives. `op-identity-headers` is a short-lived
header JSON helper, not a service or transport (CR-4).
**Acceptance:** no active `deploy/runit/` service `run` binds `:11438`, `:3003`, or
`:50052`; retired MCP service dirs are removed.

### DEP-2 — Deployment via btrfs golden image

Deployment MUST use `deploy/runit/build-golden.sh` (btrfs send/receive), not
hand-copied binaries; network-critical services are not auto-restarted.
**Acceptance:** the deployment task uses `CXXFLAGS="-include cstdint" cargo build
--workspace --release` then `sudo deploy/runit/build-golden.sh` (with `--dry-run`
review first); no task hand-copies a binary.

### DEP-3 — Restart durability

Restarting the bridge MUST restore sealed routes, identity projections, and memory
access, reopen the independent replay/idempotency stores, recover open audit intents
and the memory outbox, and restore durable context-journal cursors before protected
traffic is admitted.
**Acceptance:** after `sudo sv restart op-grpc-bridge`, reflection lists the same
sealed routes, identity resolution and authenticated memory access succeed, a
pre-restart nonce remains consumed, a retained context cursor resumes, memory outbox
and open audit intents reconcile, and duplicate operation ids remain deduplicated.

### DEP-4 — Live artifact parity

Live deployed binary hashes/timestamps MUST match built artifacts.
**Acceptance:** `[manual]` compare `sha256sum`/mtime of the deployed bridge/web
binaries against the built artifacts after `build-golden.sh`. (Baseline notes the
current binaries lag source; parity is a post-deploy criterion.)

### DEP-5 — Coordinated data migration, rollback, and security-state preservation

Binary/Btrfs rollback MUST NOT be treated as sufficient data rollback. Before any
schema migration or irreversible deletion, deployment MUST create and verify a
coordinated checkpoint of: cognitive Cozo memory plus outbox; durable context journal;
auth replay/approval nonce store; audit/idempotency intents; the sealed-blob manifest;
active Qdrant aliases/generations; and the **sanitized identity-grant version/hash**
(never a raw committed live grants file). Migrations are versioned, forward-only until
the checkpoint is verified, and either backward-readable by the prior binary or paired
with an explicit restore procedure. Rolling back MUST NOT resurrect wildcard grants,
sentinel identity, retired listeners, or consumed assertion/approval nonces. Data
created after cutover is either preserved through a backward-compatible schema or
explicitly exported/replayed; silent loss is forbidden.

**Acceptance:** a staging test writes pre-upgrade memory/context/audit/replay data,
deploys and migrates, writes post-upgrade data, then boots the prior golden snapshot
and executes the documented coordinated restore/compatibility path. Pre-upgrade data
and consumed nonces remain valid/security-preserving, the expected disposition of
post-upgrade data is proven, no deleted memory/tombstone resurfaces, no wildcard grant
returns, Qdrant points match the restored Cozo revision/alias, and forward deployment
can be retried. Checkpoint checksums and restore logs are retained without raw secrets.

---

## Compatibility / Migration Requirements

### CR-1 — No stranded consumer during cutover; preserve inventory

Migration MUST move clients to `:8090` bridge paths before deleting old
listeners/routes; each step has rollback. The typed underlying capability inventory
captured in Phase 0 MUST be preserved. The four former compact meta-tool names are an
explicit removal and are replaced by `toolsets`; they are excluded from the post-
migration name/count floor.
**Acceptance:** ordered cutover with per-step rollback; a consumer inventory
(`.mcp.json`, `~/.factory/mcp.json`, `.kiro/settings/mcp.json`,
`deploy/config/*mcp*.json`, container gateways, Xray routes) migrated before
deletion; post-migration tool count ≥ captured baseline. Baseline artifacts committed
to the repository contain only sanitized inventories and hashes/counts; raw live grant
files, assertion/approval material, principal/session/footprint metadata, and secrets are never copied
or committed. Rollback follows DEP-5 and MUST NOT restore the known wildcard grant.

### CR-2 — Context/feedback affects later retrieval

A recorded context event, and a user correction or accepted/rejected suggestion, MUST
affect a later ranked suggestion/retrieval.
**Acceptance:** after a context event a later retrieval reflects it; after a
correction/accepted/rejected suggestion a later ranked suggestion changes.

### CR-3 — Graceful degradation

Qdrant/Voyage failure degrades to durable memory without disabling auth or durable
storage (DR-4, DR-1a).

### CR-4 — Legacy names only in removal/negative contexts

Retired names (`op-cognitive-mcp`, `op-mcp-agents`, `op-mcp-compact`,
`op-mcp-blob-schema`, `op-mcp-cognitive`, `op-waypipe-grpc`) and forbidden
ports/paths (`:3003`, `:50051`, `:50052`, `:11438`, `10.200.0.2:3003`,
`plugin_schema.dat`) MAY appear only in removal tasks, migration notes, or negative
tests. No surviving MCP shim/proxy is part of the Codex path: Codex uses native direct
Streamable HTTP plus the header-only helper.
**Acceptance:** CI grep gates (NFR-7) fail on any active reference.

### CR-5 — Orthogonal specs de-conflicted and linked

Remaining specs (op-web, netmaker-custom-json-render-ui, runit-sv-migration,
schemars-to-reflection-plugin-pipeline, netmaker-xray-identity-handoff,
accountability-audit-trail, dead-signal-and-tool-cleanup) MUST NOT recommend old
listeners, standalone MCP daemons, HTTP loopbacks, or unauthenticated MCP routes, and
MUST link to this canonical spec.
**Acceptance:** those specs carry no conflicting MCP-architecture statement and link
here (inspection + grep).

### CR-6 — Reconcile the duplicate `kiro/specs/` tree

The non-dot `kiro/specs/` tree duplicates spec content. The consolidation MUST apply
the same deletions/edits in both trees (or record the duplicate for removal) so the
canonical outcome is unambiguous.
**Acceptance:** after consolidation, neither tree contains the subsumed specs, both
contain (or link to) the canonical spec, and the report states the duplicate-tree
disposition.

### CR-7 — Preserve and require the existing E2E suites

The following end-to-end tests MUST be preserved and required as gates:
- the existing **real Voyage/Qdrant** semantic E2E, using a unique temporary
  generation/collection, manifest-pinned blob set, deterministic cleanup, explicit
  non-skip assertion, and cross-scope negative query;
- an **authenticated `:8090` E2E** against both canonical binds (loopback and
  `svc0`): SAN-valid TLS, direct native Codex initialize/list/call with
  exact selected-sled SID1, canonical optional OIA1 HTTP/native-gRPC encoding and replay,
  OIG1 onboarding fixture, discovery, safe read, D-Bus route spy, per-tool denial,
  idempotent mutation fixture, event intent/outcome, and no alternate listener;
- a **two-turn chatbot-memory E2E** where turn 1 writes a typed redacted scoped record
  and turn 2, after bridge restart, recalls it with full provenance; another
  identity/container/workspace using the same stable key cannot retrieve it, and a
  prompt-injection/secret sentinel is delimited/redacted;
- a **real-browser gRPC-Web E2E** from the exact configured dashboard Origin plus
  wrong-origin/preflight/redirect negatives (SEC-14);
- the **coordinated Btrfs + data rollback E2E** of DEP-5, not merely a binary hash or
  service restart.

**Acceptance:** all five suites emit machine-readable proof of fixture setup, exact
endpoint/process owner, non-skip execution, assertions, cleanup/checkpoint result, and
pass/fail. A skipped, partially exercised, cleanup-leaking, or unverified suite is a
failure and release blocker. Live-only destructive portions run in the designated
acceptance environment; hermetic CI must still run the protocol/auth/isolation
fixtures rather than marking them ignored.

---

## Non-Functional Requirements

### NFR-1 — No new crate dependencies for the consolidation

Reuse existing edges (bridge → op-cognitive-mcp → op-mcp `ToolRegistry`); no new
proto packages or HTTP clients for dispatch. (Derived from the workspace's Rust-first
policy in `CLAUDE.md`/`AGENTS.md`.)
**Acceptance:** no new dependency added to `op-grpc-bridge/Cargo.toml` for in-process
dispatch; `reqwest` dropped if only the removed loopback used it.

### NFR-2 — Reactive, not polled (scoped)

Identity, context, and dispatch paths MUST NOT use periodic state-discovery/
evaluation polling; arrival triggers action. This ban does NOT prohibit legitimate
timers: deadlines, expiry, confidence-decay schedules, rate limits, or SSE
heartbeats.
**Acceptance:** `rg` finds no periodic state-evaluation `interval`/`sleep` loop in
the context/identity/dispatch paths (the current 5 s context poll is removed);
deadline/heartbeat timers are permitted.

### NFR-3 — Bridge must not fail closed on optional-dependency loss

If Cozo or Qdrant is unreachable at startup, the bridge still starts, serves all
plugins, and enforces auth; the affected cognitive/memory tools return an explicit
error (Qdrant → degraded semantic; durable Cozo → `Unavailable`, DR-1a), never a
crash. Here "durable Cozo" means the cognitive memory/context store; the independent
auth replay database of SEC-13 is a mandatory security dependency. If that replay
database is unavailable, the bridge starts for liveness/diagnostics but every
protected request fails closed with `Unavailable` until durable replay admission is
restored.
**Acceptance:** with Qdrant unreachable the bridge starts and serves; a
Qdrant-dependent tool returns an error; other plugins work.

### NFR-4 — Tool execution: bounded concurrency, cancellation, deadlines, backpressure

Long-running tools MUST run on spawned tasks with bounded concurrency, cancellation
propagation (tied to MCP cancellation, FR-2a), per-call deadlines/timeouts, and
backpressure; blocking work uses `spawn_blocking`. The dispatch loop stays responsive.
Cancellation follows FR-3g: it prevents uncommitted admission/commit and propagates to
the implementation, but never reports an already committed external effect as rolled
back; such a race is audited/reconciled.
**Acceptance:** a concurrent unrelated-plugin call returns promptly during a
long-running tool; a cancelled MCP request stops uncommitted work and classifies any
committed/partial effect; a tool exceeding its deadline records the exact terminal or
reconciliation state (SEC-9/FR-3g); concurrency is bounded (a flood does not exhaust
the runtime).

### NFR-5 — OSCAL subid coverage

Every new method/tool/event carries an OSCAL subid registered in
`oscal_subid_registry.rs`; uniqueness CI-enforced. (Tool descriptors carry `subid`,
FR-3b.)
**Acceptance:** `cargo test -p op-plugins -- all_plugin_subids_are_valid_and_unique`
passes with the new subids.

### NFR-6 — No Python; Rust-first; shell scripts only

No new Python (per `CLAUDE.md`/`AGENTS.md` governing policy). JSON assertions use
`jq`. No MCP transport shim is introduced; non-MCP network policy uses existing
Rust/nftables mechanisms.
**Acceptance:** no acceptance criterion invokes `python3`; no new `.py` added.

### NFR-7 — Zero-trace CI grep gates

CI MUST fail on any active (non-removal, non-negative-test) occurrence in `crates/`,
`deploy/`, and active docs of:
- `plugin_schema.dat`
- `10.200.0.2:3003` (and any `:3003` MCP use)
- MCP uses of `:50051` or `:50052`
- `:11438` (and `:11437`)
- direct `op-web` MCP execution routes (`/mcp`, `/mcp/compact`, `/mcp/agents*`
  serving tools)
- standalone cognitive/Waypipe listeners
- retired `op-mcp-*` runit services with a network bind
- bridge HTTP loopback dispatch (`cognitive_mcp_endpoint`, POST to `/mcp`)
- sentinel/wildcard/embedded identity-grant authority: any `"*"` grant in any source,
  or any non-empty authoritative `PluginSchema.capability_grants`; schemas may declare
  required capabilities but grants come only from the protected exact-`principal_id`
  projection; `identity_sled` supplies session/genesis context only (FR-4a, FR-4b,
  SEC-3)
- the `op-web` alternate gRPC ingress: `grpc_proxy` forwarding `application/grpc*` to
  `:8090`, and the `/jsonrpc`, `/rpc`, `/.well-known/mcp.json`, `mcp_discovery`,
  `mcp_smart_router` aliases (TR-4)
- a Python `socket-relay`/`tcpfwd` fronting `:8090` (e.g. `fwd-8090`), and any
  non-loopback `:8090` bind owned by a non-bridge process (FR-1)
- caller-supplied capability header treated as authority / the degraded
  `identity.is_some() && capability_matches` allow path (FR-4a)
- first-prefix blob resolution instead of exact schema-hash (`DR-8`)
- raw sealed schema/grant/secret fields reaching ordinary reflection/blob/resource/
  vector/cache surfaces instead of the FR-8b public projection
- direct network-adapter-to-`ToolRegistry::execute` dispatch that bypasses the
  canonical D-Bus/MutationEngine admission path
- active OAuth discovery/token/callback/PKCE code or a same-endpoint OAuth-to-OIA
  exchange for Codex
- a Codex MCP `command`/stdio shim, HTTP proxy, header-helper listener, or alternate
  MCP URL; `op-identity-headers` may only emit header JSON and exit
- requiring native Codex to synthesize non-standard `Mcp-Method`/`Mcp-Name` headers
  instead of deriving method/target from the parsed JSON-RPC body
- active MCP exposure of `list_tools`, `search_tools`, `get_tool_schema`,
  `execute_tool`, or `invoke_tool`; the names may appear only in removal/negative tests

The second-`ToolRegistry` and no-polling gates are **scoped** (production execution
ownership; periodic state-discovery only) per FR-4 and NFR-2 so they do not flag
tests, isolated libraries, or legitimate timers.

**Acceptance:** the gate script exits non-zero when any pattern is present in an
active context and zero on the cleaned tree.

---

## Out of Scope

- Redefining the Oracle-assertion identity model (owned by
  `netmaker-xray-identity-handoff`).
- Redefining the schemars → reflection pipeline mechanics (owned by
  `schemars-to-reflection-plugin-pipeline`).
- The operator UI component implementation (owned by
  `netmaker-custom-json-render-ui`); this spec only corrects its endpoint references
  and links it here.
- Model fine-tuning / weight changes (gated behind a separate reviewed dataset and
  approval; FR-16).
- Full de-Pythoning of the mesh forwarder layer beyond deleting forwarders this spec
  retires.
- op-web's users Cozo DB (distinct from cognitive memory; DR-1 does not touch it).
