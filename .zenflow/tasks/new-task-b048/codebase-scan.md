# Codebase Scan: OpenClaw Cognitive Platform / operation-dbus-proto

## Summary
The codebase is a highly modular Rust workspace containing 31 crates organized into layers (Foundation, Storage, State, Tools, Agents, MCP, Networking, Security, Deployment, Workflows, Web). It implements a schema-driven, introspection-first architecture where D-Bus acts as the authoritative control plane and gRPC is used for internal service-to-service RPC. 

## Fully Implemented
- **gRPC Topology and Server Reflection**: The `op-grpc-bridge` crate implements the gRPC server using `tonic_reflection::server::Builder::configure()`, successfully exposing the combined `FILE_DESCRIPTOR_SET` of all domain services.
- **gRPC Dynamic Client**: `op-chat/src/grpc_client.rs` implements `GrpcAgentClient`, which dynamically connects and performs reflection discovery via `ServerReflectionClient` without hardcoded method stubs. Call dispatch is dynamically routed to `PluginService.CallMethod`.
- **StateSync Subscriptions**: The server correctly supports live streaming updates through `StateSync::subscribe` via `simd-json` over prost payloads.
- **MCP Tool Registry**: `op-mcp/src/tool_registry.rs` implements a robust memory registry for tools, offering pagination, search, schema retrieval, and execution for the Compact MCP server.
- **Multi-Zone MCP Surfaces**: Both the `op-mcp` (agents/compact) and `op-cognitive-mcp` surfaces exist as distinct crates, reflecting the required isolation zones.

## Partially Implemented or Stubbed
- **Plugin Schema Provider**: Within `op-grpc-bridge/src/grpc_server.rs`, `PluginSchemaProvider` is currently backed by an `EmptyPluginProvider` stub that returns empty vectors for plugins and schemas. The real D-Bus plugin registry integration isn't wired.
- **Agent Definitions**: Instead of dynamically sourcing agent capabilities from the registry, `op-mcp/src/agents_main.rs` contains hardcoded tool definitions (e.g., `agent_sequential_thinking`, `agent_memory`, `agent_code_review`).
- **Session Management**: Agent startup currently reads from an environment variable (`OP_RUN_ON_CONNECTION_AGENTS`) instead of querying the ComponentRegistry.
- **Application Service Plane (ASP) Validation**: While requests are marshalled through `CallMethodRequest` with `actor_id` and `capability_id` fields, advanced policy validation checks prior to tool dispatch remain largely stubbed.

## Key Patterns and Interfaces Discovered
- **simd-json over Protobuf**: High-performance JSON processing is heavily relied upon, converting `prost_types::Value` to/from `simd_json::OwnedValue` to pass state seamlessly between gRPC and the JSON-RPC live-state substrate.
- **Reflective Service Discovery**: Using the tonic reflection service ensures all clients naturally adapt to newly registered plugins, avoiding protocol drift.
- **Trust Context Forwarding**: All gRPC requests encapsulate the calling session and authorization details to respect the zero-trust paradigm.

## Notable Gaps Between Stated Design and Current Code
- The design requires dynamic "Plugin Registry Lifecycle" and "Agent Registry" discovery, but the current code relies on static lists and env vars.
- The ASP governance layer needs full schema validation and permission enforcement to be truly integrated before reaching the execution layers.
