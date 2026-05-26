# OP-LLM Quality and Security Audit

## 1. Storage Backend Inventory

As specified in the architectural guidelines, the codebase defines multiple storage options inside its root metadata. However, a strict audit of the provided source files for the `op-llm` crate shows **no direct implementation** or instantiation of these engines within this specific crate's source directory (`crates/op-llm/src/*`). The configuration is managed at the workspace level:

| Backend | Found at File:Line | Role |
| :--- | :--- | :--- |
| `cozo` (with `storage-sled`) | `Cargo.toml:80` | Relational-graph-vector database with Sled persistence used for knowledge graph storage. |
| `sqlx` (with `sqlite`) | `Cargo.toml:104` | SQLite storage engine for system state and configuration. |
| `rusqlite` | `Cargo.toml:105` | SQLite wrapper used for persistent storage of local states and caches. |
| `redis` | `Cargo.toml:106` | Key-value store and message queue for state mirroring across the network. |

---

## 2. Dependencies & Feature Inventory

The following table documents all direct dependencies specified inside `crates/op-llm/Cargo.toml` and their configuration:

| Dependency | Version / Source | Workspace Default / Explicit Features | Security / Quality Notes |
| :--- | :--- | :--- | :--- |
| `tokio` | Workspace | Inherited `["full"]` | Large attack surface; pulls in all async primitives. |
| `serde` | Workspace | Inherited `["derive"]` | Standard serialization library. |
| `simd-json` | Workspace | Inherited `["serde", "serde_impl"]` | High-performance JSON parser; utilizes extensive `unsafe` blocks. |
| `anyhow` | Workspace | Default features | Used for general error reporting. |
| `thiserror` | Workspace | Default features | Used for custom enum error definitions. |
| `tracing` | Workspace | Default features | Structured logging framework. |
| `async-trait` | Workspace | Default features | Desugars async traits. |
| `reqwest` | Workspace | Inherited `["json", "stream"]` | HTTP client used for external provider API connections. |
| `chrono` | Workspace | Inherited `["serde"]` | Used for timestamps. |
| `rsa` | `"0.9.9"` | Explicit version (not workspace) | Used for cryptographic operations. |
| `sha2` | Workspace | Default features | Secure Hash Algorithm 2. |
| `base64` | Workspace | Default features | Base64 encoding/decoding. |
| `jsonwebtoken` | `"9"` | Explicit version (not workspace) | Risk of validation-bypass vulnerabilities in legacy versions. |
| `uuid` | `version = "1.0", features = ["v4"]` | Explicit override (mismatched with workspace `1.6`) | Mismatched version causes duplicate compilation and binary bloat. |
| `dirs` | `"5.0"` | Explicit version (not workspace) | Used to locate OS-specific configuration directories. |

### Crate Features
The `crates/op-llm/Cargo.toml` file defines **no custom [features]** section; all code compilation path-gates are governed by the presence of target environment variables.

---

## 3. Schema-As-Code Gaps & Deficiencies

The workspace metadata defines schema-as-code libraries (`prost`, `tonic`, `prost-types`, and `jsonschema`), but `op-llm` displays **major schema discipline deficiencies**:

* **Ad-Hoc Structs Instead of Versioned Schemas:**
  External API payloads are defined using manual Rust structs decorated with Serde serialization macros rather than shared, compiled, and versioned Protocol Buffers or OpenAPI schemas:
  * `AnthropicRequest` and `AnthropicMessage` defined in `crates/op-llm/src/anthropic.rs:60-112`.
  * `GeminiRequest` and `GeminiTool` defined in `crates/op-llm/src/gemini.rs:411-477`.
  * `PerplexityRequest` defined in `crates/op-llm/src/perplexity.rs:63-95`.
* **Untyped JSON Arbitrary Schema Containers:**
  The `ToolDefinition` and `ToolCallInfo` structs inside `crates/op-llm/src/provider.rs:115` use raw, untyped `simd_json::OwnedValue` (`Value`) objects for inputs and arguments:
  ```rust
  pub struct ToolDefinition {
      pub name: String,
      pub description: String,
      pub input_schema: simd_json::OwnedValue, // ◄── Untyped / Lack of schema schema validation
  ```
  This bypasses compiled schemas entirely, allowing malformed schema payloads to go undetected until they reach the remote LLM endpoint.
* **Lack of OSCAL Alignment:**
  Security compliance criteria are not programmatically integrated or enforced. System configurations for accessing external models are defined as arbitrary environment strings (`LLM_PROVIDER`, `LLM_MODEL`) without structured machine-readable control validation schemas.

---

## 4. Detailed Vulnerability & Quality Findings

### [CRITICAL] World-Readable OAuth Token File via Insecure File Writing
* **Reference:** `crates/op-llm/src/headless_oauth.rs:258-262`
* **Vulnerability Type:** CWE-732: Incorrect Permission Assignment for Critical Resource
* **Exploitability:** **Directly Exploitable.** The function `save_token` writes highly sensitive Google OAuth refresh tokens and credentials back to the disk:
  ```rust
  async fn save_token(&self, token: &OAuthToken) -> Result<()> {
      let contents = simd_json::to_string_pretty(token)?;
      tokio::fs::write(&self.token_file, contents).await?;
      Ok(())
  }
  ```
  `tokio::fs::write` creates or overwrites the file using default file-creation permissions (governed by `umask`, typically `0644` or `0622`). In a multi-user environment or on a shared development host, any unprivileged local attacker can read `~/.config/antigravity/token.json` or `/tmp/antigravity-token.json` to steal the OAuth access and refresh tokens, gaining persistent unauthorized access to the victim's Google Cloud Platform (GCP) subscriptions.
