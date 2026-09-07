# Standalone EMQX Identity MCP — Requirements

## Overview

This specification defines the requirements for integrating EMQX as a **standalone local service** into the OP-DBUS identity pipeline. EMQX is no longer associated with Netmaker and must be managed independently through the existing PluginSchema contract, authenticated D-Bus, and generated gRPC methods.

**Unified Fabric Surface:** This spec supersedes the topology lock in
`.kiro/specs/README.md`. `10.0.0.3:8090` is the authoritative mesh-private unified
fabric surface itself—not merely one endpoint on a larger fabric. The same bridge-owned
surface carries MCP at `https://10.0.0.3:8090/mcp`, native gRPC/gRPC-Web, generated
plugin methods, and mutation/control capabilities. EMQX remains internal on loopback
MQTT/ExHook behind that surface and never exposes a second external endpoint.

### Identity and Footprint Terminology Lock

These names are different namespaces and MUST NOT be substituted for one another:

| Name | Meaning | Permitted use |
|------|---------|---------------|
| `principal_id` | Stable authorization identity resolved from the authoritative HumanPrincipal/service-principal registry | Grant lookup, audience selection, actor identity, D-Bus binding |
| `session_id` | Session/container correlation handle | Session lookup and request correlation; never authentication by itself |
| `session_genesis` | Immutable anchor for one authenticated session term | Session-chain stamping and equality verification; never a grant or audience key |
| `PluginFootprint` / footprint | Canonical per-mutation payload envelope emitted by the sled and carried by the Shuttle | Snowball hash-chain append and asynchronous vectorization only |
| `event_hash` | Hash-chain result authored by the Snowball append operation | Chain linking, receipt, and verification; never identity |

No authorization identifier may be derived from a footprint, genesis, `data_hash`,
`content_hash`, `event_hash`, chain head, or any other existing digest. In particular,
the implementation MUST NOT hash a footprint (or another hash) to manufacture an
identity. The stable `principal_id` is injected into request/event metadata as
`actor_id`; it is not transformed into a footprint.

---

## Functional Requirements

### FR-1: Standalone EMQX Service

| ID | Requirement |
|----|-------------|
| FR-1.1 | EMQX runs as a standalone local service supervised by runit, not as a Netmaker dependency or container service. |
| FR-1.2 | A runit service definition exists at `/etc/runit/sv/emqx/run` and is tracked in `deploy/runit/emqx/run` in the repository. |
| FR-1.3 | The service is registered and enabled via `deploy/runit/build-golden.sh` managed-service registration — not manual symlink creation. |
| FR-1.4 | Human/operator host-service lifecycle commands use `sudo sv` exclusively. Plugin lifecycle methods do not invoke a shell; they delegate to the authorized D-Bus runit service-manager boundary with the service name hard-coded to `emqx`. |
| FR-1.5 | The EMQX service reports healthy within 30 seconds of `sudo sv start emqx`. |
| FR-1.6 | EMQX and any retained local ExHook/MQTT integration plugin versions are pinned with checksums in deployment configuration. No MCP Gateway plugin is installed. |
| FR-1.7 | No boot-time downloads — EMQX and plugins are pre-staged in the golden subvolume. |
| FR-1.8 | Persistent data is separated from the golden subvolume (e.g., `/var/lib/emqx` on a data subvolume). |

### FR-2: PluginSchema Contract (emqx)

| ID | Requirement |
|----|-------------|
| FR-2.1 | The `emqx` PluginSchema in `crates/op-plugins/src/state_plugins/emqx.rs` is the sole typed contract for EMQX present-state/control methods projected over D-Bus, generated/generic gRPC, and MCP. The ExHook callback protocol remains the separate FR-5 module/service boundary. |
| FR-2.2 | All Netmaker-specific fields, paths, sockets, descriptions, and assumptions are removed from the plugin. |
| FR-2.3 | The plugin state reflects standalone EMQX: local configuration paths, listener addresses, dashboard URL, and API endpoints. |
| FR-2.4 | Every method uses named Schemars `#[derive(JsonSchema)]` input/output types. |
| FR-2.5 | `AckOutput` (from `plugin_scaffold_helpers.rs`) MAY be used for acknowledgment-only methods where no richer result is semantically meaningful. Lifecycle methods (`start`, `stop`, `restart`) MUST return richer results including PID, uptime, and status details. |
| FR-2.6 | Every method declares a unique OSCAL subid following the taxonomy in CLAUDE.md. |
| FR-2.7 | Subid uniqueness MUST be validated by CI — manual edits to `oscal_subid_registry.rs` alone are insufficient. A `cargo test` target must fail if duplicate subids exist. |
| FR-2.8 | Every method declares a required capability following the `cap.<category>.<plugin>.<resource>.<action>@v1` pattern. |
| FR-2.9 | Schema construction, defaults, and examples are deterministic and side-effect free. They MUST NOT inspect live sockets, call the EMQX API, or depend on service state; live queries occur only during bounded method dispatch. |
| FR-2.10 | State, schemas, examples, method results, and audit payloads MUST NOT expose EMQX API credentials, OIA material, MQTT credentials, or other secrets. |

### FR-3: Typed EMQX Methods

The plugin must expose at minimum these typed methods:

