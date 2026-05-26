| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `ChatMessage` | Rust Struct | `provider.rs:60` | No | Data contract for chat messages is expressed as an ad-hoc Rust struct rather than a versioned Protobuf schema. |
| `ChatRequest` | Rust Struct | `provider.rs:194` | No | Chat configuration payload is declared as a native struct with untyped JSON fields, violating schema-as-code principles. |
| `ChatResponse` | Rust Struct | `provider.rs:253` | No | Response metadata payload is declared as a native struct, creating ad-hoc API boundaries. |
| `ToolDefinition` | Rust Struct | `provider.rs:133` | No | Uses untyped `simd_json::OwnedValue` for `input_schema` instead of a strongly-typed, schema-validated payload definition. |
| `ToolCallInfo` | Rust Struct | `provider.rs:125` | No | Uses an untyped JSON representation (`simd_json::OwnedValue`) for dynamic tool invocation arguments. |
| `ModelInfo` | Rust Struct | `provider.rs:264` | No | Dynamic model capabilities and metadata are serialized manually as ad-hoc fields instead of a versioned schema. |
| `AnthropicRequest` | Rust Struct | `anthropic.rs:70` | No | Private serialization payload mapping to Anthropic’s proprietary REST API; lacks a formal Protobuf schema representation. |
| `AnthropicResponse` | Rust Struct | `anthropic.rs:118` | No | Private deserialization structure matching Anthropic's message output. |
| `GeminiRequest` | Rust Struct | `gemini.rs:271` | No | Manual JSON payload representation for Google Gemini API completions. |
| `GeminiResponse` | Rust Struct | `gemini.rs:328` | No | Deserialization struct for raw JSON responses from the Gemini endpoint. |
| `CapturedSession` | Rust Struct | `antigravity_replay.rs:43` | No | Session replay structure that records and serializes raw HTTP headers and tokens. |

## OSCAL Control Coverage

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **Access Control (AC-2 / AC-3)**<br>OAuth Token & Session Extraction | `headless_oauth.rs:127`<br>`antigravity_replay.rs:90` | None | Headless VNC-based OAuth token harvesting and local credential replay mechanism lack documentation in an OSCAL `component-definition` or System Security Plan (SSP). |
| **Cryptographic Storage (SC-28)**<br>Plaintext Key and Token Storage | `headless_oauth.rs:228`<br>`headless_oauth.rs:159` | None | Storing plain OAuth access and refresh tokens inside unencrypted user-directory config files (`~/.config/antigravity/token.json`) is not mapped to any compliance control. |
| **System & Comm Protection (SC-8)**<br>Hardcoded Network Client Timeouts | `openclaw.rs:35`<br>`huggingface.rs:35` | None | Hardcoded transport timeout constraints (120s/180s) on HTTP clients lack formal specification in machine-readable compliance profiles. |
| **Information Flow Enforcement (AC-4)**<br>Anti-Hallucination Guardrails | `provider.rs:164` | None | Enforcing model tool execution via `ToolChoice::Required` operates as an implicit system safety safeguard with no OSCAL mapping. |

---

## Detailed Findings & Recommendations

### [CRITICAL] Memory Safety Vulnerability (CWE-125) via Unpadded `simd_json::from_str` on Remote Input

#### VULNERABILITY EXPLANATION
Multiple modules parse unvalidated HTTP responses from external endpoints using the unsafe fast-path of `simd-json`:
* `gemini.rs:556` and `gemini.rs:743` (Google Gemini API responses)
* `huggingface.rs:228` (HuggingFace API responses)
* `openclaw.rs:112`, `openclaw.rs:258`, and `openclaw.rs:290` (OpenClaw model API and chat completions)

`simd_json::from_str` is fundamentally `unsafe` because it mutates the underlying string buffer in-place to parse JSON tokens rapidly. Crucially, the SIMD parsing instructions process memory in 32-byte chunks and **strictly require the input slice buffer to have trailing padding bytes** (`simd_json::PADDING`). 

