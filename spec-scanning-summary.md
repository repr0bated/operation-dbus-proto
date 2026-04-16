# Spec Scanning Summary

Include these areas explicitly in the codebase-wide spec scan.

## Core Framing

- Actual architecture, not intended architecture only
- Control-plane vs execution-plane boundaries
- Authoritative interfaces vs convenience wrappers
- Live conventions, legacy aliases, and partially migrated paths

## Container Model

- All container concepts in code, scripts, docs, env, and deployment artifacts
- Current runtimes/backends: Incus, Podman, systemd, user services, anything else
- Where container control is exposed today: D-Bus, CLI wrappers, scripts, APIs
- Missing conventions for families, naming, lifecycle, networking, mounts, trust zones
- Which parts are bootstrap-only vs long-term supported interfaces

## D-Bus Surfaces

- Well-known names, object paths, interfaces, methods, signals
- Which services are actually registered at runtime
- Gaps between documented and implemented D-Bus contracts
- Areas where direct shell/CLI still bypasses intended D-Bus authority

## LLM/Provider Layer

- Provider registry and selection flow
- Legacy provider ids and aliases
- Env/config contracts
- Model discovery, auth modes, and runtime defaults
- All provider-specific ad hoc behavior that should become schema- or registry-driven

## MCP, Chat, and Agent Boundaries

- Chatbot cognition vs execution
- MCP surfaces: internal, compact, cognitive, external
- Agent registry and orchestration assumptions
- Tool governance and permission boundaries
- Places where the chatbot still has hidden direct dependencies on execution backends

## Deployment Reality

- What actually starts on host boot
- systemd, dinit, and user-service split
- Required env files, config locations, runtime sockets, and ports
- Host vs container responsibilities
- Drift between deploy scripts and active runtime assumptions

## State and Registry Model

- Tool, plugin, agent, provider, and container registries
- Typed schemas already present
- Implicit registries hiding in scripts, config, or env
- Missing canonical sources of truth

## Networking

- Port allocations and endpoint assumptions
- localhost vs bridge IP vs container IP assumptions
- Auth boundaries on each surface
- Hardcoded addresses that should be normalized

## Legacy Detection

- Old names still used for compatibility
- Duplicate implementations of the same concept
- Direct endpoints that bypass newer abstractions
- Dead code vs compatibility shims vs active migration paths

## Verification

- What can be verified locally
- What requires host runtime
- What requires container runtime
- What is blocked by missing exposed interfaces
- CI and build assumptions that do not match real deployment

## Required Outputs

- What exists
- What is authoritative
- What is legacy
- What is missing
- What needs convention/spec before more implementation
- What can be safely changed now vs deferred until conventions are formalized

## Explicit Flag

Flag architectural seams where the project assumes a convention that is not yet named, typed, or exposed.

## UI & JSON Streaming

- Catalog-driven guardrails: define available components/actions with typed schemas (Zod-like) so AI-generated JSON is constrained to approved UI primitives.
- Streaming JSON rendering: handle progressive arrival of event frames, support React/React Native render paths, and keep bindings up to date via `$state`/`$bindState` semantics.
- Guardrails vs export: document how AI prompts translate into UI tree nodes, actions, and optional code export for standalone React components.
- Data binding contracts: map live state (`launchpad` or MCP stream events) to UI props, metrics, and forms using `statePath`, `on.change`, `setState` triggers.
- Dashboard integration: note how SSE/WebSocket JSON streams feed the catalog, map stream schema (MetricEvent, ToolEvent, AuditEvent) to components, and define validation/transform rules.
