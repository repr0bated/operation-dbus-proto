# Architecture & Module Map

## Overview
The `op-mcp-aggregator` crate acts as a proxying and caching coordinator for multiple upstream Model Context Protocol (MCP) servers. Its core role is to unify diverse tool interfaces, partition tools using named profiles, apply IP-based access zones, and optionally compress hundreds of raw tools into a high-efficiency "Compact Mode" (4-5 meta-tools) to prevent context-window exhaustion in LLM environments.

---

## Module Tree
```text
op-mcp-aggregator (crates/op-mcp-aggregator/src/lib.rs)
 ├── aggregator (aggregator.rs) - Primary coordination engine (clients, cache, profiles)
 ├── cache (cache.rs) - Thread-safe LRU cache with TTL eviction for tool schemas
 ├── client (client.rs) - Upstream connection client supporting SSE and Stdio transports
 ├── compact (compact.rs) - Compact mode meta-tools (list, search, schema, execute)
 ├── config (config.rs) - File & environment variable configuration parsers
 ├── groups (groups.rs) - Granular tool groups & network IP-based access controllers
 ├── profile (profile.rs) - Profile matching, categorization, and routing managers
 └── [Dangling] unused/context.rs - Standalone context extraction source (not in module tree)
```

---

## Entry Points
* **Library Entry Point**: `crates/op-mcp-aggregator/src/lib.rs`
* This crate does not declare any bin targets within its workspace manifest. Instead, it exposes APIs to be consumed by gateway/web service crates like `op-web` or `op-dbus`.

---

## Notes
* **Module Disconnection**: The module file `crates/op-mcp-aggregator/src/unused/context.rs` is present in the filesystem but is entirely missing from the `lib.rs` module declarations. This leaves context-aware auto-loading mechanics dead/uncompiled.
* **Workspace Setup**: The crate relies on cargo workspace definitions (`Cargo.toml`) that pull workspace-shared dependencies including `tokio`, `simd-json`, `reqwest`, and internal sibling crates (`op-core`, `op-tools`, `op-plugins`).

---

# Critical Findings

### Undefined Behavior via UTF-8 Invariant Violation in Config Parsing
* **File & Line**: `crates/op-mcp-aggregator/src/config.rs:105`
* **Vulnerability Type**: Memory Safety / Undefined Behavior
* **Description**:
  The configuration loader performs an unsafe in-place mutation of a Rust `String` allocation:
  ```rust
  let mut content = content;
  let mut content_bytes = unsafe { content.as_bytes_mut() };
  simd_json::from_slice(&mut content_bytes)
  ```
  Calling `as_bytes_mut()` on a `String` is unsafe because Rust assumes a `String` contains exclusively valid UTF-8 throughout its entire lifetime. `simd_json::from_slice` is a destructive in-place parser. During execution, it mutates the underlying slice by writing invalid UTF-8 bytes (such as placing raw null terminators `\0` in the middle of JSON strings, removing escape sequences, etc.). 
  Because this mutation occurs directly inside the buffer owned by the `content` `String` variable, the UTF-8 invariant of the `String` is violated. Even if the variable is dropped immediately afterward, violating this invariant at any point during execution constitutes immediate Undefined Behavior (UB), allowing the compiler to perform incorrect optimizations, or trigger memory corruption depending on target-specific codegen.
* **Remediation**:
  Avoid passing the mutable bytes of an active `String` to `simd-json`. Instead, consume the string and convert it directly into a raw byte vector (`Vec<u8>`), which has no UTF-8 invariants:
  ```rust
  let mut content_bytes = content.into_bytes();
  simd_json::from_slice(&mut content_bytes)
  ```

---

# Security & Quality Findings

### Ad-hoc Structs & Unstructured Data Contracts (Schema-as-Code Deviation)
* **File & Line**: 
  * `crates/op-mcp-aggregator/src/client.rs:43-69` (JSON-RPC requests and responses)
  * `crates/op-mcp-aggregator/src/client.rs:81-94` (Tool definition)
  * `crates/op-mcp-aggregator/src/aggregator.rs:608-614` (Exposed tool definition)
* **Vulnerability Type**: Quality / Code Discipline
* **Description**:
  The workspace defines a schema-as-code discipline using Protocol Buffers and OSCAL compliance. However, this crate deviates by utilizing ad-hoc structs and loose JSON values (`simd_json::OwnedValue` / `Value`) to represent incoming requests, outgoing parameters, security annotations, and input schemas:
  ```rust
  pub struct ToolDefinition {
      pub name: String,
      pub description: String,
      pub input_schema: Value,
      ...
      pub annotations: Option<Value>,
  }
  ```
  Exposing unversioned raw JSON payloads allows malicious or malformed schemas from upstream servers to bypass structured validation, increasing the risk of downstream parsing vulnerabilities or unexpected system state variations.
