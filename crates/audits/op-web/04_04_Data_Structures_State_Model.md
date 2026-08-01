# Production Security and Quality Audit: op-web

## 1. Data Structure Statistics

### Concurrency and State Control Primitive Counts

| File Path | Arc | Rc | RefCell | RwLock | Mutex | OnceCell | .clone() |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `crates/op-web/src/email.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 2 |
| `crates/op-web/src/groups_admin.rs` | 0 | 0 | 0 | 2 | 0 | 0 | 5 |
| `crates/op-web/src/lib.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/main.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 2 |
| `crates/op-web/src/mcp.rs` | 4 | 0 | 0 | 0 | 0 | 0 | 2 |
| `crates/op-web/src/mcp_agents.rs` | 6 | 0 | 0 | 2 | 0 | 0 | **27** |
| `crates/op-web/src/mcp_compact.rs` | 5 | 0 | 0 | 0 | 0 | 0 | 12 |
| `crates/op-web/src/mcp_discovery.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/mcp_smart_router.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/privacy_container.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 8 |
| `crates/op-web/src/privacy_openflow.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 8 |
| `crates/op-web/src/privacy_routes.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 8 |
| `crates/op-web/src/router.rs` | 2 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/server.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| `crates/op-web/src/sse.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/state.rs` | 17 | 0 | 0 | 4 | 0 | 0 | 14 |
| `crates/op-web/src/state_manager_client.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| `crates/op-web/src/system_prompt_loader.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/websocket.rs` | 2 | 0 | 0 | 0 | 0 | 0 | 7 |
| `crates/op-web/src/embedded_ui.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/privacy_network.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/users.rs` | 2 | 0 | 0 | 5 | 0 | 0 | 18 |
| `crates/op-web/src/wireguard.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/bin/op-dbus.rs` | 4 | 0 | 0 | 1 | 0 | 0 | 0 |
| `crates/op-web/src/handlers/agents.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/handlers/auth_bridge.rs` | 1 | 0 | 0 | 1 | 0 | 0 | 2 |
| `crates/op-web/src/handlers/chat.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 10 |
| `crates/op-web/src/handlers/dashboard.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/handlers/health.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/handlers/llm.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 2 |
| `crates/op-web/src/handlers/logs.rs` | 2 | 0 | 0 | 0 | 0 | 0 | 1 |
| `crates/op-web/src/handlers/mcp.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/handlers/status.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/handlers/tools.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/handlers/users.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/handlers/vpn.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/handlers/websocket.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 5 |
| `crates/op-web/src/handlers/mail.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/handlers/mod.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/handlers/openclaw.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 2 |
| `crates/op-web/src/handlers/privacy.rs` | 1 | 0 | 0 | 0 | 1 | 0 | 12 |
| `crates/op-web/src/middleware/mod.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/middleware/security.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/orchestrator/anti_hallucination.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/orchestrator/execution.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| `crates/op-web/src/orchestrator/formatting.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| `crates/op-web/src/orchestrator/mod.rs` | 2 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/orchestrator/parsing.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 2 |
| `crates/op-web/src/orchestrator/process.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 14 |
| `crates/op-web/src/orchestrator/tools.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| `crates/op-web/src/orchestrator/types.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/routes/admin.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/routes/chat.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| `crates/op-web/src/routes/llm.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-web/src/routes/mod.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 0 |

---

### Data Structure Alerts

#### 1. High `.clone()` Call Volumes (> 20)
*   **`crates/op-web/src/mcp_agents.rs`**: **27 clone/cloned operations**. 
    *   *Code Quality Note:* This high frequency of cloning is largely driven by serialization boilerplate, extracting values from JSON parameters, and creating runtime configurations for cognitive agents. Many of these clones could be optimized away by passing references or storing state using lifetime specifiers instead of fully owned structures.

