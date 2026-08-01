# Production Security & Quality Audit: op-mcp

---

## 1. D-Bus & IPC Attack Surface Registry & Analysis

### 1.1 D-Bus Interface Registry

The `op-mcp` crate primarily acts as a D-Bus client/proxy, interacting with the system bus to manage systemd units, and with both system and session buses to discover and interact with dynamic agent services.

| Interface Name | Object Path | Methods Called / Registered | Caller Identity Checked? | System/Session Bus |
| :--- | :--- | :--- | :--- | :--- |
| `org.dbusmcp.Agent` | `/org/dbusmcp/Agent/{AgentType}` | `name`, `description`, `operations`, `Execute` | **No** (Assumes transport auth) | System/Session (Env dependent) |
| `org.freedesktop.systemd1.Manager` | `/org/freedesktop/systemd1` | `GetUnit`, `ListUnits`, `StartUnit`, `StopUnit`, `RestartUnit`, `EnableUnitFiles`, `DisableUnitFiles`, `Reload` | **No** (Proxied directly) | System Bus |
| `org.freedesktop.systemd1.Unit` | Dynamic path from `GetUnit` | Properties: `ActiveState`, `SubState`, `LoadState`, `Description` | **No** | System Bus |

### 1.2 IPC Mutation & Process-Spawning Points

The following methods execute highly privileged actions (state mutation or process execution) without validating the identity of the underlying caller at the D-Bus invocation site:

*   **`Execute` (`org.dbusmcp.Agent`)**  
    *   *Citation*: `crates/op-mcp/src/agents_server.rs:297`  
    *   *Risk*: Proxies JSON payloads directly into agents that may spawn processes or run cognitive loops on behalf of unauthenticated users.
*   **`StartUnit` / `StopUnit` / `RestartUnit` (`org.freedesktop.systemd1.Manager`)**  
    *   *Citation*: `crates/op-mcp/src/tools/systemd.rs:119`  
    *   *Risk*: Direct control over the lifecycle of system-level services.
*   **`EnableUnitFiles` / `DisableUnitFiles` (`org.freedesktop.systemd1.Manager`)**  
    *   *Citation*: `crates/op-mcp/src/tools/systemd.rs:134`, `148`  
    *   *Risk*: Modifies system persistence, allowing malicious services to be enabled at boot.

### 1.3 Bus Connection Security

*   **System Bus vs Session Bus**: In `crates/op-mcp/src/main.rs:204`, the `AgentsServer` connects to either the `Session` or `System` bus depending on the existence of the `DBUS_AGENT_SESSION` environment variable. Systemd unit tools (`crates/op-mcp/src/tools/systemd.rs:21`) always connect to the highly privileged **System Bus** (`Connection::system()`).
*   **Deserialization Gaps**: String outputs returned from D-Bus methods (such as the output of `Execute` in `crates/op-mcp/src/agents_server.rs:303`) are directly deserialized into structured types using `unsafe { simd_json::from_str }`. No validation is performed on the format, length, or payload size before processing, exposing the system to potential memory corruption or panic states.

---

## 2. Critical Security Vulnerabilities

### Finding 1: Host Header Authentication Bypass in HTTP/SSE Transport [CRITICAL]

*   **Citations**:  
    *   `crates/op-mcp/src/transport/http.rs:39-48` (`is_localhost_host`)  
    *   `crates/op-mcp/src/transport/http.rs:52-70` (`wireguard_auth_middleware`)  

#### Description
The `wireguard_auth_middleware` is designed to enforce secure Bearer authentication for all remote requests entering the HTTP/SSE transport layer. However, the middleware contains an explicit bypass for loopback requests determined by `is_localhost_host(&headers)`:

```rust
// Allow health check and loopback without auth
if request.uri().path() == "/health" || is_localhost_host(&headers) {
    return Ok(next.run(request).await);
}
```

The `is_localhost_host` helper function verifies if the client-supplied `Host` header split by colon equals `"127.0.0.1"`, `"localhost"`, or `"::1"`:

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