| Method | Input Type | Output Type | Side Effect | Capability |
|--------|-----------|-------------|-------------|------------|
| `get_status` | `GetStatusInput` | `GetStatusOutput` | Read | `cap.network.emqx.status.get@v1` |
| `get_fabric_status` | `GetFabricStatusInput` | `GetFabricStatusOutput` | Read | `cap.network.emqx.fabric.status.get@v1` |
| `list_listeners` | `ListListenersInput` | `ListListenersOutput` | Read | `cap.network.emqx.listeners.list@v1` |
| `list_hooks` | `ListHooksInput` | `ListHooksOutput` | Read | `cap.network.emqx.hooks.list@v1` |
| `get_hook_status` | `GetHookStatusInput` | `GetHookStatusOutput` | Read | `cap.network.emqx.hooks.status.get@v1` |
| `configure_hooks` | `ConfigureHooksInput` | `ConfigureHooksOutput` | Mutation | `cap.network.emqx.hooks.configure@v1` |
| `start` | `StartInput` | `StartOutput` (with PID, status) | Mutation | `cap.network.emqx.lifecycle.start@v1` |
| `stop` | `StopInput` | `StopOutput` (with confirmation) | Mutation | `cap.network.emqx.lifecycle.stop@v1` |
| `restart` | `RestartInput` | `RestartOutput` (with PID, status) | Mutation | `cap.network.emqx.lifecycle.restart@v1` |

### FR-4: MutationEngine Dispatch Wiring

| ID | Requirement |
|----|-------------|
| FR-4.1 | The `emqx` plugin is wired into `MutationEngine::dispatch_method_call` with an explicit match arm. |
| FR-4.2 | The plugin does NOT fall through to the generic echo fallback (`_ => serde_json::to_value(&parsed_value)`). |
| FR-4.3 | A dedicated `dispatch_emqx_method` async function handles all emqx method dispatch. |
| FR-4.4 | Lifecycle methods (`start`, `stop`, `restart`) delegate through an authorized service-management boundary — they do not shell out or become a second service manager. |
| FR-4.5 | The lifecycle delegation target is always the literal service `emqx`; neither plugin input nor caller-controlled state may select another service. |
| FR-4.6 | `configure_hooks` accepts only supported hook names and a validated local destination compatible with the pinned EMQX version. Configuration is written atomically, reloaded through the authorized service boundary, rolled back on failure, and redacted in logs/audit. |

### FR-5: ExHook Integration (Separate Module Boundary)

| ID | Requirement |
|----|-------------|
| FR-5.1 | ExHook callback handling is implemented in `crates/op-grpc-bridge/src/emqx_hook_provider.rs`. This is a module within `op-grpc-bridge`, not a separate crate. If extraction to a dedicated crate is required for cleaner boundaries, a new `crates/op-emqx-hooks/` crate must be created. |
| FR-5.2 | The HookProvider service implements `emqx.exhook.v3.HookProvider`, required by the pinned EMQX 6.2.2 release. |
| FR-5.3 | ExHook MUST be mounted on a dedicated authenticated local UDS or loopback-only listener with mTLS or peer-credential verification — NOT on the shared client-facing gRPC route without an interceptor. |
| FR-5.4 | Anonymous MQTT connections MUST be denied by EMQX configuration. |
| FR-5.5 | EMQX carries no MCP request/response topic namespace. Reserved `$mcp-*` topics are denied outright. Internal `$opdbus/fabric/*` health/catalog/lifecycle topics are default-deny and accessible only to the pinned local service identities. |
| FR-5.6 | Forged ExHook calls (from sources other than the local EMQX instance) MUST be rejected. The selected transport uses the service-identity binding in FR-5.11 or a dedicated pinned mTLS identity; UID-only or bearer/shared-secret-only authentication is insufficient. |
| FR-5.7 | Hook callbacks are recorded as signals/events through `MutationEngine` for accountability. |
| FR-5.8 | Hooks cover: connection, authentication, authorization, subscription, publication, and session lifecycle events. |
| FR-5.9 | The hook service returns `IGNORE` for auth decisions ONLY when EMQX's own authentication and topic authorization are independently enforced (per FR-5.4 and FR-5.5). |
| FR-5.10 | Phase 0 MUST prove which ExHook target transports and URI schemes the pinned EMQX release supports. An unverified `unix://` URI MUST NOT be placed in production configuration; if UDS is unsupported, use a loopback-only gRPC listener with mTLS and a dedicated EMQX client identity. |
| FR-5.11 | UDS authentication, when supported, binds a dedicated EMQX service account to socket ownership plus peer UID/GID/PID, process start time, and executable/cgroup provenance. UID alone is not sufficient. A group-accessible socket uses `0660` with a non-world-traversable parent directory, not an internally contradictory `0600 root/emqx` policy. |
| FR-5.12 | A UDS implementation captures `SO_PEERCRED` on the accepted `UnixStream` and propagates immutable peer metadata into tonic request extensions through a tested `Connected` wrapper/interceptor path. It MUST NOT trust request metadata or a caller-claimed PID as peer credentials. |

### FR-6: Generated gRPC and Reflection

| ID | Requirement |
|----|-------------|
| FR-6.1 | `build.rs` generates per-method typed gRPC services from the plugin's `schema.methods`. |
| FR-6.2 | Generated `.proto` and route files are never hand-edited. |
| FR-6.3 | tonic-reflection exposes the generated EMQX request/response message types. |
| FR-6.4 | Both D-Bus `PluginV1.Call` and gRPC `PluginService.CallMethod` execute the same `dispatch_emqx_method` implementation. |
| FR-6.5 | Generated per-method typed gRPC RPCs (e.g., `EmqxPluginMethods.GetStatus`) also execute the same implementation. |

### FR-7: D-Bus Identity Binding

