### 1. [CRITICAL] Authentication Bypass via Spooofable Host Header in HTTP Transport Middleware
* **Suggestion**: Replace `is_localhost_host` with a socket-level peer address check utilizing Axum's `ConnectInfo`.
* **Rationale**: The `wireguard_auth_middleware` allows complete authentication bypass if `is_localhost_host` evaluates to `true`. However, `is_localhost_host` retrieves and validates only the HTTP `Host` header provided by the client. Any remote attacker can send an HTTP request with `Host: localhost` or `Host: 127.0.0.1` to bypass authentication checks and call system tools. Using `ConnectInfo<SocketAddr>` to verify that the socket peer IP is a loopback address (`127.0.0.1` or `::1`) is the only secure way to establish loopback identity.
* **Example**: `crates/op-mcp/src/transport/http.rs:45`

### 2. [CRITICAL] Arbitrary Code Execution (RCE) via Argument Injection in Shell Whitelist Tool
* **Suggestion**: Deprecate the generic `args` parameter in `ShellExecuteTool` or restrict whitelisted binaries to those without execution flags (e.g., remove `find`, `python`, `node`, `cargo`, `docker`, and `kubectl`).
* **Rationale**: The `ShellExecuteTool` checks if a command is whitelisted but allows the caller to supply arbitrary command-line arguments via the `args` array. Many of the whitelisted commands natively support arbitrary command execution via their parameters. For example, an attacker can invoke `find` with `["/tmp", "-exec", "sh", "-c", "malicious_payload", ";"]` or run `python` with `["-c", "import os; os.system(...)"]`. This allows complete, sandboxed-escaped arbitrary code execution under the permissions of the MCP process.
* **Example**: `crates/op-mcp/src/tools/shell.rs:56`

### 3. [SCHEMA-AS-CODE] Ad-Hoc Data Contracts and Hardcoded JSON Schemas for Tool Definitions
* **Suggestion**: Define all tool input and output schemas as versioned Protobuf messages (using `prost`/`tonic` structures) or compliance-checked OSCAL document models instead of inline, ad-hoc `serde_json::json!` values.
* **Rationale**: The codebase frequently defines input schemas dynamically using ad-hoc `serde_json` objects (e.g., `json!({ "type": "object", ... })`). This bypasses versioning controls and limits compile-time validation. Standardizing on versioned Protocol Buffer descriptors or OSCAL schema-as-code mappings guarantees strict API contracts between models, the gateway, and backend agents.
* **Example**: `crates/op-mcp/src/tools/filesystem.rs:25`

### 4. Path Traversal Bypass via Weak String-Matching Blocklist in Filesystem Tools
* **Suggestion**: Replace string prefix matching (`starts_with`) with path canonicalization using `std::fs::canonicalize` validated against a designated strict root directory.
* **Rationale**: `ReadFileTool` and `WriteFileTool` use weak string prefix checks (e.g., `path.starts_with("/etc/shadow")`) to prevent unauthorized system file access. This is trivial to bypass using directory traversal sequences, such as passing `path: "/etc/shadow/../passwd"` or relative directories from a writable directory. Paths must be fully resolved and canonicalized prior to validating boundaries.
* **Example**: `crates/op-mcp/src/tools/filesystem.rs:35`

### 5. Excessive String Allocations during JSON-RPC and MCP Parsing
* **Suggestion**: Refactor `McpRequest` and `McpResponse` structs to parse and hold reference types like `Arc<str>` or `bytes::Bytes` instead of heap-allocated `String` types.
* **Rationale**: High-throughput transports (like gRPC and WebSocket) process many JSON-RPC requests containing large text blocks and payloads. Storing these strings as raw heap-allocated `String` fields causes significant memory churn and garbage collection pressure under load. Zero-copy deserialization using `Arc<str>` or borrowing lifetime-bounded slices from the raw receive buffer improves parsing performance.
* **Example**: `crates/op-mcp/src/protocol.rs:13`

### 6. Lack of Structured Tracing Fields in Logging Operations
* **Suggestion**: Update log statements to utilize structured key-value pairs (e.g., `tracing::info!(client_ip = %client_ip, token_type = "wireguard", "Accepted HTTP MCP request")`) instead of interpolating variables into unstructured strings.
* **Rationale**: Centralized log analytics platforms require structured telemetry data to perform query indexing and correlation analysis efficiently. Currently, variables are often formatted directly into unstructured print statements or standard logs, which requires costly regular expression parsing in log collectors.
* **Example**: `crates/op-mcp/src/transport/http.rs:73`

### 7. Transient Session and Active Tool Tracking State Storage
* **Suggestion**: Persist active session context, running agents, and execution state in a local `CozoDB` graph or durable SQLite store rather than keeping them in a transient `RwLock<HashMap<String, Session>>`.
* **Rationale**: The `McpGrpcService` manages session lifetimes and connected agents purely in-memory using an asynchronous `RwLock` map. In the event of a daemon crash, update, or restart, all active session states, diagnostic records, and agent coordination histories are lost. Using the workspace's configured `CozoDB` database ensures durable and queryable state persistence.
* **Example**: `crates/op-mcp/src/grpc/service.rs:114`