#### Impact
This implementation constitutes a severe **Authentication Bypass** via Host Header Injection. An attacker anywhere on the network can send arbitrary HTTP POST requests to `/mcp`, `/tools/call`, or other sensitive routes, set the `Host: localhost` header, and bypass all authentication checks. This grants unauthenticated remote attackers full access to the exposed MCP tools.

#### Remediation
Never rely on client-provided HTTP headers to verify physical loopback origin. Instead, inspect the local peer socket address provided by Axum's `ConnectInfo` extractor:

```rust
// In your Axum middleware, check the actual socket IP address
let peer_ip = request
    .extensions()
    .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
    .map(|info| info.ip());

if let Some(ip) = peer_ip {
    if ip.is_loopback() {
        // Safe bypass
    }
}
```

---

### Finding 2: Global Blocklist Bypass in `RequestHandler` [CRITICAL]

*   **Citations**:  
    *   `crates/op-mcp/src/request_handler.rs:188-232` (`load_tools`)  
    *   `crates/op-mcp/src/request_handler.rs:105-177` (`handle_tools_call` / `handle`)  
    *   `crates/op-mcp/src/request_context.rs:217-231` (`execute_tool`)  

#### Description
The codebase defines a secure global blocklist of mutating and dangerous tools inside the `McpServerConfig::default()` configuration (`crates/op-mcp/src/server.rs:46-54`):

```rust
blocked_patterns: vec![
    "shell_execute".into(),
    "write_file".into(),
    "systemd_start".into(),
    "systemd_stop".into(),
    "systemd_restart".into(),
    "systemd_enable".into(),
    "systemd_disable".into(),
],
```

This blocklist is enforced within the standard `McpServer` implementation in `crates/op-mcp/src/server.rs:218`. 

However, when `op-mcp-server` runs in the standard HTTP/SSE/Compact mode, it routes requests through the `RequestHandler` struct. `RequestHandler::load_tools` preloads every dangerous tool directly into the per-request context:

```rust
// Shell tools
ctx.load_tool(Arc::new(tools::shell::ShellExecuteTool::new()));
```

During tool execution in `RequestHandler::handle_tools_call`, the handler processes the execution request via:

```rust
let result = match tool_name {
    "execute_tool" => self.meta_execute_tool(&ctx, arguments).await,
    ...
```

This calls `RequestContext::execute_tool`, which retrieves the tool from the inner map and runs it without checking any blocklist:

```rust
pub async fn execute_tool(&self, name: &str, input: Value) -> Result<Value> {
    // Check turn limit
    self.increment_turn()?;
    ...
    let tool = self.tools.get(name)
        .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", name))?;
    
    tool.execute(input).await
}
```

#### Impact
An attacker exploiting the Host Header bypass can directly invoke `shell_execute` and execute arbitrary binary commands, write arbitrary code via `write_file` to compromise persistence vectors (e.g., `.ssh/authorized_keys`), or mutate services via `systemd_start_unit` because **the global blocklist is completely ignored** in the HTTP/SSE processing pipeline.

#### Remediation
Unify configuration contexts. Ensure that `RequestHandler` reads from `McpServerConfig` and applies the `blocked_patterns` check on both tool registration (`load_tools`) and tool invocation:

```rust
if self.config.blocked_patterns.iter().any(|pattern| name.contains(pattern)) {
    return Err(anyhow::anyhow!("Tool '{}' is blocked", name));
}
```

---

### Finding 3: Dead Code / Unenforced Administrative Session Authorization [HIGH]

*   **Citations**:  
    *   `crates/op-mcp/src/compact.rs:56-58` (`can_execute_controller_tools`)  
    *   `crates/op-mcp/src/request_context.rs:108-110` (`can_access_controller_tools`)  

#### Description
The codebase includes fields and helper methods to distinguish between regular sessions and administrator-controlled sessions (e.g., `is_controller`). For instance, `can_access_controller_tools` is defined as:

