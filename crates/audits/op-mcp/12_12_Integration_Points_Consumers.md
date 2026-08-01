# Production Quality and Security Audit: `op-mcp`

---

## 1. Integration & Architecture Overview

### Crates Depending on `op-mcp`
Based on the workspace configuration and lock file, the following crates explicitly depend on `op-mcp`:
*   **`op-dbus`** (Workspace root crate, declared in `Cargo.toml` dependencies via `op-mcp.workspace = true`)
*   **`op-web`** (Workspace member, declared in `Cargo.lock` with dependency on `op-mcp`)
*   **`op-chat`** (Workspace member, declared in `Cargo.lock` with dependency on `op-mcp`)
*   **`op-cognitive-mcp`** (Workspace member, declared in `Cargo.lock` with dependency on `op-mcp`)
*   **`op-mcp-proxy`** (Workspace member, declared in `Cargo.lock` with dependency on `op-mcp`)

### Registered D-Bus Service Names & Object Paths
The MCP server interacts with and registers the following D-Bus interfaces, services, and object paths:
*   **Agent Discovery Services** (`crates/op-mcp/src/agents_server.rs:120-121`):
    *   Service Filter: `org.dbusmcp.Agent.*`
    *   Interface: `org.dbusmcp.Agent` (line 195, 273)
    *   Object Path: `/org/dbusmcp/Agent/{AgentTypePascalCase}` (dynamically generated in line 191)
*   **Systemd Management D-Bus** (`crates/op-mcp/src/tools/systemd.rs:24-27`):
    *   Service Name: `org.freedesktop.systemd1`
    *   Object Path: `/org/freedesktop/systemd1`
    *   Interface: `org.freedesktop.systemd1.Manager`
    *   Unit Interface Path: `/org/freedesktop/systemd1/unit/*` via interface `org.freedesktop.systemd1.Unit` (line 46)

### Exposed HTTP & gRPC Endpoints

#### HTTP / SSE Endpoints
*   **Axum MCP Router** (`crates/op-mcp/src/router.rs:53-61`):
    *   `POST /api/mcp/` — JSON-RPC 2.0 handler
    *   `GET /api/mcp/health` — JSON-based health checks
    *   `GET /api/mcp/sse` — Server-Sent Events subscription stream
    *   `GET /api/mcp/tools` — Retrieve available tools
    *   `POST /api/mcp/tools/:name` — Call specific tool directly
    *   `POST /api/mcp/initialize` — Lifecycle initialization
*   **Standalone SSE Server** (`crates/op-mcp/src/sse.rs:44-47`):
    *   `GET /sse` — SSE listener
    *   `POST /message` — Incoming JSON-RPC requests
    *   `GET /health` — Simple health check
*   **HTTP Proxy Endpoints** (`crates/op-mcp/src/http_server.rs:173-181`):
    *   `GET /` — Unified SSE connection
    *   `POST /` — Route standard JSON-RPC payloads
    *   `GET /health` — Server health status
    *   `POST /mcp` — Generic endpoint for routing requests
    *   `POST /initialize` — Force initialization protocol
    *   `POST /tools/list` — Explicit retrieval of registered tools
    *   `POST /tools/call` — Explicit tool invocation execution
    *   `GET /sse` — Alternative entry for SSE event loops

#### gRPC Endpoints
Declared in `crates/op-mcp/src/grpc/generated/op.mcp.v1.rs` and implemented in `crates/op-mcp/src/grpc/service.rs`:
*   `op.mcp.v1.McpService/Call` (Unary: `McpRequest` -> `McpResponse`)
*   `op.mcp.v1.McpService/Subscribe` (Server Streaming: `SubscribeRequest` -> stream `McpEvent`)
*   `op.mcp.v1.McpService/Stream` (Bidirectional Streaming: stream `McpRequest` -> stream `McpResponse`)
*   `op.mcp.v1.McpService/Health` (Unary: `()` -> `HealthResponse`)
*   `op.mcp.v1.McpService/Initialize` (Unary: `InitializeRequest` -> `InitializeResponse`)
*   `op.mcp.v1.McpService/ListTools` (Unary: `ListToolsRequest` -> `ListToolsResponse`)
*   `op.mcp.v1.McpService/GetToolSchema` (Unary: `GetToolSchemaRequest` -> `GetToolSchemaResponse`)
*   `op.mcp.v1.McpService/CallTool` (Unary: `CallToolRequest` -> `CallToolResponse`)
*   `op.mcp.v1.McpService/CallToolStreaming` (Server Streaming: `CallToolRequest` -> stream `ToolOutput`)

