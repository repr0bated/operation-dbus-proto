# Design Document: gRPC Gateway for Assistant Integration

## 1. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                        gRPC Clients                                 │
│  (op-agents, op-chat, op-llm, etc.)                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │  AgentService│  │SessionService│  │  TaskService │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │  SoulService │  │NamespaceSvc │  │ MemoryService│              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ gRPC (127.0.0.1:50052)
                                    │
┌─────────────────────────────────────────────────────────────────────┐
│              op-assistant-grpc  (runs on the host)                  │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  WireGuard Identity → Auth Interceptor                     │  │
│  │  (x-wireguard-pubkey from connection metadata)             │  │
│  └──────────────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  Transport: D-Bus first → HTTP-RPC fallback                │  │
│  │  + SchemaTags (x-ghostbridge-footprint/trace-id from sled) │  │
│  └──────────────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  Soul/Namespace/Memory → op-cognitive-mcp (CozoDB in-proc) │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                    │                           │
         D-Bus (shared socket)     HTTP-RPC + ghostbridge headers
                    │                           │
┌─────────────────────────────────────────────────────────────────────┐
│             wg-xray Incus Container (10.200.0.1/30)                │
│  ┌────────────────────┐  ┌──────────────────────────────────────┐ │
│  │  op-grpc-bridge    │  │  Xray (OpenFlow + PluginSchema tags) │ │
│  │  :50051            │  │  Routes via footprint/trace-id hdrs  │ │
│  └────────────────────┘  └──────────────────────────────────────┘ │
│  D-Bus sockets bind-mounted from host:                             │
│    /run/host-dbus/system_bus_socket                                │
│    /run/host-op-dbus/session-bus.sock                               │
└─────────────────────────────────────────────────────────────────────┘
                    │
          wg0 / wgcf egress
                    │
          ┌─────────┴─────────┐
          │  WireGuard peers   │
          │  (self-hosted      │
          │   Assistant nodes) │
          └────────────────────┘