```rust
/// Check if caller can access controller-only tools
pub fn can_access_controller_tools(&self) -> bool {
    self.is_controller
}
```

However, a static analysis of `request_context.rs`, `request_handler.rs`, `compact.rs`, and `server.rs` reveals that neither `can_execute_controller_tools` nor `can_access_controller_tools` is ever called or enforced before executing any mutation tools. 

#### Impact
Even if a caller is restricted to a non-controller session, they can execute any tool available in the system. The logical framework for Role-Based Access Control (RBAC) exists as dead code, creating a false sense of security while offering zero actual protection.

#### Remediation
Implement active gating in both `McpServer::handle_tools_call` and `RequestContext::execute_tool`. Map sensitive tools to a `SecurityLevel` or role, and reject execution if the session's `is_controller` flag is `false`:

```rust
if tool.is_controller_only() && !self.can_access_controller_tools() {
    return Err(anyhow::anyhow!("Unauthorized: Controller session required"));
}
```

---

## 3. Schema-as-Code & Protocol Compliance

### Finding 4: Ad-hoc JSON Contracts and Inline Schema Violations [MEDIUM]

The workspace claims a schema-as-code discipline using Protocol Buffers and OSCAL. However, several modules bypass versioned structures in favor of ad-hoc JSON literals:

*   **Ad-hoc D-Bus Payload Construction**  
    *   *Citation*: `crates/op-mcp/src/agents_server.rs:271-279`  
    *   *Violation*: Task parameters are packed into an untyped inline JSON object, serialized to a string, and sent over D-Bus as a raw string `Execute` parameter rather than utilising a versioned Protobuf struct or schema model.
*   **Inline Tool Schemas**  
    *   *Citation*: `crates/op-mcp/src/tools/plugin.rs:52`  
    *   *Violation*: Exposes ad-hoc inline schemas (`"desired_state": {"type": "object"}`) representing state changes rather than calling a versioned serialization library.
*   **Ad-hoc Agent Tools definitions**  
    *   *Citation*: `crates/op-mcp/src/agents_main.rs:94-282`  
    *   *Violation*: Hand-crafts JSON schema models manually via `json!({ ... })` for 10+ agents instead of deriving these schemas from a single source-of-truth declarative schema definition.

#### Remediation
Transition all tool definition structures to utilize the compiled `ToolSchema` and `ToolParameter` messages generated from the Protocol Buffer spec found in `crates/op-mcp/src/grpc/generated/op.mcp.v1.rs`.

---

## 4. Additional Quality & Security Gaps

### Finding 5: Undefined Behavior / Out-of-Bounds Read Risk via Unsafe `simd_json::from_str` on Unpadded Strings [HIGH]

*   **Citations**:  
    *   `crates/op-mcp/src/transport/stdio.rs:50`  
    *   `crates/op-mcp/src/transport/websocket.rs:121`  
    *   `crates/op-mcp/src/agents_main.rs:564`  

#### Description
Throughout the transports and entry points, incoming string messages are parsed using `unsafe { simd_json::from_str(...) }`:

```rust
let mut line_mut = line.to_string();
let response = match unsafe { simd_json::from_str::<McpRequest>(&mut line_mut) } {
```

`simd-json` relies on the parsed buffer being padded with a minimum of `SIMDJSON_PADDING` (typically 32 bytes) of extra addressable memory. This padding allows the vectorization algorithms to load chunks of data (e.g., AVX2 or SSE) without reading past allocated boundaries. Calling `unsafe simd_json::from_str` directly on a standard `String` or slice generated from a stream buffer without ensuring that the buffer has the required padding is unsafe and leads to undefined behavior or out-of-bounds memory access.

#### Remediation
Either:
1.  Use the safe `simd_json::serde::from_slice` API after ensuring the allocation contains the required trailing padding.
2.  Use standard safe JSON parsers for standard stream lines where padding cannot be guaranteed.