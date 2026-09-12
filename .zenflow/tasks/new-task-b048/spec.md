# Technical Specification: Openclaw Cognitive Platform Kiro Spec

## Difficulty Assessment

**Complexity: HARD**

This task requires documenting and structuring a complete, production-scale cognitive infrastructure platform with 31+ interdependent Rust crates, multiple protocol layers (D-Bus, gRPC, MCP, JSON-RPC), strict architectural constraints, and philosophical decisions already baked into the codebase. The output is three Kiro-format documents (requirements.md, design.md, tasks.md) that must accurately reflect the existing system and provide actionable tasks for completing/wiring the remaining gaps.

---

## Technical Context

### Language & Runtime
- **Primary**: Rust (edition 2021, workspace-managed)
- **Frontend**: TypeScript/React (Vite, Vitest, ESLint)
- **Scripting/utilities**: Python (openclaw-indexer)
- **Serialization**: `simd-json` / `serde_json` (high-perf)
- **Async runtime**: Tokio

### Key Dependencies (existing)
- `zbus` — D-Bus via Rust (authoritative control plane)
- `tonic` / `prost` — gRPC (internal service RPC)
- `axum` — HTTP/WebSocket server
- `sqlx` + SQLite — persistent state store
- `rmcp` — MCP protocol implementation
- `anyhow` / `thiserror` — error handling
- `tracing` — structured observability

### Deployment Context
- Host OS: Chimera Linux (dinit-managed services)
- Openclaw gateway runs in Incus container named `services`
  - Endpoint: `http://127.0.0.1:18789/v1/chat/completions` (OpenAI-compatible)
  - Access: internal OpenClaw network endpoint via `OPENCLAW_BASE_URL`
  - Models: `google-gemini-cli/*`, `opencode/*`
- op-dbus MCP servers exposed at `:8080/mcp/{compact,agents,sse}`
- Host IP for container bridge: `10.149.181.1`

### Existing Crate Architecture (31 crates in `crates/crates/`)

| Layer | Crates |
|-------|--------|
| Foundation | `op-core`, `op-execution-tracker` |
| Storage | `op-snowball`, `op-cache`, `op-state-store`, `op-dbus-model` |
| State | `op-state`, `op-plugins`, `op-dbus-mirror` |
| Tools | `op-tools`, `op-dynamic-loader`, `op-introspection`, `op-inspector` |
| Agents | `op-agents`, `op-chat`, `op-llm` |
| MCP | `op-mcp`, `op-mcp-aggregator`, `op-mcp-proxy`, `op-cognitive-mcp` |
| Networking | `op-network`, `op-grpc-bridge`, `op-http`, `op-jsonrpc` |
| Security | `op-gateway`, `op-identity` |
| Deployment | `op-deployment`, `op-services` |
| Workflows/ML | `op-workflows`, `op-ml` |
| Web | `op-web` |

### Key Design Invariants (from AGENTS.md and existing code)
- **D-Bus is the authoritative control plane** — no CLI wrappers, native zbus calls only
- **Chatbot never executes directly**: `Chatbot → MCP → Orchestrator → Tools`
- **simd-json** over serde_json for hot paths
- **gRPC** for internal service-to-service RPC
- **Schema-as-code**: every subsystem has a typed, validated schema
- **Zero trust**: all LLM claims verified against execution log
- **Live state is truth**: introspection-first, not desired-state modeling

---

## Implementation Approach

The task is to produce three Kiro spec documents for the `.kiro/specs/openclaw-cognitive-platform/` spec:

1. **`requirements.md`** — Functional and non-functional requirements grounded in the actual system architecture. Must cover: schema governance, live-state reasoning, plugin extensibility, tool governance, agent management, memory integration, JSON stream observability, isolation/trust boundaries, auditability, chatbot-first interface.

2. **`design.md`** — Full architectural design mapping each requirement to components. Covers: D-Bus authority model, gRPC service topology, MCP surface (internal/external), RCP/JSON-RPC live-state substrate, ASP (Application Service Plane), tool and plugin registries, agent registry and orchestration hierarchy, memory architecture, dashboard/JSON stream, trust zones, chatbot-first interface strategy.

3. **`tasks.md`** — Ordered implementation tasks that reflect actual gaps between the current codebase and the full vision. Starts with foundational schemas and registries, progresses through protocol surfaces, memory, observability, and UI integration.

---

## Source Code Structure: Files to Create or Modify

### New files (Kiro spec outputs)
| File | Purpose |
|------|---------|
| `.kiro/specs/openclaw-cognitive-platform/requirements.md` | Functional/non-functional requirements |
| `.kiro/specs/openclaw-cognitive-platform/design.md` | Full architectural design |
| `.kiro/specs/openclaw-cognitive-platform/tasks.md` | Ordered implementation task list |

### Existing files relevant to the spec (reference only)
| File | Relevance |
|------|----------|
| `crates/crates/op-chat/src/llm.rs` | `create_provider()` factory — needs openclaw variant |
| `crates/crates/op-llm/src/provider.rs` | `ProviderType` enum — add `OpenClaw` |
| `crates/crates/op-llm/src/lib.rs` | Re-export new openclaw provider |
| `crates/crates/op-mcp/src/tool_registry.rs` | Tool registry — governed tool discovery |
| `crates/crates/op-plugins/src/state_plugins/` | Domain plugins — schema-as-code pattern |
| `crates/crates/op-agents/src/` | Agent registry and specializations |
| `crates/crates/op-cognitive-mcp/src/` | Cognitive memory server |
| `crates/crates/op-gateway/src/` | WireGuard auth, trust boundary enforcement |
| `crates/crates/op-state/src/` | State management plugin framework |
| `crates/crates/op-jsonrpc/src/` | OVSDB/NonNet JSON-RPC mirror |
| `src/chatbot/mod.rs` | Cognitive control plane |
| `schemas/` | JSON schemas for plugins and config |

