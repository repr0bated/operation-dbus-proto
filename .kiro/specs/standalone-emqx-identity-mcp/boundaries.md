# Standalone EMQX Identity MCP — Boundaries

## Overview

This document defines the architectural boundaries, constraints, and invariants that govern the standalone EMQX identity pipeline integration. These boundaries are **non-negotiable** — they represent fundamental design decisions that must not be violated during implementation.

---

## Unified Fabric Surface

**This spec supersedes the topology lock in `.kiro/specs/README.md`.**

`10.0.0.3:8090` is the mesh-private unified fabric surface itself—not merely one
endpoint on a larger fabric. The bridge owns the surface and multiplexes MCP at
`/mcp`, native gRPC/gRPC-Web, generated plugin methods, and mutation/control
capabilities through one authentication and dispatch authority. EMQX is internal on
loopback MQTT/ExHook behind the surface and never exposes another external endpoint.

---

## System Boundaries

### B-1: EMQX Deployment Boundary

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              HOST (runit PID 1)                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────────────┐    ┌──────────────────────┐                       │
│  │    emqx (runit)      │    │   op-grpc-bridge     │                       │
│  │  /etc/runit/sv/emqx  │    │   (zeroclaw host)    │                       │
│  │  tracked in repo at  │    │                      │                       │
│  │  deploy/runit/emqx/  │    │  - ExHook provider   │                       │
│  │                      │    │    (proven UDS or    │                       │
│  │                      │    │     loopback mTLS)   │                       │
│  │  - MQTT loopback     │◄───┤  - Plugin dispatch   │                       │
│  │  - Dashboard :18083  │    │  - gRPC :8090        │                       │
│  │  - ExHook client     │    │  - MCP /mcp          │                       │
│  │  - NO public binds   │    │                      │                       │
│  └──────────────────────┘    └──────────────────────┘                       │
│           │                            │                                     │
│           │ ExHook gRPC (local only)   │ D-Bus                               │
│           ▼                            ▼                                     │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                     MutationEngine (authoritative)                   │    │
│  │  - Event chain (audit)                                               │    │
│  │  - State cache                                                       │    │
│  │  - Method dispatch                                                   │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Constraints:**
- EMQX is a **standalone local service**, NOT:
  - A Netmaker dependency
  - A container-managed service
  - A systemd service
- Service supervision uses runit exclusively
- No shared fate with Netmaker or any container orchestrator
- Listeners bind to loopback or UDS only — NO public network bindings

### B-2: Identity Authority Boundary

```text
selected local chatbot sled                 other authenticated callers
MutationEngine-authored SID1                optional fresh OIA1
          │                                         │
          │ Codex native http_headers_helper        │ direct header
          │ (header JSON only)                      │
          └─────────────────┬───────────────────────┘
                            ▼
                 10.0.0.3:8090 unified fabric
                            │
             require exactly one identity credential
                ┌───────────┴───────────┐
                ▼                       ▼
        exact active-sled match    signature/time/binding/
        of complete SID1           durable nonce validation
                └───────────┬───────────┘
                            ▼
        registered principal_id → exact grants → projection
                            ▼
                 canonical MutationEngine dispatch

D-Bus caller → sender/peer/process/session resolver → same exact-principal grants
```

**Invariants:**
- SID1 is authored once by MutationEngine into the selected active identity sled;
  `op-identity-headers` forwards its exact bytes and opens no listener or transport.
- OIA1 remains an optional fresh, one-use path for other callers; an OIA retry mints
  another assertion. SID1 is not reminted per request or governed by the OIA nonce.
- A request carries exactly one credential. Missing, dual, malformed, stale, revoked,
  or non-exact credentials fail before discovery or dispatch.
- The bridge is the sole identity validator for gRPC/MCP; D-Bus uses its separate
  sender resolver and no path has an implicit-local bypass.
- Capability enforcement is exact-`principal_id`, never footprint/hash keyed or wildcard.
- Tool audience and tool-set projection are derived only after either credential
  resolves the same registered `principal_id`.
