# Production Security and Quality Audit: op-mcp

---

## 1. Build Check

### Cargo.toml Analysis
*   **Edition**: Both the workspace root and the `op-mcp` crate use the `2021` edition.
*   **Rust-version**: Not specified in either the workspace or local `Cargo.toml`. 
*   **Bins**: 
    *   `op-mcp-server` (`src/main.rs`)
    *   `op-mcp-compact` (`src/compact_main.rs`)
    *   `op-mcp-agents` (`src/agents_main.rs`)
*   **Examples**: None defined in `crates/op-mcp/Cargo.toml`.
*   **Build.rs**: No `build.rs` is present in the provided source files for `op-mcp`, although build-dependencies such as `tonic-build` are defined.
*   **Workspace Inheritance vs. Local Overrides**:
    *   `crates/op-mcp/Cargo.toml` overrides and inherits workspace dependencies using `.workspace = true` (e.g., `simd-json`, `reqwest`, `tonic`, `prost`, `tonic-build`).
    *   *Syntax Note*: In `crates/op-mcp/Cargo.toml:25`, the dependency is specified as `reqwest.workspace = true` using dotted key TOML syntax rather than the standard `reqwest = { workspace = true }`. While valid TOML, standardizing syntax improves tool compatibility.

---

## 2. Schema-as-Code Build Check

*   **Codegen Invocation**: No `build.rs` is provided in the repository files, but `tonic-build` is included as an optional build-dependency under `crates/op-mcp/Cargo.toml:39` and activated under the `grpc` feature.
*   **Source of Truth (.proto files)**: No `.proto` source files are checked into the provided repository paths.
*   **Generated Files Committed**: **Yes (Violation)**. The pre-generated Rust protobuf bindings are committed directly inside the source tree at `crates/op-mcp/src/grpc/generated/op.mcp.v1.rs` and included via `include!` in `crates/op-mcp/src/grpc/mod.rs:30`.
*   **Runtime Compilation**: No compilation of `.proto` schemas is performed at runtime.

---

## 3. Schema-as-Code Code Violations

Data contracts are expressed as ad-hoc Rust structs and JSON macro strings instead of versioned Proto or OSCAL schemas across the following files:

*   **Ad-hoc Structs**:
    *   `crates/op-mcp/src/protocol.rs:11-58`: `McpRequest` and `McpResponse` are hand-rolled Serde JSON-RPC wrappers instead of generated schemas.
    *   `crates/op-mcp/src/agents_main.rs:26-55`: `JsonRpcRequest`, `JsonRpcResponse`, and `JsonRpcError` are redefined as ad-hoc Serde structs.
    *   `crates/op-mcp/src/agents_main.rs:81-86`: `ServerInfo`, `ServerCapabilities`, and `ToolsCapability` are defined as ad-hoc structs.
    *   `crates/op-mcp/src/external_client.rs:15-56`: `ExternalMcpConfig`, `AuthMethod`, and `ExternalTool` are defined as ad-hoc configuration and metadata models.
*   **Ad-hoc JSON Strings and Schemas**:
    *   `crates/op-mcp/src/agents_main.rs:102-126`: Tool input schemas are specified using the ad-hoc `simd_json::json!` macro.
    *   `crates/op-mcp/src/compact.rs:418-485`: `compact_tools_schema` exposes the 4 meta-tools using hand-written JSON values.

---

## 4. Security and Quality Findings

### CRITICAL: Authentication Bypass via Host Header Spoofing
*   **Citation**: `crates/op-mcp/src/transport/http.rs:43` and `crates/op-mcp/src/transport/http.rs:59`
*   **Impact**: Authentication bypass allowing unauthorized remote access to all registered model context tools (including terminal execution and filesystem mutation).
*   **Description**: The HTTP server's middleware `wireguard_auth_middleware` allows requests to bypass authorization if they originate from loopback/localhost. However, the determination of "localhost" is performed by checking the client's `Host` header via `is_localhost_host`:
    ```rust
    fn is_localhost_host(headers: &HeaderMap) -> bool {
        headers
            .get("host")
            ...
            .map(|h| {
                let host = h.split(':').next().unwrap_or(h);
                host == "127.0.0.1" || host == "localhost" || host == "::1"
            })
    }
    ```
    An external attacker can send a request to the server's public IP address while setting the `Host` header to `localhost` or `127.0.0.1`. The server will identify the connection as loopback and completely bypass the WireGuard authorization checks, providing the attacker with administrative control over all system tools.
*   **Remediation**: Remove the header-based local check. Determine the client's loopback status solely by inspecting the peer IP address from the transport layer's connection information (e.g., using `axum::extract::ConnectInfo`).

---