---

## Data Model / API / Interface Changes

The spec documents will define and reference these interfaces (existing or to be completed):

### gRPC Server Reflection (tonic-reflection)

**Status: partially implemented** — present in `op-grpc-bridge` and `op-chat`, absent from requirements.

This is the gRPC-layer equivalent of D-Bus introspection and must be a first-class requirement:

- **Server side** (`op-grpc-bridge`): All proto services are compiled into a combined `operation_descriptor.bin` file descriptor set. `tonic_reflection::server::Builder` serves this so any gRPC client can enumerate all available services and methods at runtime without pre-generated stubs.
- **Client side** (`op-chat/grpc_client.rs`): `OpDbusClient` uses `ServerReflectionClient` to discover available methods at connect time — reflection-driven dispatch, not hardcoded stubs. This is the mechanism by which the cognitive layer learns what internal gRPC services expose.
- **Requirement**: Every gRPC service added to the platform MUST register its file descriptor in the combined descriptor set so it is automatically discoverable via reflection. New services without reflection registration violate the introspection-first principle.
- **Scope**: Reflection must be accounted for in requirements section B (gRPC) and the design must document the combined descriptor build process in `build.rs` files.

### Tool Registry Contract
```
ToolEntry {
  id: ToolId,
  name: String,
  schema: JsonSchema,
  permissions: Vec<Permission>,
  dbus_path: Option<DBusPath>,
  audit_log: AuditRef,
  executor: ExecutorKind,  // DBus | gRPC | Internal
}
```

### Plugin Registry Contract
```
PluginEntry {
  id: PluginId,
  schema_version: SemVer,
  published_schema: JsonSchema,
  lifecycle: PluginLifecycle,  // Load | Activate | Deactivate | Unload
  capabilities: Vec<Capability>,
  dbus_object_path: DBusPath,
}
```

### Agent Registry Contract
```
AgentEntry {
  id: AgentId,
  identity: Identity,
  role: AgentRole,
  allowed_tools: Vec<ToolId>,
  memory_access: MemoryScope,
  orchestration_parent: Option<AgentId>,
  lifecycle: AgentLifecycle,
}
```

### MCP Surface Boundaries
- **Internal MCP** (`/mcp/agents`): Full 145+ tools, D-Bus-backed, WireGuard-authenticated
- **Compact MCP** (`/mcp/compact`): 4 meta-tools for discovery, externally accessible
- **Cognitive MCP** (`/mcp/cognitive`): Memory, sequential thinking — governed access
- **External-facing**: Filtered subset, no unrestricted infrastructure control

### JSON Stream / Dashboard
- Source: D-Bus signal listeners + plugin state change events
- Transport: SSE or WebSocket from `op-web`
- Format: Typed JSON events (`StateChangeEvent`, `ToolExecutionEvent`, `RegistryChangeEvent`, `AuditEvent`)
- Consumer: Dashboard UI in `crates/op-web/ui`

---

## Verification Approach

After producing the three Kiro documents, verify:

1. **No compilation breakage**: `cargo check --workspace`
2. **Lint clean**: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
3. **Tests pass**: `cargo test --workspace --all-targets --all-features`
4. **Frontend typecheck**: `cd crates && npm run typecheck`
5. **Frontend lint**: `cd crates && npm run lint`

Spec documents themselves are verified by review against:
- Existing crate SPECs in `crates/crates/SPECS/`
- Existing `spec.md` (root) for native command patterns
- `OPENCLAW-CONTEXT.md` for integration topology
- `AGENTS.md` for engineering invariants

---

## Implementation Plan Structure

The Implementation step will be broken into the following concrete tasks (to replace the generic Implementation step in plan.md):

### Task 1: Requirements Document
Generate `.kiro/specs/openclaw-cognitive-platform/requirements.md` covering all 11 areas (A–K from the brief) with user stories, acceptance criteria, constraints, and non-goals.

### Task 2: Design Document
Generate `.kiro/specs/openclaw-cognitive-platform/design.md` mapping each requirement to architecture: D-Bus authority model, MCP surface topology, gRPC topology, JSON-RPC live-state mirror, ASP definition, tool/plugin/agent registry designs, memory architecture, trust zones, dashboard/stream design, chatbot-first interface strategy.

### Task 3: Tasks Document
Generate `.kiro/specs/openclaw-cognitive-platform/tasks.md` with ordered implementation tasks reflecting actual gaps, starting with schema foundations and progressing to UI integration.

### Task 4: Openclaw LLM Provider Integration
Implement the openclaw provider in `op-llm` and wire it into `op-chat`'s `create_provider()` factory, enabling the chatbot to use OpenClaw's gateway as its LLM backend with model switching.

### Task 5: Verify and Lint
Run full CI-equivalent verification: `cargo check`, `cargo clippy`, `cargo test`, frontend typecheck and lint.
