# Architecture & Module Map

### Overview
The `op-llm` crate is a multi-provider LLM (Large Language Model) integration layer that facilitates dynamic model discovery, interactive tool-calling (with forced anti-hallucination constraints), and CLI PTY bridging. It is designed to prioritize enterprise identity mechanisms (like Google Application Default Credentials and local VS Code extension proxies) before falling back to static API keys.

* **Total .rs files**: 16
* **Top-Level Modules**: 13
* **Bin Targets**: None (this is a library crate consumed by the wider workspace, specifically `op-dbus`).

---

### Module Tree
The module tree is defined in `crates/op-llm/src/lib.rs:1` and is structured as follows:

```
op-llm (lib.rs)
 ├── anthropic (anthropic.rs) — Claude API completions & forced tool usage
 ├── antigravity (antigravity.rs) — Headless enterprise OAuth & Gemini proxy
 ├── chat (chat.rs) — Central ChatManager for routing and failover orchestration
 ├── gcloud_adc (gcloud_adc.rs) — Direct Google Cloud ADC helper via gcloud CLI
 ├── gemini (gemini.rs) — Vertex AI and AI Studio Gemini client integration
 ├── gemini_cli (gemini_cli.rs) — PTY-bridged execution of local gemini binaries
 ├── headless_oauth (headless_oauth.rs) — OAuth state synchronization from local VNC sessions
 ├── huggingface (huggingface.rs) — HuggingFace serverless inference API client
 ├── mcp_proxy (mcp_proxy.rs) — Bridge executing op-mcp-proxy in DIRECT_MODE
 ├── openclaw (openclaw.rs) — Local network OpenAI-compatible gateway connector
 ├── perplexity (perplexity.rs) — Perplexity Sonar online-search LLM client
 ├── provider (provider.rs) — Generic trait definitions, message models, and schemas
 └── pty_bridge (pty_bridge.rs) — Interactive process spawning with browser/device-flow pattern matching
```

---

### Entry Points
* **Library Entry Point**: `crates/op-llm/src/lib.rs`
  Re-exports the core LLM clients, providers, and prelude structs for consumption by system control-planes like `op-dbus`.

---

### Notes
* The crate leverages a polymorphic trait architecture `LlmProvider` (defined in `crates/op-llm/src/provider.rs`) allowing `ChatManager` to swap execution backends dynamically at runtime based on environmental availability.
* High-performance serialization is offloaded to `simd-json` via unsafe zero-copy deserialization interfaces where strings are mutated in-place.

---

# Production Security & Quality Findings

