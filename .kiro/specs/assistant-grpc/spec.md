# Technical Specification: gRPC Gateway for Assistant Integration

## 1. Technical Context

- **Languages & Frameworks**: Rust, Tonic/gRPC (using `prost` for proto generation), Serde for JSON serialization
- **Dependencies**: `tonic`, `prost`, `reqwest`, `serde`, `serde_json`, `tokio`, `tracing`, `thiserror`, `zbus`, `op-cognitive-mcp`
- **Deployment Target**: Runs on the host, targeting `op-grpc-bridge` inside the `wg-xray` Incus container (`10.200.0.1:50051` via the `grpc-uplink` bridge)
- **Routing**: Xray OpenFlow rules route outbound RPC using schema-tag headers (`x-ghostbridge-footprint`, `x-ghostbridge-trace-id`) sourced from `/dev/shm/plugin_schema.dat`
- **Authentication**: WireGuard identity-based (zero-trust via network-level authentication)
- **Memory/Soul**: Backed directly by `op-cognitive-mcp`'s `CognitiveMemoryStore` and `SoulMemoryStore` (CozoDB) — no HTTP round-trip

## 2. Implementation Approach

### 2.1. gRPC Service Architecture

- Create a new crate `op-assistant-grpc` for the gRPC gateway
- Implement a gRPC-to-HTTP proxy that translates between gRPC and Assistant's HTTP API
- Use Protocol Buffers (proto3) for all service definitions
- Each Assistant API surface (agents, sessions, tasks, models, cron) gets its own gRPC service

### 2.2. gRPC-to-HTTP Proxy Pattern

```
gRPC Client → gRPC Server → Assistant HTTP Client → Assistant HTTP Gateway
```

- gRPC server receives requests from internal services
- Primary transport: D-Bus session bus (shared into wg-xray container via Incus disk device)
- Fallback: JSON-RPC over HTTP to `10.200.0.1:50051` (the wg-xray bridge IP)
- Every outbound RPC carries `x-ghostbridge-footprint` and `x-ghostbridge-trace-id` headers sourced from `/dev/shm/plugin_schema.dat` so Xray's OpenFlow controller can route correctly
- Soul/Namespace/Memory services bypass RPC entirely and call `op-cognitive-mcp`'s CozoDB store in-process

### 2.3. WireGuard Identity Authentication

- Extract WireGuard public key from connection metadata (`x-wireguard-pubkey`)
- Validate the pubkey format and authorization
- Add pubkey to request extensions for downstream use
- Remove all HTTP-level security checks (API keys, tokens, IP whitelisting)
- Trust is established at the WireGuard network level, not application level

### 2.4. Error Handling

- Map Assistant HTTP errors to appropriate gRPC status codes:
  - 404 → `Status::not_found()`
  - 401 → `Status::unauthenticated()`
  - 403 → `Status::permission_denied()`
  - 400 → `Status::invalid_argument()`
  - 500 → `Status::internal()`
  - Other → `Status::unknown()`
- Preserve error messages from Assistant in gRPC status details

### 2.5. Streaming Support

- Use gRPC server streaming for long-running operations (run events, task execution)
- Use gRPC client streaming for upload operations (if needed)
- Use bidirectional streaming for interactive sessions (if needed)

## 3. Source Code Structure Changes

### New Crate: `crates/op-assistant-grpc/`

```
crates/op-assistant-grpc/
├── src/
│   ├── lib.rs              # Module roots and re-exports
│   ├── server.rs           # tonic server wiring + CozoDB store construction
│   ├── auth.rs             # WireGuard identity interceptor
│   ├── transport.rs        # D-Bus + RPC dual transport with auto-failover
│   ├── client.rs           # AssistantClient JSON-RPC envelope unwrapping
│   ├── convert.rs          # proto ↔ JSON helpers (prost_types::Struct ↔ serde_json::Value)
│   ├── incus.rs            # wg-xray endpoint + IdentitySled schema-tag reader
│   ├── error.rs            # AssistantError enum with Status mapping
│   ├── dbus_service.rs     # zbus interface ai.assistant.v1
│   ├── agents.rs           # AgentService (with StreamRunEvents)
│   ├── sessions.rs         # SessionService
│   ├── tasks.rs            # TaskService (with StreamTaskExecution)
│   ├── models.rs           # ModelService
│   ├── cron.rs             # CronService
│   ├── soul.rs             # SoulService → SoulMemoryStore (CozoDB)
│   ├── namespace.rs        # NamespaceMemoryService → SoulMemoryStore bindings
│   ├── memory.rs           # MemoryService → CognitiveMemoryStore (CozoDB)
│   └── bin/
│       └── op-assistant-grpc.rs  # Binary entry point
├── proto/
│   └── assistant/
│       ├── common.proto    # Shared messages (Empty, Pagination, etc.)
│       ├── agent.proto     # AgentService + messages
│       ├── session.proto   # SessionService
│       ├── task.proto      # TaskService
│       ├── model.proto     # ModelService
│       ├── cron.proto      # CronService
│       ├── soul.proto      # SoulService
│       ├── namespace.proto # NamespaceMemoryService
│       └── memory.proto    # MemoryService
├── build.rs                # tonic-build for 9 protos
├── Cargo.toml
└── tests/
    └── integration.rs      # 3 in-memory Cozo integration tests
```

