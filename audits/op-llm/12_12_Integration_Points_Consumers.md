# Integration & Quality Audit Report: `op-llm`

## 1. Crates Depending on `op-llm`

Based on the workspace configuration and the lockfile, the following crates within the workspace depend on `op-llm`:

*   **`op-chat`**: Declared as a dependency in `Cargo.lock` under the `[[package]]` entry for `op-chat` (line 1205).
*   **`op-web`**: Declared as a dependency in `Cargo.lock` under the `[[package]]` entry for `op-web` (line 1726).

Additionally, `op-llm` is declared as a workspace-level dependency in the root `Cargo.toml` (line 30).

---

## 2. D-Bus Service Names and Object Paths Registered

Based on the provided source code, **no D-Bus service names or object paths are registered by the `op-llm` crate.** 

*   `op-llm` is purely a client library for various LLM backends and lacks any direct dependency on D-Bus client libraries like `zbus` in its crate-level configuration (`crates/op-llm/Cargo.toml`).
*   While comments in `crates/op-llm/src/pty_bridge.rs` (line 21) mention emitting signals (e.g. via D-Bus) when authentication is required, the actual implementation of D-Bus interfaces and orchestration resides in consuming crates (such as `op-dbus`).

---

## 3. HTTP/gRPC Endpoints Exposed

### Exposed Endpoints (Servers)
In production, `op-llm` **does not expose any HTTP or gRPC server endpoints**. 

However, a mock HTTP server is spun up during testing to simulate the OpenClaw API:
*   `spawn_test_server` in `crates/op-llm/src/openclaw.rs` (lines 680–725) binds to `127.0.0.1:0` to intercept and mock `/v1/models` and `/v1/chat/completions` routes.

### Consumed Endpoints (Clients)
The crate acts as an HTTP client to the following external and internal API endpoints:

| Service / Provider | Endpoint URL / Path | Purpose | Citation |
| :--- | :--- | :--- | :--- |
| **Anthropic API** | `https://api.anthropic.com/v1/messages` | Chat completions | `crates/op-llm/src/anthropic.rs:36-37` |
| **Gemini / Antigravity API** | `https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent` | Token-based content generation | `crates/op-llm/src/antigravity.rs:43`, `crates/op-llm/src/antigravity_replay.rs:35`, `crates/op-llm/src/gemini.rs:58` |
| **Google Cloud AI Companion** | `{GCP_BASE_URL}/projects/{GCP_PROJECT}/locations/{GCP_LOCATION}/publishers/google/models/{model}:generateContent` | Enterprise subscription Chat API | `crates/op-llm/src/gcloud_adc.rs:24-25` |
| **HuggingFace Inference API** | `https://api-inference.huggingface.co/models/{model}/v1/chat/completions` | Open-source LLM chat completions | `crates/op-llm/src/huggingface.rs:21`, `crates/op-llm/src/huggingface.rs:105-106` |
| **OpenClaw Agent Platform** | `{base_url}/v1/chat/completions` and `{base_url}/v1/models` | Local / trusted network agent routing | `crates/op-llm/src/openclaw.rs:21-22` |
| **Perplexity Search API** | `https://api.perplexity.ai/chat/completions` | Online search chat completions | `crates/op-llm/src/perplexity.rs:39-40` |
| **Google OAuth2** | `https://oauth2.googleapis.com/token` | Token exchanges and refresh operations | `crates/op-llm/src/gemini.rs:55`, `crates/op-llm/src/headless_oauth.rs:22` |

---

## 4. Cross-Crate Circular Dependency Risk Assessment

There is **no circular dependency risk** introduced by the `op-llm` crate. 

*   As defined in `crates/op-llm/Cargo.toml` (lines 10–25), `op-llm` depends exclusively on external third-party crates (such as `tokio`, `serde`, `simd-json`, `reqwest`, etc.). 
*   It does not declare any dependencies on other internal workspace crates (such as `op-core`, `op-web`, `op-chat`, or `op-mcp`).
*   Consequently, `op-llm` sits at the bottom of the internal dependency hierarchy alongside other core utilities, acting as a leaf node.

---

## 5. Schema-as-Code Compliance Flags

The workspace strictly prescribes a *schema-as-code* discipline using versioned Protocol Buffers or OSCAL schemas. However, `op-llm` relies entirely on **ad-hoc Rust structs, raw strings, and dynamically-constructed JSON objects** to define internal data contracts. This introduces severe risk of silent breaking changes during workspace upgrades.

### Ad-Hoc Data Contracts
The core interface contracts between the control plane and LLM providers are represented as unversioned Rust structs:
*   `ChatMessage`, `ToolCallInfo`, `ToolDefinition`, `ChatRequest`, `ChatResponse`, and `ModelInfo` in `crates/op-llm/src/provider.rs` (lines 59–250).

### Untyped JSON Schemas
Tool call definitions utilize dynamically-typed `simd_json::OwnedValue` instead of structured schema contracts:
*   `arguments` in `ToolCallInfo` and `input_schema` in `ToolDefinition` (`crates/op-llm/src/provider.rs:90`, `crates/op-llm/src/provider.rs:98`).

