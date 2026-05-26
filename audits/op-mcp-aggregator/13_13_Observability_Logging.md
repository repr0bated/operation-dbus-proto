# Production Security & Quality Audit: `op-mcp-aggregator`

## 1. Observability Audit

### Macro Usage Counts
A comprehensive scan of the provided codebase shows a total of **50** `tracing` macros and exactly **0** instances of standard `println!` or `eprintln!` macros. This indicates strong alignment with asynchronous logging best practices.

*   `info!`: **23**
*   `debug!`: **20**
*   `warn!`: **5**
*   `error!`: **2**
*   `println!`: **0**

#### Macro Counts by File
*   **`crates/op-mcp-aggregator/src/aggregator.rs`**: 8 `info!`, 2 `error!`, 2 `debug!`, 1 `warn!` (Total: 13)
*   **`crates/op-mcp-aggregator/src/cache.rs`**: 1 `info!`, 4 `debug!` (Total: 5)
*   **`crates/op-mcp-aggregator/src/client.rs`**: 3 `info!`, 1 `error!`, 3 `debug!`, 2 `warn!` (Total: 9)
*   **`crates/op-mcp-aggregator/src/compact.rs`**: 1 `info!`, 5 `debug!` (Total: 6)
*   **`crates/op-mcp-aggregator/src/config.rs`**: 6 `info!` (Total: 6)
*   **`crates/op-mcp-aggregator/src/groups.rs`**: 3 `info!` (Total: 3)
*   **`crates/op-mcp-aggregator/src/profile.rs`**: 1 `info!`, 1 `debug!`, 3 `warn!` (Total: 5)
*   **`crates/op-mcp-aggregator/src/unused/context.rs`**: 1 `info!`, 2 `debug!` (Total: 3)

---

### Swallowed Errors Without Logging
Several locations in the codebase discard operational errors silently or replace them with defaults without generating diagnostic records:

1.  **Silent Parsing Failure of Upstream Tools**:
    *   **Citation**: `crates/op-mcp-aggregator/src/client.rs:293`
    *   **Context**: 
        ```rust
        let tools: Vec<ToolDefinition> = result
            .as_object()
            .and_then(|obj| obj.get("tools"))
            .and_then(|t| simd_json::serde::from_owned_value(t.clone()).ok())
            .unwrap_or_default();
        ```
    *   **Impact**: If the upstream MCP server returns tools that fail JSON deserialization against `ToolDefinition`, `.ok()` converts the error into `None`, and the function silently returns an empty list. No `error!` or `warn!` macro records that tool mapping failed, making it difficult to debug invalid upstream contracts.

2.  **Silent Health Check Failure**:
    *   **Citation**: `crates/op-mcp-aggregator/src/client.rs:388`
    *   **Context**:
        ```rust
        TransportType::Sse => {
            let url = format!("{}/health", transport_root(&self.config.url));
            self.http_client
                .get(&url)
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        }
        ```
    *   **Impact**: If the network connection to the upstream server fails entirely or times out during a health check, the resulting `reqwest::Error` is discarded by `.unwrap_or(false)` without any diagnostic logging.

---

### PII or Secrets Exposure in Log Output
High-risk logging locations have been identified where sensitive credentials, system configurations, or user privacy details are serialized directly to log drains:

1.  **Arbitrary Tool Argument Serialization**:
    *   **Citations**: `crates/op-mcp-aggregator/src/compact.rs:200` & `crates/op-mcp-aggregator/src/aggregator.rs:162`
    *   **Context** (`crates/op-mcp-aggregator/src/compact.rs:200`):
        ```rust
        debug!("execute_tool: {} with args {:?}", tool_name, arguments);
        ```
    *   **Impact**: When a client executes a tool via Compact Mode, the full list of arguments is written to the log under `debug` level. Because MCP tools process highly sensitive parameters—such as credentials, API keys, database connection secrets, system configuration modifications, or private user files—this dumps raw secrets directly into system logs.