The string slices parsed in these files are obtained directly from `reqwest::Response::text()` buffers, which are *unpadded*. When parsing the end of the JSON payload, `simd-json` will perform out-of-bounds reads up to 32 bytes past the heap allocation limit. This leads to unpredictable memory disclosure or immediate process segmentation faults (Denial of Service). Since these inputs are returned by external systems (including models that can output arbitrary token lengths), this is directly exploitable to crash the control plane.

#### REMEDIATION
1. Convert the input to a padded vector of bytes using `simd_json::to_padded_bin` or manually extend a `Vec<u8>` before parsing.
2. Alternatively, switch to safe parsing via `simd_json::from_slice` after ensuring padding, or use standard `serde_json::from_str` for network-sourced strings.

```rust
// REFACTOR (openclaw.rs:258): Safe parsing with SIMD-aligned padding
let response_text = response.text().await?;
let mut padded_bytes = response_text.into_bytes();
padded_bytes.resize(padded_bytes.len() + simd_json::PADDING, 0);

let response_json: Value = simd_json::from_slice(&mut padded_bytes)
    .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;
```

---

### [MAJOR] Execution of Unvalidated Binaries via Implicit `PATH` Search (CWE-427)

#### VULNERABILITY EXPLANATION
In `gcloud_adc.rs:83` and `gcloud_adc.rs:93`, the provider executes system commands using standard relative binary invocation:
```rust
let output = Command::new("gcloud")
    .args(["auth", "print-access-token"])
```
Similarly, `mcp_proxy.rs:29` evaluates local execution of `op-mcp-proxy` by trusting the environment `PATH` implicitly when the binary name is relative.

If the calling environment's `PATH` variable is manipulated (either by an unprivileged daemon, a modified user profile, or during container orchestration), an attacker can plant a compromised `gcloud` or `op-mcp-proxy` binary inside a user-writable directory (e.g., `/tmp` or `~/.local/bin`), achieving arbitrary privilege escalation or unauthorized system compromise when the daemon initiates an LLM check.

#### REMEDIATION
1. Mandate the use of absolute paths for system tools via configuration or compile-time environments.
2. Clean and control environment variables prior to subprocess execution.

```rust
// REFACTOR (gcloud_adc.rs:83): Restrict resolution to verified paths
let gcloud_path = std::env::var("OP_GCLOUD_BIN")
    .unwrap_or_else(|_| "/usr/bin/gcloud".to_string());

let output = Command::new(&gcloud_path)
    .env_clear() // Drop inherited, potentially contaminated environment variables
    .args(["auth", "print-access-token"])
    .output()?;
```

---

### [MAJOR] Plaintext Local Storage of Sensitive Google OAuth Refresh Tokens (CWE-312)

#### VULNERABILITY EXPLANATION
In `headless_oauth.rs:228` and `headless_oauth.rs:159`, the headless token manager loads and serializes Google Cloud Platform access and refresh tokens directly to the disk in plaintext inside `~/.config/antigravity/token.json`:
```rust
async fn save_token(&self, token: &OAuthToken) -> Result<()> {
    let contents = simd_json::to_string_pretty(token)?;
    tokio::fs::write(&self.token_file, contents).await?;
```
Refresh tokens permit long-term authentication to cloud environments without user interaction. Storing these credentials in plaintext without encryption at rest violates NIST 800-53 (SC-28) and FedRAMP high-impact control baselines. Any local user, malicious process, or unauthorized backup reader with access to the home directory can permanently compromise the Google Cloud resource tenancy.

#### REMEDIATION
1. Leverage the system's native keyring interface (via DBus Secret Service or OS Keychain) using the `keyring` crate already present in `Cargo.toml`.
2. If direct file storage is required, encrypt the token payloads with AES-GCM (using `aes-gcm` from the workspace) with a key derived from system-level hardware or the security keyring.