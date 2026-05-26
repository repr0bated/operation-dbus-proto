# Production Security and Quality Audit: `op-mcp`

---

## 1. Documentation & Quality Audit

### Crate-Level Documentation
* **Status**: **Present**
* Crate-level `//!` documentation is correctly defined in `crates/op-mcp/src/lib.rs:1`. It provides a high-level overview of the unified MCP protocol server modes (Compact, Agents, Full) and the supported transport layers (Stdio, HTTP, SSE, WebSocket, gRPC).

### README.md Presence
* **Status**: **Absent / Unverified**
* No `README.md` is present in the provided source files. To maintain production-grade quality, a root `README.md` must be added to document deployment procedures, configuration options, and transport authentication mechanisms.

### Public Unsafe Functions
* **Status**: **Pass (None Defined)**
* There are no definitions of public `unsafe fn` items within the provided crate files. Unsafe code is limited to callers parsing JSON via `simd_json::from_str` (e.g., `crates/op-mcp/src/transport/stdio.rs:47`).

### Public Items Missing `/// rustdoc` (10 Sampled Items)
The following public structs, enums, functions, or traits are exposed publicly but completely lack rustdoc block comments:

1. **`Settings` (struct)** — `crates/op-mcp/src/config.rs:6`
2. **`ToolConfig` (struct)** — `crates/op-mcp/src/config.rs:13`
3. **`HttpMcpServer` (struct)** — `crates/op-mcp/src/http_server.rs:172`
4. **`McpGrpcService` (struct)** — `crates/op-mcp/src/grpc/service.rs:80`
5. **`ReadFileTool` (struct)** — `crates/op-mcp/src/tools/filesystem.rs:16`
6. **`WriteFileTool` (struct)** — `crates/op-mcp/src/tools/filesystem.rs:49`
7. **`ListDirectoryTool` (struct)** — `crates/op-mcp/src/tools/filesystem.rs:82`
8. **`PluginQueryTool` (struct)** — `crates/op-mcp/src/tools/plugin.rs:21`
9. **`RespondToUserTool` (struct)** — `crates/op-mcp/src/tools/response.rs:15`
10. **`ShellExecuteTool` (struct)** — `crates/op-mcp/src/tools/shell.rs:15`

### Schema-as-Code Discipline Violations
The codebase regularly violates the schema-as-code discipline by expressing critical model/tool data contracts using ad-hoc `simd_json::json!` structures or unstructured JSON schemas rather than versioned Protocol Buffers or OSCAL-based schema models:

* **Ad-hoc Agent schemas**: `crates/op-mcp/src/agents_main.rs:95-354` specifies tool constraints directly as inline JSON maps.
* **Ad-hoc Default operation schemas**: `crates/op-mcp/src/agents_server.rs:192-206` defines unstructured fallback inputs.
* **Ad-hoc Meta-tool schemas**: `crates/op-mcp/src/compact.rs:434-506` and `crates/op-mcp/src/request_handler.rs:239-299` hardcode data contract validations inside procedural code.

---

## 2. Critical Security Vulnerabilities

### [CRITICAL] Authentication Bypass via Attacker-Controlled HTTP Host Header
* **Location**: `crates/op-mcp/src/transport/http.rs:74` (inside `wireguard_auth_middleware`), `crates/op-mcp/src/transport/http.rs:38` (`is_localhost_host`).
* **Vulnerability Type**: Authentication Bypass
* **Description**:
  The HTTP transport authentication middleware attempts to bypass authentication checks for loopback connections using `is_localhost_host(&headers)`:
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
  Because the HTTP `Host` header is supplied entirely by the client, a remote attacker targeting the public network interface can construct an HTTP request setting `Host: localhost`. The middleware will evaluate this as a local loopback request and bypass the token authentication check entirely.
* **Exploit Vector**:
  A remote attacker sends an unauthenticated `POST /mcp` request with the header `Host: localhost` containing a `tools/call` payload. Axum parses the attacker-forged host header, bypassing the WireGuard authentication check and allowing unrestricted tool execution.

---