## 1. [CRITICAL] Cleartext OAuth Credentials Written to Globally Readable `/tmp` Directory
* **File:Line**: `crates/op-llm/src/headless_oauth.rs:281` (and fallback path configured at `crates/op-llm/src/headless_oauth.rs:351`)
* **Impact**: Local privilege escalation and token hijacking.
* **Description**: 
  The `HeadlessOAuthProvider` serializes and saves highly sensitive OAuth credentials (including cleartext `access_token`, `refresh_token`, `client_id`, and `client_secret` fields) to a local file. When no specific `GOOGLE_AUTH_TOKEN_FILE` is defined, the system defaults to writing this configuration to `/tmp/antigravity-token.json`. 
  
  The file is written using standard asynchronous file writes (`tokio::fs::write`):
  ```rust
  async fn save_token(&self, token: &OAuthToken) -> Result<()> {
      let contents = simd_json::to_string_pretty(token)?;
      tokio::fs::write(&self.token_file, contents).await?;
      Ok(())
  }
  ```
  This creates the file with default system permissions (typically governed by the user's `umask`, resulting in `0644` or `0666` permissions). Because `/tmp` is a shared global space on Linux systems, any unprivileged local user or process can read `/tmp/antigravity-token.json`, extract the enterprise Google Cloud refresh and access tokens, and hijack the user's billing context or GCP project resources.
* **Remediation**: 
  1. Avoid defaulting to `/tmp` for sensitive token storage. Use standard XDG base directory configurations (e.g., `dirs::config_dir` or `dirs::runtime_dir`).
  2. Before writing the token file, verify or explicitly enforce strict POSIX permissions (mode `0600` / `S_IRUSR | S_IWUSR`) on both the containing directory and the output file. In Rust, this can be achieved using `std::os::unix::fs::OpenOptionsExt` to create the file with the correct mode flag:
     ```rust
     use std::os::unix::fs::OpenOptionsExt;
     let mut options = std::fs::OpenOptions::new();
     options.write(true).create(true).truncate(true).mode(0o600);
     ```

---

## 2. [HIGH] Violation of Schema-as-Code Discipline via Ad-Hoc Structs & Untyped JSON Contracts
* **File:Line**: `crates/op-llm/src/provider.rs:114` (and duplicate ad-hoc contracts in `crates/op-llm/src/anthropic.rs:69` and `crates/op-llm/src/gemini.rs:360`)
* **Impact**: Data drift, operational fragility, compliance failures, and serialization panic risks.
* **Description**: 
  The codebase violates the schema-as-code discipline. Rather than enforcing versioned, single-source-of-truth Protocol Buffers or structured OSCAL schemas for machine-to-machine messaging and tool execution, data contracts are defined as ad-hoc Rust structs or untyped `simd_json::OwnedValue` objects. 
  
  For example, `ToolDefinition` is defined as:
  ```rust
  pub struct ToolDefinition {
      pub name: String,
      pub description: String,
      pub input_schema: simd_json::OwnedValue,
      #[serde(default)]
      pub schema_version: String,
      ...
  }
  ```
  By relying on dynamic `simd_json::OwnedValue` shapes (such as `input_schema` and `arguments`), the execution runtime lacks compile-time validation for incoming payloads and API response variations. Upstream changes in remote LLM providers or intermediate bridge proxies can introduce unexpected data shapes that cause runtime parsing failures and operational disruption in the control plane.
* **Remediation**: 
  Transition all dynamic schema structures (`ToolDefinition`, `ToolCallInfo`, `ChatRequest`, and API translation models) to structured, versioned Protocol Buffer contracts compiled via `prost` (as already configured for other modules in `Cargo.toml`). Ensure changes are validated against structured schemas before execution or storage.

---

## 3. [MEDIUM] PATH Hijacking and Command Execution Vulnerability in GCloud Token Fetching
* **File:Line**: `crates/op-llm/src/gcloud_adc.rs:73` (and repeated at `crates/op-llm/src/gcloud_adc.rs:82`)
* **Impact**: Arbitrary command execution with control-plane privileges.
* **Description**: 
  The `GCloudADCProvider` executes the host system's `gcloud` executable to retrieve active access tokens:
  ```rust
  let output = Command::new("gcloud")
      .args(["auth", "print-access-token"])
      .output()
      .context("Failed to execute gcloud auth print-access-token")?;
  ```
  By calling `Command::new("gcloud")` with a relative binary name, the Rust process relies entirely on the system's `PATH` environment variable to resolve the path of the utility. If the `PATH` variable can be manipulated, or if a local user can place a malicious executable named `gcloud` in a writable folder on the search path (such as `/tmp`, `/usr/local/bin`, or user-local binary paths), the execution tracker will invoke the attacker's binary under the user context of the running service (potentially `root` if operating as a native Linux control daemon).
* **Remediation**: 
  Resolve the system executable using a strictly configured, fully qualified absolute path (e.g., `/usr/bin/gcloud` or `/snap/bin/gcloud`), or construct a secure PATH resolver that sanitizes the environment variable prior to executing child processes.

---

## 4. [MEDIUM] Unencrypted Local Network Communication for Trusted Agent Routes
* **File:Line**: `crates/op-llm/src/openclaw.rs:25`
* **Impact**: Information disclosure, local traffic interception, and MITM of tool execution.
* **Description**: 
  The `OpenClawProvider` connects to the local OpenClaw agent platform over an unencrypted HTTP route by default:
  ```rust
  const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8090";
  ```
  While labeled as a "trusted internal network" endpoint, relying on cleartext HTTP means all prompt contents, tool definitions, execution arguments, and session outputs are transmitted unencrypted. On systems with multi-tenant namespaces, containerized overlays, or shared virtual interfaces, attackers on the same segment can inspect or tamper with internal control packets. This exposes sensitive metadata, internal database structures, and dynamic system interaction schemas to network-level interception.
* **Remediation**: 
  Configure the provider to enforce HTTPS by default (e.g. `https://127.0.0.1:8090`), requiring TLS certificate verification even on internal or loopback networks. If HTTP is explicitly required for local diagnostic environments, issue a warning when the target IP resolves outside safe loopback boundaries.

---

## 5. [LOW] Risks Associated with Mutating Zero-Copy Deserialization with Unsafe `simd_json`
* **File:Line**: `crates/op-llm/src/gemini.rs:651` (and repeated in `huggingface.rs:198` and `openclaw.rs:249`)
* **Impact**: Potential undefined behavior or runtime memory corruption under concurrent access.
* **Description**: 
  The codebase uses `simd_json::from_str` wrapped in an `unsafe` block for performance reasons:
  ```rust
  let result: GeminiResponse = match unsafe { simd_json::from_str(&mut raw_body_mut) }
  ```
  `simd_json` parses JSON payloads via in-place mutation of the input buffer. While this approach avoids allocations, it requires absolute assurance that the underlying mutable buffer (`&mut raw_body_mut`) is never aliased, accessed concurrently, or shared across thread boundaries prior to complete garbage collection of the deserialized reference fields. Although no explicit concurrency violation was observed in the isolated providers, using raw `unsafe` bindings to parse untrusted web payloads from upstream LLM models introduces a wider attack surface and increases code maintenance risks.
* **Remediation**: 
  Replace the `unsafe simd_json::from_str` calls with safe interfaces (such as `simd_json::from_slice` or standard `serde_json::from_str` patterns) unless profiling indicates that serialization is a primary bottleneck. If `simd_json` is required, use safe wrapper libraries that guarantee string ownership invariants are not violated during multi-threaded async execution.

---
## ⚠ Citation Warnings
- `crates/op-llm/src/headless_oauth.rs:351`: file has 335 lines