### Ad-Hoc API Integration Mappings
Each client module implements its own custom serialization mapping to translate payloads into provider-specific shapes:
*   **Anthropic**: `AnthropicRequest`, `AnthropicMessage`, `AnthropicContent`, `ContentBlock` in `crates/op-llm/src/anthropic.rs` (lines 56–116).
*   **Gemini / Google AI**: `GeminiRequest`, `GeminiTool`, `GeminiFunctionDeclaration`, `GeminiToolConfig`, `GeminiContent` in `crates/op-llm/src/gemini.rs` (lines 182–277).
*   **Perplexity**: `PerplexityRequest`, `PerplexityMessage`, `PerplexityResponse` in `crates/op-llm/src/perplexity.rs` (lines 72–106).
*   **Antigravity Session Recording**: `CapturedSession`, `CapturedToken`, `CapturedEndpoint` in `crates/op-llm/src/antigravity_replay.rs" (lines 40–75).

### Inline Dynamic JSON Construction
In-place construction of payloads bypasses structural types completely:
*   `crates/op-llm/src/antigravity.rs` (lines 207–285) constructs nested JSON blocks dynamically for Google's API using the `json!` macro.
*   `crates/op-llm/src/gcloud_adc.rs` (lines 163–199) uses a dynamic `HashMap` and inline `json!` structures to populate request payloads.

---

## 6. Security and Quality Findings

### Finding 1: Arbitrary Binary Execution / Privilege Escalation via Environment Variable
*   **Severity**: High (Potential Critical if daemon is run as root)
*   **Location**: `crates/op-llm/src/mcp_proxy.rs` (lines 20–21, 45)
*   **Description**: The `McpProxyProvider` obtains its executable target path directly from the `OP_MCP_PROXY_BIN` environment variable without any validation or sanitization:
    ```rust
    let bin = std::env::var("OP_MCP_PROXY_BIN").unwrap_or_else(|_| "op-mcp-proxy".to_string());
    ```
    Later, it spawns this binary directly:
    ```rust
    let mut child = cmd.spawn().with_context(...)
    ```
*   **Impact**: If the control plane (e.g. `op-dbus`) runs in a privileged context or as a system service, any local attacker who can influence the environment variables of the calling process can inject an arbitrary binary path. When the proxy is triggered, it will execute the attacker's binary under the daemon's host security context, leading to Local Privilege Escalation (LPE).

### Finding 2: Insecure File Permissions on Saved Plaintext OAuth Tokens
*   **Severity**: High
*   **Location**: `crates/op-llm/src/headless_oauth.rs` (lines 260–264)
*   **Description**: When refreshing OAuth tokens, `HeadlessOAuthProvider` serializes the credential state (which includes sensitive `access_token`, `refresh_token`, `client_id`, and `client_secret` fields) and writes it back to disk:
    ```rust
    async fn save_token(&self, token: &OAuthToken) -> Result<()> {
        let contents = simd_json::to_string_pretty(token)?;
        tokio::fs::write(&self.token_file, contents).await?;
        Ok(())
    }
    ```
*   **Impact**: `tokio::fs::write` creates or overwrites files with default system permissions (respecting only the active shell `umask`, which is often `0644` or `0622` on standard environments). This makes highly sensitive long-lived refresh tokens and API secrets readable by other local unprivileged system users, allowing credential theft and lateral host movement.

### Finding 3: Memory Safety and Crash Risks via Unpadded `unsafe simd_json::from_str`
*   **Severity**: High
*   **Location**: `crates/op-llm/src/openclaw.rs` (lines 126–127, 252), `crates/op-llm/src/huggingface.rs" (line 141), `crates/op-llm/src/gemini.rs` (lines 114, 136, 163, 511), `crates/op-llm/src/headless_oauth.rs` (line 252), `crates/op-llm/src/gemini_cli.rs` (line 242).
*   **Description**: Throughout `op-llm`, HTTP responses and credentials files are parsed using `unsafe { simd_json::from_str(...) }`. For example:
    ```rust
    let mut response_text_mut = response_text;
    let response_json: Value = unsafe { simd_json::from_str(&mut response_text_mut) }
    ```
    The `simd_json` parser operates on 128-bit or 256-bit SIMD registers and requires that the input string buffer is mutable and *must* have `simd_json::PADDING` bytes (typically 32 bytes) of allocated padding beyond the logical length of the string to avoid out-of-bounds over-reads. Standard `String` allocations returned by `reqwest::Response::text()` do not guarantee this padding.
*   **Impact**: If a remote backend (e.g. HuggingFace or a hijacked OpenClaw node) returns a JSON payload that terminates near an allocation boundary, `simd-json`'s SIMD registers will over-read past the buffer boundary. This can result in:
    1.  Memory disclosure of adjacent heap contents.
    2.  Immediate segmentation faults (SIGSEGV), causing Denial of Service (DoS) of the system control plane.

### Finding 4: Hardcoded Google Cloud Project ID Fallback
*   **Severity**: Medium (Information Leakage)
*   **Location**: `crates/op-llm/src/gcloud_adc.rs` (line 29)
*   **Description**: In the absence of an overriding `GCP_PROJECT` or `GOOGLE_CLOUD_PROJECT` environment variable, the GCloud ADC provider falls back to a hardcoded Google Cloud Project ID:
    ```rust
    fn project_id() -> String {
        std::env::var("GCP_PROJECT")
            .or_else(|_| std::env::var("GOOGLE_CLOUD_PROJECT"))
            .unwrap_or_else(|_| "geminidev-479406".to_string())
    }
    ```
*   **Impact**: The leakage of specific internal infrastructure names ("geminidev-479406") exposes the underlying GCP account naming conventions and setup. This facilitates reconnaissance and targeted scanning by malicious actors trying to discover and compromise internal cloud services.