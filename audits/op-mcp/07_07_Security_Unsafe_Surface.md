# PRODUCTION SECURITY & QUALITY AUDIT REPORT
**Target Crate:** `op-mcp`  
**Status:** Failing / Action Required (Critical Vulnerabilities Found)

---

## 1. Executive Summary

This security and quality audit evaluates the `op-mcp` codebase for cryptographic safety, memory safety, structural validation, schema-as-code discipline, and network security. 

During this evaluation, two **Critical** directly exploitable vulnerabilities were identified:
1. **Remote Authentication Bypass via Host Header Spoofing** in the HTTP transport layer.
2. **Arbitrary File Read/Write via Path Canonicalization Bypass** (Directory Traversal) in the filesystem tools.

Additionally, multiple violations of the codebase's strict command restrictions, lack of safety documentation for `unsafe` blocks, and systemic deviations from "schema-as-code" rules have been cataloged.

---

## 2. Critical Exploitable Vulnerabilities

### 2.1 Remote Authentication Bypass via Client-Controlled Host Header
* **Citation:** `crates/op-mcp/src/transport/http.rs:35-51`
* **Vulnerability Type:** Authentication Bypass / Privilege Escalation
* **Severity:** Critical (Directly Exploitable)

#### Analysis
The HTTP/SSE transport implements a `wireguard_auth_middleware` meant to enforce authorization tokens on all non-loopback clients. However, it relies on `is_localhost_host` to determine whether the incoming request is originating from loopback:

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

The middleware then completely bypasses authentication if this helper returns `true`:

```rust
async fn wireguard_auth_middleware(
    headers: HeaderMap,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    // Allow health check and loopback without auth
    if request.uri().path() == "/health" || is_localhost_host(&headers) {
        return Ok(next.run(request).await);
    }
    // ...
```

#### Exploitation Vector
Because the HTTP `Host` header is fully controlled by the remote client, a network attacker can connect to the public-facing port of the MCP server and supply a forged header: `Host: localhost` or `Host: 127.0.0.1`. The middleware will accept this header as proof of local origin, bypassing WireGuard auth entirely and allowing the attacker to execute arbitrary tools on the host.

---

### 2.2 Arbitrary File Read and Write via Non-Canonicalized Directory Traversal
* **Citation:** `crates/op-mcp/src/tools/filesystem.rs:35-38` (ReadFileTool), and `crates/op-mcp/src/tools/filesystem.rs:71-74` (WriteFileTool)
* **Vulnerability Type:** Path Traversal / Arbitrary File Read & Write
* **Severity:** Critical (Directly Exploitable)

#### Analysis
Both `ReadFileTool` and `WriteFileTool` attempt to restrict access to sensitive directories (such as `/etc` or `/boot`) by inspecting string prefixes directly on the user-supplied string:

```rust
// ReadFileTool Check
if path.starts_with("/etc/shadow") || path.starts_with("/etc/sudoers") {
    return Ok(json!({"success": false, "error": "Access denied"}));
}
```
```rust
// WriteFileTool Check
if path.starts_with("/etc/") || path.starts_with("/boot/") {
    return Ok(json!({"success": false, "error": "Access denied"}));
}
```

#### Exploitation Vector
No path canonicalization (e.g., calling `std::fs::canonicalize` or resolving intermediate relative components) is performed before these prefix checks. An attacker can easily read `/etc/shadow` by calling `read_file` with the path `/tmp/../etc/shadow` or `/./etc/shadow`. Similarly, an attacker can overwrite arbitrary system configuration files using `write_file` with paths such as `/tmp/../etc/cron.d/malicious`.

---

## 3. Security & Unsafe Code Audit

### 3.1 Unsafe Blocks Inventory & Missing Safety Explanations
All `unsafe` blocks in this crate invoke `simd_json::from_str`. Every single block **violates the Rust standards** by omitting the mandatory `// SAFETY:` explanation.

1. **`crates/op-mcp/src/agents_main.rs:538`**
   ```rust
   let request: JsonRpcRequest = match unsafe { simd_json::from_str(&mut line) } {
   ```
   * *Finding:* Missing `// SAFETY:` comment.

2. **`crates/op-mcp/src/agents_server.rs:312`**
   ```rust
   let result_value: Value = unsafe { simd_json::from_str(&mut result_mut) }
   ```
   * *Finding:* Missing `// SAFETY:` comment.

3. **`crates/op-mcp/src/protocol.rs:183`**
   ```rust
   let parsed: McpRequest = unsafe { simd_json::from_str(&mut json_buf) }.unwrap();
   ```
   * *Finding:* Missing `// SAFETY:` comment.

4. **`crates/op-mcp/src/protocol.rs:203`**
   ```rust
   let parsed: McpResponse = unsafe { simd_json::from_str(&mut json_buf) }.unwrap();
   ```
   * *Finding:* Missing `// SAFETY:` comment.

5. **`crates/op-mcp/src/external_client.rs:417`**
   ```rust
   let response: Value = unsafe { simd_json::from_str(&mut response_line) }
   ```
   * *Finding:* Missing `// SAFETY:` comment.

6. **`crates/op-mcp/src/external_client.rs:458`**
   ```rust
   let configs: Vec<ExternalMcpConfig> =
       unsafe { simd_json::from_str(&mut content) }.context("Failed to parse MCP config")?;
   ```
   * *Finding:* Missing `// SAFETY:` comment.

7. **`crates/op-mcp/src/transport/stdio.rs:43`**
   ```rust
   let response = match unsafe { simd_json::from_str::<McpRequest>(&mut line_mut) } {
   ```
   * *Finding:* Missing `// SAFETY:` comment.

