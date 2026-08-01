### Data Structures Analysis

This section analyzes the use of reference-counting pointers, cell wrappers, synchronization primitives, copying overhead, and oversized structures across the provided files.

#### Metric Counts per File

| File | `Arc` | `Rc` | `RefCell` | `RwLock` | `Mutex` | `OnceCell` / `OnceLock` | `.clone()` Count |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `crates/op-llm/src/anthropic.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 11 |
| `crates/op-llm/src/antigravity.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 2 |
| `crates/op-llm/src/antigravity_replay.rs` | 0 | 0 | 0 | 1 | 0 | 0 | 3 |
| `crates/op-llm/src/gcloud_adc.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 2 |
| `crates/op-llm/src/gemini.rs` | 0 | 0 | 0 | 2 | 0 | 1 | **25** |
| `crates/op-llm/src/gemini_cli.rs` | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| `crates/op-llm/src/headless_oauth.rs` | 0 | 0 | 0 | 1 | 0 | 0 | 6 |
| `crates/op-llm/src/huggingface.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| `crates/op-llm/src/mcp_proxy.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| `crates/op-llm/src/perplexity.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 2 |
| `crates/op-llm/src/pty_bridge.rs` | 2 | 0 | 0 | 2 | 0 | 0 | 8 |
| `crates/op-llm/src/chat.rs` | 1 | 0 | 0 | 1 | 0 | 0 | **21** |
| `crates/op-llm/src/lib.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-llm/src/openclaw.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 5 |
| `crates/op-llm/src/provider.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 3 |

---

### Excessive Clone Activity Flags

Files exceeding the threshold of 20 `.clone()` calls:

#### 1. `crates/op-llm/src/gemini.rs` (25 `.clone()` calls)
*   Excessive duplication of security credentials, JSON schemas, content block arrays, and raw string buffers. Examples:
    *   `creds.client_email.clone()` (Lines 173, 174)
    *   `creds.token_uri.clone()` (Line 175)
    *   `creds.private_key_id.clone()` (Line 184)
    *   `t.input_schema.clone()` (Line 709)
    *   `fc.args.clone()` (Line 819)

#### 2. `crates/op-llm/src/chat.rs` (21 `.clone()` calls)
*   High frequency of read-lock clones on `current_provider` and `current_model` variables during chat requests. This introduces critical lock-acquisition and allocation overheads in fast-path runtime loops. Examples:
    *   `self.current_provider.read().await.clone()` (Lines 222, 282, 301, 343, 355, 365, 381, 421)
    *   `self.current_model.read().await.clone()` (Lines 227, 376, 422)

---

### Large Structs (> 5 Public Fields)

The following public structs expose more than five public fields, violating low-coupling encapsulation principles:

#### 1. `ServiceAccountCredentials` — `crates/op-llm/src/gemini.rs:114`
Exposes 7 public fields:
```rust
pub struct ServiceAccountCredentials {
    pub cred_type: String,
    pub project_id: String,
    pub private_key_id: String,
    pub private_key: String,
    pub client_email: String,
    pub client_id: String,
    pub token_uri: String,
}
```

#### 2. `OAuthToken` — `crates/op-llm/src/headless_oauth.rs:41`
Exposes 11 public fields:
```rust
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: Option<String>,
    pub expires_in: Option<u64>,
    pub expires_at: Option<f64>,
    pub expiry: Option<String>,
    pub scope: Option<String>,
    pub saved_at: Option<f64>,
    pub source: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}
```

#### 3. `AuthRequirement` — `crates/op-llm/src/pty_bridge.rs:56`
Exposes 7 public fields:
```rust
pub struct AuthRequirement {
    pub id: String,
    pub auth_type: AuthType,
    pub url: Option<String>,
    pub device_code: Option<String>,
    pub message: String,
    pub detected_at: i64,
    pub completed: bool,
}
```

#### 4. `ToolDefinition` — `crates/op-llm/src/provider.rs:112`
Exposes 7 public fields:
```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: simd_json::OwnedValue,
    pub schema_version: String,
    pub category: String,
    pub tags: Vec<String>,
    pub namespace: String,
}
```

