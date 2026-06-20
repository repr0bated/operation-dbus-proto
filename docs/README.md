# Documentation Index

This folder is now organized by topic so architecture, schema, plugins, and operations are easy to navigate.

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

## UI Docs

- `docs/ui/README.md`
- `docs/ui/API.md`
- `docs/ui/COMPONENTS.md`

## WireGuard / Identity Notes

- `docs/WG-SESSION-ID.md`