### Modified Files

- **`crates/op-web/src/handlers/openclaw.rs`**: Mark as deprecated, consider removing in future
- **`crates/op-web/src/routes/mod.rs`**: Update comments to indicate gRPC is preferred

### New Files

- **`crates/op-assistant-grpc/Cargo.toml`**: New crate configuration
- **`crates/op-assistant-grpc/proto/assistant.proto`**: Main proto definition
- **`crates/op-assistant-grpc/src/*.rs`**: All implementation files

## 4. Data Model / API / Interface Changes

### gRPC Services

| Service        | Methods                                                                                           | Description                      |
| -------------- | ------------------------------------------------------------------------------------------------- | -------------------------------- |
| AgentService   | ListAgents, GetAgent, CreateAgent, UpdateAgent, DeleteAgent, StartRun, StreamRunEvents, CancelRun | Agent management and run control |
| SessionService | ListSessions, GetSession, CreateSession, DeleteSession, GetSessionHistory, SendMessage            | Session management               |
| TaskService    | ListTools, ExecuteTask, StreamTaskExecution, GetTaskResult                                        | Task execution                   |
| ModelService   | ListModels, GetModel, SwitchModel                                                                 | Model management                 |
| CronService    | ListCronJobs, CreateCronJob, DeleteCronJob, TriggerCronJob                                        | Cron job management              |

### Request/Response Messages

Each service defines request and response messages for all operations:

- `ListAgentsRequest` / `ListAgentsResponse`
- `GetAgentRequest` / `Agent`
- `CreateAgentRequest` / `Agent`
- `UpdateAgentRequest` / `Agent`
- `DeleteAgentRequest` / `Empty`
- `StartRunRequest` / `Run`
- `StreamRunEventsRequest` / `stream RunEvent`
- `CancelRunRequest` / `Empty`
- And more for each service...

### Authentication Metadata

- `x-wireguard-pubkey`: WireGuard public key for service authentication
- Extracted from connection metadata
- Validated before request processing

## 5. Verification Approach

### Build Commands

```bash
# Build the new crate
cargo build -p op-assistant-grpc

# Check all crates
cargo check --workspace

# Run clippy
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run tests
cargo test -p op-assistant-grpc --all-targets --all-features
```

### Test Coverage

- **Unit Tests**: Test each gRPC method implementation
- **Integration Tests**: Test full request flow through proxy
- **E2E Tests**: Test against real Assistant instance

### Verification Checklist

- [ ] Proto files compile without errors
- [ ] gRPC server starts on configured port
- [ ] Authentication middleware extracts WireGuard identity
- [ ] HTTP client successfully connects to Assistant gateway
- [ ] All gRPC methods return correct responses
- [ ] Error mapping works correctly
- [ ] Streaming endpoints work correctly
- [ ] Integration tests pass
- [ ] No clippy warnings

## 6. Configuration

### Environment Variables

```bash
# gRPC server configuration
OP_ASSISTANT_GRPC_HOST=127.0.0.1
OP_ASSISTANT_GRPC_PORT=50052

# Assistant RPC endpoint (wg-xray container's op-grpc-bridge)
OP_ASSISTANT_RPC_ENDPOINT=http://10.200.0.1:50051

# CozoDB path for memory/soul/namespace stores (empty = in-memory)
OP_ASSISTANT_COZO_PATH=/var/lib/op-dbus/assistant-grpc.db

# Identity sled for ghostbridge header injection
OP_ASSISTANT_SCHEMA_PATH=/dev/shm/plugin_schema.dat

# D-Bus session address (shared into wg-xray container)
DBUS_SESSION_BUS_ADDRESS=unix:path=/run/op-dbus/session-bus.sock

# Transport override: "dbus" or "rpc"/"http"
OP_ASSISTANT_TRANSPORT=

# Logging
RUST_LOG=op_assistant_grpc=info,info
OP_ASSISTANT_HTTP_TIMEOUT=30

# Logging
RUST_LOG=info
```

### Cargo.toml Dependencies

```toml
[dependencies]
tonic = "0.11"
prost = "0.12"
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
tracing = "0.1"
thiserror = "1.0"
```

## 7. Migration Path

