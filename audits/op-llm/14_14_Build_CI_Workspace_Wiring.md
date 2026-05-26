# Production Security & Quality Audit Report: `op-llm`

## 1. Build & Codegen Security Analysis

### Cargo.toml & Workspace Analysis
* **Edition**: `2021` (inherited from workspace package via `edition.workspace = true`).
* **Rust Version**: No minimum supported Rust version (`rust-version`) is declared in either the workspace `Cargo.toml` or the local `crates/op-llm/Cargo.toml`.
* **Bins & Examples**: There are no binary targets or examples defined in `crates/op-llm/Cargo.toml`.
* **Workspace Inheritance**: `crates/op-llm` heavily leverages workspace inheritance for key dependencies (`tokio`, `serde`, `simd-json`, `anyhow`, `thiserror`, `tracing`, `async-trait`, `reqwest`, `chrono`, `sha2`, `base64`). It defines local overrides for crate-specific dependencies: `rsa = "0.9.9"`, `jsonwebtoken = "9"`, `uuid = { version = "1.0", features = ["v4"] }`, and `dirs = "5.0"`.

### Schema-as-Code Build Check
* **Build Codegen**: No `build.rs` exists in `crates/op-llm`, meaning there is no build-time code generation or compilation of Protocol Buffers within this specific crate.
* **Proto Compilation**: Although workspace dependencies list `prost`, `tonic-build`, and other protobuf tools, `op-llm` does not compile `.proto` files at build time or runtime. Instead, it defines its contracts manually as ad-hoc Rust structs.

---

## 2. Security & Quality Findings

### [CRITICAL] Memory Corruption via Unpadded `simd_json::from_str`
* **Vulnerability Type**: Insecure Deserialization / Undefined Behavior
* **Citations**:
  * `crates/op-llm/src/openclaw.rs:114`
  * `crates/op-llm/src/openclaw.rs:257`
  * `crates/op-llm/src/openclaw.rs:299`
  * `crates/op-llm/src/gemini.rs:139`
  * `crates/op-llm/src/gemini.rs:171`
  * `crates/op-llm/src/gemini.rs:191`
  * `crates/op-llm/src/gemini.rs:615`
  * `crates/op-llm/src/gemini.rs:811`
  * `crates/op-llm/src/huggingface.rs:251`
  * `crates/op-llm/src/huggingface.rs:293`
  * `crates/op-llm/src/headless_oauth.rs:244`
  * `crates/op-llm/src/headless_oauth.rs:266`
  * `crates/op-llm/src/gemini_cli.rs:283`
* **Description**: The codebase uses `unsafe { simd_json::from_str(...) }` extensively to deserialize JSON string slices obtained from local configuration files or remote HTTP response bodies. 
  According to `simd-json`'s strict safety contract, the input buffer **must be mutable and padded with `simd_json::SIMDJSON_PADDING` bytes** of extra capacity. Passing standard `String` buffers or slices borrowed from `reqwest::Response::text()` that lack this padding violates the memory safety invariants of the SIMD operations, causing out-of-bounds reads and writes.
* **Impact**: A malformed or maliciously crafted JSON payload from an LLM API endpoint or local configuration file can trigger undefined behavior, arbitrary memory corruption, segmentation faults, or remote code execution (RCE).
* **Remediation**: Avoid using `simd_json::from_str` directly on unpadded buffers. Instead, convert the string into a `Vec<u8>` and use `simd_json::from_slice` which automatically copies/pads the input if necessary, or strictly allocate a padded buffer with `simd_json::to_padded_bin`.

---

### [HIGH] Denial of Service: Unbounded Process Wait in `PtyAuthBridge`
* **Vulnerability Type**: Resource Management / Thread Hang
* **Citations**: `crates/op-llm/src/pty_bridge.rs:191-260`
* **Description**: `PtyAuthBridge::execute` spawns a process and reads its stdout/stderr inside a `tokio::time::timeout`. If this timeout expires, the function proceeds to block on `child.wait().await`. However, `child.wait().await` is not wrapped in any timeout, and the child process is never explicitly killed. 
* **Impact**: If the spawned child process (such as `gemini` CLI) hangs waiting for interactive terminal input or network connectivity, `child.wait().await` will block indefinitely. This completely bypasses the `timeout_secs` parameter, locking up the async worker thread.
* **Remediation**: If the read timeout expires, explicitly invoke `child.kill().await` before awaiting the process status to guarantee the child process terminates and frees its resources.

---

### [HIGH] Denial of Service via Argument List Overflow in `GeminiCliProvider`
* **Vulnerability Type**: Resource Management / Process Spawning Failure
* **Citations**: `crates/op-llm/src/gemini_cli.rs:214-219`
* **Description**: `GeminiCliProvider::chat` serializes the full chat history (`prompt`) and passes it directly as a command-line argument to the configured CLI binary via `self.bridge.execute(&self.binary, &args, ...)`.
* **Impact**: On Unix/Linux systems, process execution fails with `E2BIG` (Argument list too long) if the serialized prompt length exceeds `ARG_MAX` limits (typically 128KB to 2MB). Since LLM interactions often accumulate long context windows, this will cause process spawning to fail reliably in extended chat sessions.
* **Remediation**: Pass the prompt payload to the command via `stdin` piping rather than command-line arguments.

