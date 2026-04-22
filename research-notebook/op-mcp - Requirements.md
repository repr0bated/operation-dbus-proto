# op-mcp - Requirements

## Problem Statement

The system requires a unified, high-performance implementation of the Model Context Protocol (MCP) that can serve as a bridge between LLMs (clients) and the underlying system state (tools, resources, and prompts). It must support multiple transports and provide a pluggable architecture for tools.

## Goals

1.  **Transport Versatility**: Support HTTP/SSE, WebSockets, Stdio, and gRPC transports.
2.  **Unified Tool Interface**: Provide a consistent way to define and execute tools across different system components (D-Bus, Shell, Filesystem, etc.).
3.  **State Management**: Integrate with `op-state` and `op-state-store` for persistent tool and resource data.
4.  **Extensibility**: Allow for easy addition of new tools and resources via a registry system.
5.  **Performance**: Utilize `simd-json` and asynchronous processing for low-latency request handling.

## Functional Requirements

### FR1: MCP Protocol Support
- Fully implement the MCP specification (Tools, Resources, Prompts).
- Support JSON-RPC 2.0 message framing.
- Handle request/response lifecycle and notification streams.

### FR2: Multiple Transports
- **Stdio**: For local CLI-based integration (e.g., with IDEs).
- **HTTP/SSE**: For web-based clients and long-lived connections.
- **WebSocket**: For bidirectional, real-time communication.
- **gRPC**: For high-performance, internal service-to-service communication.

### FR3: Tool Registry and Adapters
- Maintain a registry of available tools with their JSON schemas.
- Provide adapters for different tool types:
    - `Systemd`: Manage systemd units.
    - `Shell`: Execute controlled shell commands.
    - `Filesystem`: Secure file operations.
    - `D-Bus`: Interact with D-Bus services (via `op-introspection`).
    - `OVS`: Open vSwitch configuration.

### FR4: Resource Management
- Expose system state as MCP resources (e.g., logs, configuration files, D-Bus object properties).
- Support resource templates and dynamic resource discovery.

### FR5: Agent Execution
- Provide a `trait_agent_executor` for running complex, multi-step tasks.
- Support orchestration between multiple specialized agents.

## Non-Functional Requirements

### NFR1: Performance
- High-throughput JSON parsing with `simd-json`.
- Non-blocking I/O using `tokio`.
- Minimal memory footprint for long-running server processes.

### NFR2: Security
- Secure transport options (TLS for HTTP/WebSocket).
- Input validation against JSON schemas for all tool calls.
- Resource access control and sandboxing for shell/filesystem tools.

### NFR3: Observability
- Integrated tracing using the `tracing` crate.
- Detailed logging of MCP requests and tool executions.
- Error reporting with `anyhow` and `thiserror`.

## Success Criteria

1.  Successful connection and interaction via all supported transports.
2.  Full discovery and execution of tools registered in the `ToolRegistry`.
3.  Resources correctly exposed and readable by MCP clients.
4.  Seamless integration with `op-core` and other internal crates.