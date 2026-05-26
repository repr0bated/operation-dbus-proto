# Observability and Security Audit

## 1. Observability Instrumentation Metrics

### Macro Counts
* **`tracing::` macros**: **176** occurrences (comprising `info!`, `warn!`, `error!`, and `debug!`).
* **`println!` macros**: **4** occurrences (all located in `crates/op-web/src/email.rs` at lines 60, 61, 62, and 63).

### Metrics Instrumentation
No standardized metrics instrumentation (using crates like `prometheus` or `metrics`) exists in the provided source files. Despite the presence of `prometheus = { workspace = true, features = ["process"] }` in `crates/op-web/Cargo.toml`, actual metrics collection is implemented via ad-hoc filesystem parsing:
* System stats (CPU/Memory) are manually extracted by reading and parsing `/proc/loadavg` and `/proc/meminfo` in `crates/op-web/src/handlers/dashboard.rs:59-92`.
* VPN peer metrics are retrieved by executing the shell command `wg` and parsing the stdout raw dump in `crates/op-web/src/handlers/vpn.rs:36-69`.

---

## 2. Swallow Errors Without Logging

### Silent Swallowing of Core Database Initialization Errors
* **`crates/op-web/src/users.rs:77`**
  ```rust
  store.load().await.ok();
  ```
  **Impact:** Any error encountered when reading, opening, or parsing the user database JSON file is silently discarded via `.ok()`. If the database file is corrupted or unreadable due to permission issues, the system will silently bootstrap with an empty in-memory user list, potentially causing state desynchronization or loss of existing user data without any warning.

### Silent Directory Creation Failures
* **`crates/op-web/src/users.rs:136`**
  ```rust
  tokio::fs::create_dir_all(parent).await.ok();
  ```
  **Impact:** Fails silently when creating parent directories for the user database path. Subsequent database writes will fail with an untracked I/O error.

### Ignored Group Config Parse Failures
* **`crates/op-web/src/groups_admin.rs:43`**
  ```rust
  if let Ok(content) = std::fs::read_to_string(GROUPS_CONFIG_PATH) { ... }
  ```
  **Impact:** If reading the tool groups configuration file fails, or if the `simd-json` deserialization of it fails, the error is swallowed with no logs, falling back directly to defaults.

### Silently Swallowed Process Execution Failures
* **`crates/op-web/src/handlers/dashboard.rs:46`**
  ```rust
  Command::new("wg").args(&["show", "wg0", "peers"]).output().ok()
  ```
  **Impact:** If the `wg` binary is missing from the environment PATH or fails to run, the system silently returns 0 active connections without logging the underlying process execution error.

---

## 3. PII and Secret Exposure in Logs

### Logging of Magic Link Tokens (Authentication Secrets)
* **`crates/op-web/src/email.rs:58-59`**
  ```rust
  info!("   URL: {}", magic_url);
  info!("   Token: {}", token);
  ```
  **Impact:** Active magic link tokens (which grant full VPN and API access to a user) are printed directly to the application logs during testing configuration. Any attacker or monitoring system with read access to the service logs can hijack user sessions instantly.

### Plaintext Logging of Tool and Agent Arguments
* **`crates/op-web/src/mcp_compact.rs:416`**
  ```rust
  info!("Executing underlying tool: {} with args: {}", tool_name, arguments);
  ```
* **`crates/op-web/src/mcp_agents.rs:587`**
  ```rust
  info!("MCP Agents tool call: {} with args: {}", tool_name, arguments);
  ```
  **Impact:** Arguments passed to the Compact MCP execution framework and Cognitive Agents are logged in plain text. If an LLM executes a tool designed to write secrets, provision API keys, or manage private files, those sensitive parameters will leak directly into the system log.

### Plaintext PII Logging (Email Addresses & IP Addresses)
* **`crates/op-web/src/email.rs:57` & `106`**: Logs the target user's email address when preparing or sending magic links.
* **`crates/op-web/src/handlers/privacy.rs:142`**: `info!("New privacy user registered: {}", email);` logs registered emails.
* **`crates/op-web/src/middleware/security.rs:102`**: `debug!("Request from IP: {} [Zone: {:?}]", client_ip, zone);` logs incoming client IP addresses.