### Cross-Crate Circular Dependency Risks
A severe circular dependency risk exists between **`op-mcp`** and **`op-chat`**.
*   **Analysis**: In `crates/op-mcp/Cargo.toml`, there is a feature flag `op-chat = []`. Inside `crates/op-mcp/src/resources.rs:104-108`, `op_chat::generate_system_prompt()` is conditionally compiled if the `op-chat` feature is activated.
*   However, `Cargo.lock` shows that `op-chat` depends directly on `op-mcp` to support its own cognitive workflows and tool adaptations.
*   **Impact**: Attempting to resolve compile dependencies when building with the `op-chat` feature enabled will introduce a direct cycle (`op-mcp` -> `op-chat` -> `op-mcp`). Cargo will fail to compile the dependency graph, resulting in an unbuildable workspace state.

---

## 2. Schema-As-Code Violations

The codebase frequently constructs and communicates data structures and API contracts as ad-hoc, in-memory JSON schemas using the `json!` macro rather than compiling them from versioned, single-source Protocol Buffers or OSCAL JSON schemas.

*   **`crates/op-mcp/src/agents_main.rs:92-348`**: Tool inputs and structures for 10 distinct agents (e.g., `agent_sequential_thinking`, `agent_memory`, `agent_code_review`) are entirely handwritten as complex inline JSON values rather than validated against standardized structural schemas.
*   **`crates/op-mcp/src/agents_server.rs:235-251`**: `get_operation_schema` returns an unversioned, ad-hoc JSON structure directly inside Rust code to dictate the schema format for agent operations.
*   **`crates/op-mcp/src/compact.rs:410-482`**: The core compact mode definitions (`list_tools`, `search_tools`, `get_tool_schema`, `execute_tool`) represent critical interface contracts constructed as hardcoded `json!` macros.
*   **`crates/op-mcp/src/request_handler.rs:245-325`**: Dictates the 5 primary tools shown to the LLM via ad-hoc JSON parameters.
*   **`crates/op-mcp/src/tools/filesystem.rs:25-31`**: Handwritten `input_schema` for `read_file` and `write_file` tools.
*   **`crates/op-mcp/src/tools/plugin.rs:44, 68, 92`**: Inline validation schemas for query, diff, and apply mechanisms across 9 system plugins.
*   **`crates/op-mcp/src/tools/response.rs:24, 48, 72`**: Communication contracts (`respond_to_user`, `cannot_perform`) constructed manually via raw JSON.
*   **`crates/op-mcp/src/tools/shell.rs:40-48`**: Critical validation parameters for whitelisted command execution constructed as ad-hoc nested JSON maps.

---

## 3. Detailed Security & Quality Findings

### Finding 1: Critical — Authentication Bypass via Host Header Spoofing
*   **Path**: `crates/op-mcp/src/transport/http.rs:92-101` (implementation of spoof check in `http.rs:40-50`)

#### Explanation
The `wireguard_auth_middleware` functions as the gatekeeper for all HTTP and SSE routes, requiring WireGuard public keys or session IDs as bearer tokens. However, it implements a loopback bypass check to allow local connections to operate without authentication:
```rust
async fn wireguard_auth_middleware(
    headers: HeaderMap,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    // Allow health check and loopback without auth
    if request.uri().path() == "/health" || is_localhost_host(&headers) {
        return Ok(next.run(request).await);
    }
    ...
}
```
The helper function `is_localhost_host` validates loopback status by inspecting the client's HTTP `Host` header:
```rust
fn is_localhost_host(headers: &HeaderMap) -> bool {
    headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .map(|h| {
            let host = h.split(':').next().unwrap_or(h);
            host == "127.0.0.1" || host == "localhost" || host == "::1"
        })
        .unwrap_or(false)
}
```