```

## 2. gRPC Service Definitions

### 2.1. AgentService

```protobuf
service AgentService {
  // List all configured agents
  rpc ListAgents(ListAgentsRequest) returns (ListAgentsResponse);

  // Get agent by ID
  rpc GetAgent(GetAgentRequest) returns (Agent);

  // Create a new agent
  rpc CreateAgent(CreateAgentRequest) returns (Agent);

  // Update an existing agent
  rpc UpdateAgent(UpdateAgentRequest) returns (Agent);

  // Delete an agent
  rpc DeleteAgent(DeleteAgentRequest) returns (Empty);

  // Start a run for an agent
  rpc StartRun(StartRunRequest) returns (Run);

  // Stream run events
  rpc StreamRunEvents(StreamRunEventsRequest) returns (stream RunEvent);

  // Cancel a running run
  rpc CancelRun(CancelRunRequest) returns (Empty);
}
```

### 2.2. SessionService

```protobuf
service SessionService {
  // List all active sessions
  rpc ListSessions(ListSessionsRequest) returns (ListSessionsResponse);

  // Get session by ID
  rpc GetSession(GetSessionRequest) returns (Session);

  // Create a new session
  rpc CreateSession(CreateSessionRequest) returns (Session);

  // Delete a session
  rpc DeleteSession(DeleteSessionRequest) returns (Empty);

  // Get session history
  rpc GetSessionHistory(GetSessionHistoryRequest) returns (SessionHistory);

  // Send a message to a session
  rpc SendMessage(SendMessageRequest) returns (Message);
}
```

### 2.3. TaskService

```protobuf
service TaskService {
  // List available tools
  rpc ListTools(ListToolsRequest) returns (ListToolsResponse);

  // Execute a task
  rpc ExecuteTask(ExecuteTaskRequest) returns (TaskResult);

  // Stream task execution events
  rpc StreamTaskExecution(StreamTaskExecutionRequest) returns (stream TaskEvent);

  // Get task result
  rpc GetTaskResult(GetTaskResultRequest) returns (TaskResult);
}
```

### 2.4. ModelService

```protobuf
service ModelService {
  // List available models
  rpc ListModels(ListModelsRequest) returns (ListModelsResponse);

  // Get model details
  rpc GetModel(GetModelRequest) returns (Model);

  // Switch active model
  rpc SwitchModel(SwitchModelRequest) returns (Model);
}
```

### 2.5. CronService

```protobuf
service CronService {
  // List scheduled cron jobs
  rpc ListCronJobs(ListCronJobsRequest) returns (ListCronJobsResponse);

  // Create a new cron job
  rpc CreateCronJob(CreateCronJobRequest) returns (CronJob);

  // Delete a cron job
  rpc DeleteCronJob(DeleteCronJobRequest) returns (Empty);

  // Trigger a cron job
  rpc TriggerCronJob(TriggerCronJobRequest) returns (CronJob);
}
```

## 3. Implementation Approach

### 3.1. Project Structure

```
crates/op-assistant-grpc/
├── src/
│   ├── lib.rs              # Main module
│   ├── server.rs           # gRPC server setup
│   ├── auth.rs             # WireGuard identity authentication
│   ├── client.rs           # Assistant HTTP client wrapper
│   ├── agents.rs           # AgentService implementation
│   ├── sessions.rs         # SessionService implementation
│   ├── tasks.rs            # TaskService implementation
│   ├── models.rs           # ModelService implementation
│   └── cron.rs             # CronService implementation
├── proto/
│   ├── assistant.proto     # Main proto file with all services
│   └── assistant/
│       ├── agent.proto     # Agent service definitions
│       ├── session.proto   # Session service definitions
│       ├── task.proto      # Task service definitions
│       ├── model.proto     # Model service definitions
│       └── cron.proto      # Cron service definitions
├── build.rs                # tonic-build configuration
└── Cargo.toml
```

### 3.2. Authentication Flow

```rust
// Extract WireGuard identity from connection metadata
fn extract_wireguard_identity(metadata: &MetadataMap) -> Result<String, Status> {
    // Get public key from metadata
    let pubkey = metadata
        .get("x-wireguard-pubkey")
        .ok_or(Status::unauthenticated("Missing WireGuard identity"))?;

    // Validate pubkey format
    let pubkey_str = pubkey
        .to_str()
        .map_err(|_| Status::invalid_argument("Invalid pubkey format"))?;

    Ok(pubkey_str.to_string())
}

// Middleware to authenticate all gRPC requests
async fn wireguard_auth(
    req: Request<()>,
    next: Next<()>,
) -> Result<Response<()>, Status> {
    let metadata = req.metadata();
    let pubkey = extract_wireguard_identity(metadata)?;

    // Check if pubkey is authorized (via WireGuard network policies)
    if !is_authorized(&pubkey) {
        return Err(Status::permission_denied("Unauthorized"));
    }

    // Add pubkey to request extensions for downstream use
    let mut req = req;
    req.extensions_mut().insert(pubkey);

    next.run(req).await
}
```

### 3.3. gRPC-to-HTTP Proxy

```rust
// Convert gRPC request to Assistant HTTP request
fn convert_to_http_request(grpc_request: &ListAgentsRequest) -> HttpRequest {
    HttpRequest::builder()
        .method("GET")
        .uri("http://127.0.0.1:18789/api/agents")
        .body(serde_json::to_vec(&grpc_request).unwrap())
        .unwrap()
}