1. **Phase 1**: Create proto definitions and generate Rust code
2. **Phase 2**: Implement gRPC server skeleton with authentication
3. **Phase 3**: Implement Assistant HTTP client wrapper
4. **Phase 4**: Implement each gRPC service (agents, sessions, tasks, models, cron)
5. **Phase 5**: Add comprehensive tests
6. **Phase 6**: Deprecate HTTP handlers in favor of gRPC (optional)

## 8. Security Considerations

- **Zero-Trust Architecture**: WireGuard identity provides network-level zero-trust
- **No API Keys**: gRPC communication doesn't require API keys or tokens
- **TLS Encryption**: Optional for gRPC transport (can be disabled in trusted networks)
- **Assistant Security**: Assistant's existing security model (DM policies, allowlists) remains in place
- **Authentication**: Happens at WireGuard network level, not application level

## 9. OpenClaw Memory Preservation

### 9.1. Soul Memory Service

- Soul memory contains persistent agent identity, behavioral patterns, and long-term personality traits
- Soul memory is preserved across sessions and agent migrations
- Provide gRPC methods to query and update soul memory for agents

### 9.2. Namespace Memory Service

- Namespace memory provides isolation for individual containers/agents
- Each agent/container has its own isolated memory namespace
- Prevent memory leakage between different agents or containers

### 9.3. Memory Operations Service

- Read/Write/Delete memory entries from namespaces
- Search memory entries across namespaces
- Get memory statistics for namespaces

### 9.4. Updated gRPC Services

| Service                | Methods                                                                                           | Description                      |
| ---------------------- | ------------------------------------------------------------------------------------------------- | -------------------------------- |
| AgentService           | ListAgents, GetAgent, CreateAgent, UpdateAgent, DeleteAgent, StartRun, StreamRunEvents, CancelRun | Agent management and run control |
| SessionService         | ListSessions, GetSession, CreateSession, DeleteSession, GetSessionHistory, SendMessage            | Session management               |
| TaskService            | ListTools, ExecuteTask, StreamTaskExecution, GetTaskResult                                        | Task execution                   |
| ModelService           | ListModels, GetModel, SwitchModel                                                                 | Model management                 |
| CronService            | ListCronJobs, CreateCronJob, DeleteCronJob, TriggerCronJob                                        | Cron job management              |
| SoulService            | GetSoulMemory, UpdateSoulMemory, DeleteSoulMemory, ListSoulMemories                               | Soul memory operations           |
| NamespaceMemoryService | GetMemoryNamespace, SetMemoryNamespace, ClearMemoryNamespace, ListMemoryNamespaces                | Namespace memory operations      |
| MemoryService          | ReadMemory, WriteMemory, DeleteMemory, SearchMemory, GetMemoryStats                               | Memory operations                |

### 9.5. Updated Project Structure

```
crates/op-assistant-grpc/
├── src/
│   ├── lib.rs              # Main module
│   ├── server.rs           # gRPC server setup with authentication
│   ├── auth.rs             # WireGuard identity authentication
│   ├── client.rs           # Assistant HTTP client wrapper
│   ├── agents.rs           # AgentService implementation
│   ├── sessions.rs         # SessionService implementation
│   ├── tasks.rs            # TaskService implementation
│   ├── models.rs           # ModelService implementation
│   ├── cron.rs             # CronService implementation
│   ├── soul.rs             # SoulService implementation
│   ├── namespace.rs        # NamespaceMemoryService implementation
│   └── memory.rs           # MemoryService implementation
├── proto/
│   ├── assistant/
│   │   ├── agent.proto     # Agent service definitions
│   │   ├── session.proto   # Session service definitions
│   │   ├── task.proto      # Task service definitions
│   │   ├── model.proto     # Model service definitions
│   │   ├── cron.proto      # Cron service definitions
│   │   ├── soul.proto      # Soul memory service definitions
│   │   ├── namespace.proto # Namespace memory service definitions
│   │   └── memory.proto    # Memory operations service definitions
│   └── assistant.proto     # Main proto file
├── build.rs                # tonic-build configuration
├── Cargo.toml
└── tests/
    └── integration.rs      # Integration tests
```

### 9.6. Updated Cargo.toml Dependencies

```toml
[dependencies]
tonic = "0.11"
prost = "0.12"
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
tracing = "0.1"
thiserror = "1.0"
```

### 9.7. Updated Migration Path

1. **Phase 1**: Create proto definitions and generate Rust code
2. **Phase 2**: Implement gRPC server skeleton with authentication
3. **Phase 3**: Implement Assistant HTTP client wrapper
4. **Phase 4**: Implement each gRPC service (agents, sessions, tasks, models, cron)
5. **Phase 5**: Add comprehensive tests
6. **Phase 6**: Implement memory services (soul, namespace, memory operations)
7. **Phase 7**: Deprecate HTTP handlers in favor of gRPC (optional)