8. **`crates/op-mcp/src/transport/websocket.rs:115`**
   ```rust
   let response = match unsafe { simd_json::from_str::<McpRequest>(&mut text_mut) } {
   ```
   * *Finding:* Missing `// SAFETY:` comment.

---

### 3.2 Command Execution & Forbidden Binaries Analysis

#### 3.2.1 Forbidden Network Command Whitelisting
* **Citation:** `crates/op-mcp/src/tools/shell.rs:21-27`
* **Vulnerability Type:** Violation of Command Restrictions / Exfiltration Risk
* **Severity:** High

The `ShellExecuteTool` whitelists commands that LLMs/clients can request to run. This whitelist explicitly includes forbidden data exfiltration and diagnostic tools:

```rust
"echo", "pwd", "whoami", "date", "uname", "df", "du", "free", "uptime",
"ps", "top", "ip", "ss", "netstat", "ping", "dig", "curl", "wget",
```

**Violation:** `curl` and `wget` are explicitly whitelisted. An LLM or remote actor leveraging this tool can perform arbitrary network calls to exfiltrate system data (including files read via the traversal vulnerability) or fetch remote payloads to execute.

---

#### 3.2.2 Forbidden OpenFlow & OVS Mutating Command Implementations
* **Citation:** `crates/op-mcp/src/tools/ovs.rs:42-51`
* **Vulnerability Type:** Forbidden Process Spawning
* **Severity:** High

The codebase implements custom OVS mutation wrappers that invoke forbidden CLI tools without sufficient system safety protections:

```rust
// crates/op-mcp/src/tools/ovs.rs:42
let output = tokio::process::Command::new("ovs-vsctl").args(args).output().await?;
```
```rust
// crates/op-mcp/src/tools/ovs.rs:51
let output = tokio::process::Command::new("ovs-ofctl").args(args).output().await?;
```

**Violation:** Raw invocations of `ovs-vsctl` and `ovs-ofctl` bypass programmatic APIs, leaving the network topology highly vulnerable to remote injection attacks.

---

## 4. D-Bus Trust Boundary Risks

### 4.1 Unauthenticated D-Bus Agent Hijacking
* **Citation:** `crates/op-mcp/src/agents_server.rs:103-128`
* **Severity:** Medium-High (Architectural Threat)

`AgentsServer::discover_agents` queries the system D-Bus for services that match the pattern `org.dbusmcp.Agent.*`. If a matching service is found, the server automatically issues introspective method calls to obtain the service's name, description, and list of operations, registering them as valid, executable MCP tools.

#### Threat Model
Any user or process running on the host with permissions to register a service on the system or session bus under `org.dbusmcp.Agent.Malicious` can register arbitrary commands. When the `AgentsServer` refreshes, it registers these malicious endpoints and proxies client executions directly to them via the unauthenticated `Execute` method on D-Bus.

---

## 5. Schema-As-Code and Quality Analysis

### 5.1 Ad-Hoc Inline JSON Schemas
The crate consistently violates the disciplined schema-as-code approach (which mandates using Protocol Buffers or versioned OSCAL structures) by implementing unversioned, ad-hoc JSON literals directly inside tool definitions.

* **Ad-Hoc Tool Definitions:** `crates/op-mcp/src/agents_main.rs:91-285`
  All tool schemas are represented as inline `simd_json` literals. For example:
  ```rust
  input_schema: json!({
      "type": "object",
      "properties": {
          "thought": {
              "type": "string",
              "description": "The current thought or reasoning step"
          },
          ...
  ```
* **Default Operation Schemas:** `crates/op-mcp/src/agents_server.rs:232-248`
  ```rust
  fn get_operation_schema(&self, _agent_type: &str, _operation: &str) -> Value {
      json!({
          "type": "object",
          "properties": { ... }
      })
  }
  ```
* **Compact Tools Schemas:** `crates/op-mcp/src/compact.rs:424-500`
  Declares list, search, and execution meta-tool contracts using ad-hoc inline structures.

**Remediation:** Declare all MCP schemas as structured Protocol Buffers (using code generation to output the required JSON-Schema equivalents) to maintain strict serialization and interface contracts between modules.

---

### 5.2 Cryptographic Weakness in Token Length Assessment
* **Citation:** `crates/op-mcp/src/transport/http.rs:26-33`
* **Severity:** Low-Medium

The helper `is_wireguard_pubkey` verifies WireGuard identity tokens based solely on length and character sets:

```rust
fn is_wireguard_pubkey(token: &str) -> bool {
    token.len() == 44
        && token.ends_with('=')
        && token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '='))
}
```

This is a structural check only and does not cryptographically validate that the key is registered or structurally secure on the active host interface.

---

## 6. Detailed Remediation Roadmap

1. **Fix Host Header Exploitation (Immediate):**
   Modify `is_localhost_host` to query the underlying TCP socket connection state rather than relying on the HTTP `Host` header. Do not trust user-controlled HTTP headers for routing security logic.
   
2. **Canonicalize Filesystem Paths (Immediate):**
   In both `ReadFileTool` and `WriteFileTool`, apply `std::fs::canonicalize` to user paths prior to running the directory prefix validations.
   
3. **Purge Forbidden Commands (Immediate):**
   Remove `"curl"` and `"wget"` from the `ShellExecuteTool` allowed list (`crates/op-mcp/src/tools/shell.rs`). Remove the `ovs` tool definition file (`crates/op-mcp/src/tools/ovs.rs`) and forbid any process execution invoking `ovs-*` or OpenFlow binaries.
   
4. **Enforce Safety Comments:**
   Review and document all `unsafe` blocks, specifying the precise memory and boundary guarantees of the input string mutations before `simd_json::from_str`.