* **Remediation:** Explicitly configure secure permissions (mode `0600`) during file creation using Unix-specific file options:
  ```rust
  use std::os::unix::fs::OpenOptionsExt;
  let mut options = std::fs::OpenOptions::new();
  options.write(true).create(true).truncate(true).mode(0o600);
  ```

---

### [HIGH] Undefined Behavior Risk via Unsafe `simd_json::from_str` on Untrusted API Responses
* **Reference:** 
  * `crates/op-llm/src/gemini.rs:703`
  * `crates/op-llm/src/gemini.rs:898`
  * `crates/op-llm/src/huggingface.rs:214`
  * `crates/op-llm/src/openclaw.rs:273`
  * `crates/op-llm/src/gemini_cli.rs:273`
* **Vulnerability Type:** CWE-242: Use of Inherently Dangerous Function
* **Exploitability:** **Exploitable via Network Spoofing.** The codebase consistently uses `unsafe { simd_json::from_str(...) }` to parse raw HTTP responses received from external servers:
  ```rust
  let mut raw_body_mut = raw_body;
  let result: GeminiResponse = match unsafe { simd_json::from_str(&mut raw_body_mut) } {
  ```
  `simd-json`'s string parsing modifies the input buffer in-place and relies on strict memory-alignment and UTF-8 invariants. If an attacker can intercept, spoof, or poison the DNS of remote providers (HuggingFace, Gemini, or OpenClaw endpoints), they can return a crafted payload that violates parser assumptions, triggering undefined behavior, memory corruption, or a sudden process crash in the control plane.
* **Remediation:** Remove the `unsafe` block and use safe deserialization routes, or utilize `simd_json::serde::from_slice` which handles structural parsing safely without exposing raw unsafe pointers to unvalidated network inputs.

---

### [MEDIUM] Silent Security Bypass of Anti-Hallucination Tool Constraints
* **Reference:** 
  * `crates/op-llm/src/perplexity.rs:140`
  * `crates/op-llm/src/mcp_proxy.rs:191`
* **Vulnerability Type:** CWE-639: Bypass of Authorization Channel / Logic Error
* **Exploitability:** When executing critical control-plane system actions, the system relies on `ToolChoice::Required` to force the LLM to invoke tool schemas (anti-hallucination guard rails). However, if the active provider is switched to `PerplexityClient` or `McpProxyProvider`, the trait implementation for `LlmProvider` either omits `chat_with_request` or discards tool options:
  ```rust
  // McpProxyProvider:
  async fn chat_with_request(&self, model: &str, request: ChatRequest) -> Result<ChatResponse> {
      // For tool-calling requests, flatten to a simple prompt since
      // op-mcp-proxy only supports generateContent.
      self.chat(model, request.messages).await
  }
  ```
  This silently strips the anti-hallucination constraint and executes a standard text generation fallback. The LLM then outputs unvalidated conversational text instead of structured tool schemas, directly bypassing the control plane's verification logic.
* **Remediation:** Raise a hard execution error inside `chat_with_request` if the selected provider lacks functional capabilities for tool definition parsing and constraint enforcement.

---

### [MEDIUM] Sensitive Credentials and API Secrets Leaked to System Logs
* **Reference:** 
  * `crates/op-llm/src/pty_bridge.rs:231`
  * `crates/op-llm/src/pty_bridge.rs:348`
* **Vulnerability Type:** CWE-532: Insertion of Sensitive Information into Log File
* **Exploitability:** The PTY bridge executes interactive CLI tools (like the `gemini` CLI) and monitors outputs for authentication triggers:
  ```rust
  line = stdout_reader.next_line() => {
      match line {
          Ok(Some(line)) => {
              debug!(line = %line, "stdout"); // ◄── LEAKS EVERY STDOUT LINE TO LOGS
  ```
  If the CLI displays a temporary verification code, user password, API key, or authorization bearer token during interactive flows, the line is written directly to the system journal in cleartext.
* **Remediation:** Implement redaction filters inside the log monitoring thread before passing string buffers to the `tracing` macros, or suppress raw `stdout` logging entirely for commands operating on authorization contexts.

---

### [LOW] Cleartext Private Key Storage in Unzeroized Memory
* **Reference:** 
  * `crates/op-llm/src/gemini.rs:52`
  * `crates/op-llm/src/headless_oauth.rs:43`
* **Vulnerability Type:** CWE-316: Cleartext Storage of Sensitive Information in Memory
* **Exploitability:** The `ServiceAccountCredentials` and `OAuthToken` structures store GCP private keys and persistent refresh tokens in standard heap-allocated `String` fields:
  ```rust
  pub struct ServiceAccountCredentials {
      pub private_key: String, // ◄── Held in unzeroized memory
  ```
  Because standard heap `String` allocations do not zero out their memory blocks upon deallocation, these secrets persist in the system's memory pages indefinitely. This allows a local attacker with elevated privileges or a memory dump tool to scrape the process memory and extract Google credentials.
* **Remediation:** Wrap sensitive identity keys in security-focused zeroizing containers such as the `secrecy` crate or `zeroize::Zeroizing<String>`.

---

### [LOW] Concurrent Thundering Herd on Google API Token Endpoints
* **Reference:** `crates/op-llm/src/gemini.rs:253-272`
* **Vulnerability Type:** Concurrency/Performance Defificiency
* **Exploitability:** Under heavy concurrent load, multiple system tasks may access `get_service_account_token` simultaneously. If the local cache is empty or expired, multiple threads will drop their read-locks and proceed to issue redundant out-of-band HTTP requests to the Google token endpoint. This leads to redundant token issuances and can trigger Google-side rate limiting (429 HTTP status).
* **Remediation:** Use a double-checked locking pattern under a write lock or employ a synchronization primitive (like `tokio::sync::OnceCell` or a coalescing lock) to ensure only a single upstream token exchange is performed.