### Exposure of Truncated High-Privilege Bypass Keys
* **`crates/op-web/src/middleware/security.rs:95`**
  ```rust
  info!("API key bypass granted: IP={} key={}...{}", client_ip, &key[..8], &key[key.len() - 4..]);
  ```
  **Impact:** Logs the first 8 and last 4 characters of active bypass API keys. This reduces the keys' search space, making offline brute-force significantly easier if logs are compromised.

---

## 4. Critical Security Findings

### CRITICAL: Unauthenticated Remote Code Execution (RCE) via Direct Tool Execution
* **File:Line:** `crates/op-web/src/routes/mod.rs:114` and `crates/op-web/src/handlers/tools.rs:116-146`
* **Vulnerability:** The route `/api/tool` maps directly to `handlers::tools::execute_tool_handler`. While `ip_security_middleware` is configured globally and attaches `AccessZone` to the request, **no route inside `routes/mod.rs` actually validates this extension.** 
* **Exploitation:** An unauthenticated external attacker can send a `POST` request to `/api/tool` with the payload `{"tool_name": "shell_exec", "arguments": {"command": "id"}}` to execute arbitrary commands as the user running the server (which uses `doas` / has root privileges).
* **Remediation:** Enforce `AccessZone::TrustedMesh` verification inside the middleware or handlers before resolving and executing registry tools.

### CRITICAL: Arbitrary File Write & RCE via Path Traversal in Transcripts
* **File:Line:** `crates/op-web/src/handlers/chat.rs:260-264` and `crates/op-web/src/handlers/chat.rs:410`
* **Vulnerability:** The `save_transcript_handler` extracts the `filename` parameter from user input and writes the conversation transcript directly to a path using `format!("/tmp/{}", filename)`.
* **Exploitation:** There is no sanitization against path traversal (e.g. `..` sequences). An attacker can supply a filename like `../../etc/cron.d/malicious_job` to write arbitrary content to sensitive directories, leading to remote code execution.
* **Remediation:** Sanitize the filename to ensure it is alphanumeric only and strip any path traversal sequences.

### CRITICAL: Multi-User Chat Session Leakage via Shared WebSocket Broadcast
* **File:Line:** `crates/op-web/src/handlers/websocket.rs:48` and `crates/op-web/src/handlers/websocket.rs:82-91`
* **Vulnerability:** The WebSocket handler in `src/handlers/websocket.rs` subscribes to a global broadcast channel (`state.broadcast_tx.subscribe()`). When any client sends a message, the handler processes it through the orchestrator and sends the result to `state.broadcast_tx`.
* **Exploitation:** Every connected WebSocket client receives all messages broadcasted to `state.broadcast_tx`. Any user connected to `/ws` can read the private prompts, assistant responses, and tool executions of all other active users in real-time.
* **Remediation:** Remove the use of the global broadcast channel for session-specific responses; stream orchestrator responses exclusively to the private task queue associated with the individual socket connection.

### CRITICAL: Denial of Service (DoS) via CSRF Token Store Wiping
* **File:Line:** `crates/op-web/src/handlers/privacy.rs:481-483`
* **Vulnerability:** The OAuth CSRF token state-tracking mechanism utilizes a simplistic cleanup heuristic:
  ```rust
  if tokens.len() > 1000 {
      tokens.clear();
  }
  ```
* **Exploitation:** An attacker can issue 1001 automated requests to `/api/privacy/google/auth` to artificially grow the `tokens` map. When the limit is hit, the application deletes **all** stored CSRF tokens, immediately breaking and invalidating any legitimate users' active login flows.
* **Remediation:** Implement a time-to-live (TTL) expiration strategy using a chronologically sorted queue or eviction pool instead of wiping the entire database.

### CRITICAL: Authentication Bypass via Hardcoded API Bypass Keys
* **File:Line:** `crates/op-web/src/middleware/security.rs:13-16`
* **Vulnerability:** Active authorization bypass keys are hardcoded directly in the source code:
  ```rust
  const BYPASS_API_KEYS: &[&str] = &[
      "4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f", // Primary MCP access key
      "test-key-huggingface-2024",            // Hugging Face test key
  ];
  ```
* **Exploitation:** Any external attacker who decompiles the binary or accesses the repository can extract these keys and pass them via the `x-api-key` header to instantly elevate their privilege zone to `TrustedMesh` and bypass all security constraints.
* **Remediation:** Load authorized API tokens dynamically from a secured configuration store or database with strong hash-based validation.