| ID | Requirement |
|----|-------------|
| FR-7.1 | D-Bus does NOT pass through the gRPC interceptor. A separate D-Bus sender identity resolution mechanism is required. |
| FR-7.2 | Production D-Bus `PluginV1.Call` MUST resolve the sender's unique bus name to a stable, registered `principal_id` plus its current `session_id`; footprint and genesis are not authorization identifiers. |
| FR-7.3 | The resolution MUST include: sender unique-name from the D-Bus message header, D-Bus peer credentials, sender PID and process start time, an authoritative session registration mapping, and revocation on owner/process exit. |
| FR-7.4 | Process-reuse protection: a new process with a recycled PID MUST NOT inherit the previous process's identity. |
| FR-7.5 | UID-only authority is insufficient — the full binding chain must be established. |
| FR-7.6 | Anonymous calls are denied — no wildcard grants or test identities in production. |
| FR-7.7 | Until the sender-to-Ghostbridge binding exists, production D-Bus calls MUST fail closed (current behavior is correct). |
| FR-7.8 | D-Bus and gRPC callers receive identical authorization enforcement once identity is resolved. |
| FR-7.9 | Registration is capability-gated and resolves `principal_id` from an already authenticated session/OIA mapping. A caller MUST NOT submit or choose its own `principal_id`, genesis, footprint, target unique-name, UID, or PID. |
| FR-7.10 | Registration and logout operate on the current message sender. They MUST NOT accept an arbitrary D-Bus unique-name to bind or revoke. |
| FR-7.11 | `SchemaBackedInterface` obtains the sender through supported zbus message/header injection; the design MUST NOT rely on a fictional interface method such as `self.message_sender()`. |

### FR-8: Local Identity Blob and Optional Oracle Assertion Requirements

| ID | Requirement |
|----|-------------|
| FR-8.1 | The protected local singleton chatbot authenticates MCP HTTP with the exact MutationEngine-authored `SID1` envelope stored in its selected active identity sled and sent as `x-opdbus-sealed-id-bin`. OIA1 is not mandatory for this local chatbot path. |
| FR-8.2 | **One SID1 Author:** After session genesis is minted or recovered, `MutationEngine` seals the complete immutable session identity exactly once into that session sled's `sealed_id` as `sid1:<canonical-unpadded-base64url>`. No client, helper, proxy, footprint component, or catalog reader may mint, rebuild, or reseal it. |
| FR-8.3 | **Header-Only Helper:** Codex invokes the short-lived `op-identity-headers` program through native per-request `http_headers_helper`. The helper resolves the explicitly selected current session, validates the inline envelope against that projection, and writes exactly one JSON object containing the header name/value to stdout. It opens no listener, implements no MCP transport, and is not a shim, proxy, broker, or OAuth exchange. |
| FR-8.4 | **Direct Codex Transport:** Codex connects directly to `https://10.0.0.3:8090/mcp`. Its project configuration contains that URL and `http_headers_helper = "/usr/local/bin/op-identity-headers --session <session-id>"`; it contains no OAuth configuration, bearer token, static identity header, alternate MCP command, or second endpoint. |
| FR-8.5 | **Exact Sled Match:** The bridge decodes canonical SID1, verifies its integrity seal and MCP transport scope, derives its identifiers from the WireGuard key, exact-matches the forwarded bytes and every claim against the selected active, anchored, unexpired authoritative sled, checks principal revocation, and then loads grants only by the exact registered `principal_id`. |
| FR-8.6 | Missing, duplicate, padded/non-canonical, oversized, malformed, tampered, expired, inactive, revoked, wrong-transport, ambiguous-session, projection-drifted, or non-exact SID1 material fails before discovery or dispatch. The bridge rejects a request carrying both SID1 and OIA1. |
| FR-8.7 | SID1 is authored once per immutable session term and may be forwarded unchanged on that session's requests. It is not an OIA1 nonce and is not reminted on retry; deactivating, expiring, revoking, or replacing the authoritative sled immediately makes it unusable. |
| FR-8.8 | SID1 is an identity projection only. Its integrity hash is never a principal, grant key, footprint, Snowball input, vector payload, catalog blob, capability, or authorization shortcut. |
| FR-8.9 | **Optional OIA1 Path:** Other callers may authenticate the same endpoint with a fresh source/target-bound OIA1 from `op-decoy-issuer`. OIA1 remains single-use, receives durable replay protection, and must be newly minted for an OIA-authenticated retry. |
| FR-8.10 | OIA1 issuance authenticates the real principal and any constrained delegation. A dashboard broker, when retained, returns `Cache-Control: no-store`; assertions are never static configuration, logged, persisted as application credentials, or published through MQTT. |
| FR-8.11 | Exactly one identity credential is accepted per request: the exact active-sled SID1 for the selected protected local client, or a fresh valid OIA1 for another caller. Both resolve the same `VerifiedIdentity`, exact principal grants, audience projection, schema validation, MutationEngine admission, and audit path. |
| FR-8.12 | OAuth is absent from this system. No OAuth discovery, authorization, callback, token, refresh-token, PKCE, or same-endpoint OAuth-to-OIA exchange is designed, configured, implemented, or required for Codex. |
| FR-8.13 | EMQX carries only authenticated, schema-versioned internal MQTT/ExHook health, catalog-invalidation, and lifecycle events. It never carries SID1/OIA1 bytes, forwards MCP requests, mints for a principal, or supplies a broker-wide identity. |
| FR-8.14 | `op-identity-headers` logs no blob/header material, accepts only a bounded explicit session selector, fails closed when selection is absent or ambiguous, and exits after writing header JSON. stderr diagnostics are redacted. |
| FR-8.15 | The SID1 sled projection and optional OIA1 assertion path are credential alternatives, not alternate transports or endpoints. Both terminate at the same bridge-owned `/mcp` frontend on `:8090`. |
| FR-8.16 | The deployed Codex version MUST recognize `http_headers_helper` in strict local configuration and invoke it for Streamable HTTP requests. Release acceptance pins and drives the installed client (currently `codex-cli 0.151.0`); a version lacking the helper fails deployment rather than falling back to OAuth, a static header, or a transport shim. |
| FR-8.17 | **Private Credential Projection:** The full sled (including SID1 `sealed_id`) persists only in the root-only durable identity store and `/dev/shm/opdbus/credentials/identity_sled.json` (`0750` directory, `0640 root:secrets` file). `/dev/shm/opdbus/state/identity_sled.json` and every generic D-Bus, gRPC StateSync, subscription, snapshot, method-result, Snowball, vector, schema, web, log, and error surface recursively omit `sealed_id`/SID1. The header helper reads only the private projection and never falls back to public or legacy state. |