2.  **Context-Aware Information Disclosure**:
    *   **Citation**: `crates/op-mcp-aggregator/src/unused/context.rs:242`
    *   **Context**:
        ```rust
        debug!("Updated context: {:?}", self.context);
        ```
    *   **Impact**: The `self.context` struct contains raw fields such as `files`, `keywords`, `recent_commands`, `open_files`, and `dbus_services` analyzed from the conversation history. This results in sensitive file paths, personal keywords, and executed terminal commands being written to debug logs.

---

### Metrics Instrumentation
No native metrics telemetry is instrumented within the `op-mcp-aggregator` crate. 
While `prometheus` is included as a workspace dependency inside the root `Cargo.toml`, the `op-mcp-aggregator` module only tracks internal, in-memory counters in ad-hoc structs (such as `CacheStats` at `crates/op-mcp-aggregator/src/cache.rs:43` and `AggregatorStats` at `crates/op-mcp-aggregator/src/aggregator.rs:567`). 

These counters are only exposed via RPC status responses and are not exported to any Prometheus registry or registered with a telemetry system (such as the `metrics` crate).

---

## 2. Schema-as-Code Violations

The codebase frequently constructs and transmits data contracts using ad-hoc dynamically-typed JSON structures and stringly-typed schemas rather than strictly typed, versioned, and compiled schemas (e.g., Protocol Buffers or OSCAL standard representations).

### Violations Identified
1.  **Ad-Hoc JSON Schema Declarations**:
    *   **Citations**: `crates/op-mcp-aggregator/src/compact.rs:136`, `183`, `246`, `296`, `360`
    *   **Violation**: Each meta-tool within `compact.rs` specifies its interface contract via an inline dynamically constructed JSON object (`simd_json::json!`). For instance:
        ```rust
        fn input_schema(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "category": { ... }
                }
            })
        }
        ```
        This relies on raw string mapping and runtime interpretation, bypassing any compile-time versioning or contract validation.

2.  **Generic JSON-RPC Protocol Structs**:
    *   **Citations**: `crates/op-mcp-aggregator/src/client.rs:25` & `crates/op-mcp-aggregator/src/client.rs:37`
    *   **Violation**: The requests and responses (`McpRequest`, `McpResponse`) rely on `simd_json::OwnedValue` as a catch-all container:
        ```rust
        pub struct McpRequest {
            pub jsonrpc: String,
            pub id: Value,
            pub method: String,
            pub params: Option<Value>,
        }
        ```
        Using untyped JSON payloads allows arbitrary unstructured data to cross transport boundaries without enforceability against a central schema registry.

3.  **Untyped Dynamic Tool Interfaces**:
    *   **Citation**: `crates/op-mcp-aggregator/src/client.rs:69`
    *   **Violation**: The `ToolDefinition` struct represents the core operational contract of the aggregator. However, `input_schema` and `annotations` are modeled using dynamic `Value` structures:
        ```rust
        pub struct ToolDefinition {
            pub name: String,
            pub description: String,
            pub input_schema: Value,
            pub annotations: Option<Value>,
            ...
        }
        ```
        This forces consumers to rely on dynamic schema-validation engines at runtime instead of statically validated contracts.

---

## 3. Security & Quality Findings

### [CRITICAL] Security Profile & IP Access Zone Bypass in Compact Mode
*   **Vulnerability Type**: Security Policy Bypass / Privilege Escalation
*   **Citations**: `crates/op-mcp-aggregator/src/compact.rs:214`, `crates/op-mcp-aggregator/src/compact.rs:445`, `crates/op-mcp-aggregator/src/aggregator.rs:179`

#### Description
The aggregator implements two security enforcement mechanisms:
1.  **Named Profiles** (`crates/op-mcp-aggregator/src/profile.rs`): Selects and filters the maximum allowed subset of tools and restricts tool queries based on designated scopes (e.g., `sysadmin`, `dev`, `minimal`).
2.  **IP-Based Access Control** (`crates/op-mcp-aggregator/src/groups.rs:159`): Restricts enabling sensitive/dangerous tools (such as root shells, user administration, or file modifications) unless the client is classified within a trusted network range or `localhost`.