- OAuth, bearer/static identity headers, MCP shims/proxies, MQTT credential relay, and
  broker-wide identities are prohibited.

### B-2a: Footprint Transport Boundary

```text
VerifiedIdentity
  principal_id ───────────────┐
  session_id ─────────────────┤ injected as typed event metadata
  session_genesis ────────────┘
                              ▼
                     ┌─────────────────────────┐
sled emits payload ─►│ PluginFootprint envelope│
                     │ canonical event payload │
                     └────────────┬────────────┘
                                  │ Shuttle delivers the same payload
                         ┌────────┴─────────┐
                         ▼                  ▼
                 Snowball append       vectorization
                 one chain hash        payload text
                 → event receipt       → async projection
```

The footprint boundary begins only after authentication, exact-principal grant/audience
checks, schema validation, and MutationEngine admission. `actor_id = principal_id` is
metadata inside the footprint; it is not converted into a footprint. The footprint is
never an SID1 source, grant key, audience key, D-Bus identity, session credential, or
tool-set handle. Snowball is the only current-event chain-hash author: no component
prehashes a footprint/current event and asks Snowball to hash that digest again.

The A.N.N.A. Scribe persona and its established email identity remain outside this
type cleanup and are preserved. Only Scribe's unsafe duplicate
`/dev/shm/plugin_schema.dat` mmap reader and duplicate identity derivation are removed;
any necessary identity read uses the canonical selected-sled projection.

### B-3: SID1 Header-Helper Boundary and Optional OIA1

```text
MutationEngine ──authors once──▶ selected sled `sealed_id = sid1:<base64url>`
                                      │
                  private `root:secrets` credential projection
                  (public state + generic surfaces omit `sealed_id`)
                                      ▼
                           op-identity-headers
                     stdout: one JSON header object
                     listeners/transports/proxies: none
                                      │
                                      ▼
                    native Codex direct HTTPS request
                    to 10.0.0.3:8090/mcp
                                      │
                                      ▼
                      bridge exact active-sled match

other caller ──trusted issuer──▶ fresh OIA1 ──same direct endpoint/validator
```

The helper cannot mint, edit, cache as authority, or reconstruct SID1; its only output
is `x-opdbus-sealed-id-bin` JSON containing the selected sled's exact canonical
value. The same router handles native Codex initialize/list/call and derives the MCP
method/target from JSON-RPC rather than requiring custom method/name headers. Optional
OIA1 retains source/target binding, short TTL, and durable nonce replay protection.
Neither credential crosses MQTT, and EMQX is not an MCP gateway.

### B-4: Plugin Schema Boundary