### FR-9: MCP Endpoint Configuration

| ID | Requirement |
|----|-------------|
| FR-9.1 | `10.0.0.3:8090` is the one externally configured unified fabric surface itself, carrying MCP at `https://10.0.0.3:8090/mcp` together with gRPC, plugins, mutation, and control capabilities through the bridge's common authority. |
| FR-9.2 | This supersedes the previous topology lock of `10.0.0.2:8090` in `.kiro/specs/README.md`. |
| FR-9.3 | EMQX is an internal standalone broker attached only through loopback MQTT and the dedicated authenticated ExHook listener. It exposes no MCP listener and does not forward MCP transport. |
| FR-9.4 | The retained `emqx` PluginSchema is adapted to the local standalone installation and continues to expose typed D-Bus/generated-gRPC methods and hook controls; it does not become another MCP gateway or endpoint. |
| FR-9.5 | The canonical MCP path follows the stateless protocol version selected by `unified-authenticated-mcp-cognitive-control-plane`: `server/discover` when used, direct `tools/list`, and `tools/call` do not require `initialize`; each request independently presents either the exact selected-sled SID1 or a fresh OIA1. |
| FR-9.6 | Stale or non-exact SID1 and replayed or expired OIA1 are rejected. |
| FR-9.7 | No MCP shim or proxy is introduced for Codex. Native Streamable HTTP connects directly to the sole URL and native `http_headers_helper` supplies only request headers. Any protocol-compatibility code, if retained inside the bridge, creates neither a listener nor identity authority. |
| FR-9.8 | The same bridge router accepts native Codex `initialize` and `notifications/initialized` compatibility messages without creating standing authorization or another endpoint. It derives method/target from the parsed JSON-RPC body; custom `Mcp-Method`/`Mcp-Name` headers are not required from Codex, and `MCP-Protocol-Version` follows standard initialize/subsequent-request behavior. Every protected message still presents exactly one SID1 or OIA1 credential. |

### FR-10: Identity-Sled and Session Genesis

| ID | Requirement |
|----|-------------|
| FR-10.1 | The identity-sled/session-genesis split defined by `session-genesis-identity` is preserved: genesis anchors one session term and footprint means a per-mutation snowball/vector payload. The former "footprint = identity anchor" interpretation remains superseded. |
| FR-10.2 | Exact-`principal_id` capability grants are enforced — no wildcard grants and no footprint-keyed grants. |
| FR-10.3 | Dashboard identity resolution remains session-specific. |
| FR-10.4 | The process-global `/api/identity/sled` fallback is NOT restored. |
| FR-10.5 | Production grant loading rejects the wildcard `"*"` identity entry and any legacy footprint/genesis/hash key. Existing grants are migrated to explicit registered `principal_id` keys before the new projections are enabled; ambiguous or unmapped legacy entries fail closed. |
| FR-10.6 | `principal_id` is resolved directly from the authoritative HumanPrincipal/service-principal record. It MUST NOT be computed from `PluginFootprint`, `session_genesis`, `data_hash`, `content_hash`, `event_hash`, a chain head, or another digest. |
| FR-10.7 | The sled emits one canonical `PluginFootprint` payload envelope per admitted mutation. The Shuttle delivers that envelope to the Snowball append path and the asynchronous vectorization projection; it is not used for authentication, grants, audience, SID1, D-Bus binding, cache partitioning, or tool-set handles. |
| FR-10.8 | Each footprint carries `actor_id = principal_id` and the applicable `session_id`/`session_genesis` as metadata. Consumers read those fields; they MUST NOT derive a new identity by hashing the envelope or any embedded hash. |
| FR-10.9 | The Snowball append operation is the sole author of the chain `event_hash`. It hashes the canonical bounded/redacted payload and prior-link input once, returns the resulting receipt, and MUST NOT rehash a precomputed footprint/content/current-payload hash as a substitute for the payload. `json_args_footprint` (a prehashed arguments digest) is removed rather than fed into the event hash. Vectorization consumes deterministic payload text, not a digest-only surrogate. Explicit hashes of external state/reference artifacts may remain clearly named provenance fields; they are never called a footprint or substituted for the current payload. |
| FR-10.10 | The legacy `derive_human_footprint`, `HumanPrincipalIdentity.footprint`, `GhostbridgeIdentity.footprint`, footprint-keyed `AuthenticatedCaller`, and `capabilities_for_footprint` authorization path are removed or migrated. A compatibility header may be parsed only as a time-bounded genesis alias under `session-genesis-identity`; it never participates in authorization. |
| FR-10.11 | The A.N.N.A. Scribe persona and its established email identity are preserved. Only the unsafe duplicate `anna_scribe` shared-memory/mmap reader and duplicate identity derivation are removed or replaced by the canonical selected-sled projection; implementation work MUST NOT delete or rename the Scribe persona/email surface. This requirement supersedes broader deletion wording in `session-genesis-identity`. |