#### 2. Large Structs (> 5 Public Fields)
*   **`EmailConfig`** (`crates/op-web/src/email.rs:13`): Contains 7 public fields.
*   **`ManagedAgentInfo`** (`crates/op-web/src/mcp_agents.rs:65`): Contains 7 public fields.
*   **`OpenFlowConfig`** (`crates/op-web/src/privacy_openflow.rs:12`): Contains 6 public fields.
*   **`FlowEntry`** (`crates/op-web/src/privacy_openflow.rs:29`): Contains 7 public fields.
*   **`PrivacyRoute`** (`crates/op-web/src/privacy_routes.rs:14`): Contains 13 public fields.
*   **`AppState`** (`crates/op-web/src/state.rs:60`): Contains 17 fields, all public.
*   **`PrivacyNetworkHostConfig`** (`crates/op-web/src/privacy_network.rs:24`): Contains 8 public fields.
*   **`PrivacyUser`** (`crates/op-web/src/users.rs:16`): Contains 15 fields, all public.
*   **`PendingAuth`** (`crates/op-web/src/handlers/auth_bridge.rs:25`): Contains 8 public fields.
*   **`ChatResponse`** (`crates/op-web/src/handlers/chat.rs:40`): Contains 7 public fields.
*   **`DashboardMetrics`** (`crates/op-web/src/handlers/dashboard.rs:13`): Contains 7 public fields.
*   **`McpServer`** (`crates/op-web/src/handlers/mcp.rs:13`): Contains 7 public fields.
*   **`Agent`** (`crates/op-web/src/handlers/mcp.rs:24`): Contains 7 public fields.
*   **`SystemInfo`** (`crates/op-web/src/handlers/status.rs:43`): Contains 9 public fields.
*   **`StatusResponse`** (`crates/op-web/src/handlers/status.rs:32`): Contains 6 public fields.
*   **`UserResponse`** (`crates/op-web/src/handlers/users.rs:11`): Contains 9 public fields.
*   **`VpnConnection`** (`crates/op-web/src/handlers/vpn.rs:30`): Contains 8 public fields.
*   **`MailQueueItem`** (`crates/op-web/src/handlers/mail.rs:19`): Contains 7 public fields.
*   **`OpenClawStatusResponse`** (`crates/op-web/src/handlers/openclaw.rs:48`): Contains 6 public fields.
*   **`OpenClawConfigResponse`** (`crates/op-web/src/handlers/openclaw.rs:59`): Contains 5 public fields.
*   **`SystemPromptResponse`** (`crates/op-web/src/routes/admin.rs:31`): Contains 8 public fields.
*   **`AdminConfigResponse`** (`crates/op-web/src/routes/admin.rs:72`): Contains 7 public fields.

#### 3. Globally Mutable and Static State
*   **`GROUPS_CONFIG`** (`crates/op-web/src/groups_admin.rs:94`): Defined inside a `lazy_static!` block. Relies on internal `RwLock` structures to manage mutable profile and trusted network state.
*   **`GLOBAL_BROADCASTER`** (`crates/op-web/src/mcp.rs:40`): Global SSE broadcaster instantiated inside a `lazy_static!` block.
*   **`GLOBAL_AGENTS_STATE`** (`crates/op-web/src/mcp_agents.rs:374`): A static `Arc<AgentsMcpState>` wrapping a `RwLock<CriticalAgentsState>`, initialized inside a `lazy_static!` block to facilitate stateless Axum handlers.
*   **`LAST_SIGNUP`** (`crates/op-web/src/handlers/privacy.rs:55`): A static thread-safe `tokio::sync::Mutex` managing an option-wrapped `HashMap` of last signup timestamps for rate limiting.

---

## 2. Critical Security Vulnerabilities

### [CRITICAL] Authentication and Authorization Bypass in `ip_security_middleware`
*   **Location**: `crates/op-web/src/middleware/security.rs:115-144` and `crates/op-web/src/routes/mod.rs:247-252`
*   **Exploitability**: Directly Exploitable

#### Vulnerability Analysis
The application registers a global security middleware `ip_security_middleware` in the routing stack:
```rust
router
    .layer(Extension(state))
    .layer(axum::middleware::from_fn(security::ip_security_middleware))
```
This middleware is intended to establish IP-based security zones. However, the implementation of `ip_security_middleware` ONLY parses the incoming IP address, resolves the `AccessZone`, and inserts this zone into the Axum request extensions:
```rust
    // Attach AccessZone to the request extensions
    request.extensions_mut().insert(zone);

    next.run(request).await
```
The middleware **never aborts or rejects unauthorized requests**. It completely delegates enforcement to downstream handlers. 

Crucially, none of the primary mutating endpoints—such as the direct tool execution routes (`/api/tool` and `/api/tools/:name/execute` mapped to `execute_tool_handler` and `execute_named_tool_handler`)—retrieve or validate this extension before executing commands. Since "tools run as root already" (as noted in `anti_hallucination.rs:43`), any external attacker on the public internet can call `/api/tool` or `/api/tools/shell_exec/execute` to execute arbitrary shell commands or alter D-Bus configurations without authentication.