### CRITICAL: Remote Code Execution (RCE) via Unrestricted Compact Mode Tools
*   **Citation**: `crates/op-mcp/src/request_handler.rs:197` and `crates/op-mcp/src/request_handler.rs:330`
*   **Impact**: Arbitrary system command execution by any caller with access to the MCP server.
*   **Description**: In `request_handler.rs`, `load_tools` registers `tools::shell::ShellExecuteTool` and `tools::filesystem::WriteFileTool` into the request context. When a tool call is processed in compact mode via `meta_execute_tool`, it directly runs the target tool from the context:
    ```rust
    async fn meta_execute_tool(&self, ctx: &RequestContext, args: Value) -> Result<Value> {
        let tool_name = args.as_object().and_then(|o| o.get("tool_name")).and_then(|v| v.as_str()) ...
        ctx.execute_tool(tool_name, arguments).await
    }
    ```
    Unlike `ToolAdapter` (which actively enforces `BLOCKED_PATTERNS` to restrict `shell_execute` and mutation tools), the `RequestHandler` lacks any validation or blocking layers. Furthermore, the `ShellExecuteTool` whitelist includes highly dangerous binaries such as `python`, `pip`, `cargo`, `npm`, `node`, `docker`, and `kubectl`. A caller can use `execute_tool` to invoke `python` with arbitrary code execution arguments, fully compromising the underlying host.
*   **Remediation**: Enforce strict blocklist validations (similar to `BLOCKED_PATTERNS` in `tool_adapter.rs`) inside `request_handler.rs` or directly within `RequestContext::execute_tool` before invoking any execution or writing utility.

---

### CRITICAL: Path Traversal Bypass in Read/Write File Tools
*   **Citation**: `crates/op-mcp/src/tools/filesystem.rs:34` and `crates/op-mcp/src/tools/filesystem.rs:74`
*   **Impact**: Arbitrary read/write access to restricted system files, bypassing basic security rules.
*   **Description**: The safety boundaries for both `ReadFileTool` and `WriteFileTool` rely entirely on simple prefix matching using the path provided by the caller:
    ```rust
    // ReadFileTool Check
    if path.starts_with("/etc/shadow") || path.starts_with("/etc/sudoers") { ... }

    // WriteFileTool Check
    if path.starts_with("/etc/") || path.starts_with("/boot/") { ... }
    ```
    Because path canonicalization is not performed before validating the prefix, an attacker can bypass these constraints using basic path traversal or normalization bypasses. For example, passing `./../../etc/shadow` or `/etc/./shadow` evades the `starts_with` check, allowing unauthorized actors to read the hash shadow database or write malicious crontab configurations into `/etc/cron.d`.
*   **Remediation**: Canonicalize the target path using `std::fs::canonicalize` to resolve symlinks and traversals prior to verifying safe directory bounds, and sandbox file operations to a designated data root directory.

---

### HIGH: Lock Ordering Inversion causing Server Deadlocks
*   **Citation**: `crates/op-mcp/src/agents_server.rs:120` and `crates/op-mcp/src/agents_server.rs:202`
*   **Impact**: Thread starvation and server-wide denial of service (DoS) under concurrent execution.
*   **Description**: There is a lock ordering inconsistency between the agent discovery sequence and the tool execution path:
    *   In `discover_agents` (`crates/op-mcp/src/agents_server.rs:120-121`), `discovered_agents` is write-locked first, followed by `tools`:
        ```rust
        let mut discovered = self.discovered_agents.write().await;
        let mut tools = self.tools.write().await;
        ```
    *   In `execute_tool` (`crates/op-mcp/src/agents_server.rs:202-208`), the order is inverted; `tools` is read-locked first, and then `discovered_agents` is read-locked:
        ```rust
        let tools = self.tools.read().await;
        ...
        let agents = self.discovered_agents.read().await;
        ```
    If discovery and execution happen concurrently, thread A can hold a write-lock on `discovered_agents` and block waiting for `tools`, while thread B holds a read-lock on `tools` and blocks waiting for `discovered_agents`, resulting in an unrecoverable deadlock.
*   **Remediation**: Unify lock-acquisition patterns across the entire codebase to always lock resources in the exact same order (e.g., `tools` then `discovered_agents`), or use a single unified structure protected by a single lock.

---

### HIGH: Compilation Failure in Qdrant Tool Module
*   **Citation**: `crates/op-mcp/src/tools/qdrant.rs:1` and `crates/op-mcp/src/tools/qdrant.rs:75`
*   **Impact**: The crate completely fails to build when the default tools registration is invoked.
*   **Description**: 
    1.  The module tries to import `ToolResult` on line 1: `use crate::tool_registry::{Tool, ToolResult};`. However, `ToolResult` is not defined anywhere in `tool_registry.rs`.
    2.  The implementation of the `Tool` trait on lines 75-78 is invalid:
        ```rust
        async fn execute(&self, input: &str) -> Result<ToolResult> { ... }
        ```
        The actual `Tool` trait defined in `crates/op-mcp/src/tool_registry.rs:26` requires `async fn execute(&self, input: Value) -> Result<Value>;`. This signature mismatch prevents compilation of any target calling `register_all` (which is hard-coded in the standard startup sequence in `crates/op-mcp/src/tools/mod.rs:54`).
*   **Remediation**: Update `QdrantTool::execute` to accept a `simd_json::OwnedValue` as the input parameter and return `Result<simd_json::OwnedValue>` to match the canonical `Tool` trait specification. Remove the invalid `ToolResult` imports.