### FR-11: EMQX Listener and Access Configuration

| ID | Requirement |
|----|-------------|
| FR-11.1 | EMQX listeners bind to loopback (`127.0.0.1`) or Unix domain sockets only — no public network bindings. |
| FR-11.2 | Anonymous MQTT access is disabled in EMQX configuration. |
| FR-11.3 | EMQX API credentials are protected and not hardcoded in source. |
| FR-11.4 | All Netmaker-specific EMQX paths and socket bindings are removed from configuration. |

### FR-12: Audience-Specific MCP Projection

| ID | Requirement |
|----|-------------|
| FR-12.1 | The single endpoint computes discovery and callability from the SID1/OIA1-resolved exact `principal_id`. There is no second compact, Grok, agent, or EMQX MCP server/listener. |
| FR-12.2 | Exactly one configured chatbot/control-plane `principal_id` initially discovers exactly five typed HOT tools: `memory_recall`, `memory_store`, `workflow_query`, `workflow_run`, and `toolsets`. |
| FR-12.3 | The former compact meta-tools `list_tools`, `search_tools`, `get_tool_schema`, and `execute_tool` (including `invoke_tool` aliases) are removed from the active MCP catalog and denied by name for every principal. `toolsets` replaces that lazy-loading surface. |
| FR-12.4 | `clientInfo`, model name, user-agent, request headers, MQTT client ID, and self-declared role are routing hints only and MUST NOT grant the chatbot audience. |
| FR-12.5 | The singleton chatbot `principal_id` is server-side configuration with an explicit, audited rotation procedure; fail-closed behavior applies when it is absent, unregistered, revoked, or ambiguous. |
| FR-12.6 | This section replaces the compact visibility rules in `unified-authenticated-mcp-cognitive-control-plane` FR-3/FR-5. Every selected typed tool still receives exact target-capability enforcement, direct input-schema validation, canonical dispatch, and audit. |
| FR-12.7 | The singleton chatbot's initial `tools/list` is the exact five-tool HOT surface rather than a compact meta-tool or full-inventory surface. `toolsets` selects one authorized typed WARM/COLD set and the client re-lists; there is no generic search/schema/execute escape hatch. |
| FR-12.8 | Audience policy is loaded from protected, versioned server configuration containing the exact singleton `principal_id` and rotation epoch—not a footprint, genesis, client profile name, or glob pattern. Tool-set membership is a separate versioned manifest and contains no grants. |

### FR-13: Always-On HOT Tool Surface

| ID | Requirement |
|----|-------------|
| FR-13.1 | The configured singleton chatbot `principal_id` is provisioned for and initially discovers exactly these five typed HOT tools: `memory_recall`, `memory_store`, `workflow_query`, `workflow_run`, and `toolsets`. Other identities receive only their capability-authorized HOT subset, never compact/generic tools. |
| FR-13.2 | HOT implementations and schemas are in-process, pre-warmed, and available without an EMQX, MQTT, provider-discovery, or other network round trip after canonical request admission. |
| FR-13.3 | HOT does not mean trusted: every HOT request still requires exact active-sled SID1 or fresh OIA1 validation, exact-`principal_id` capability checks, typed input validation, canonical MutationEngine admission, and inline immutable audit before success. |
| FR-13.4 | Durable memory/workflow stores remain authoritative. In-memory indexes and caches are accelerators only and MUST NOT become the sole copy of state. |
| FR-13.5 | The `toolsets` tool and its local catalog snapshot are themselves HOT, so an EMQX outage cannot prevent toolset discovery or reset clients to the full inventory. |
| FR-13.6 | The five names are stable typed projection/facade contracts over existing canonical memory/workflow implementations where those exist. They MUST NOT create a second ToolRegistry, memory store, workflow engine, or duplicate state. Namespace/legacy-alias collisions are resolved fail-closed before catalog activation, and each facade enters canonical dispatch exactly once. |

### FR-14: External-Agent Tool Sets

| ID | Requirement |
|----|-------------|
| FR-14.1 | `toolsets` lists authorized tool sets and selects one typed WARM/COLD projection; selection never grants a capability. The resulting view is `exact grants ∩ audience policy ∩ (HOT core ∪ selected set) ∩ provider health`, with provider health not removing HOT tools. |
| FR-14.2 | In the canonical stateless MCP path, selection is carried as a validated request `_meta` value or server-signed projection handle bound to the resolved principal, authenticated client/session binding, and catalog generation. It is not authentication or server-held authorization state. |
| FR-14.3 | With no valid selection, the singleton chatbot sees the five-tool HOT surface and another identity sees only its authorized subset. An unknown, expired, forged, cross-principal, or unhealthy selection fails closed to that identity's default HOT view or a typed error—never the full inventory. |
| FR-14.4 | After selection and a new `tools/list`, the agent sees the five HOT tools plus actual typed tools in that set, intersected with its exact grants; it does not receive a generic `invoke_tool`/`execute_tool` escape hatch. |
| FR-14.5 | The selection response emits `notifications/tools/list_changed` when negotiated. Clients without that capability are instructed to call `tools/list` explicitly; reconnect/default behavior is deterministic. |
| FR-14.6 | A direct call to a tool outside the current HOT/selected projection is denied even if the name is guessed. Authorization and projection are both checked at call time. |
| FR-14.7 | Tool-set definitions are versioned, deterministic configuration. Promotion/demotion between HOT, WARM, and COLD is an explicit reviewed change, not an automatic consequence of observed call frequency. |
| FR-14.8 | Tool-set drill-down requires MCP-host support, not model behavior: the host client MUST capture the selector returned by `toolsets`, issue a new selector-bearing `tools/list`, install the returned typed schemas into the model-visible catalog, and carry the selector on later calls. Schemas returned inside a tool result do not register callable tools. Real installed Codex and Grok clients MUST be capability-tested; a client without selector-aware re-list stays on the exact HOT five and MUST NOT be reported as having WARM/COLD callability. |