```
┌───────────────────────────────────────────────────────────────────────────────┐
│                          PLUGIN SCHEMA BOUNDARY                                │
├───────────────────────────────────────────────────────────────────────────────┤
│                                                                                │
│  Source of Truth                     Generated Artifacts (NEVER hand-edit)    │
│  ───────────────                     ────────────────────────────────────     │
│                                                                                │
│  ┌─────────────────────────────┐     ┌─────────────────────────────────────┐  │
│  │  emqx.rs                    │     │  plugin_methods.proto               │  │
│  │  (crates/op-plugins/        │────►│  (target/*/build/op-grpc-bridge-*/  │  │
│  │   src/state_plugins/)       │     │   out/)                             │  │
│  │                             │     │                                     │  │
│  │  - EmqxState struct         │     │  - EmqxPluginMethods service        │  │
│  │  - emqx_schema() function   │     │  - GetStatusRequest/Response        │  │
│  │  - Input/Output types       │     │  - ListListenersRequest/Response    │  │
│  │  - MethodDecl entries       │     │  - etc.                             │  │
│  │  - CapabilityDecl entries   │     │                                     │  │
│  └─────────────────────────────┘     └─────────────────────────────────────┘  │
│              │                                       │                        │
│              │                                       │                        │
│              ▼                                       ▼                        │
│  ┌─────────────────────────────┐     ┌─────────────────────────────────────┐  │
│  │  Sealed Blob                │     │  plugin_method_routes.rs            │  │
│  │  /dev/shm/opdbus/           │     │  (generated Axum/tonic routing)     │  │
│  │  plugin-blobs/emqx.*.blob   │     │                                     │  │
│  └─────────────────────────────┘     └─────────────────────────────────────┘  │
│                                                                                │
│  INVARIANTS:                                                                   │
│  • PluginSchema IS the contract — there is no separate IDL                     │
│  • build.rs regenerates on every `cargo build -p op-grpc-bridge`               │
│  • Field numbers use FNV-1a hash (stable_field_number), not sequential         │
│  • Empty schema.methods → plugin skipped by build.rs                           │
│  • AckOutput MAY be used for ack-only methods (from plugin_scaffold_helpers)   │
│  • Lifecycle methods MUST return richer results (PID, status, etc.)            │
│  • Subid uniqueness enforced by CI test, not manual registry edits             │
│  • Schema/default/example construction is deterministic and has no live I/O     │
│  • Credentials and assertion material never appear in state/schema/audit        │
│                                                                                │
└───────────────────────────────────────────────────────────────────────────────┘
```

### B-5: ExHook Module Boundary

```
┌───────────────────────────────────────────────────────────────────────────────┐
│                     EXHOOK MODULE BOUNDARY                                     │
├───────────────────────────────────────────────────────────────────────────────┤
│                                                                                │
│  Current Location (Module within op-grpc-bridge)                              │
│  ───────────────────────────────────────────────                              │
│                                                                                │
│  crates/op-grpc-bridge/                                                       │
│  └── src/                                                                     │
│      └── emqx_hook_provider.rs    ◄── ExHook HookProvider implementation      │
│                                                                                │
│  This is a MODULE, not a separate crate. If cleaner boundaries are required,  │
│  extraction to `crates/op-emqx-hooks/` is an option.                          │
│                                                                                │
│  ┌─────────────────────────────────────────────────────────────────────────┐  │
│  │                     SECURITY BOUNDARY (CRITICAL)                        │  │
│  │                                                                          │  │
│  │  Current State (grpc_server.rs:867):                                    │  │
│  │  - ExHook mounted on shared route WITHOUT interceptor                    │  │
│  │  - Relies on EMQX being the only caller + IGNORE returns                 │  │
│  │  - UNSAFE: forged calls could be accepted                                │  │
│  │                                                                          │  │
│  │  Required State:                                                         │  │
│  │  - First prove the pinned EMQX release supports the chosen target URI     │  │
│  │  - ExHook on DEDICATED local UDS (if supported) or loopback listener      │  │
│  │  - UDS: ownership + UID/GID/PID/start-time/process provenance             │  │
│  │  - Loopback: mTLS with a dedicated, pinned EMQX client identity           │  │
│  │  - Rejection of calls from any other source                              │  │
│  │                                                                          │  │
│  └─────────────────────────────────────────────────────────────────────────┘  │
│                                                                                │
│  Present-State Plugin                Hook Provider (Same Crate, Separate File)│
│  ───────────────────                 ─────────────────────────────────────────│
│                                                                                │
│  ┌─────────────────────────────┐     ┌─────────────────────────────────────┐  │
│  │  emqx.rs                    │     │  emqx_hook_provider.rs              │  │
│  │  (op-plugins crate)         │     │  (op-grpc-bridge crate)             │  │
│  │                             │     │                                     │  │
│  │  RESPONSIBILITIES:          │     │  RESPONSIBILITIES:                  │  │
│  │  - Plugin state definition  │     │  - HookProvider gRPC service        │  │
│  │  - Schema declaration       │     │  - emqx.exhook.v3 implementation    │  │
│  │  - Method dispatch          │     │  - Event recording via              │  │
│  │  - Status/listener queries  │     │    MutationEngine                   │  │
│  │  - Lifecycle delegation     │     │  - Audit trail integration          │  │
│  │                             │     │                                     │  │
│  │  DOES NOT:                  │     │  DOES NOT:                          │  │
│  │  ✗ Implement hook callbacks │     │  ✗ Define plugin schema             │  │
│  │  ✗ Handle ExHook protocol   │     │  ✗ Query EMQX state                 │  │
│  │  ✗ Make auth decisions      │     │  ✗ Make auth decisions              │  │
│  └─────────────────────────────┘     │    (returns IGNORE only when        │  │
│                                      │     EMQX auth is enforced)          │  │
│                                      └─────────────────────────────────────┘  │
│                                                                                │
│  BOTH SHARE:                                                                   │
│  • MutationEngine for accountability                                           │
│  • Event chain for audit                                                       │
│  • Same identity/capability model                                              │
│                                                                                │
└───────────────────────────────────────────────────────────────────────────────┘
```

