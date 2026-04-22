# Requirements

## Introduction
The OpenClaw Cognitive Platform is a schema-driven, introspection-first, AI-orchestrated infrastructure system. Operating entirely over D-Bus as the authoritative control plane, it strictly isolates cognitive reasoning from execution authority. A zero-trust framework governs interactions where all capabilities are dynamically discovered, and interactions between orchestrators and tools pass through the Application Service Plane (ASP). gRPC acts as the backbone for internal service-to-service RPC, ensuring that all services register via a reflection-driven registry for robust and resilient operations.

## Requirements

### A. Cognitive MCP / MCP Surfaces
**User Story**: As an external system or LLM client, I want distinct, purpose-built MCP surfaces so that I can interact securely without compromising the execution plane.

#### Acceptance Criteria
1. WHEN an external user connects to the Compact MCP THEN they only gain access to 4 discovery/introspection meta-tools.
2. WHEN the Chatbot uses Cognitive MCP THEN it can perform reasoning and access memory, but cannot invoke execution tools without explicit orchestration.
3. WHEN orchestration connects to the Internal MCP THEN it uses WireGuard authentication to access the full D-Bus-backed tool surface (145+ tools).
4. WHEN an SSE MCP client connects THEN it can stream real-time events corresponding to system state changes.

#### Constraints
- External users never gain execution authority directly through Cognitive or Compact MCP.
- Trust boundaries are enforced as strict zones, not simple routing.

#### Non-Goals
- Full execution tools on the Compact MCP surface.

### B. gRPC Topology and Server Reflection
**User Story**: As an internal client (like `op-chat`), I want to discover available gRPC services at runtime so that I don't need hardcoded stubs for newly registered plugins.

#### Acceptance Criteria
1. WHEN the `op-grpc-bridge` starts THEN it serves a combined `operation_descriptor.bin` via `tonic_reflection::server::Builder`.
2. WHEN `GrpcAgentClient` connects THEN it uses `ServerReflectionClient` to enumerate and index all available methods.
3. WHEN dispatching a request THEN `op-chat` dynamically maps the request to `PluginService.CallMethod`.

#### Constraints
- Every gRPC service MUST register its file descriptor in the combined descriptor set.
- gRPC services are internal-only and never exposed externally without Gateway mediation.

#### Non-Goals
- Exposing gRPC directly to external clients.

### C. RCP / JSON-RPC Live-State Substrate
**User Story**: As the cognitive layer, I want to query a JSON-RPC-based operational graph so that I can inspect the live D-Bus object tree without requiring raw D-Bus access.

#### Acceptance Criteria
1. WHEN D-Bus emits a signal THEN the `op-dbus-mirror` updates the structured JSON-RPC mirror synchronously.
2. WHEN the Chatbot queries the live-state mirror THEN it receives real-time operational state representations.

#### Constraints
- The mirror is NOT a cache; it is the absolute live operational state representation.
- Synchronization must be purely event-driven.

#### Non-Goals
- Allowing mutation directly through the read-only mirror.

### D. ASP (Application Service Plane)
**User Story**: As a system administrator, I want an intermediary governance layer so that LLM requests are validated before executing system changes.

#### Acceptance Criteria
1. WHEN an orchestrator requests execution THEN the ASP receives the typed schema and validates it against current policies.
2. WHEN permission validation passes THEN the ASP dispatches the request to the target execution plugin/tool and writes an audit trail.
3. WHEN validation fails THEN the request is rejected with a clear `ConstraintFail` or `ReadOnlyViolation`.

#### Constraints
- The ASP is NOT a chatbot or tool; it is exclusively a mediation/validation layer.

#### Non-Goals
- Moving execution logic into the ASP itself.

### E. Tools and Tool Registry
**User Story**: As an agent, I want to invoke system mutations via formally defined tools so that all actions are tracked, schemas are validated, and permissions are enforced.

#### Acceptance Criteria
1. WHEN a tool is invoked THEN the tool registry confirms its typed schema, checks permissions, and maps it to the respective D-Bus path or gRPC method.
2. WHEN an executor fires THEN the output is captured in the audit trail.
3. WHEN tools are enumerated THEN discovery works seamlessly across both D-Bus introspection and gRPC reflection.

#### Constraints
- Chatbots and agents may NEVER bypass the tool registry.
- No tool can be invoked without a registered schema and active permission grant.

#### Non-Goals
- Support for unstructured or ad-hoc shell execution bypassing schemas.