### FR-15: EMQX Role in the Tool Plane

| ID | Requirement |
|----|-------------|
| FR-15.1 | EMQX is optional infrastructure for asynchronous provider health, catalog invalidation, lifecycle events, and WARM/COLD coordination. It is not an authorization authority or an MCP execution bypass. |
| FR-15.2 | EMQX is never on the synchronous HOT request path and never carries OIA material. Audit is committed inline through MutationEngine before any best-effort MQTT event mirror. |
| FR-15.3 | When EMQX is stopped, the one MCP endpoint, authentication, the five HOT tools, and `toolsets` remain operational. Affected WARM/COLD sets report typed unavailability without widening discovery. |
| FR-15.4 | EMQX-delivered catalog/health messages are authenticated, schema-validated, generation/version checked, and treated as hints until reconciled with authoritative local configuration. |

---

## Non-Functional Requirements

### NFR-1: Fail-Closed Security

| ID | Requirement |
|----|-------------|
| NFR-1.1 | All gRPC and MCP calls are fail-closed by default. |
| NFR-1.2 | Missing or invalid identity assertions result in denial, not fallback. |
| NFR-1.3 | Capability checks fail closed when the capability is not granted. |
| NFR-1.4 | ExHook calls from unauthorized sources are rejected. |
| NFR-1.5 | Production D-Bus calls fail closed until the complete sender/session binding resolves, then receive the same exact-grant enforcement as gRPC/MCP. |

### NFR-2: Deployment

| ID | Requirement |
|----|-------------|
| NFR-2.1 | Deployment uses `deploy/runit/build-golden.sh` btrfs release path exclusively. |
| NFR-2.2 | Binaries are never hand-copied onto the host as a deployment step. |
| NFR-2.3 | `CXXFLAGS="-include cstdint"` is used for all Cargo workspace builds. |
| NFR-2.4 | The runit service definition is tracked at `deploy/runit/emqx/run` in the repository. |
| NFR-2.5 | Service registration and enablement are managed by `build-golden.sh`, not manual symlinks. |
| NFR-2.6 | EMQX version is pinned with checksum in deployment configuration. |
| NFR-2.7 | Any retained local ExHook/MQTT integration plugin version is pinned with checksum; no EMQX MCP Gateway artifact is deployed. |
| NFR-2.8 | Golden subvolume staging separates runtime binaries from persistent data. |
| NFR-2.9 | No boot-time downloads of EMQX or plugins — all artifacts are pre-staged. |

### NFR-3: Service Management

| ID | Requirement |
|----|-------------|
| NFR-3.1 | Host services use runit only — `sudo sv` commands. |
| NFR-3.2 | `systemctl` and `s6` commands are never used. |
| NFR-3.3 | Network-critical services are never auto-restarted without deliberate console action. |

### NFR-4: Xray Configuration Policy

| ID | Requirement |
|----|-------------|
| NFR-4.1 | Xray's live configuration exists only at `/etc/xray/xray_config.json` inside the container. |
| NFR-4.2 | Models do not write or reload Xray directly. |

### NFR-5: Accountability

| ID | Requirement |
|----|-------------|
| NFR-5.1 | Every method call is recorded in the immutable event chain before returning success. |
| NFR-5.2 | The audit trail includes `actor_id = principal_id`, `plugin_id`, `method_name`, `capability_id`, and a canonical bounded/redacted mutation payload. The Snowball append path—not identity/auth code—authors its chain hash and receipt. |
| NFR-5.3 | ExHook callbacks are recorded through the MutationEngine audit pipeline. |
| NFR-5.4 | Exactly one canonical footprint envelope type and one Snowball chain-hash author remain. Duplicate/legacy footprint generators and `json_args_footprint` paths that prehash the current payload and then include that digest in another current-event hash are removed. |

### NFR-6: HOT-Path Performance and Availability

| ID | Requirement |
|----|-------------|
| NFR-6.1 | Phase 0 records p50/p95 admission-to-dispatch baselines for the five HOT tools; implementation adds no synchronous broker/provider hop and defines a reviewed regression budget before rollout. |
| NFR-6.2 | The HOT catalog and schemas are resident before the MCP listener reports ready. |
| NFR-6.3 | EMQX failure is tested as a degraded WARM/COLD condition, not as MCP endpoint or HOT-path failure. |

---

## Current State Analysis (Known Gaps)

