# Documentation Index

This folder is now organized by topic so architecture, schema, plugins, and operations are easy to navigate.

## Consolidated Documentation (generated 2026-07-02)

Whole-workspace docs generated from current source and the June/July 2026 design
records. Start here:

- [`docs/overview/architecture.md`](overview/architecture.md) — whole-workspace
  architecture overview: layers, crate map, schema-as-contract, schemars →
  reflection pipeline, state flow, subid taxonomy, MCP topology.
- [`docs/reference/api-reference.md`](reference/api-reference.md) — API/technical
  reference: `PluginSchema` contract, `StatePlugin` trait, D-Bus object model,
  gRPC surface, MCP gateway, ports, subids.
- [`docs/guides/user-guide.md`](guides/user-guide.md) — build/run/test, inspect
  and mutate state, connect a client, add or migrate a plugin.
- [`docs/reference/proto/README.md`](reference/proto/README.md) — per-file gRPC/proto
  contract reference for all 26 project-owned `.proto` files (services, RPCs, streaming).

Authoritative design records: `.kiro/specs/schemars-to-reflection-plugin-pipeline/`
and `.kiro/specs/voyage-plugin-cognitive-mcp-boundaries/`.

> Note: some older topic docs below predate June 2026 and may be stale; the
> consolidated docs above and the Kiro specs are the current source of truth.

## Architecture

- `docs/architecture/state-flow.md`  
  End-to-end state flow and control points (JSON-RPC/gRPC -> D-Bus -> StateManager -> plugins -> footprint).
- `docs/architecture/completion-status.md`  
  Execution status for architecture/functionality milestones.
- `docs/HIERARCHICAL_DBUS_DESIGN.md`  
  Hierarchical D-Bus model.
- `docs/DBUS_INDEXER_IMPLEMENTATION_GUIDE.md`  
  D-Bus indexer details.
- `docs/d_bus_introspection_with_zbus.md`  
  zbus introspection patterns.

## Schema

- `docs/schema/plugin-contracts.md`  
  Contract envelope model (`stub`, `immutable`, `tunable`, `observed`, `meta`, `semantic_index`, `privacy_index`).
- `docs/schema/registry-coverage.md`  
  State store registry coverage and materialization behavior.
- `docs/schema-as-code.md`  
  Existing schema-as-code principles.

## Plugins

- `docs/plugins/plugin-catalog.md`  
  Current plugin inventory and purpose.
- `docs/PLUGIN-DEVELOPMENT-GUIDE.md`  
  Plugin development guidance.

## Operations

- `docs/operations/mutation-paths.md`  
  Canonical mutation ingress and strict flow path.
- `docs/SNAPSHOT_AUTOMATION.md`  
  Snapshot automation details.
- `docs/kiro-spec-workflow.md`  
  How to generate and compare Kiro spec folders in this repository.
- `docs/operations/op-dbus-dinit.md`  
  dinit service setup for standalone `op-dbus` + `op-mcp-proxy` runtime.
- `docs/mcp-vscode-bridge.md`  
  MCP bridge setup that emulates the VS Code extension flow.

## Service-Specific Docs

- `docs/op-services/README.md`
- `docs/op-gateway/README.md`

## Plugin Invocation

- `docs/schema-coupled-plugin-blob-reflection-whitepaper.md`  
  Canonical explanation of plugin object blobs, reflection, and how to call a plugin object.

Quick call pattern:

1. Discover the object with `busctl --system introspect org.opdbus.v1.plugins /org/opdbus/v1/plugins/<plugin_id>`.
2. Call the object method on `org.opdbus.v1.plugins` using the plugin interface.
3. For typed RPC, call `operation.v1.PluginService.CallMethod` with the plugin id, object path, interface name, method name, and structured arguments.

## UI Docs

- `docs/ui/README.md`
- `docs/ui/API.md`
- `docs/ui/COMPONENTS.md`

## WireGuard / Identity Notes

- `docs/WG-SESSION-ID.md`
