### 1. Observability: Tracing vs. Print Macros

The `op-mcp` crate primarily implements logging through the `tracing` crate. A specialized STDIO transport server falls back to standard error streams (`eprintln!`) to prevent corruption of the stdout channel used for JSON-RPC transport.

#### Macro Counts
* **Tracing Macros (`tracing::` / `info!`, `warn!`, `error!`, `debug!`, `trace!`):** **140**
  * `info!` / `tracing::info!`: **74**
  * `warn!` / `tracing::warn!`: **18**
  * `error!` / `tracing::error!`: **21**
  * `debug!` / `tracing::debug!`: **24**
  * `trace!` / `tracing::trace!`: **3**
* **Print Macros (`println!`, `eprintln!`):** **6**
  * `println!`: **0**
  * `eprintln!`: **6** (All found in `crates/op-mcp/src/agents_main.rs` to keep stdout dedicated to JSON-RPC payload transport).

---

### 2. Gaps: Swallowed Errors without Logging

The following locations handle error states but fail to log them, or swallow the underlying error structures entirely:

* **Compiling & Registration Future Dropped (Qdrant):**  
  `crates/op-mcp/src/tools/qdrant.rs:79`
  ```rust
  registry.register(Box::new(tool));
  ```
  The `register` function on `ToolRegistry` is `async`. Calling it here without `.await` generates a compile warning (or error under strict flags), and the future is discarded immediately. The tool is never actually registered, and any potential error is silently swallowed.

* **Silenced D-Bus Interface Property Errors:**  
  `crates/op-mcp/src/tools/systemd.rs:73-76`
  ```rust
  let active: String = unit_proxy.get_property("ActiveState").await.unwrap_or_else(|_| "unknown".into());
  ```
  If D-Bus connectivity is lost, or if there is an authorization failure querying unit properties, the errors are quietly swallowed and replaced with fallback strings like `"unknown"` or `"No description"` without logging.

* **Silenced Hardware / Network I/O Failures:**  
  `crates/op-mcp/src/tools/system.rs:34-39`
  ```rust
  let state = tokio::fs::read_to_string(format!("/sys/class/net/{}/operstate", name))
      .await.unwrap_or_else(|_| "unknown".into()).trim().to_string();
  ```
  Missing sysfs paths or system permission failures are silently swallowed and mapped to `"unknown"`, hiding localized I/O errors.

* **Silent Thread Join Gaps (HTTP Proxy):**  
  `crates/op-mcp/src/http_server.rs:383`
  ```rust
  let errors = error_handle.await.unwrap_or_default();
  ```
  If the stderr logging task crashes or fails to join, the join error is swallowed by `unwrap_or_default()`.

* **Silent Stream Failures during Reading Stderr:**  
  `crates/op-mcp/src/http_server.rs:367-372`
  ```rust
  while let Some(line) = error_reader.next_line().await.unwrap_or(None)
  ```
  If reading from the stderr pipe encounters an I/O error, it is silenced via `unwrap_or(None)`, terminating the log forwarding thread silently.

* **Streaming Channel Send Errors Swallowed:**  
  `crates/op-mcp/src/grpc/service.rs:394-406` and `crates/op-mcp/src/grpc/service.rs:445`
  ```rust
  if tx.send(Ok(start_msg)).await.is_err() {
      return;
  }
  ```
  When streaming tool outputs, disconnection failures or channel capacity exhaustion discard errors silently via `let _ =` or quick returns.

---

### 3. Gaps: PII & Secrets Logged

Multiple log points leak entire request/response payloads containing potentially sensitive data:

* **Raw Request Body Output at INFO Level:**  
  `crates/op-mcp/src/http_server.rs:342`
  ```rust
  info!("MCP Request: {}", request_json);
  ```
  If the user calls a filesystem tool, DB query tool, or passes authentication tokens in configuration arguments, the complete payload is written directly to the logs at `INFO` level.

* **Raw Response Body Output at INFO Level:**  
  `crates/op-mcp/src/http_server.rs:403`
  ```rust
  info!("MCP Response: {}", response_str);
  ```
  Outputs of tool executions (e.g. database rows, files read via `read_file`, configuration data) are written directly to system logs at `INFO` level.

* **Detailed JSON-RPC Log Leaks on DEBUG Level:**  
  `crates/op-mcp/src/external_client.rs:368` and `crates/op-mcp/src/external_client.rs:375`
  ```rust
  tracing::debug!("Sent request to {}: {}", self.config.name, request_str);
  ```
  Dumps raw outbound external client communications and inbound responses into the logs when `DEBUG` is active.