// Convert Assistant HTTP response to gRPC response
fn convert_to_grpc_response(http_response: HttpResponse) -> ListAgentsResponse {
    let body: Value = serde_json::from_slice(&http_response.body()).unwrap();

    ListAgentsResponse {
        agents: body
            .get("agents")
            .and_then(|a| a.as_array())
            .map(|agents| {
                agents
                    .iter()
                    .map(|a| Agent {
                        id: a.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        name: a.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        // ... map other fields
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}
```

### 3.4. Error Handling

```rust
// Map Assistant HTTP errors to gRPC status codes
fn map_error_to_status(error: &AssistantError) -> Status {
    match error {
        AssistantError::NotFound => Status::not_found(error.to_string()),
        AssistantError::Unauthorized => Status::unauthenticated(error.to_string()),
        AssistantError::Forbidden => Status::permission_denied(error.to_string()),
        AssistantError::InvalidRequest => Status::invalid_argument(error.to_string()),
        AssistantError::Internal => Status::internal(error.to_string()),
        _ => Status::unknown(error.to_string()),
    }
}
```

## 4. Configuration

### 4.1. Environment Variables

```bash
# gRPC server configuration
OP_ASSISTANT_GRPC_PORT=50051
OP_ASSISTANT_GRPC_HOST=0.0.0.0

# Assistant HTTP gateway configuration
OP_ASSISTANT_HTTP_ENDPOINT=http://127.0.0.1:18789
OP_ASSISTANT_HTTP_TIMEOUT=30

# WireGuard identity (for self-authentication)
WIREGUARD_PUBKEY=...
```

### 4.2. Cargo.toml Dependencies

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

## 5. Testing Strategy

### 5.1. Unit Tests

- Test gRPC-to-HTTP request conversion
- Test HTTP-to-gRPC response conversion
- Test WireGuard identity extraction
- Test error mapping

### 5.2. Integration Tests

- Test full request flow: gRPC client → proxy → Assistant HTTP → gRPC response
- Test authentication flow with WireGuard identity
- Test error scenarios (Assistant unavailable, invalid requests)

### 5.3. E2E Tests

- Test all gRPC services against a real Assistant instance
- Test concurrent requests
- Test streaming endpoints

## 6. Migration Path

1. **Phase 1**: Create proto definitions and generate Rust code
2. **Phase 2**: Implement gRPC server skeleton with authentication
3. **Phase 3**: Implement Assistant HTTP client wrapper
4. **Phase 4**: Implement each gRPC service (agents, sessions, tasks, models, cron)
5. **Phase 5**: Add comprehensive tests
6. **Phase 6**: Deprecate HTTP handlers in favor of gRPC (optional)

## 7. Security Considerations

- WireGuard identity provides network-level zero-trust
- No API keys or tokens required for gRPC communication
- TLS encryption for gRPC transport (optional, can be disabled in trusted networks)
- Assistant's existing security model (DM policies, allowlists) remains in place at the HTTP layer
- All authentication happens at the WireGuard network level, not at the application level

## 8. OpenClaw Memory Preservation

### 8.1. Soul Memory Service

```protobuf
service SoulService {
  // Get soul memory for an agent
  rpc GetSoulMemory(GetSoulMemoryRequest) returns (SoulMemory);

  // Update soul memory for an agent
  rpc UpdateSoulMemory(UpdateSoulMemoryRequest) returns (SoulMemory);

  // Delete soul memory for an agent
  rpc DeleteSoulMemory(DeleteSoulMemoryRequest) returns (Empty);

  // List all soul memories
  rpc ListSoulMemories(ListSoulMemoriesRequest) returns (ListSoulMemoriesResponse);
}
```

### 8.2. Namespace Memory Service

```protobuf
service NamespaceMemoryService {
  // Get memory namespace for an agent
  rpc GetMemoryNamespace(GetMemoryNamespaceRequest) returns (MemoryNamespace);

  // Set memory namespace for an agent
  rpc SetMemoryNamespace(SetMemoryNamespaceRequest) returns (MemoryNamespace);

  // Clear memory namespace for an agent
  rpc ClearMemoryNamespace(ClearMemoryNamespaceRequest) returns (Empty);

  // List all memory namespaces
  rpc ListMemoryNamespaces(ListMemoryNamespacesRequest) returns (ListMemoryNamespacesResponse);
}
```

### 8.3. Memory Operations Service

```protobuf
service MemoryService {
  // Read memory entries from a namespace
  rpc ReadMemory(ReadMemoryRequest) returns (ReadMemoryResponse);

  // Write memory entries to a namespace
  rpc WriteMemory(WriteMemoryRequest) returns (WriteMemoryResponse);

  // Delete memory entries from a namespace
  rpc DeleteMemory(DeleteMemoryRequest) returns (DeleteMemoryResponse);

  // Search memory entries across namespaces
  rpc SearchMemory(SearchMemoryRequest) returns (SearchMemoryResponse);

  // Get memory statistics for a namespace
  rpc GetMemoryStats(GetMemoryStatsRequest) returns (MemoryStats);
}
```

### 8.4. Updated Project Structure

```
crates/op-assistant-grpc/
├── src/
│   ├── lib.rs              # Main module
│   ├── server.rs           # gRPC server setup
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
└── Cargo.toml
```

### 8.5. Memory Preservation Implementation

- Soul memory contains persistent agent identity, behavioral patterns, and long-term personality traits
- Namespace memory provides isolation for individual containers/agents
- Memory operations should integrate with existing OpenClaw memory infrastructure
- Ensure backward compatibility with existing OpenClaw HTTP API memory operations
- gRPC interface should be additive, not replacing existing memory operations

### 8.6. Memory Namespace Isolation

- Each agent/container has its own isolated memory namespace
- Prevent memory leakage between different agents or containers
- Support memory namespace operations via gRPC:
  - `GetMemoryNamespace` - Get memory namespace for an agent
  - `SetMemoryNamespace` - Set memory namespace for an agent
  - `ClearMemoryNamespace` - Clear memory namespace for an agent
  - `ListMemoryNamespaces` - List all memory namespaces

### 8.7. Soul Memory Operations

- Soul memory preserves agent identity and personality across sessions
- Provide gRPC methods to query and update soul memory for agents
- Ensure soul memory is preserved when agents are migrated through the gRPC interface
- Support soul memory versioning and history tracking

## 9. Migration Path

1. **Phase 1**: Create proto definitions and generate Rust code
2. **Phase 2**: Implement gRPC server skeleton with authentication
3. **Phase 3**: Implement Assistant HTTP client wrapper
4. **Phase 4**: Implement each gRPC service (agents, sessions, tasks, models, cron)
5. **Phase 5**: Add comprehensive tests
6. **Phase 6**: Implement memory services (soul, namespace, memory operations)
7. **Phase 7**: Deprecate HTTP handlers in favor of gRPC (optional)

## 10. Security Considerations

- WireGuard identity provides network-level zero-trust
- No API keys or tokens required for gRPC communication
- TLS encryption for gRPC transport (optional, can be disabled in trusted networks)
- Assistant's existing security model (DM policies, allowlists) remains in place at the HTTP layer
- All authentication happens at the WireGuard network level, not at the application level
- Memory operations respect existing OpenClaw memory access controls

## 11. Transport Layer Strategy

### 11.1. D-Bus First Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        gRPC Clients                                 │
│  (op-agents, op-chat, op-llm, etc.)                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │  AgentService│  │SessionService│  │  TaskService │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    │                               │
              ┌─────▼─────┐                 ┌───────▼───────┐
              │   D-Bus   │                 │     RPC       │
              │  (Primary)│                 │  (Fallback)   │
              └─────┬─────┘                 └───────┬───────┘
                    │                               │
              ┌─────▼─────┐                 ┌───────▼───────┐
              │  op-dbus  │                 │  HTTP/JSON    │
              │  service  │                 │  (port 18789) │
              └───────────┘                 └───────────────┘
```

### 11.2. D-Bus Implementation

- Use `dbus` crate for D-Bus communication
- Create D-Bus service interface for Assistant operations
- D-Bus path: `/ai/assistant`
- D-Bus interface: `ai.assistant.v1`
- D-Bus provides built-in authentication via Unix socket permissions
- Zero-trust via D-Bus policy files (no additional auth needed)

### 11.3. RPC Fallback Implementation

- Fall back to HTTP/JSON-RPC when D-Bus is unavailable
- Use `reqwest` for HTTP requests
- RPC endpoint: `http://127.0.0.1:18789`
- Automatic failover from D-Bus to RPC

### 11.4. Transport Selection Logic

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

### 11.5. Updated Project Structure

```
crates/op-assistant-grpc/
├── src/
│   ├── lib.rs              # Main module
│   ├── server.rs           # gRPC server setup
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

### 11.6. D-Bus Service Interface

```protobuf
// D-Bus interface for Assistant operations
interface ai.assistant.v1 {
    // Agent operations
    method ListAgents() returns (array<Agent>);
    method GetAgent(string id) returns (Agent);
    method CreateAgent(Agent agent) returns (string);
    method UpdateAgent(Agent agent) returns (bool);
    method DeleteAgent(string id) returns (bool);

    // Session operations
    method ListSessions() returns (array<Session>);
    method GetSession(string id) returns (Session);
    method CreateSession(Session session) returns (string);
    method DeleteSession(string id) returns (bool);

    // Memory operations
    method GetSoulMemory(string id) returns (SoulMemory);
    method UpdateSoulMemory(SoulMemory memory) returns (bool);
    method GetMemoryNamespace(string id) returns (string);
    method SetMemoryNamespace(string id, string namespace) returns (bool);
}
```

### 11.7. Configuration

```bash
# Transport configuration
OP_ASSISTANT_TRANSPORT=dbus  # or rpc
OP_ASSISTANT_RPC_ENDPOINT=http://127.0.0.1:18789

# D-Bus configuration
OP_ASSISTANT_DBUS_NAME=ai.assistant.v1
OP_ASSISTANT_DBUS_PATH=/ai/assistant
```

### 11.8. Benefits

- **D-Bus**: Lower latency, built-in authentication, local-only, zero-trust via policy files
- **RPC**: Remote access, fallback when D-Bus unavailable
- **Automatic failover**: Seamless transition between transports
- **Zero-trust**: D-Bus policy files provide authentication without additional overhead

## 12. Migration Path

1. **Phase 1**: Create proto definitions and generate Rust code
2. **Phase 2**: Implement gRPC server skeleton with authentication
3. **Phase 3**: Implement transport layer (D-Bus + RPC fallback)
4. **Phase 4**: Implement Assistant client wrapper
5. **Phase 5**: Implement each gRPC service (agents, sessions, tasks, models, cron)
6. **Phase 6**: Add comprehensive tests
7. **Phase 7**: Implement memory services (soul, namespace, memory operations)
8. **Phase 8**: Deprecate HTTP handlers in favor of gRPC (optional)

## 13. Security Considerations

- WireGuard identity provides network-level zero-trust
- No API keys or tokens required for gRPC communication
- TLS encryption for gRPC transport (optional, can be disabled in trusted networks)
- Assistant's existing security model (DM policies, allowlists) remains in place at the HTTP layer
- All authentication happens at the WireGuard network level, not at the application level
- Memory operations respect existing OpenClaw memory access controls
- D-Bus provides additional zero-trust via policy files (no additional auth needed)

## 14. s6 Deployment (Artix Linux)

### 14.1. Service Layout

Artix uses the `producer-for` / `consumer-for` pipeline pattern for logging, not the `log/` subdirectory approach.

```
/etc/s6/sv/
├── op-assistant-grpc-srv/          # Main longrun service
│   ├── type                         # "longrun"
│   ├── run                          # Shell entry point
│   ├── producer-for                 # "op-assistant-grpc-log"
│   ├── notification-fd              # "3"
│   └── dependencies.d/
│       ├── dbus-session
│       └── op-cognitive-mcp
└── op-assistant-grpc-log/           # Log companion longrun
    ├── type                         # "longrun"
    ├── run                          # execlineb s6-log script
    ├── consumer-for                 # "op-assistant-grpc-srv"
    ├── pipeline-name                # "op-assistant-grpc"
    └── notification-fd              # "3"

/etc/s6/config/
└── op-assistant-grpc.conf           # DIRECTIVES="n5 s2000000 T"
```

### 14.2. Deploy Script

`deploy/op-assistant-grpc-deploy.sh`:

- Builds the release binary
- Installs to `/usr/local/bin/op-assistant-grpc`
- Copies s6 source definitions into `/etc/s6/sv/`
- Copies log config into `/etc/s6/config/`
- Runs `recompile-and-update.sh` to compile and atomically activate the new s6-rc database

### 14.3. Service Activation

```bash
# Deploy
sudo ./deploy/op-assistant-grpc-deploy.sh

# Start
sudo s6-rc -u change op-assistant-grpc-srv

# View logs
sudo cat /var/log/op-assistant-grpc/current

# Recompile after source changes
sudo ./deploy/s6/recompile-and-update.sh
```
