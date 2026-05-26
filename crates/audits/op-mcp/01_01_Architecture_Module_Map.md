# Unified MCP Protocol Server Audit Report

## Architecture & Module Map

### Overview
The `op-mcp` crate acts as a unified model-interaction boundary offering Model Context Protocol (MCP) servers and tools. It integrates local command capabilities, active cognitive agents, and dynamic tooling systems with multiple transport layers, supporting standard streams (Stdio), REST endpoints with Server-Sent Events (SSE), WebSockets, and gRPC endpoints.

### Module Tree
*   `agents_server`: Implements the cognitive agent manager and discovery mechanism.
*   `builtin_trait_agents`: Built-in Rust trait fallbacks for agent tasks when D-Bus is unavailable.
*   `compact`: Meta-tool layer wrapping multi-tool setups into 4 discrete endpoints.
*   `config`: Settings and tool discovery switches.
*   `external_client`: Proxies external MCP server actions and tracks active tooling.
*   `grpc`: Complete gRPC transport definitions.
    *   `client`: Native client wrapping Tonic channels.
    *   `server`: Main gRPC server listener with BTRFS cache and state integrations.
    *   `service`: Maps Protobuf request models to interior MCP handlers.
    *   `generated`: Compiled proto definitions.
*   `protocol`: Structures standard JSON-RPC 2.0 frames.
*   `request_context`: Request-level tool lifecycles (loaded on request, deleted at termination).
*   `request_handler`: High-level entry router for processing request contexts.
*   `resources`: Built-in markdown documentation and architecture prompts.
*   `router`: REST routes for axum-based embedding.
*   `server`: Agnostic core MCP dispatcher with category/blocked pattern support.
*   `sse`: Event stream broker.
*   `tool_adapter`: Tool listing filters and blocklists.
*   `tool_adapter_orchestrated`: Active orchestration adapter embedding skills and workstacks.
*   `tool_registry`: Simplified registry hosting static tooling.
*   `trait_agent_executor`: Direct memory-space cognitive agent execution.
*   `transport`: Transport trait interface.
    *   `http`: Combined HTTP and SSE endpoint server.
    *   `stdio`: Stdin/Stdout terminal stream dispatcher.
    *   `websocket`: Duplex WebSocket socket upgraders.

### Entry Points
*   **Library Entry Point**: `crates/op-mcp/src/lib.rs`
*   **Main Binaries**:
    *   `op-mcp-server` via `crates/op-mcp/src/main.rs` (Unified multiprotocol transport server).
    *   `op-mcp-compact` via `crates/op-mcp/src/compact_main.rs` (Exposes compact meta-tools).
    *   `op-mcp-agents` via `crates/op-mcp/src/agents_main.rs` (Stdio cognitive agent server).

---

## Security & Quality Findings

### [Finding 1] Host Header Spoofing Authentication Bypass
*   **File**: `crates/op-mcp/src/transport/http.rs`
*   **Lines**: 43–52, 73–77
*   **Severity**: Critical
*   **Description**: The HTTP/SSE server implements a WireGuard-derived bearer authentication middleware. To facilitate local usage, the middleware allows bypasses for loopback requests. However, this bypass is determined solely by parsing the client's untrusted HTTP `Host` header.
*   **Impact**: Any remote attacker who can access the public-facing HTTP/SSE port of the MCP server can bypass the entire authentication layer simply by injecting a `Host: 127.0.0.1` or `Host: localhost` header into their request. This grants the attacker instant unauthenticated access to system tooling, custom local plugins, and dangerous administration commands.
*   **Explinement Code**:
    ```rust
    fn is_localhost_host(headers: &HeaderMap) -> bool {
        headers
            .get("host") // <--- Reads untrusted user header
            .and_then(|v| v.to_str().ok())
            .map(|h| {
                let host = h.split(':').next().unwrap_or(h);
                host == "127.0.0.1" || host == "localhost" || host == "::1"
            })
            .unwrap_or(false)
    }

    async fn wireguard_auth_middleware(...) {
        if request.uri().path() == "/health" || is_localhost_host(&headers) { // <--- Bypass triggered
            return Ok(next.run(request).await);
        }
        ...
    }
    ```
