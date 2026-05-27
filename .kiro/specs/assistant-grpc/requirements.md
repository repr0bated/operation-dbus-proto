# Product Requirements Document: gRPC Gateway for Assistant Integration

## 1. Executive Summary
Assistant is a self-hosted personal AI assistant gateway that connects chat apps to AI coding agents. Currently, the system exposes Assistant via HTTP endpoints (port 18789) and WebSocket control plane. The goal of this initiative is to add a gRPC interface that wraps the existing Assistant HTTP gateway, enabling high-performance, typed communication between system components while maintaining full compatibility with Assistant's baseline functionality.

## 2. Goals & Objectives
- **gRPC Interface**: Add a gRPC service layer that proxies requests to Assistant's HTTP gateway, enabling typed, high-performance inter-service communication
- **Zero-Trust Architecture**: Leverage WireGuard identity for service-to-service authentication, removing the need for API keys or token-based security
- **Assistant Baseline**: Maintain full compatibility with Assistant's existing functionality (agents, sessions, tasks, tools, skills, cron jobs)
- **Security Simplification**: Remove HTTP-level security checks since WireGuard provides network-level zero-trust for gRPC services

## 3. Scope
**In Scope:**
- gRPC service definitions for Assistant control plane operations
- gRPC-to-HTTP gateway proxy that forwards requests to Assistant's HTTP gateway (port 18789)
- Agent management operations (list, create, update, delete, run)
- Session management operations (list, create, delete, history)
- Task management operations (list, execute, stream)
- Tool and model discovery via gRPC
- Cron job management via gRPC
- WireGuard identity-based authentication for gRPC services
- Removal of HTTP-level security checks (API keys, tokens, IP whitelisting)

**Out of Scope:**
- Replacing Assistant's WebSocket control plane
- Modifying Assistant's core functionality
- Changing Assistant's HTTP API
- Adding new Assistant features beyond the existing API surface

## 4. Functional Requirements

### 4.1. gRPC Service Definitions
- Define Protocol Buffers (proto3) for all Assistant control plane operations
- Services must include: AgentService, SessionService, TaskService, ToolService, ModelService, CronService
- Each service must map to corresponding Assistant HTTP endpoints
- Support bidirectional streaming for long-running operations

### 4.2. gRPC-to-HTTP Gateway Proxy
- Implement a proxy that translates gRPC requests to HTTP requests to Assistant's HTTP gateway
- Forward all request headers and body content to Assistant's HTTP gateway
- Parse Assistant's HTTP responses and convert to gRPC responses
- Handle Assistant's JSON-RPC response format

### 4.3. WireGuard Identity Authentication
- Use WireGuard public key as the identity for gRPC service authentication
- Extract WireGuard identity from connection metadata
- Validate that the connecting service is authorized via WireGuard network policies
- Remove all HTTP-level security checks (API keys, tokens, IP whitelisting)

### 4.4. Agent Operations
- List all configured agents
- Create new agents with configuration
- Update existing agents
- Delete agents
- Start agent runs with input
- Stream run events
- Cancel running runs

### 4.5. Session Operations
- List all active sessions
- Create new sessions
- Delete sessions
- Retrieve session history
- Send messages to sessions

### 4.6. Task Operations
- List available tools
- Execute tasks with parameters
- Stream task execution events
- Retrieve task results

### 4.7. Model Operations
- List available models
- Get model details
- Switch active model

### 4.8. Cron Operations
- List scheduled cron jobs
- Create new cron jobs
- Delete cron jobs
- Trigger cron jobs

## 5. Non-Functional Requirements

### 5.1. Performance
- gRPC proxy should add minimal latency (<10ms) compared to direct HTTP calls
- Support concurrent connections for high-throughput scenarios
- Efficient serialization using Protocol Buffers

### 5.2. Reliability
- Graceful handling of Assistant gateway unavailability
- Automatic reconnection to Assistant gateway
- Proper error propagation from Assistant to gRPC clients