### B-6: Unified Fabric Surface Boundary

```
┌───────────────────────────────────────────────────────────────────────────────┐
│                         UNIFIED FABRIC SURFACE                                 │
├───────────────────────────────────────────────────────────────────────────────┤
│                                                                                │
│  EXTERNAL FABRIC (exactly ONE)       INTERNAL EMQX ATTACHMENT                 │
│  ─────────────────────────────       ────────────────────────                 │
│                                                                                │
│  ┌─────────────────────────────┐     ┌─────────────────────────────────────┐  │
│  │  10.0.0.3:8090 fabric       │     │  EMQX loopback MQTT + ExHook       │  │
│  │  /mcp + gRPC + plugins +    │     │  (internal only)                    │  │
│  │  mutation/control           │     │                                     │  │
│  │                             │     │                                     │  │
│  │  - Mesh-private surface     │     │  - NOT an MCP endpoint              │  │
│  │  - exact SID1 or fresh OIA1 │     │  - Catalog/health/hook events       │  │
│  │  - Capability-gated         │     │  - Principal calls only after       │  │
│  │  - One bridge authority     │     │    canonical bridge admission       │  │
│  │                             │     │  - NO credential/tool forwarding   │  │
│  └─────────────────────────────┘     └─────────────────────────────────────┘  │
│                                                                                │
│  PROHIBITED:                                                                   │
│  ✗ Second external fabric or MCP endpoint                                      │
│  ✗ EMQX exposing or forwarding an MCP listener                                 │
│  ✗ Unauthenticated MCP access                                                  │
│  ✗ MCP bypass of capability enforcement                                        │
│  ✗ MCP shim/proxy/gateway in the Codex path                                    │
│  ✗ SID1/OIA1 relay through MQTT                                                │
│                                                                                │
└───────────────────────────────────────────────────────────────────────────────┘
```

### B-7: D-Bus Identity Boundary