*   **Remediation**: Re-architect loopback checking to query the actual transport peer address. Use Axum's `ConnectInfo` extractor to obtain the real IP of the connection socket and ensure it resides within `127.0.0.1/8` or `::1/128`.

---

### [Finding 2] Arbitrary File Write & Path Traversal via `WriteFileTool`
*   **File**: `crates/op-mcp/src/tools/filesystem.rs`
*   **Lines**: 73–77
*   **Severity**: Critical
*   **Description**: The `WriteFileTool` attempts to prevent writing to critical system directories by performing a naive prefix validation on the user-controlled `path` parameter. It only checks whether the path starts with `"/etc/"` or `"/boot/"`.
*   **Impact**: An attacker can easily bypass this prefix check and write to arbitrary files. For instance, using relative path traversals (e.g. `"/tmp/../../etc/cron.d/malicious"` or `"/usr/lib/systemd/system/malicious.service"`), they can overwrite or create sensitive files. This directly enables Remote Code Execution (RCE) with the privileges of the running daemon.
*   **Explinement Code**:
    ```rust
    // Naive blocklist prefix checks are trivial to bypass with path traversal patterns:
    if path.starts_with("/etc/") || path.starts_with("/boot/") {
        return Ok(json!({"success": false, "error": "Access denied"}));
    }
    ```
*   **Remediation**: Resolve the input path to its absolute canonical path on the filesystem using `std::fs::canonicalize` (or check it safely prior to creation) and verify that the target path is strictly confined to a sandbox or predefined base directory.

---

### [Finding 3] Chunking Offset Subtraction Underflow Panic (Remote DoS)
*   **Files**: 
    *   `crates/op-mcp/src/tool_adapter.rs` (Lines 330–343)
    *   `crates/op-mcp/src/tool_adapter_orchestrated.rs` (Lines 119–123)
*   **Severity**: High
*   **Description**: Both the standard `ToolAdapter` and the `OrchestratedToolAdapter` implement a pagination check when slicing available tools. If an `offset` is specified without a corresponding `limit` constraint, the system attempts to calculate the slice limit dynamically as `end - offset`. However, if the provided `offset` exceeds the length of the total allowed tools array (`allowed_tools.len()`), the subtraction `end - offset` underflows.
*   **Impact**: An unauthenticated or minimally privileged client invoking the tool listing endpoint with a large `offset` parameter (e.g. `offset = 9999`) will trigger a thread panic due to unsigned integer subtraction underflow. This results in an immediate Denial of Service (DoS) for that worker thread or crashes the entire asynchronous runtime depending on target thread handling.
*   **Explinement Code** (`crates/op-mcp/src/tool_adapter.rs`):
    ```rust
    if offset > 0 || limit.is_some() {
        // If limit is None, end defaults to allowed_tools.len()
        let end = limit.map(|l| offset + l).unwrap_or(allowed_tools.len());
        let before_count = allowed_tools.len();
        all_tools = allowed_tools
            .into_iter()
            .skip(offset)
            .take(end - offset) // <--- Will panic if offset > allowed_tools.len() due to subtraction underflow
            .collect();
    ```
*   **Remediation**: Implement a protective guard validating that the `offset` is less than or equal to the size of the retrieved tools list prior to execution, or use `saturating_sub` to safely handle bounds boundaries. For example:
    ```rust
    let end = limit.map(|l| offset.saturating_add(l)).unwrap_or(allowed_tools.len());
    let take_len = end.saturating_sub(offset);
    ```

---

