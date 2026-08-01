# License Audit

## License Field in Cargo.toml
The primary package in the workspace `op-dbus` inherits `license = "Apache-2.0"` from `[workspace.package]` in the root `Cargo.toml`. However, the sub-crate `crates/op-mcp/Cargo.toml` fails to include a `license` field or inherit it via `license.workspace = true`.

## GPL/AGPL/SSPL Crate Scan
A scan of `Cargo.lock` shows no GPL, AGPL, or SSPL licensed crates. The dependency tree includes `cozo` (version `0.7.6`), which is licensed under the weak copyleft **Mozilla Public License 2.0 (MPL-2.0)**. MPL-2.0 is compatible with the workspace's Apache-2.0 license, provided any modifications to Cozo's own files are distributed under the MPL-2.0.

## Crates with No License Field
The `op-mcp` crate (`crates/op-mcp/Cargo.toml`) lacks an explicit `license` field in its package manifest. It should be updated to inherit the workspace license:
```toml
[package]
name = "op-mcp"
version = "0.4.0"
license.workspace = true
```

---

# Production Security & Quality Audit

## Critical Vulnerabilities

### 1. Authentication Bypass via Client-Controlled Host Header Spoofing
*   **File:Line**: `crates/op-mcp/src/transport/http.rs:52`, `crates/op-mcp/src/transport/http.rs:65`
*   **Description**: The authentication middleware `wireguard_auth_middleware` permits any request to completely bypass WireGuard bearer token authentication if `is_localhost_host(&headers)` evaluates to `true`. This function validates the loopback status by inspecting the client-supplied HTTP `Host` header. Because HTTP headers are entirely controlled by the client, any remote network attacker can bypass authentication on the HTTP/SSE transport simply by sending an HTTP request with `Host: localhost` or `Host: 127.0.0.1`.
*   **Remediation**: Never trust client-supplied HTTP headers to make security or routing decisions. Instead, validate the remote socket address of the incoming connection (`axum::extract::ConnectInfo`) to verify that the traffic originated from a local interface.

### 2. Arbitrary File Read and Write via Path Traversal Bypass
*   **File:Line**: `crates/op-mcp/src/tools/filesystem.rs:39`, `crates/op-mcp/src/tools/filesystem.rs:74`
*   **Description**: The `ReadFileTool` and `WriteFileTool` execute superficial prefix checks (`path.starts_with`) to deny access to sensitive paths such as `/etc/shadow` or `/boot/`. However, because the input `path` is not canonicalized, these checks are easily bypassed using relative directories or traversal sequences. An attacker can access `/etc/shadow` by requesting `/etc/shadow/../etc/shadow` or using relative paths.
*   **Remediation**: Canonicalize all input paths using `std::fs::canonicalize` prior to performing any boundary or prefix validations, and ensure the canonicalized path resides within an allowed sandbox directory.

### 3. Complete Blocklist Bypass in Compact Mode Request Handler
*   **File:Line**: `crates/op-mcp/src/request_handler.rs:141`, `crates/op-mcp/src/request_handler.rs:188`
*   **Description**: While `McpServer` and `ToolAdapter` implement blocklists to prevent execution of destructive tools (e.g., `shell_execute`, `write_file`, `systemd_*`), the compact mode `RequestHandler` completely bypasses these protections. The handler explicitly registers `ShellExecuteTool` and `WriteFileTool` in `load_tools` and permits arbitrary tool execution via the `execute_tool` meta-tool without validating the targets against any blocklist. This allows any authenticated client (or unauthenticated network attacker exploiting the Host header bypass) to execute arbitrary shell commands on the host.
*   **Remediation**: Enforce a unified, centralized permission and blocklist check within `RequestContext::execute_tool` and `RequestHandler::meta_execute_tool`.

---

## High Vulnerabilities

### 1. Undefined Behavior / Out-of-Bounds Read via Unpadded `simd_json` Parsing
*   **File:Line**: `crates/op-mcp/src/agents_main.rs:605`, `crates/op-mcp/src/transport/stdio.rs:43`, `crates/op-mcp/src/transport/websocket.rs:136`
*   **Description**: The codebase repeatedly parses incoming text streams by calling `unsafe { simd_json::from_str(&mut line) }` on unpadded standard library `String` and `str` types. The `simd_json` parser relies on SIMD vector instructions which read memory in 16-byte or 32-byte chunks. To prevent out-of-bounds memory access, `simd_json` explicitly requires the input buffer to be padded with `simd_json::SIMDJSON_PADDING` extra bytes. Passing unpadded Rust strings to `unsafe simd_json::from_str` triggers undefined behavior, potentially causing segmentation faults or memory disclosure.
*   **Remediation**: Use `simd_json::from_slice` on a mutable, padded `Vec<u8>` or use the safe `serde_json` crate for parsing strings where padding cannot be guaranteed.

### 2. Privilege Escalation via Interactive Whitelisted Commands in `ShellExecuteTool`
*   **File:Line**: `crates/op-mcp/src/tools/shell.rs:25`
*   **Description**: `ShellExecuteTool` restricts command execution to a whitelist. However, the whitelist contains highly interactive binaries and execution engines including `python`, `python3`, `npm`, `cargo`, `docker`, and `kubectl`. A user with access to this tool can achieve arbitrary remote code execution (RCE) on the host by passing execution flags (e.g., `python -c "..."` or mounting the host filesystem via `docker run -v /:/host`).
*   **Remediation**: Remove execution runtimes, container engines, and package managers from the whitelisted commands, or enforce strict argument validation patterns instead of allowing arbitrary argument arrays.

---

## Medium Vulnerabilities

### 1. Data Contract Schema-as-Code Violations
*   **File:Line**: `crates/op-mcp/src/agents_main.rs:28-66`, `crates/op-mcp/src/agents_server.rs:35-51`, `crates/op-mcp/src/protocol.rs:10-74`, `crates/op-mcp/src/tools/plugin.rs:49`, `crates/op-mcp/src/tools/ovs.rs:47`
*   **Description**: The system deviates from the schema-as-code discipline. Multiple core data contracts (JSON-RPC requests, discovered agents, MCP tool definitions) and tool responses are defined as ad-hoc Rust structs with custom Serde annotations or generated dynamically via unversioned JSON macros (`json!({...})`) rather than using versioned Protocol Buffers or OSCAL schemas.
*   **Remediation**: Define all core data exchanges as versioned Protocol Buffers or OSCAL schemas, compiling them to Rust structures using build-time generators.

### 2. Blind Trust of Spoofable Proxy Headers for Client IP Parsing
*   **File:Line**: `crates/op-mcp/src/http_server.rs:31`
*   **Description**: The `extract_client_ip` function parses `X-Forwarded-For` and `X-Real-IP` headers directly from the HTTP request without ensuring the request originated from a trusted downstream reverse proxy. Any external attacker can inject arbitrary IP addresses into these headers to manipulate logging and audit trails.
*   **Remediation**: Only extract IP information from `X-Forwarded-For` if the immediate peer socket connection is confirmed to belong to a trusted local proxy (e.g., Nginx or Envoy).

---

## Low/Advisory Findings

### 1. Non-Functional Qdrant Search Tool
*   **File:Line**: `crates/op-mcp/src/tools/qdrant.rs:33`
*   **Description**: The `QdrantTool` search implementation sends an empty vector (`vector: vec![]`) inside the payload to the vector database. Because text embeddings are not resolved before execution, the tool is non-functional and will always cause search failures.
*   **Remediation**: Integrate a localized embedding model or call an LLM-aligned embedding service to translate the raw string query into a valid vector prior to querying Qdrant.