#### Exploitability
This constitutes a severe, zero-day authentication bypass. Because the HTTP `Host` header is fully controlled by the client, any remote attacker can craft a network packet targeting the system's public IP address with the header set to `Host: localhost`. The middleware will evaluate the spoofed header, bypass all cryptographic WireGuard bearer checks, and permit full administrative execution of the model tools.

#### Remediation
Never trust HTTP metadata to establish network locality. Verify the true peer socket address via connection metadata, or enforce local-only access by binding the Axum listener strictly to `127.0.0.1` or the specific WireGuard interface address rather than utilizing wildcard addresses (`0.0.0.0`).

---

### Finding 2: Critical — Authentication-Bypassing Arbitrary Command Execution via gRPC Direct Tool Invocation
*   **Path**: `crates/op-mcp/src/grpc/service.rs:462-488` (and `service.rs:492-588`)

#### Explanation
The `McpServer` configuration (`crates/op-mcp/src/server.rs:33`) defines a critical blocklist of system mutations and high-risk tools that must never be exposed to clients:
```rust
blocked_patterns: vec![
    "shell_execute".into(),
    "write_file".into(),
    "systemd_start".into(),
    ...
]
```
While the standard JSON-RPC endpoints (`handle_tools_call` in `crates/op-mcp/src/server.rs:274`) enforce this blocklist via `is_tool_blocked`, the structured gRPC handlers for `CallTool` and `CallToolStreaming` in `crates/op-mcp/src/grpc/service.rs` bypass this logic entirely:
```rust
async fn call_tool(
    &self,
    request: Request<CallToolRequest>,
) -> Result<Response<CallToolResponse>, Status> {
    ...
    let registry = self
        .infrastructure
        .tool_registry
        .clone()
        .ok_or_else(|| Status::internal("No tool registry"))?;
    let tool = registry
        .get(&req.tool_name)
        .await
        .ok_or_else(|| Status::not_found("Tool not found"))?;

    let result = tool
        .execute(arguments)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;
    ...
}
```

Because `call_tool` and `call_tool_streaming` resolve tools directly from the low-level `tool_registry` and execute them, they perform no blocklist verification. 

Additionally, the `ShellExecuteTool` whitelists dangerous binaries such as `python`, `python3`, `node`, `npm`, `yarn`, `cargo`, and `kubectl` (`crates/op-mcp/src/tools/shell.rs:23-28`):
```rust
"cargo", "rustc", "python", "python3", "pip", "pip3", "node", "npm"
```
These binaries natively support arbitrary code execution via standard flags (e.g., `python -c "import os; os.system(...)"`).

#### Exploitability
An attacker with gRPC interface access can call `CallTool` or `CallToolStreaming` with `tool_name: "shell_execute"`. Since the blocklist is completely bypassed, the gRPC handler will retrieve `ShellExecuteTool` from the registry and run it. The attacker can specify `command: "python"` and pass system execution commands as arguments, gaining remote shell access to the underlying host.

#### Remediation
Enforce the blocklist validation logic uniformly across all entrypoints. Refactor `call_tool` and `call_tool_streaming` in `McpGrpcService` to pass requests through the `McpServer` validation layer or manually invoke `is_tool_blocked` before tool retrieval.

---

### Finding 3: High — Arbitrary File Read and Write via Weak Prefix Sanitization
*   **Path**: `crates/op-mcp/src/tools/filesystem.rs:35-36` (and `filesystem.rs:66-70`)

#### Explanation
The `ReadFileTool` and `WriteFileTool` filesystem mechanisms attempt to prevent access to restricted paths by verifying string prefixes:
```rust
// ReadFileTool prefix validation
if path.starts_with("/etc/shadow") || path.starts_with("/etc/sudoers") {
    return Ok(json!({"success": false, "error": "Access denied"}));
}
```
```rust
// WriteFileTool prefix validation
if path.starts_with("/etc/") || path.starts_with("/boot/") {
    return Ok(json!({"success": false, "error": "Access denied"}));
}
```

