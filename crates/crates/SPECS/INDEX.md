# Operation D-Bus - Crate Specifications Index

## All 31 Crates

### Core Infrastructure
1. [op-agents](01-op-agents.md) - Secure agent registry and D-Bus agent implementations
2. [op-blockchain](02-op-blockchain.md) - Streaming blockchain with BTRFS subvolumes
3. [op-cache](03-op-cache.md) - BTRFS-based caching with NUMA awareness and gRPC services
4. [op-chat](04-op-chat.md) - Chat functionality and LLM integration
5. [op-cognitive-mcp](05-op-cognitive-mcp.md) - Cognitive MCP integration
6. [op-core](06-op-core.md) - Core types and utilities for op-dbus-v2
7. [op-dbus-mirror](07-op-dbus-mirror.md) - 1:1 D-Bus projection of internal databases (OVSDB, NonNet)
8. [op-dbus-model](08-op-dbus-model.md) - Database models
9. [op-deployment](09-op-deployment.md) - Container and image deployment management
10. [op-dynamic-loader](10-op-dynamic-loader.md) - Dynamic tool loading with caching and execution tracking

### Execution & Gateway
11. [op-execution-tracker](11-op-execution-tracker.md) - MCP execution tracking layer
12. [op-gateway](12-op-gateway.md) - MCP Gateway with WireGuard authentication and smart routing
13. [op-grpc-bridge](13-op-grpc-bridge.md) - Bidirectional D-Bus <-> gRPC bridge
14. [op-http](14-op-http.md) - Central HTTP/TLS server for all op-dbus modules
15. [op-identity](15-op-identity.md) - Identity management

### Introspection & Inspection
16. [op-inspector](16-op-inspector.md) - Inspector Gadget - Universal object inspector with AI gap-filling
17. [op-introspection](17-op-introspection.md) - DBus introspection capabilities
18. [op-jsonrpc](18-op-jsonrpc.md) - JSON-RPC server with OVSDB and NonNet database support

### LLM & MCP Integration
19. [op-llm](19-op-llm.md) - LLM provider integration with dynamic model discovery
20. [op-mcp-aggregator](20-op-mcp-aggregator.md) - MCP Server Aggregator - proxies multiple MCP servers
21. [op-mcp-proxy](21-op-mcp-proxy.md) - MCP proxy service
22. [op-mcp](22-op-mcp.md) - Unified MCP Protocol Server with multiple transport support
23. [op-ml](23-op-ml.md) - ML/Embedding support: model management, text embedder, vector storage

### Networking & Plugins
24. [op-network](24-op-network.md) - Native networking: OpenFlow, OVSDB, rtnetlink, Proxmox API
25. [op-plugins](25-op-plugins.md) - Plugin system with state management and blockchain footprints

### Services & State
26. [op-services](26-op-services.md) - System-wide service manager - systemd replacement with dinit backend
27. [op-state-store](27-op-state-store.md) - MCP Execution State Store - Persistent job ledger
28. [op-state](28-op-state.md) - State management system with plugin infrastructure and crypto

### Tools & Web
29. [op-tools](29-op-tools.md) - Tool registry and execution for op-dbus-v2
30. [op-web](30-op-web.md) - Unified web server - consolidates all HTTP services
31. [op-workflows](31-op-workflows.md) - Workflow engine with plugin/service nodes

## By Category

### Foundation Layer
- **op-core** - Core types, D-Bus utilities, execution context
- **op-execution-tracker** - Execution monitoring and metrics

### Storage Layer
- **op-blockchain** - Immutable audit trail with BTRFS
- **op-cache** - NUMA-aware caching with gRPC
- **op-state-store** - SQLite/Redis persistent storage
- **op-dbus-model** - Database models

### State Management
- **op-state** - Plugin-based state management
- **op-plugins** - Plugin system with footprints
- **op-dbus-mirror** - D-Bus database projection

### Tool System
- **op-tools** - Tool registry and execution
- **op-dynamic-loader** - Dynamic tool loading
- **op-introspection** - D-Bus introspection
- **op-inspector** - Universal object inspector

### Agent System
- **op-agents** - Agent registry and implementations
- **op-chat** - Chat orchestration and LLM integration
- **op-llm** - LLM provider integration

### MCP Protocol
- **op-mcp** - MCP protocol server (3 binaries)
- **op-mcp-aggregator** - Multi-server aggregation
- **op-mcp-proxy** - MCP proxy service
- **op-cognitive-mcp** - Cognitive tools

### Networking
- **op-network** - OpenFlow, OVSDB, rtnetlink, Proxmox
- **op-grpc-bridge** - D-Bus ↔ gRPC bridge
- **op-http** - Central HTTP/TLS server
- **op-jsonrpc** - JSON-RPC server

### Security & Identity
- **op-gateway** - WireGuard authentication
- **op-identity** - Identity and session management

### Deployment & Services
- **op-deployment** - Container deployment
- **op-services** - Service manager (systemd replacement)

### Workflows & ML
- **op-workflows** - Workflow engine
- **op-ml** - ML/embedding support

### Web Interface
- **op-web** - Unified web server with UI

## Dependency Graph

```
op-web
  ├── op-chat
  │   ├── op-llm
  │   ├── op-tools
  │   │   ├── op-introspection
  │   │   ├── op-inspector
  │   │   ├── op-network
  │   │   └── op-http
  │   ├── op-introspection
  │   ├── op-execution-tracker
  │   └── op-agents
  ├── op-mcp
  │   ├── op-tools
  │   ├── op-plugins
  │   ├── op-introspection
  │   ├── op-state
  │   └── op-state-store
  ├── op-mcp-aggregator
  ├── op-grpc-bridge
  ├── op-state
  │   ├── op-blockchain
  │   ├── op-jsonrpc
  │   └── op-state-store
  └── op-identity

op-plugins
  ├── op-state
  ├── op-state-store
  ├── op-blockchain
  │   └── op-cache
  ├── op-network
  ├── op-dynamic-loader
  └── op-execution-tracker

op-core (foundation - no internal deps)
op-execution-tracker (foundation - no internal deps)
```

## Quick Stats

- **Total Crates**: 31
- **Binaries**: 8 (dbus-agent, op-agent-manager, op-services, systemctl, systemctl-native, op-mcp-server, op-mcp-compact, op-mcp-agents, op-web-server)
- **gRPC Services**: 4 (op-cache, op-grpc-bridge, op-services, op-mcp with grpc feature)
- **D-Bus Services**: Multiple (op-agents, op-state, op-introspection, op-dbus-mirror, op-services)
- **Web Servers**: 2 (op-http, op-web)

## Documentation

Each specification includes:
- **Purpose**: What the crate does and why it exists
- **Architecture**: ASCII diagrams showing structure
- **Core Modules**: Detailed module descriptions
- **Data Structures**: Key types and their usage
- **API Examples**: Rust code examples
- **Configuration**: Config file formats and options
- **Dependencies**: Internal and external dependencies
- **Use Cases**: Real-world usage scenarios

## Status

✅ All 31 crate specifications generated  
🔄 Comprehensive detailed versions being created by subagents  
📝 Each spec will be 300-500 lines with full technical details

## See Also

- [README.md](README.md) - How to use these specifications
- [SUMMARY.md](SUMMARY.md) - High-level architecture overview