### 9.8. Updated Security Considerations

- **Zero-Trust Architecture**: WireGuard identity provides network-level zero-trust
- **No API Keys**: gRPC communication doesn't require API keys or tokens
- **TLS Encryption**: Optional for gRPC transport (can be disabled in trusted networks)
- **Assistant Security**: Assistant's existing security model (DM policies, allowlists) remains in place
- **Authentication**: Happens at WireGuard network level, not application level
- **Memory Security**: Memory operations respect existing OpenClaw memory access controls
- **Namespace Isolation**: Prevents memory leakage between different agents or containers

## 10. Transport Layer Strategy

### 10.1. D-Bus First Architecture

- Use D-Bus as the primary transport for all Assistant control plane operations
- D-Bus provides low-latency, local inter-process communication
- Leverage existing D-Bus infrastructure in the project (op-dbus)
- D-Bus provides built-in authentication via Unix socket permissions
- Zero-trust via D-Bus policy files (no additional auth needed)

### 10.2. RPC Fallback

- Fall back to RPC (JSON-RPC over HTTP) when D-Bus is unavailable
- RPC fallback should be transparent to clients
- Implement automatic failover from D-Bus to RPC
- RPC should be used for remote connections where D-Bus is not available

### 10.3. Transport Selection Logic

```rust
enum Transport {
    Dbus(dbus::Connection),
    Rpc(reqwest::Client),
}

impl Transport {
    fn new() -> Result<Self> {
        // Try D-Bus first
        match dbus::Connection::new_session() {
            Ok(conn) => Ok(Transport::Dbus(conn)),
            Err(_) => {
                // Fall back to RPC
                Ok(Transport::Rpc(reqwest::Client::new()))
            }
        }
    }
}
```

### 10.4. Updated Project Structure

```
crates/op-assistant-grpc/
├── src/
│   ├── lib.rs              # Main module
│   ├── server.rs           # gRPC server setup with authentication
│   ├── auth.rs             # WireGuard identity authentication
│   ├── transport.rs        # D-Bus/RPC transport layer
│   ├── client.rs           # Assistant client wrapper
│   ├── agents.rs           # AgentService implementation
│   ├── sessions.rs         # SessionService implementation
│   ├── tasks.rs            # TaskService implementation
│   ├── models.rs           # ModelService implementation
│   ├── cron.rs             # CronService implementation
│   ├── soul.rs             # SoulService implementation
│   ├── namespace.rs        # NamespaceMemoryService implementation
│   └── memory.rs           # MemoryService implementation
├── proto/
│   ├── assistant/
│   │   ├── agent.proto     # Agent service definitions
│   │   ├── session.proto   # Session service definitions
│   │   ├── task.proto      # Task service definitions
│   │   ├── model.proto     # Model service definitions
│   │   ├── cron.proto      # Cron service definitions
│   │   ├── soul.proto      # Soul memory service definitions
│   │   ├── namespace.proto # Namespace memory service definitions
│   │   └── memory.proto    # Memory operations service definitions
│   └── assistant.proto     # Main proto file
├── build.rs                # tonic-build configuration
├── Cargo.toml
└── tests/
    └── integration.rs      # Integration tests
```

### 10.5. Updated Cargo.toml Dependencies

```toml
[dependencies]
tonic = "0.11"
prost = "0.12"
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
tracing = "0.1"
thiserror = "1.0"
dbus = "0.9"
```

### 10.6. Updated Migration Path

1. **Phase 1**: Create proto definitions and generate Rust code
2. **Phase 2**: Implement gRPC server skeleton with authentication
3. **Phase 3**: Implement transport layer (D-Bus + RPC fallback)
4. **Phase 4**: Implement Assistant client wrapper
5. **Phase 5**: Implement each gRPC service (agents, sessions, tasks, models, cron)
6. **Phase 6**: Add comprehensive tests
7. **Phase 7**: Implement memory services (soul, namespace, memory operations)
8. **Phase 8**: Deprecate HTTP handlers in favor of gRPC (optional)

### 10.7. Updated Security Considerations

- **Zero-Trust Architecture**: WireGuard identity provides network-level zero-trust
- **No API Keys**: gRPC communication doesn't require API keys or tokens
- **TLS Encryption**: Optional for gRPC transport (can be disabled in trusted networks)
- **Assistant Security**: Assistant's existing security model (DM policies, allowlists) remains in place
- **Authentication**: Happens at WireGuard network level, not application level
- **Memory Security**: Memory operations respect existing OpenClaw memory access controls
- **Namespace Isolation**: Prevents memory leakage between different agents or containers
- **D-Bus Zero-Trust**: D-Bus policy files provide authentication without additional overhead