These checks are fundamentally insufficient because they do not resolve canonical paths.

#### Exploitability
An attacker can easily bypass these checks by appending standard directory traversal components (e.g. `/tmp/../etc/shadow` or `/var/lib/../etc/cron.d/malicious`). Because these strings do not start with the forbidden literal prefixes, the traversal sequences pass validation and are executed as raw paths by `tokio::fs::read_to_string` and `tokio::fs::write`, enabling arbitrary files to be read or overwritten.

#### Remediation
Convert all paths to their canonical form using `std::fs::canonicalize` or parent-directory checking before performing safety comparisons:
```rust
let canonical_path = tokio::fs::canonicalize(path).await?;
if canonical_path.starts_with("/etc") {
    return Err(anyhow::anyhow!("Access denied"));
}
```

---

### Finding 4: Medium — Denial of Service via Loop Over Broadcast Event Lag
*   **Path**: `crates/op-mcp/src/grpc/service.rs:414-432`

#### Explanation
Inside `McpGrpcService::subscribe`, real events are forwarded from a shared `broadcast` channel to a per-session `mpsc` queue:
```rust
recv_result = event_rx.recv() => {
    match recv_result {
        Ok(mut event) => {
            ...
            if tx.send(Ok(event)).await.is_err() {
                break; // client disconnected
            }
        }
        Err(broadcast::error::RecvError::Lagged(n)) => {
            warn!("Subscription lagged, dropped {} events", n);
            // Continue receiving, don't kill the stream
        }
        Err(broadcast::error::RecvError::Closed) => {
            break; // sender side dropped
        }
    }
}
```

If a client session is slow to process events, the broadcast receiver will experience a `Lagged` error. In this event, the loop logs the dropped events and immediately resumes receiving. 

#### Exploitability
If a slow client falls significantly behind during high-throughput event spikes, the receiver will repeatedly return `Lagged(n)`. Because the loop does not implement backpressure, sleep states, or drop triggers on high lagged counts, it can spin continuously in a high-CPU state, causing thread exhaustion and a denial-of-service (DoS) condition on the entire MCP server instance.

#### Remediation
Terminate the subscription stream if the lagged count exceeds a reasonable safety threshold, or introduce moderate artificial delay to prevent high-frequency loop spinning when a receiver falls out of sync.

---

### Finding 5: Medium — Unsafe Memory Mutation via unpadded SIMD Operations
*   **Path**: `crates/op-mcp/src/agents_main.rs:649` (and `agents_server.rs:285`)

#### Explanation
The server parses JSON strings using `simd_json::from_str` wrapped in an `unsafe` block:
```rust
let request: JsonRpcRequest = match unsafe { simd_json::from_str(&mut line) } { ... }
```
```rust
let result_value: Value = unsafe { simd_json::from_str(&mut result_mut) }
```

`simd_json` operates by mutating the source string buffer in-place to perform parsing. To safely utilize SIMD instructions without triggering memory access violations, the input slice must contain padding bytes (typically `simd_json::PADDING` size). While standard `String` vectors may occasionally happen to allocate extra capacity, raw strings read directly from standard input streams or returned from D-Bus methods may lack the necessary padding buffer.

#### Exploitability
If an input string lacks the required trailing padding capacity, the SIMD parsing algorithms will perform out-of-bounds reads or writes beyond the string buffer's memory boundary. This can cause the process to crash (segfault) or result in memory corruption during peak execution periods.

#### Remediation
Replace the `unsafe simd_json::from_str` call with the safe, padded equivalent `simd_json::from_slice` using a byte vector with explicit padding, or fall back to standard `serde_json` deserialization for unpadded system streams:
```rust
let mut bytes = line.into_bytes();
let request: JsonRpcRequest = simd_json::from_slice(&mut bytes)?;
```