However, when running in **Compact Mode**, the aggregator exposes meta-tools, specifically `execute_tool` and `batch_execute`. These tools completely bypass both of the security validation pipelines:

```rust
// In crates/op-mcp-aggregator/src/compact.rs:214 (ExecuteToolTool)
async fn execute(&self, input: Value) -> Result<Value> {
    ...
    let result = self.aggregator.call_tool(tool_name, arguments).await?;
    ...
}
```

This delegates execution directly to `aggregator.call_tool(name, arguments)`. 

Looking at `aggregator.rs:179`, we see that `call_tool` contains absolutely no validation checks. It retrieves the server ID from the global cache (which holds all tools from all servers, regardless of profile or network segment) and directly submits the invocation request to the upstream client:

```rust
// In crates/op-mcp-aggregator/src/aggregator.rs:179
pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallResult> {
    self.ensure_initialized().await?;
    debug!("Calling tool: {}", name);

    let server_id = self
        .cache
        .get_server_id(name)
        .await
        .ok_or_else(|| anyhow!("Tool '{}' not found in any server", name))?;

    let client = self
        .clients
        .get_client(&server_id)
        .await
        .ok_or_else(|| anyhow!("Server '{}' not connected", server_id))?;

    let result = client
        .call_tool(name, arguments.clone())
        .await
        ...
}
```

#### Exploitation Vector
An untrusted client connecting over a public IP address (which should be restricted to the `minimal` profile and public read-only tools) can target the `execute_tool` meta-tool. By submitting:

```json
{
  "tool_name": "shell_root",
  "arguments": {
    "command": "rm -rf /"
  }
}
```

The call is routed directly to `call_tool`. Because `call_tool` does not match the request against `call_tool_in_profile` or query `ToolGroups::should_include`, the call bypasses the profile boundary entirely. Consequently, the client executes arbitrary, highly privileged operations on any upstream server, completely undermining the security model.

#### Remediation
Refactor `ExecuteToolTool` and `BatchExecuteTool` to enforce profile-level verification, or force them to route through `call_tool_in_profile` passing the profile name corresponding to the user's active session/connection context:

```rust
// Proposed remediation in ExecuteToolTool:
let result = self.aggregator.call_tool_in_profile(tool_name, arguments, active_profile).await?;
```

---

### [MEDIUM] Command Injection and Resource Leaks in Unimplemented Stdio Transport
*   **Vulnerability Type**: Insecure Architecture / Resource Management
*   **Citations**: `crates/op-mcp-aggregator/src/client.rs:218`, `272` & `crates/op-mcp-aggregator/src/config.rs:280`

#### Description
The config configuration allows operators to register stdio-based upstream servers using a shell string as the command:

```rust
pub fn stdio(id: &str, name: &str, command: &str) -> Self {
    Self {
        id: id.to_string(),
        name: name.to_string(),
        url: command.to_string(),
        transport: TransportType::Stdio,
        ...
    }
}
```

Currently, `McpClient::initialize_stdio` and `McpClient::send_stdio_request` are stubbed out with `warn!` logs or immediate `Err` returns. 

If this command string is executed as a shell command (e.g., via `sh -c` or similar) once stdio initialization is implemented, it creates a high-severity Command Injection vector if the configuration is dynamically registered. 

Furthermore, if background workers start processes without structured supervision, child processes may become orphaned (zombies), leading to resource leaks on the host control plane.

#### Remediation
1. Ensure the final implementation of Stdio transport executes processes by strictly splitting arguments (`std::process::Command::new(command).args(...)`) rather than passing the command directly to a shell interpreter.
2. Ensure that any spawned child process is tied to an asynchronous supervisor that terminates the child process when the `McpClient` drop guard is triggered.