### [Finding 4] Missing Blocklist Enforcement in Stdio/Compact Request Handler
*   **File**: `crates/op-mcp/src/request_handler.rs`
*   **Lines**: 263–275, 347–352
*   **Severity**: High
*   **Description**: The unified `McpServerConfig` define a rigorous default blocklist pattern (e.g., blocking `shell_execute` and `write_file`). However, the `RequestHandler` component—which runs the compact STDIO and HTTP endpoints—instantiates its own internal `RequestContext` and manually loads dangerous shell/write tools (including `tools::shell::ShellExecuteTool` and `tools::filesystem::WriteFileTool`) without checking or enforcing any configuration blocklists during execution.
*   **Impact**: Even if administrators configure `McpServerConfig` blocklists, attackers connecting via the Stdio/Compact transport layers can completely bypass these restrictions and execute raw shell actions or arbitrary file writes.
*   **Explinement Code**:
    ```rust
    async fn load_tools(&self, ctx: &mut RequestContext) -> Result<()> {
        ...
        // Naively loads write and shell execution capabilities bypassing administrative config
        ctx.load_tool(Arc::new(tools::filesystem::WriteFileTool));
        ctx.load_tool(Arc::new(tools::shell::ShellExecuteTool::new()));
        ...
    }
    ```
*   **Remediation**: Pass the master server configurations (`McpServerConfig`) down into the `RequestHandler` and ensure that `load_tools` validates tool identifiers against `blocked_patterns` before adding them to the active context structure.

---

### [Finding 5] Undefined Behavior via Unaligned & Unpadded `simd_json` Deserialization
*   **Files**:
    *   `crates/op-mcp/src/agents_main.rs` (Line 456)
    *   `crates/op-mcp/src/external_client.rs` (Lines 419, 506)
    *   `crates/op-mcp/src/agents_server.rs` (Line 365)
*   **Severity**: High
*   **Description**: The codebase frequently calls `unsafe { simd_json::from_str(&mut buffer) }` on strings read directly from Standard Input or external command streams. The `simd-json` deserializer requires strict memory alignment and explicitly mandates that input buffers have a 32-byte padding boundary at the end to allow for SIMD vector vectorizations without reading out-of-bounds memory.
*   **Impact**: Parsing unpadded string slices returned directly from standard stream readers (such as `stdin.lock().lines()`) triggers memory-safety violations, undefined behavior (UB), or core-dumps due to out-of-bounds SIMD vector loads.
*   **Explinement Code**:
    ```rust
    // Reads a generic line from stdin:
    for line in stdin.lock().lines() {
        let mut line = line?;
        // Directly parses utilizing unsafe without validating padding:
        let request: JsonRpcRequest = match unsafe { simd_json::from_str(&mut line) } { ... }
    }
    ```
*   **Remediation**: Use `simd_json::from_slice` on an explicitly padded vector buffer (such as `simd_json::to_padded_bin`), or completely transition to the safe parsing variants or `serde_json` for processing unaligned input streams.

---

### [Finding 6] Ad-hoc Schemes and Unversioned Data Contracts (OSCAL Violation)
*   **Files**:
    *   `crates/op-mcp/src/agents_main.rs` (Lines 29–65)
    *   `crates/op-mcp/src/protocol.rs` (Lines 9–68)
    *   `crates/op-mcp/src/compact.rs` (Lines 316–412)
    *   `crates/op-mcp/src/agents_server.rs` (Lines 245–261)
*   **Severity**: Quality / Compliance
*   **Description**: The system relies on ad-hoc, untyped structures (using raw `simd_json::OwnedValue` parameters) to define vital boundaries, RPC message structures, and tool input schemas. Hand-rolled JSON strings (e.g., `json!({...})`) are used to represent schemas inside Rust source files instead of using compiled and versioned schemas.
*   **Compliance Impact**: This violates the mandatory schema-as-code discipline, which requires all contract interactions and payload formats to compile from structured, versioned Protocol Buffers or conformant OSCAL model definitions. Ad-hoc JSON values reduce structural type safety and make automated validation impossible.
*   **Explinement Code**:
    ```rust
    // Ad-hoc JSON-RPC frames implemented with raw, unvalidated generic structures:
    #[derive(Debug, Deserialize)]
    struct JsonRpcRequest {
        jsonrpc: String,
        id: Option<Value>,
        method: String,
        #[serde(default)]
        params: Value, // Unstructured raw value
    }
    ```
*   **Remediation**: Generate all core schema properties from standard, centralized Protobuf models (located under `grpc/proto/`) or OSCAL profiles. Strongly type parameters and map input structures strictly to these schemas before they pass boundary middlewares.