#### 5. `ChatResponse` — `crates/op-llm/src/provider.rs:219`
Exposes 6 public fields:
```rust
pub struct ChatResponse {
    pub message: ChatMessage,
    pub model: String,
    pub provider: String,
    pub finish_reason: Option<String>,
    pub usage: Option<TokenUsage>,
    pub tool_calls: Option<Vec<ToolCallInfo>>,
}
```

#### 6. `ModelInfo` — `crates/op-llm/src/provider.rs:230`
Exposes 8 public fields:
```rust
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<String>,
    pub available: bool,
    pub tags: Vec<String>,
    pub downloads: Option<u64>,
    pub updated_at: Option<String>,
}
```

---

### Globally Mutable State

The codebase uses a globally accessible lazy/once-initialized state wrapper for caching sensitive authentication data.

*   **`crates/op-llm/src/gemini.rs:81`**
    ```rust
    static TOKEN_CACHE: std::sync::OnceLock<RwLock<Option<CachedToken>>> = std::sync::OnceLock::new();
    ```
    This pattern introduces a globally shared static container containing mutable tokens protected by an `RwLock`. If threads block waiting for writes, it can degrade runtime responsiveness under high concurrency.

---

### Schema-as-Code Compliance

The codebase uses an ad-hoc schema architecture rather than unified, code-generated schemas (e.g., Protocol Buffers or JSON Schemas mapped from OSCAL standard models). Ad-hoc manual schemas lead to structural drifts and security/compatibility failures between microservices.

#### Violations:
*   **`crates/op-llm/src/provider.rs:112`**: `ToolDefinition` defines dynamic JSON schemas using raw `simd_json::OwnedValue` instead of strict, versioned protobuf schemas.
*   **`crates/op-llm/src/provider.rs:125` & `provider.rs:134`**: Structural layouts are manually constructed on the fly for downstream systems (`to_anthropic_format` and `to_openai_format`) via untyped maps rather than validated interfaces.
*   **`crates/op-llm/src/anthropic.rs:90`**: Defines `AnthropicRequest` with dynamic untyped components: `tools: Option<Vec<Value>>` and `tool_choice: Option<Value>`, completely bypassing schema validation.
*   **`crates/op-llm/src/mcp_proxy.rs:60`**: Employs ad-hoc `simd_json::json!` JSON-RPC generation over pipes rather than code-generated gRPC or structural RPC contracts.
*   **`crates/op-llm/src/headless_oauth.rs:41`**: Uses manual serde-derived deserialization for configuration tokens without strict structural version checks.

---

### Security and Quality Issues

#### 1. Insecure Saved Token File Permissions (High Severity)
*   **Location**: `crates/op-llm/src/headless_oauth.rs:224`
*   **Context**:
    ```rust
    async fn save_token(&self, token: &OAuthToken) -> Result<()> {
        let contents = simd_json::to_string_pretty(token)?;
        tokio::fs::write(&self.token_file, contents).await?;
        Ok(())
    }
    ```
*   **Defect**: Sensitive credentials (such as Google OAuth token, refresh token, and client secrets) are saved directly to a file (`~/.config/antigravity/token.json` or `/tmp/antigravity-token.json`) using standard file permissions. On multi-user systems, this file may be created with default `0644` permissions, making sensitive OAuth tokens world-readable.
*   **Remediation**: Set Unix file permissions explicitly to `0600` (read/write only by owner) using `std::os::unix::fs::PermissionsExt` before writing, or write files securely using appropriate system calls.

#### 2. Unsafe Zero-Copy Mutability with `simd-json` (Medium Severity)
*   **Location**: `crates/op-llm/src/gemini.rs:107`, `headless_oauth.rs:198`, `gemini.rs:596`, `openclaw.rs:93`, `huggingface.rs:195`
*   **Context**:
    ```rust
    let mut contents_mut = contents;
    let creds: ServiceAccountCredentials = unsafe { simd_json::from_str(&mut contents_mut) }
    ```
*   **Defect**: The use of `unsafe` with `simd_json::from_str` allows parsing by modifying the source string slice in-place. If the returned structures have their lifetimes tied to the source string buffer but are held past its destruction, it can lead to use-after-free bugs.
*   **Remediation**: Use the safe variant of `simd_json` parsing when parsing parameters that do not have strict performance-critical parsing loops, or explicitly constrain lifetimes to avoid lifetime safety concerns.