### F. Plugin Registry Lifecycle
**User Story**: As a developer, I want to deploy plugins as schema-as-code units so that they are safely loaded, validated, and integrated into the system dynamically.

#### Acceptance Criteria
1. WHEN a plugin is registered THEN it publishes its schema to the D-Bus object tree.
2. WHEN a plugin transitions (Load -> Activate -> Deactivate -> Unload) THEN validation hooks run appropriately.
3. WHEN plugins define dependencies THEN semver compatibility checks are enforced.

#### Constraints
- Each plugin MUST clearly define its domain schema, types, and constraints.

#### Non-Goals
- Permitting plugins that do not publish schemas.

### G. Agent Registry and Orchestration
**User Story**: As the Chatbot, I want to delegate tasks to specific agents so that operations are strictly scoped by role, allowed tools, and memory context.

#### Acceptance Criteria
1. WHEN an agent is orchestrated THEN it acts strictly within its defined identity, memory scope, and `allowed_tools` list.
2. WHEN a cognitive thought maps to execution THEN it flows downward in the hierarchy (Chatbot -> Orchestrator Agents -> Execution Agents -> Tools).
3. WHEN an agent requests execution THEN the ASP enforces the `allowed_tools` limitations defined in the registry.

#### Constraints
- Chatbot cognition remains completely distinct from agent execution roles.

#### Non-Goals
- Global tool access for all agents.

### H. Memory Architecture
**User Story**: As an AI agent, I want to access various tiers of memory so that I can maintain session context and rely on long-term persisted knowledge across sessions.

#### Acceptance Criteria
1. WHEN the cognitive layer needs past context THEN it queries the persisted, indexed long-term memory.
2. WHEN reasoning within a session THEN contextual memory handles the working context (e.g. tool calls, intermediate reasoning steps).
3. WHEN taking operational snapshots THEN memory captures ephemeral live-state seeded from the D-Bus mirror.

#### Constraints
- Memory may inform reasoning but NEVER grants execution authority.
- All memory reads and writes are heavily audited.

#### Non-Goals
- Memory directly acting as the truth for live execution state.

### I. Dashboard and JSON Stream
**User Story**: As an operator, I want to observe system health via a live streaming dashboard so that I have real-time visibility into state changes and audit events.

#### Acceptance Criteria
1. WHEN state changes or tools execute THEN a strongly typed JSON stream (e.g., `StateChangeEvent`, `ToolExecutionEvent`) is broadcast via SSE or WebSocket from `op-web`.
2. WHEN the Dashboard consumes the stream THEN it displays real-time tool logs, audit information, and state tracking.

#### Constraints
- The stream is strictly a visibility layer, not a control channel.
- The path is unidirectional: D-Bus Signals -> `op-dbus-mirror` -> `op-web` stream -> Dashboard.

#### Non-Goals
- Dashboard-driven command issuance outside of governed tool requests.

### J. Isolation and Trust Boundaries
**User Story**: As a security architect, I want to ensure prompt injections or cognitive errors cannot arbitrarily modify my infrastructure so that the execution plane is safe.

#### Acceptance Criteria
1. WHEN an external client interfaces THEN they enter Trust Zone 4 (External interfaces) and cannot reach Zone 3 (Execution) directly.
2. WHEN prompt injection happens in Trust Zone 1 (Cognition) THEN it cannot force mutation in Zone 3 due to strict registry validation.
3. WHEN an operation bridges a trust zone THEN the ASP validates the cross-zone call against the agent registry's capability lists.

#### Constraints
- Exactly four trust zones MUST be respected (Cognition, Orchestration, Execution, External Interfaces).

#### Non-Goals
- "Soft" trust boundaries where simple token passing allows unrestricted execution.

### K. Interface Strategy (Chatbot-First)
**User Story**: As a user, I want the Chatbot to be the primary interaction surface so that I can naturally query state, inspect registries, and manage tools via conversation.

#### Acceptance Criteria
1. WHEN I ask about system state THEN the Chatbot interprets my request, queries live state via tools, and explains the outcome.
2. WHEN I need to view real-time changes THEN the Dashboard acts as a read-only visual surface alongside the chat.
3. WHEN I need to trace an action THEN the Chatbot can explain the audit trail of the previous operations.

#### Constraints
- CLI interfaces are deprecated as the primary control mechanism.
- The Chatbot must rely on tools to inspect live state and may not execute mutations directly without registry governance.

#### Non-Goals
- Making the Chatbot act as a direct shell wrapper.
