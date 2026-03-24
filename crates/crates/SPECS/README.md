# Operation D-Bus Crate Specifications

This directory contains individual specifications for all 31 crates in the operation-dbus workspace.

## Purpose

These specifications provide:
- **Quick Reference**: Essential information from Cargo.toml
- **Structure Overview**: Source file organization
- **Dependencies**: Internal and external dependencies
- **Build Configuration**: Binaries, features, and build settings
- **Module Layout**: Key modules and their purposes

## How to Use

### For Development
1. **Starting a new feature**: Check the relevant crate spec to understand its structure
2. **Understanding dependencies**: See which crates depend on each other
3. **Finding modules**: Locate where specific functionality lives

### For Documentation
- Use specs as a starting point for detailed documentation
- Reference when writing architecture docs
- Include in onboarding materials

### For Code Review
- Verify changes align with crate purpose
- Check dependency additions are appropriate
- Ensure module organization follows patterns

## Specification Files

Each spec file follows the naming convention: `{number}-{crate-name}.md`

### Core Infrastructure (01-10)
- 01-op-agents - Agent registry and implementations
- 02-op-blockchain - BTRFS-based blockchain
- 03-op-cache - NUMA-aware caching
- 04-op-chat - Chat and LLM integration
- 05-op-cognitive-mcp - Cognitive MCP tools
- 06-op-core - Core types and utilities
- 07-op-dbus-mirror - D-Bus database projection
- 08-op-dbus-model - Database models
- 09-op-deployment - Container deployment
- 10-op-dynamic-loader - Dynamic tool loading

### Execution & Tracking (11-15)
- 11-op-execution-tracker - Execution monitoring
- 12-op-gateway - MCP gateway with WireGuard
- 13-op-grpc-bridge - D-Bus ↔ gRPC bridge
- 14-op-http - Central HTTP/TLS server
- 15-op-identity - Identity management

### Introspection & Inspection (16-18)
- 16-op-inspector - Universal object inspector
- 17-op-introspection - D-Bus introspection
- 18-op-jsonrpc - JSON-RPC server

### LLM & MCP (19-23)
- 19-op-llm - LLM provider integration
- 20-op-mcp-aggregator - MCP server aggregator
- 21-op-mcp-proxy - MCP proxy service
- 22-op-mcp - Unified MCP protocol server
- 23-op-ml - ML/embedding support

### Networking & Plugins (24-25)
- 24-op-network - Native networking (OpenFlow, OVSDB, rtnetlink)
- 25-op-plugins - Plugin system with state management

### Services & State (26-28)
- 26-op-services - System service manager (systemd replacement)
- 27-op-state-store - Persistent state storage
- 28-op-state - State management system

### Tools & Web (29-31)
- 29-op-tools - Tool registry and execution
- 30-op-web - Unified web server
- 31-op-workflows - Workflow engine

## Generating Specs

To regenerate all specs:

```bash
cd /home/jeremy/git/operation-dbus/crates
./generate_specs.sh
```

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     op-web (HTTP/WebSocket)                 │
│                    Unified Web Interface                    │
└────────────┬────────────────────────────────────────────────┘
             │
    ┌────────┴────────┐
    │                 │
    ▼                 ▼
┌─────────┐      ┌──────────────┐
│op-chat  │      │ op-mcp       │
│LLM      │◄────►│ MCP Protocol │
│Integration│     │ Server       │
└────┬────┘      └──────┬───────┘
     │                  │
     ▼                  ▼
┌──────────────────────────────────┐
│        op-tools                  │
│    Tool Registry & Execution     │
└────────┬─────────────────────────┘
         │
    ┌────┴────┐
    │         │
    ▼         ▼
┌─────────┐ ┌──────────────┐
│op-agents│ │op-introspection│
│Registry │ │D-Bus Scanner  │
└─────────┘ └──────────────┘
    │              │
    └──────┬───────┘
           ▼
    ┌──────────────┐
    │   op-core    │
    │  D-Bus Core  │
    └──────┬───────┘
           │
    ┌──────┴───────┐
    │              │
    ▼              ▼
┌─────────┐  ┌──────────┐
│op-state │  │op-plugins│
│Manager  │  │System    │
└────┬────┘  └────┬─────┘
     │            │
     └─────┬──────┘
           ▼
    ┌──────────────┐
    │op-blockchain │
    │BTRFS Ledger  │
    └──────────────┘
```

## Key Relationships

### Dependency Layers
1. **Foundation**: op-core, op-execution-tracker
2. **Storage**: op-blockchain, op-cache, op-state-store
3. **State**: op-state, op-plugins
4. **Tools**: op-tools, op-introspection, op-inspector
5. **Services**: op-agents, op-chat, op-llm, op-mcp
6. **Interface**: op-web, op-http

### Cross-Cutting Concerns
- **Networking**: op-network, op-grpc-bridge, op-http
- **Identity**: op-identity, op-gateway
- **Deployment**: op-deployment, op-services
- **Workflows**: op-workflows, op-dynamic-loader

## Contributing

When adding or modifying crates:
1. Update the crate's Cargo.toml with accurate description
2. Regenerate specs with `./generate_specs.sh`
3. Add detailed documentation in the crate's README.md
4. Update this README if adding new categories

## Questions?

For detailed information about specific crates, see their individual spec files or source code documentation.