```
┌───────────────────────────────────────────────────────────────────────────────┐
│                         D-BUS IDENTITY BOUNDARY                                │
├───────────────────────────────────────────────────────────────────────────────┤
│                                                                                │
│  D-Bus does NOT use the gRPC interceptor. A separate mechanism is required.   │
│                                                                                │
│  ┌─────────────────────────────────────────────────────────────────────────┐  │
│  │                    D-BUS SENDER IDENTITY RESOLUTION                      │  │
│  │                                                                          │  │
│  │  Required Binding Chain:                                                 │  │
│  │  ┌──────────────┐                                                        │  │
│  │  │ D-Bus Sender │                                                        │  │
│  │  │ unique-name  │ ─► peer creds/PID/start ─► authoritative session      │  │
│  │  │ :1.XXX       │                            ─► registered principal_id   │  │
│  │  └──────────────┘                                                        │  │
│  │                                                                          │  │
│  │  Properties:                                                             │  │
│  │  • unique-name: bus-assigned, connection-scoped                          │  │
│  │  • Peer credentials: read from the bus, never supplied by the caller      │  │
│  │  • PID + process start time: protects against PID reuse                   │  │
│  │  • Session: already-authenticated Ghostbridge session                     │  │
│  │  • Principal: resolved from the authoritative registry, never supplied    │  │
│  │  • Session/genesis: correlation + chain stamp, never grant authority      │  │
│  │                                                                          │  │
│  │  Protections:                                                            │  │
│  │  • Revocation on process exit (unique-name disappears)                   │  │
│  │  • Process-reuse protection (PID + start time + unique owner)             │  │
│  │  • No UID-only authority                                                 │  │
│  │  • Register/logout act only on the current D-Bus sender                   │  │
│  │                                                                          │  │
│  │  Current State (schema_router.rs:803):                                   │  │
│  │  • No authoritative sender unique-name → principal/session resolver      │  │
│  │  • Production D-Bus deliberately DENIES until binding exists             │  │
│  │  • This is CORRECT fail-closed behavior                                  │  │
│  │                                                                          │  │
│  └─────────────────────────────────────────────────────────────────────────┘  │
│                                                                                │
│  Transport Terminology:                                                        │
│  • D-Bus: PluginV1.Call                                                        │
│  • gRPC (generic): PluginService.CallMethod                                    │
│  • gRPC (generated): EmqxPluginMethods.GetStatus, etc.                         │
│                                                                                │
└───────────────────────────────────────────────────────────────────────────────┘
```

### B-8: Service Management Boundary

```
┌───────────────────────────────────────────────────────────────────────────────┐
│                        SERVICE MANAGEMENT BOUNDARY                             │
├───────────────────────────────────────────────────────────────────────────────┤
│                                                                                │
│  ALLOWED                             PROHIBITED                                │
│  ───────                             ──────────                                │
│                                                                                │
│  ┌─────────────────────────────┐     ┌─────────────────────────────────────┐  │
│  │  sudo sv start emqx         │     │  systemctl start emqx               │  │
│  │  sudo sv stop emqx          │     │  systemctl stop emqx                │  │
│  │  sudo sv restart emqx       │     │  systemctl restart emqx             │  │
│  │  sudo sv status emqx        │     │  service emqx start                 │  │
│  │                             │     │  s6-rc -u change emqx               │  │
│  │  deploy/runit/emqx/run      │     │  /run/service/* direct edits        │  │
│  │  (tracked in repo)          │     │  runsv / runsvdir by hand           │  │
│  │                             │     │  Manual symlink creation            │  │
│  │  build-golden.sh managed    │     │                                     │  │
│  │  service registration       │     │                                     │  │
│  │                             │     │                                     │  │
│  │  Plugin → authorized D-Bus  │     │  Plugin shelling out to sv          │  │
│  │  manager → literal `emqx`   │     │  Caller-selected service name       │  │
│  └─────────────────────────────┘     └─────────────────────────────────────┘  │
│                                                                                │
└───────────────────────────────────────────────────────────────────────────────┘
```

### B-9: Deployment Boundary