---

### 4. Metrics Instrumentation

* **Core Metric Gaps:** There is no direct integration with the `prometheus` or `metrics` crates in the core transport layers of `op-mcp`.
* **Basic Atomic Counters:** `crates/op-mcp/src/grpc/service.rs:114-115` provides internal counters (`request_counter`, `error_counter`) exposed primarily via the unified health RPC check.
* **Delegated Metrics:** `op-mcp` delegates structured metrics, status registration, and latency monitoring to the external `op-execution-tracker` crate inside `crates/op-mcp/src/tool_adapter.rs` and `crates/op-mcp/src/tool_adapter_orchestrated.rs` via the `execution_tracker`.

---

### 5. Schema-As-Code Discipline Compliance

While the gRPC service utilizes generated Protobuf schemas derived from versioned models (`crates/op-mcp/src/grpc/generated/op.mcp.v1.rs`), other transport boundary layers represent violations of the Schema-as-Code discipline:

* **Ad-Hoc JSON-RPC Handlers (HTTP Server):**  
  `crates/op-mcp/src/http_server.rs:352-404` communicates with backend executables by executing CLI subprocesses and dynamically parses their standard output into unstructured `simd_json::OwnedValue` dynamic maps, instead of using versioned serialization objects.
* **Ad-Hoc Meta-Tool Schema Collections:**  
  `crates/op-mcp/src/compact.rs:387-440` and `crates/op-mcp/src/request_handler.rs:188-245` hardcode dynamic JSON arrays using `json!({ ... })` directly inside the logic to describe tool schemas rather than utilizing schemas compiled from versioned definitions.
* **Ad-Hoc Memory & Thought Serialization:**  
  `crates/op-mcp/src/builtin_trait_agents.rs:60-64` returns unstructured dynamic JSON payloads as contract representations for memory storage and thought steps.

---

### 6. Security Vulnerabilities

#### CRITICAL: Remote Authentication Bypass via Client-Controlled `Host` Header
* **Citations:** `crates/op-mcp/src/transport/http.rs:46-55` and `crates/op-mcp/src/transport/http.rs:64-67`
* **Impact:** Any remote attacker can bypass authentication on the HTTP/SSE transport, gaining unauthenticated access to the exposed MCP tools.
* **Mechanism:** 
  The HTTP auth middleware uses `is_localhost_host` to check if a request should bypass authentication:
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
  If this returns `true`, the middleware permits execution:
  ```rust
  if request.uri().path() == "/health" || is_localhost_host(&headers) {
      return Ok(next.run(request).await);
  }
  ```
  Because the HTTP `Host` header is fully controlled by the client, a remote attacker targeting an interface bound to `0.0.0.0` can supply a `Host: localhost` header to bypass authentication checks and call tools.

#### HIGH: Weak Path Validation leading to Directory Traversal File Reads/Writes
* **Citations:** `crates/op-mcp/src/tools/filesystem.rs:36-39` and `crates/op-mcp/src/tools/filesystem.rs:66-69`
* **Impact:** Read/Write tool callers can read or overwrite sensitive system configurations (such as user profiles, service keys, or SSH credentials).
* **Mechanism:**
  `ReadFileTool` and `WriteFileTool` protect restricted paths using a basic `.starts_with` check:
  ```rust
  if path.starts_with("/etc/shadow") || path.starts_with("/etc/sudoers") {
  ```
  Because the paths are not canonicalized before evaluation, an attacker can bypass this check using relative paths or directory traversal sequences (e.g., `/tmp/../../../etc/shadow` or `etc/shadow`).

#### HIGH: Arbitrary Subprocess Command Execution via Unsanitized Arguments
* **Citations:** `crates/op-mcp/src/tools/shell.rs:49-65`
* **Impact:** Attackers can run arbitrary commands under the system context of the running server.
* **Mechanism:**
  `ShellExecuteTool` checks if the requested base binary is on a whitelist:
  ```rust
  if !self.allowed_commands.contains(&command.to_string()) {
  ```
  The whitelist includes interpreters and build environments such as `python3`, `npm`, `cargo`, and `kubectl`. Because the `args` parameter is passed to the command as an unsanitized array of strings, an attacker can leverage flags (e.g., `python3 -c "..."`) to execute arbitrary system code.