| Gap | Description | Impact |
|-----|-------------|--------|
| G-1 | The existing EMQX plugin still describes Netmaker and old Netmaker sockets. | Plugin state is incorrect for standalone operation. |
| G-2 | No standalone `emqx` runit service is installed. | EMQX cannot be supervised as a standalone service. |
| G-3 | The EMQX dispatcher is not connected to `MutationEngine::dispatch_method_call`. | Plugin methods fall through to generic echo. |
| G-4 | Production D-Bus `PluginV1.Call` currently fails closed because sender identity is unresolved (`schema_router.rs:803`). | D-Bus calls cannot execute real plugin methods — this is correct behavior until identity binding exists. |
| G-5 | `/mcp` correctly returns 401 without either accepted identity credential; the protected local chatbot requires a working SID1 projection/header-helper path and other callers may require OIA1 issuance. | MCP clients cannot authenticate until their selected credential path is complete. |
| G-6 | ExHook is mounted on shared route without interceptor (`grpc_server.rs:867`), relying only on IGNORE returns. | Forged hook calls could be accepted if not gated. |
| G-8 | Current MCP discovery exposes a broad capability-filtered inventory and retains four compact/generic meta-tools. | The singleton chatbot receives too many tools and a generic execution path instead of the exact five typed HOT tools plus `toolsets`. |
| G-9 | No HOT/WARM/COLD projection or typed tool-set selection exists. | Grok and other agents cannot start with the required five-tool surface. |
| G-10 | Codex must use its native per-request `http_headers_helper` to invoke `op-identity-headers` and forward the selected sled's exact SID1 while keeping the MCP URL direct. | A direct URL without the helper correctly returns 401; adding OAuth, a shim, or a proxy would implement the wrong architecture. |
| G-11 | `deploy/security/capability-grants.json` currently contains a broad `"*"` identity and a 64-hex legacy footprint/hash key; `mcp_frontend.rs` derives `human-footprint` even though it already has `principal_id`. | Authorization is bound to derived hashes and can be bypassed by the wildcard; migrate to registered `principal_id` keys and reject all hash-keyed/wildcard grants. |
| G-12 | `op-snowball` contains duplicate footprint implementations, including a legacy generator that computes `content_hash` from `data_hash`; `ChainEvent.json_args_footprint` hashes raw arguments and the event hash then commits that digest; bridge identity types also overload footprint for genesis or human identity. | Current payload is reduced to a hash and hashed again while vectorization loses its semantic body; identity, session, payload, and receipt are conflated. Consolidate to one bounded/redacted payload envelope and one Snowball hash author. |

---

## Acceptance Criteria

### AC-1: Service Health
- [ ] Standalone EMQX starts under runit and reports healthy.
- [ ] `sudo sv status emqx` returns `run:` with a process ID.
- [ ] EMQX version matches pinned version in deployment config.

### AC-2: ExHook Integration
- [ ] ExHook loads and callback events reach the audited mutation pipeline.
- [ ] Hook events are visible in the event chain.
- [ ] ExHook listener is on dedicated local UDS or loopback with peer validation.
- [ ] Forged ExHook calls from non-EMQX sources are rejected.

### AC-3: Authentication Enforcement
- [ ] Unauthenticated D-Bus calls are denied (current behavior preserved until identity binding exists).
- [ ] Unauthenticated gRPC calls are denied.
- [ ] Unauthenticated MCP calls are denied.

### AC-4: Authenticated Operations
- [ ] Authenticated D-Bus `PluginV1.Call` executes real EMQX methods (requires FR-7 implementation).
- [ ] Authenticated gRPC `PluginService.CallMethod` executes the same methods.
- [ ] Authenticated generated gRPC RPCs execute the same methods.

### AC-5: Reflection
- [ ] Generated gRPC reflection exposes typed EMQX request/response messages.
- [ ] gRPC clients can discover EMQX methods via reflection.

### AC-6: MCP Operations
- [ ] A real Codex process connects directly to `https://10.0.0.3:8090/mcp`; its native per-request helper forwards the selected sled's exact SID1 and `tools/list` returns the projected tools.
- [ ] Canonical stateless MCP `tools/call` executes an authorized typed EMQX method after `toolsets` selection with another exact SID1 header read; no shim, proxy, OAuth route, or second endpoint participates.
- [ ] Native Codex `initialize`/`notifications/initialized` succeeds in the same bridge router with one credential per protected message and no custom `Mcp-Method`/`Mcp-Name` requirement or authorization-bearing session.
- [ ] Missing, malformed, tampered, non-exact, inactive, expired, revoked, or wrong-transport SID1 is rejected, as is a request containing both SID1 and OIA1.
- [ ] An optional direct-OIA caller can list/call with a fresh OIA1; an MCP session ID alone never authenticates.
- [ ] Replayed OIA assertions are rejected.
- [ ] Expired OIA assertions are rejected.
- [ ] Source-mismatched OIA assertions are rejected.
- [ ] Concurrent replay attempts are rejected.
- [ ] Replay after service restart is rejected (nonce store survives restart).

### AC-7: Capability Enforcement
- [ ] Exact-`principal_id` capability denial cases are tested.
- [ ] Exact-`principal_id` capability success cases are tested.
- [ ] Production startup rejects wildcard, footprint-, genesis-, and hash-keyed identity grants; migrated configuration contains registered principal IDs only.

### AC-8: Endpoint Configuration
- [ ] `10.0.0.3:8090` is the only client-facing mesh-private unified fabric surface and `/mcp` is its sole MCP route; the same surface carries gRPC, plugins, mutation, and control capabilities.
- [ ] Listener scan proves no other client-facing `/mcp` endpoints.
- [ ] EMQX listeners bind only to loopback or UDS.

### AC-9: Dashboard Integration
- [ ] Dashboard pairing/session identity survives reload.
- [ ] Dashboard calls authenticated gRPC/MCP correctly.
- [ ] Session-specific identity resolution works.

### AC-10: MQTT Security
- [ ] Anonymous MQTT connections are denied.
- [ ] Every `$mcp-*` publish/subscribe is denied, and unauthorized access to the internal `$opdbus/fabric/*` event namespace is denied.
- [ ] EMQX authentication and authorization are independently enforced.