---

### [HIGH] Denial of Service via Unbounded `read_line` in `McpProxyProvider`
* **Vulnerability Type**: Resource Management / Thread Hang
* **Citations**: `crates/op-llm/src/mcp_proxy.rs:44-71`
* **Description**: `McpProxyProvider::call` writes a JSON-RPC request to the `op-mcp-proxy` child process and then performs an unbounded read: `reader.read_line(&mut response_line).await?`. 
* **Impact**: If `op-mcp-proxy` crashes, deadlocks, or hangs, the calling thread will block indefinitely on `read_line`. Because this is part of the default provider pipeline, it can cause the entire system chat path to hang.
* **Remediation**: Wrap `read_line` and `child.wait()` in a `tokio::time::timeout` block with a reasonable timeout (e.g., 30 seconds).

---

### [MEDIUM] Insecure File Permissions on Sensitive OAuth Credentials
* **Vulnerability Type**: Cryptographic/Sensitive Data Storage
* **Citations**: `crates/op-llm/src/headless_oauth.rs:258-262`
* **Description**: `HeadlessOAuthProvider::save_token` serializes active Google OAuth `access_token`, `refresh_token`, `client_id`, and `client_secret` fields into a JSON string and writes it to a file using `tokio::fs::write`.
* **Impact**: `tokio::fs::write` creates files with default system permissions (typically `0644`), making these sensitive long-lived OAuth refresh credentials readable by any local user or unprivileged process on the host.
* **Remediation**: Ensure that the target file is created with restricted Unix permissions (`0600`). Use `std::fs::OpenOptions` with unix-specific extension `mode(0o600)` to ensure the file is secure from creation.

---

### [MEDIUM] Potential Leak of Secrets to System Logs
* **Vulnerability Type**: Information Disclosure
* **Citations**:
  * `crates/op-llm/src/pty_bridge.rs:186-189`
  * `crates/op-llm/src/pty_bridge.rs:242-245`
  * `crates/op-llm/src/pty_bridge.rs:349-360`
* **Description**: The `PtyAuthBridge` logs every stdout/stderr line from spawned processes at the `debug!` level. Furthermore, `LogNotificationHandler::notify` prints the matching line containing authentication requirements to `info!` logs.
* **Impact**: If an interactive prompt or process output outputs authentication codes, passwords, or temporary session keys, these credentials will be permanently written to systemd journal logs or log aggregation services.
* **Remediation**: Implement sanitization logic to redact matching passwords, codes, and raw query parameters from logged output before passing them to the logging backend.

---

### [MEDIUM] Panics on Mutex/RwLock Poisoning in Shared State
* **Vulnerability Type**: Robustness / Panic Risk
* **Citations**:
  * `crates/op-llm/src/antigravity_replay.rs:180`
  * `crates/op-llm/src/antigravity_replay.rs:187`
  * `crates/op-llm/src/headless_oauth.rs:203`
  * `crates/op-llm/src/gemini.rs:217`
* **Description**: The codebase uses synchronous `std::sync::RwLock` across multiple providers and gets the lock via `.read().unwrap()` or `.write().unwrap()`.
* **Impact**: If any thread panics while holding a read or write lock, the `RwLock` becomes poisoned. Subsequent attempts to acquire the lock will panic the calling thread, resulting in a permanent Denial of Service (DoS) of the chat server.
* **Remediation**: Use `tokio::sync::RwLock` (as is done in `chat.rs`) or handle poisoning safely (e.g., `let lock = self.session.read().unwrap_or_else(|e| e.into_inner());`).

---

### [SCHEMA-AS-CODE] Ad-Hoc Data Contracts and Untyped Schemas
* **Vulnerability Type**: Architecture / Schema Discipline Violation
* **Citations**:
  * `crates/op-llm/src/provider.rs:98`
  * `crates/op-llm/src/provider.rs:61`
  * `crates/op-llm/src/provider.rs:192`
  * `crates/op-llm/src/provider.rs:261`
  * `crates/op-llm/src/provider.rs:273`
  * `crates/op-llm/src/antigravity_replay.rs:36-64`
  * `crates/op-llm/src/anthropic.rs:59-114`
  * `crates/op-llm/src/gemini.rs:332-414`
* **Description**: Rather than using versioned schemas, the data contracts for LLM inputs, messages, tools, responses, and captured sessions are written as ad-hoc Rust structs manually serialized to JSON. Crucially, `ToolDefinition::input_schema` is represented using an untyped JSON object (`simd_json::OwnedValue`). This breaks the schema-as-code discipline defined in the rest of the workspace (where `prost-build` and `tonic-build` generate code from versioned Protocol Buffer definitions).
* **Impact**: Decreased protocol strictness, difficulty tracing breaking changes, and risk of contract drifts between client/server models.
* **Remediation**: Define chat and tool integration data contracts using versioned Protocol Buffers and generate safe Rust structures through `prost-build`.