#### Remediation
Enforce authorization boundaries directly within `ip_security_middleware` or apply route-specific guards. If an endpoint requires `TrustedMesh` or `Localhost` zones, reject the request with `StatusCode::FORBIDDEN` in the middleware layer before calling `next.run(request).await`.

---

### [CRITICAL] Unsanitized Path Traversal in `save_transcript_handler` (Arbitrary File Write / RCE)
*   **Location**: `crates/op-web/src/handlers/chat.rs:247` and `crates/op-web/src/handlers/chat.rs:364-365`
*   **Exploitability**: Directly Exploitable

#### Vulnerability Analysis
The `save_transcript_handler` endpoint parses a user-controllable JSON body. It extracts a `filename` parameter directly from this payload without any verification:
```rust
    let filename = params
        .get("filename")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("chat-transcript-{}.txt", chrono::Utc::now().timestamp()));
```
This filename is then combined directly to form a path under `/tmp`:
```rust
async fn save_transcript_to_file(
    history: &[op_llm::ChatMessage],
    filename: &str,
    session_id: Option<&str>,
) -> Json<Value> {
    ...
    // Save to file
    let filepath = format!("/tmp/{}", filename);
    match tokio::fs::write(&filepath, &transcript).await { ... }
```
An attacker can pass a filename with path traversal sequences, such as `../../../../etc/cron.d/malicious_job`. 

Because the `messages` array or session history included in the same JSON request is also fully controlled by the attacker, the written file's contents are entirely user-controlled. An attacker can write arbitrary system files (such as cron jobs, ssh authorized keys, or configuration overrides), leading directly to Remote Code Execution (RCE) with the privileges of the web service.

#### Remediation
Sanitize the `filename` parameter by rejecting any input containing path separators (`/`, `\`) or traversal indicators (`..`). Use `std::path::Path::file_name` to extract only the base name of the file before prefixing it with the target directory path.

---

## 3. Schema-as-Code Violations

The codebase does not adhere to a unified Schema-as-Code discipline. Rather than using versioned schemas, compiled Protocol Buffers, or OSCAL declarations for network boundary contracts, data structures are expressed as ad-hoc JSON values (`simd_json::OwnedValue`) and transient serde-deserialized structs:

### 1. Ad-Hoc Tool Group Profiles
*   **Location**: `crates/op-web/src/groups_admin.rs:206-210`
*   **Violation**: `SaveProfileRequest` defines an ad-hoc JSON contract:
    ```rust
    struct SaveProfileRequest {
        groups: Vec<String>,
        preset: Option<String>,
    }
    ```
    This layout is consumed via transient JSON serialization without reference to a versioned schema.

### 2. Loose MCP Protocol Payloads
*   **Location**: `crates/op-web/src/mcp.rs:43-70`
*   **Violation**: Structures such as `McpRequest`, `McpResponse`, and `McpError` utilize un-typed `OwnedValue` fields for JSON-RPC parameters, bypassing strict schema validation.

### 3. Duplicated JSON-RPC Definitions for Cognitive Agents
*   **Location**: `crates/op-web/src/mcp_agents.rs:35-59` and `crates/op-web/src/mcp_compact.rs:32-55`
*   **Violation**: `JsonRpcRequest`, `JsonRpcResponse`, and `JsonRpcError` are redefined across multiple files as local structs with ad-hoc untyped `Value` inputs. They should be defined once in a versioned schema crate and shared.

### 4. Untyped Tool Arguments & Direct Execution Payload
*   **Location**: `crates/op-web/src/handlers/tools.rs:94-106`
*   **Violation**: `DirectToolRequest` exposes an untyped `Value` field for tool execution arguments:
    ```rust
    pub struct DirectToolRequest {
        pub tool_name: String,
        #[serde(default)]
        pub arguments: Value,
    }
    ```
    Arguments are processed blindly at the network boundary, relying on runtime validation rather than contract validation.

### 5. Weak API Credential Structs
*   **Location**: `crates/op-web/src/users.rs:45-53`
*   **Violation**: The credentials layout `UserApiCredentials` maps provider keys directly to optional strings without structured, versioned schema definitions. This increases the risk of data drift during updates.