### [CRITICAL] Authentication Bypass via Unverified Bearer Token Shape
* **Location**: `crates/op-mcp/src/transport/http.rs:47` (`is_wireguard_auth_token`), `crates/op-mcp/src/http_server.rs:163` (`is_wireguard_auth_token`).
* **Vulnerability Type**: Insecure Authentication Design
* **Description**:
  The bearer authentication mechanism verifies that a token is provided, but fails to check its authenticity against a database, a cryptographic signature, or a valid session state:
  ```rust
  fn is_wireguard_pubkey(token: &str) -> bool {
      token.len() == 44
          && token.ends_with('=')
          && token
              .chars()
              .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '='))
  }

  fn is_wireguard_session_id(token: &str) -> bool {
      Uuid::parse_str(token).is_ok()
  }

  fn is_wireguard_auth_token(token: &str) -> bool {
      is_wireguard_pubkey(token) || is_wireguard_session_id(token)
  }
  ```
  If either check passes, the middleware accepts the request as fully authorized. No cryptographic validation of the WireGuard session actually occurs in this layer.
* **Exploit Vector**:
  An attacker can bypass the authentication layer on any remote connection (where the Host bypass is not possible) by sending a random UUID or a static base64 string matching the base64-shape check (e.g. `Authorization: Bearer AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=`).

---

### [CRITICAL] Security Policy Blocklist Bypass in Standalone Compact Mode
* **Location**: `crates/op-mcp/src/compact.rs:163` (inside `meta_execute_tool`), `crates/op-mcp/src/server.rs:20` (config defaults).
* **Vulnerability Type**: Security Controls Bypass
* **Description**:
  To protect system integrity, the server defines `blocked_patterns` (such as `"shell_execute"`, `"write_file"`, and `"systemd_*"`) in `McpServerConfig` (`crates/op-mcp/src/server.rs:34`). These are enforced inside `McpServer::handle_tools_call` (`crates/op-mcp/src/server.rs:457`).
  
  However, when the binary is run in compact stdio mode via the standalone `run_compact_stdio_server()` entry point, it instantiates `CompactServer` which directly invokes `self.executor.execute_tool`:
  ```rust
  async fn meta_execute_tool(&self, id: Option<Value>, args: Value) -> McpResponse {
      ...
      match self.executor.execute_tool(tool_name, arguments).await {
          Ok(result) => { ... }
  ```
  `CompactServer` and `DefaultToolExecutor` completely bypass the `McpServer` blocklist validation logic. As a result, the critical restrictions blocking execution of system modifications are absent, allowing access to all tools.
* **Exploit Vector**:
  An operator or integrated LLM can query the standalone compact server and invoke `"shell_execute"` or `"write_file"` directly, bypassing the blocklist controls completely.

---

### [CRITICAL] Remote Code Execution via Whitelisted Interpreters in `ShellExecuteTool`
* **Location**: `crates/op-mcp/src/tools/shell.rs:20` (`allowed_commands`), `crates/op-mcp/src/tools/shell.rs:43` (`ShellExecuteTool::execute`).
* **Vulnerability Type**: Remote Code Execution (RCE)
* **Description**:
  `ShellExecuteTool` attempts to secure shell access by matching command inputs against a whitelist of `allowed_commands`. However, the whitelist contains powerful languages and package managers:
  ```rust
  "cargo", "rustc", "python", "python3", "pip", "pip3", "node", "npm", "yarn"
  ```
  By passing arbitrary argument arrays (e.g., `["-c", "import os; os.system('...')"]` to `"python3"`), an attacker can execute arbitrary shell scripts on the host, rendering the whitelist restriction ineffective.
* **Exploit Vector**:
  Combining this with either of the authentication bypasses allows a remote, unauthenticated attacker to execute arbitrary OS-level commands as the parent server process user.

---

### [CRITICAL] Path Traversal and Arbitrary File Access in Filesystem Tools
* **Location**: `crates/op-mcp/src/tools/filesystem.rs:32` (`ReadFileTool`), `crates/op-mcp/src/tools/filesystem.rs:65` (`WriteFileTool`).
* **Vulnerability Type**: Path Traversal / Arbitrary File Read and Write
* **Description**:
  The directory security logic checks paths using raw prefix matching:
  ```rust
  // ReadFileTool:
  if path.starts_with("/etc/shadow") || path.starts_with("/etc/sudoers") { ... }

  // WriteFileTool:
  if path.starts_with("/etc/") || path.starts_with("/boot/") { ... }
  ```
  Because the paths are not canonicalized (using `std::fs::canonicalize`), an attacker can easily bypass these checks using standard path traversal sequences (such as `"/tmp/../../etc/shadow"` or `"//etc/shadow"`).
* **Exploit Vector**:
  An attacker can write files to sensitive directories (e.g., `/etc/cron.d/` or `~/.ssh/authorized_keys`) by passing `"/tmp/../../etc/cron.d/exploit"`, gaining complete control over the system.