### AC-11: Netmaker Removal
- [ ] All Netmaker-specific EMQX paths are removed from configuration.
- [ ] All Netmaker-specific socket bindings are removed.
- [ ] No references to "NetMaker" remain in emqx.rs.

### AC-12: D-Bus Identity (Required for E2E)
- [ ] D-Bus sender unique-name to registered `principal_id` plus current session resolution exists.
- [ ] Process-reuse does not inherit previous identity.
- [ ] Revocation occurs on process exit.
- [ ] A caller cannot register a self-chosen principal/footprint or bind/revoke another sender.

### AC-13: Test Suite
- [ ] Relevant Rust unit tests pass.
- [ ] Relevant TypeScript tests pass.
- [ ] Integration tests pass.
- [ ] Browser tests pass (where applicable).
- [ ] CI validates OSCAL subid uniqueness.

### AC-14: Audience Projection
- [ ] The configured singleton chatbot `principal_id` initially sees exactly `memory_recall`, `memory_store`, `workflow_query`, `workflow_run`, and `toolsets`.
- [ ] The four former compact meta-tools and every generic execution alias are absent and denied for the singleton, Grok, and at least one other identity.
- [ ] Spoofing `clientInfo`, model name, headers, or MQTT client ID never changes that result.

### AC-15: Five-Tool HOT Surface and Tool Sets
- [ ] The singleton chatbot's initial `tools/list` returns exactly `memory_recall`, `memory_store`, `workflow_query`, `workflow_run`, and `toolsets`.
- [ ] Selecting an authorized set and re-listing returns the five HOT core tools plus only its authorized typed tools from that set.
- [ ] Selection cannot widen exact capability grants; forged, cross-principal, unknown, expired, and unhealthy selections fail closed.
- [ ] A guessed call outside the active projection is denied.

### AC-16: EMQX Failure Isolation
- [ ] With EMQX stopped, the single MCP endpoint authenticates requests and all five HOT tools execute successfully.
- [ ] The singleton chatbot's five-tool HOT path and `toolsets` remain available.
- [ ] EMQX-dependent WARM/COLD sets report typed unavailability and do not disappear into a full-inventory fallback.

### AC-17: Direct SID1 and Optional OIA1 Client Paths
- [ ] `MutationEngine` authors one SID1 into the selected identity sled; `op-identity-headers` forwards the exact canonical bytes as header JSON on each native Codex request and never listens or transports MCP.
- [ ] A real Codex MCP session uses only `https://10.0.0.3:8090/mcp` plus `http_headers_helper`; two requests exact-match the same active session-term SID1 without reminting it.
- [ ] The helper fails closed for absent/ambiguous/stale sessions and the bridge rejects projection drift, revocation, expiry, altered claims, or a principal without exact grants.
- [ ] Optional OIA1 callers still prove fresh issuance and replay rejection independently of the local chatbot SID1 path.
- [ ] No OAuth routes/configuration, MCP shim/proxy, static assertion, bearer token, or self-asserted footprint exists.

### AC-18: Identity / Genesis / Footprint Separation
- [ ] Two active sessions for the same `principal_id` receive the same grants/audience while retaining distinct session/genesis/footprint records.
- [ ] Changing a payload, footprint, chain head, genesis, `data_hash`, `content_hash`, or `event_hash` never changes `principal_id` or its grants.
- [ ] The same payload or digest attributed to two principals never transfers authority between them.
- [ ] `rg`/semantic gates find no footprint/genesis/hash-to-principal derivation and no authorization lookup keyed by a 64-hex digest.
- [ ] One canonical sled-emitted footprint payload reaches both Snowball append and vectorization; Snowball computes the chain hash once and vectorization receives payload text rather than a hash-only body.

### AC-19: A.N.N.A. Scribe Preservation
- [ ] The A.N.N.A. Scribe persona/name and established email identity remain unchanged.
- [ ] The unsafe duplicate mmap reader of `/dev/shm/plugin_schema.dat` and duplicate identity derivation are absent; Scribe uses the canonical selected-sled projection if it needs identity data.
- [ ] A migration from a revision that deleted the entire Scribe surface restores the preserved persona/email contract without restoring the unsafe reader.

---

## References

- `AGENTS.md` — Mandatory skill preload and host-service policy
- `.kiro/specs/README.md` — Updated topology lock and active overlay index
- `.kiro/specs/unified-authenticated-mcp-cognitive-control-plane/` — Canonical MCP spec
- `https://developers.openai.com/codex/mcp/` — Codex Streamable HTTP MCP configuration; the deployed client uses native `http_headers_helper` with the direct URL
- `.kiro/specs/netmaker-xray-identity-handoff/` — Oracle decoy, signed assertion, HumanPrincipal
- `.kiro/specs/session-genesis-identity/` — binding split: principal identity, session genesis, and per-mutation snowball footprint
- `.agents/skills/grpc-expert/SKILL.md` — PluginSchema, seal/freeze/hot pipeline
- `crates/op-plugins/src/state_plugins/emqx.rs` — Current EMQX plugin (to be modified)
- `crates/op-grpc-bridge/src/emqx_hook_provider.rs` — ExHook callback handler
- `crates/op-grpc-bridge/src/mutation_engine.rs` — Central dispatch coordinator
- `crates/op-grpc-bridge/src/mcp_frontend.rs` — MCP HTTP frontend with OIA validation
- `crates/op-grpc-bridge/src/schema_router.rs` — D-Bus interface (line 803: identity gap)
- `crates/op-grpc-bridge/src/grpc_server.rs` — ExHook mounting (line 867: no interceptor)
- `crates/op-plugins/src/state_plugins/plugin_scaffold_helpers.rs` — AckOutput definition (line 369)