* **Remediation**:
  Express all MCP tool definitions and RPC schemas as strongly-typed, versioned Protocol Buffer models. Ensure validation is strictly driven by versioned schemas (e.g., via `protovalidate`) rather than accepting arbitrary `simd_json` values.

---

### Potential SSRF in Dynamic Server Registration
* **File & Line**: `crates/op-mcp-aggregator/src/aggregator.rs:252` (Dynamic add)
* **Vulnerability Type**: Server-Side Request Forgery (SSRF)
* **Description**:
  The `add_server` function dynamically connects to new upstream servers over the network using HTTP/SSE transports:
  ```rust
  pub async fn add_server(&self, config: crate::config::UpstreamServer) -> Result<()> {
      let client = Arc::new(McpClient::new(config.clone())?);
      let tools = client.list_tools().await...
  ```
  If this interface is exposed to operators or system administrators via API endpoints (such as in `op-web`), an attacker can supply local loopback or private infrastructure URLs (e.g., `http://127.0.0.1:8500`, `http://169.254.169.254/latest/meta-data`). This allows them to pivot and scan internal network segments or query internal-only services from the aggregator's security context.
* **Remediation**:
  Validate all input URLs before initializing dynamic clients. Restrict upstream targets to a strict allowlist of domains/IP addresses, and completely reject loopback, private RFC1918 addresses, and link-local ranges unless explicitly overridden in the main bootstrap configuration.

---

### Arbitrary Environment Variable Leakage via Resolve Mechanics
* **File & Line**: `crates/op-mcp-aggregator/src/config.rs:356-363`
* **Vulnerability Type**: Information Disclosure
* **Description**:
  The configuration system resolves environment variable references inside server authorization fields:
  ```rust
  fn resolve_env_var(value: &str) -> String {
      if value.starts_with("${") && value.ends_with('}') {
          let var_name = &value[2..value.len() - 1];
          std::env::var(var_name).unwrap_or_else(|_| value.to_string())
      } else {
          value.to_string()
      }
  }
  ```
  If an upstream server configuration can be supplied dynamically or written to a writable config path by an unprivileged tenant, the tenant can insert dynamic variable lookups such as `${DATABASE_URL}` or `${AWS_SECRET_ACCESS_KEY}` into headers or bearer tokens. When the aggregator makes requests to an attacker-controlled server, it will resolve and transmit these sensitive environment secrets in the authorization headers.
* **Remediation**:
  Disallow generic environment variable extraction. If resolution is required, maintain a strict allowlist of safe, non-sensitive variables (e.g., ONLY `GITHUB_TOKEN`), or restrict configuration file resolution exclusively to the initialization phase using secure local files.

---

### Missing HTTPS Enforcement for Upstream Authentication Transmission
* **File & Line**: `crates/op-mcp-aggregator/src/client.rs:174` (SSE requests)
* **Vulnerability Type**: Cryptographic Defect / Credential Theft
* **Description**:
  When communicating with upstream servers via the SSE transport, the `McpClient` configures bearer tokens, custom API headers, or Basic Authentication. However, the client fails to enforce that the upstream URL uses TLS (`https://` scheme). Credentials are automatically dispatched over unencrypted plaintext HTTP if the configured URL uses `http://`, exposing highly sensitive tokens to local interceptors or man-in-the-middle (MITM) attacks.
* **Remediation**:
  In `McpClient::new`, assert that the target URL protocol is `https://` if `auth` details (Bearer, Basic, or custom Header) are populated in the configuration, returning an error on insecure configurations.

---

### Uncompiled / Dangling Dead Code
* **File & Line**: `crates/op-mcp-aggregator/src/unused/context.rs:1`
* **Vulnerability Type**: Code Quality
* **Description**:
  The file `unused/context.rs` contains fully written context-aware tool-loading components (such as `ContextAwareTools` and `observe_message`). However, it has been orphaned and is never declared as a module in `lib.rs`. This results in dead code that is completely ignored by the compiler, meaning it fails to undergo syntax checking, type checking, or borrow checking during subsequent code updates.
* **Remediation**:
  Either formally register the module inside `lib.rs` (e.g., `pub mod context;`) or remove the `unused` directory entirely to maintain a clean source footprint.