```
┌───────────────────────────────────────────────────────────────────────────────┐
│                           DEPLOYMENT BOUNDARY                                  │
├───────────────────────────────────────────────────────────────────────────────┤
│                                                                                │
│  ARTIFACT PROVENANCE REQUIREMENTS                                              │
│  ────────────────────────────────                                              │
│                                                                                │
│  ┌─────────────────────────────────────────────────────────────────────────┐  │
│  │  Tracked in Repository:                                                  │  │
│  │  • deploy/runit/emqx/run          (service definition)                   │  │
│  │  • deploy/config/emqx.version     (pinned version + checksum)            │  │
│  │  • optional emqx-integration.version (ExHook/MQTT integration checksum) │  │
│  │  • deterministic offline artifact source/location for retained packages  │  │
│  │  • no EMQX MCP Gateway artifact                                          │  │
│  │                                                                          │  │
│  │  Golden Subvolume:                                                       │  │
│  │  • Pre-staged EMQX binary (no boot-time download)                        │  │
│  │  • Pre-staged plugins (no boot-time download)                            │  │
│  │  • Immutable runtime artifacts                                           │  │
│  │                                                                          │  │
│  │  Data Subvolume (separate):                                              │  │
│  │  • /var/lib/emqx (persistent data)                                       │  │
│  │  • Survives golden subvolume updates                                     │  │
│  │                                                                          │  │
│  │  build-golden.sh responsibilities:                                       │  │
│  │  • Service registration                                                  │  │
│  │  • Service enablement                                                    │  │
│  │  • Version verification                                                  │  │
│  │  • Checksum validation                                                   │  │
│  └─────────────────────────────────────────────────────────────────────────┘  │
│                                                                                │
│  PROHIBITED:                                                                   │
│  ✗ scp binary to host                                                          │
│  ✗ rsync /target/release/...                                                   │
│  ✗ curl ... | sudo tee /usr/bin/...                                            │
│  ✗ Any manual binary copy                                                      │
│  ✗ Boot-time EMQX download                                                     │
│  ✗ Boot-time plugin download                                                   │
│  ✗ Manual symlink creation for service enablement                              │
│                                                                                │
└───────────────────────────────────────────────────────────────────────────────┘
```

### B-10: MCP Audience and Tool-Set Boundary

```
                    ONE MESH-PRIVATE UNIFIED FABRIC SURFACE
                                      │
                exact SID1 or fresh OIA1 → registered principal_id
                                      │
                 exact grants ∩ audience ∩ projection ∩ health
                         ┌────────────┴────────────┐
                         │                         │
              singleton chatbot              external agents
              exactly five typed HOT         authorized HOT subset
              tools, including toolsets      (no compact/invoke)
                                                   │
                                      toolsets: select exactly one
                                                   │
                              HOT + one typed authorized WARM/COLD set
```

**Invariants:**

- Only one server-configured exact chatbot `principal_id` receives the complete exact
  five-tool HOT default.
- `list_tools`, `search_tools`, `get_tool_schema`, `execute_tool`, and `invoke_tool`
  are absent from the active MCP catalog and denied for every identity.
- `clientInfo`, model name, headers, MQTT client ID, and self-declared roles never
  select an audience.
- Tool-set selection is a view restriction, never a grant. The canonical stateless
  path carries a principal/client-bound selection/projection handle in request `_meta`.
- Missing, invalid, stale, cross-principal, or unhealthy selections resolve to the
  identity's deterministic authorized HOT view or a typed error, never to the full
  catalog.
- A guessed tool call outside the active projection is rejected at call time.
- `toolsets` replaces the former compact lazy loader and may select exactly one typed
  WARM/COLD set at a time. Selected tools use direct typed validation, exact target
  grants, canonical dispatch, and inline audit.
- Drill-down crosses a client-extension boundary: the MCP host must capture the
  selector, re-list with selector `_meta`, install typed schemas, and preserve the
  selector on calls. A model cannot perform that registration itself. Clients not
  proven to support this behavior remain on the HOT five.

### B-11: Execution Temperature Boundary

| Temperature | Residency and use | EMQX relationship |
|-------------|-------------------|-------------------|
| **HOT** | Pre-warmed in-process memory, workflow, navigation, and frequently used typed operations | No synchronous EMQX/MQTT/provider-discovery hop; remains available when EMQX is down |
| **WARM** | Typed provider/domain sets loaded or activated after explicit selection | EMQX may carry authenticated health/catalog invalidation events; local config remains authoritative |
| **COLD** | Infrequent typed sets with explicit startup/unavailable behavior | EMQX may coordinate asynchronously, but never authorizes or bypasses MutationEngine |

HOT changes latency and residency only. It does not bypass exact active-sled SID1 or fresh OIA1, exact grants,
schema validation, projection checks, MutationEngine admission, durable state, or
inline audit. MQTT publication happens after the authoritative event is committed and
is best-effort. Promotion/demotion is versioned configuration, not an automatic
frequency heuristic.