### 5.3. Security
- WireGuard identity provides network-level zero-trust
- No API keys or tokens required for gRPC communication
- TLS encryption for gRPC transport (optional, can be disabled in trusted networks)

### 5.4. Maintainability
- Code should follow existing project patterns (Rust, tonic/gRPC)
- Clear separation between gRPC layer and Assistant HTTP client
- Comprehensive error handling and logging

## 6. Assumptions & Clarifications
- Assistant's HTTP gateway (port 18789) remains the source of truth for all operations
- WireGuard identity is available via system metadata for all gRPC connections
- All Assistant functionality is accessible via its HTTP API
- The gRPC interface is additive and doesn't replace existing HTTP/WebSocket interfaces
- Assistant's security model (DM policies, allowlists) remains in place at the HTTP layer

## 7. OpenClaw Memory Preservation

### 7.1. Soul Memory
- Preserve OpenClaw's "soul" memory system which stores persistent agent identity and personality
- Soul memory contains core agent identity, behavioral patterns, and long-term personality traits
- Ensure soul memory is preserved when agents are migrated through the gRPC interface
- Provide gRPC methods to query and update soul memory for agents

### 7.2. Namespace Memory
- Implement namespace memory isolation for individual containers/agents
- Each agent/container should have its own isolated memory namespace
- Prevent memory leakage between different agents or containers
- Support memory namespace operations via gRPC:
  - `GetMemoryNamespace` - Get memory namespace for an agent
  - `SetMemoryNamespace` - Set memory namespace for an agent
  - `ClearMemoryNamespace` - Clear memory namespace for an agent
  - `ListMemoryNamespaces` - List all memory namespaces

### 7.3. Memory Operations
- Implement gRPC methods for memory operations:
  - `ReadMemory` - Read memory entries from a namespace
  - `WriteMemory` - Write memory entries to a namespace
  - `DeleteMemory` - Delete memory entries from a namespace
  - `SearchMemory` - Search memory entries across namespaces
  - `GetMemoryStats` - Get memory statistics for a namespace

### 7.4. Backward Compatibility
- Ensure existing OpenClaw HTTP API memory operations remain functional
- gRPC interface should be additive, not replacing existing memory operations
- Memory operations should work seamlessly between HTTP and gRPC interfaces

## 8. Assumptions & Clarifications
- Assistant's HTTP gateway (port 18789) remains the source of truth for all operations
- WireGuard identity is available via system metadata for all gRPC connections
- All Assistant functionality is accessible via its HTTP API
- The gRPC interface is additive and doesn't replace existing HTTP/WebSocket interfaces
- Assistant's security model (DM policies, allowlists) remains in place at the HTTP layer
- OpenClaw's soul memory and namespace memory systems are preserved and enhanced via gRPC
- Memory operations via gRPC should integrate with existing OpenClaw memory infrastructure

## 9. Transport Layer Strategy

### 9.1. D-Bus First
- Use D-Bus as the primary transport for all Assistant control plane operations
- D-Bus provides low-latency, local inter-process communication
- Leverage existing D-Bus infrastructure in the project (op-dbus)
- D-Bus provides built-in authentication via Unix socket permissions

### 9.2. RPC Fallback
- Fall back to RPC (JSON-RPC over HTTP) when D-Bus is unavailable
- RPC fallback should be transparent to clients
- Implement automatic failover from D-Bus to RPC
- RPC should be used for remote connections where D-Bus is not available

### 9.3. Transport Selection Logic
- Check for D-Bus availability at startup
- If D-Bus is available, use D-Bus for all local operations
- If D-Bus is unavailable, fall back to RPC (HTTP on port 18789)
- Support explicit transport selection via configuration

### 9.4. Configuration
```bash
# Transport configuration
OP_ASSISTANT_TRANSPORT=dbus  # or rpc
OP_ASSISTANT_RPC_ENDPOINT=http://127.0.0.1:18789
```

### 9.5. Benefits
- **D-Bus**: Lower latency, built-in authentication, local-only
- **RPC**: Remote access, fallback when D-Bus unavailable
- **Automatic failover**: Seamless transition between transports