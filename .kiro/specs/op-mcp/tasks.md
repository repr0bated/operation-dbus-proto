# op-mcp - Tasks

## Phase 1: Core Protocol and Transports

- [ ] Implement the `McpProtocol` for JSON-RPC 2.0 message parsing and framing.
- [ ] Develop the `StdioTransport` with async I/O using `tokio::io`.
- [ ] Create the `HttpTransport` (SSE) using `axum` for HTTP-based clients.
- [ ] Implement the `WebSocketTransport` for bidirectional MCP streams.
- [ ] Build the `GrpcTransport` with `tonic` and `prost` for high-performance internal communication.
- [ ] Add unit tests for all transport types to verify correct message delivery and framing.

## Phase 2: Tool Registry and Adapters

- [ ] Implement the `ToolRegistry` for managing tool definitions and execution logic.
- [ ] Develop the `SystemdAdapter` for managing systemd units.
- [ ] Create the `ShellAdapter` with safety checks and pre-approved command sets.
- [ ] Build the `DbusAdapter` to dynamically generate MCP tools from D-Bus introspection data (using `op-introspection`).
- [ ] Implement the `OvsAdapter` for Open vSwitch configuration.
- [ ] Verify each tool adapter's execution and error handling.

## Phase 3: Resource Management and Integration

- [ ] Implement the `ResourceProvider` for exposing system state as MCP resources.
- [ ] Create `ResourceTemplates` for dynamic resource discovery and URI expansion.
- [ ] Integrate `op-state` and `op-state-store` for persistent tool and resource data.
- [ ] Build the `McpDispatcher` to route incoming requests to the appropriate handlers.
- [ ] Add unit and integration tests for resource access and discovery.

## Phase 4: Agent Execution and Orchestration

- [ ] Implement the `AgentExecutor` trait for multi-step task orchestration.
- [ ] Develop the `OrchestratedToolAdapter` for combining multiple tool sources.
- [ ] Integrate with `op-chat` and other internal services.
- [ ] Add detailed tracing and logging using the `tracing` crate.
- [ ] Perform a full security review of tool execution and resource access.
- [ ] Write end-to-end integration tests for the full MCP server lifecycle.