---

## Transport Terminology Reference

| Term | Protocol | Interface |
|------|----------|-----------|
| D-Bus `PluginV1.Call` | D-Bus | `org.opdbus.PluginV1` interface, `Call` method |
| gRPC `PluginService.CallMethod` | gRPC | Generic plugin call (any plugin) |
| gRPC `EmqxPluginMethods.*` | gRPC | Generated typed RPCs for emqx plugin |
| MCP `/mcp` | HTTP | MCP protocol over HTTPS |

---

## Invariant Summary

| ID | Invariant | Enforcement |
|----|-----------|-------------|
| INV-1 | EMQX is standalone, not Netmaker-dependent | Service definition, plugin state |
| INV-2 | PluginSchema is the sole contract | build.rs, no hand-written proto |
| INV-3 | Every method has typed I/O, capability, subid | Schema validation, CI |
| INV-4 | Every protected MCP call presents exactly one accepted credential: exact active-sled SID1 or fresh OIA1 | Interceptor, MCP frontend |
| INV-5 | MutationEngine authors SID1 once per immutable session term; optional OIA1 is fresh per request/retry | Exact sled match; OIA nonce consumption |
| INV-6 | Capability enforcement is exact-`principal_id`; footprint/genesis/hash keys are invalid | Grant loading, no wildcards/digest keys |
| INV-7 | Event chain records before dispatch returns | MutationEngine ordering |
| INV-8 | ExHook on dedicated local listener only | No shared route without interceptor |
| INV-9 | `10.0.0.3:8090` is the one external unified fabric surface for MCP, gRPC, plugins, mutation, and control | Configuration, no second listener |
| INV-10 | runit for host services only | No systemctl/s6 |
| INV-11 | btrfs deployment only | build-golden.sh, no hand-copy |
| INV-12 | D-Bus identity separate from gRPC interceptor | Dedicated resolver required |
| INV-13 | Subid uniqueness enforced by CI | Automated test, not manual registry |
| INV-14 | Unified fabric surface at 10.0.0.3:8090 | Supersedes previous 10.0.0.2 |
| INV-15 | EMQX listeners loopback/UDS only | No public bindings |
| INV-16 | Anonymous MQTT denied | EMQX configuration |
| INV-17 | SID1/OIA1 is never logged or published to MQTT; SID1 persists only in the root-only durable identity store and private `root:secrets` credential projection, never the generic state tree | Redaction + permissions + internal-broker tests |
| INV-18 | The four compact meta-tools are absent; `toolsets` is the only lazy drill-down contract | Catalog and direct-call negative tests |
| INV-19 | The singleton chatbot default discovery is exactly five typed HOT tools; other identities receive only their authorized subset | Projection tests |
| INV-20 | Tool-set selection can only narrow exact grants | Discovery and guessed-call tests |
| INV-21 | HOT execution is independent of EMQX availability | Broker-down integration test |
| INV-22 | D-Bus registration derives identity; caller cannot choose it | Capability-gated registration tests |
| INV-23 | Native Codex uses direct Streamable HTTP plus header-only `op-identity-headers`; no MCP shim/proxy | Real client and listener E2E tests |
| INV-24 | OAuth is absent; bridge-native initialize compatibility remains on the same router and requires one credential | Route/config/handshake tests |
| INV-25 | Footprint is only the sled/Shuttle payload envelope for Snowball and vectorization | Type boundary + semantic CI gate |
| INV-26 | Identity metadata is copied into the footprint; no footprint/hash is converted back into identity | Principal stability + hash-key rejection tests |
| INV-27 | Snowball authors one chain hash from canonical payload; no current-payload hash-of-hash | Canonical footprint/receipt tests |
| INV-28 | A.N.N.A. Scribe persona/email is preserved; only its unsafe duplicate mmap reader is removed | Preservation and negative mmap tests |
