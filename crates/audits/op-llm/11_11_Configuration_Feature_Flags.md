# Security and Quality Audit: Configuration and Schema-as-Code

This audit documents environment configuration patterns, Cargo feature compositions, hardcoded infrastructure elements, and data contract compliance with the Schema-as-Code discipline across the `op-llm` codebase.

---

## 1. List of all `std::env::var` Reads

The following table catalogs every read from `std::env::var` within the designated files:

| File | Line | Environment Variable | Purpose / Usage |
| :--- | :--- | :--- | :--- |
| `crates/op-llm/src/anthropic.rs` | 150 | `ANTHROPIC_API_KEY` | Retrieves the API key for Anthropic client initialization. |
| `crates/op-llm/src/antigravity.rs` | 92 | `ANTIGRAVITY_BRIDGE_URL` | Inspects whether a local Antigravity proxy bridge is configured. |
| `crates/op-llm/src/antigravity.rs` | 95 | `LLM_MODEL` | Fetches the default model when bridge URL is active. |
| `crates/op-llm/src/antigravity.rs` | 115 | `GEMINI_API_KEY` | Fetches direct API key when headless OAuth is missing/invalid. |
| `crates/op-llm/src/antigravity.rs` | 128 | `GEMINI_API_KEY` | Falls back to checking Gemini API key outside OAuth workflow. |
| `crates/op-llm/src/antigravity.rs` | 137 | `LLM_MODEL` | Resolves the default Gemini model name. |
| `crates/op-llm/src/antigravity_replay.rs` | 124 | `ANTIGRAVITY_SESSION_FILE` | Retrieves the path to the captured replay session JSON. |
| `crates/op-llm/src/antigravity_replay.rs` | 144 | `ANTIGRAVITY_MODEL` | Resolves default model configuration for session replay. |
| `crates/op-llm/src/antigravity_replay.rs` | 146 | `ANTIGRAVITY_AUTO_ROUTING` | Controls automatic reasoning model routing preference. |
| `crates/op-llm/src/gcloud_adc.rs` | 24 | `GCP_BASE_URL` | Configures Cloud AI Companion base endpoint. |
| `crates/op-llm/src/gcloud_adc.rs` | 29 | `GCP_PROJECT` | Reads the active Google Cloud project ID. |
| `crates/op-llm/src/gcloud_adc.rs` | 30 | `GOOGLE_CLOUD_PROJECT` | Secondary project variable mapping for ADC. |
| `crates/op-llm/src/gcloud_adc.rs` | 34 | `GCP_LOCATION` | Dictates target GCP region. |
| `crates/op-llm/src/gcloud_adc.rs` | 38 | `OP_ENABLE_ADC_FALLBACK` | Controls fallback behavior to metadata server/GCP credentials. |
| `crates/op-llm/src/gcloud_adc.rs` | 57 | `LLM_MODEL` | Defaults the GCP Gemini deployment model name. |
| `crates/op-llm/src/gcloud_adc.rs` | 64 | `GCLOUD_TOKEN` | Direct token injection bypasses gcloud executable execution. |
| `crates/op-llm/src/gemini.rs` | 115 | `GOOGLE_APPLICATION_CREDENTIALS` | Inspects location of the Service Account key file. |
| `crates/op-llm/src/gemini.rs` | 123 | `HOME` | Determines gcloud configurations location. |
| `crates/op-llm/src/gemini.rs` | 150 | `HOME` | Resolves standard path for application defaults. |
| `crates/op-llm/src/gemini.rs` | 534 | `GOOGLE_GENAI_USE_VERTEXAI` | Toggles client mode between AI Studio (API) and Vertex AI (OAuth). |
| `crates/op-llm/src/gemini.rs` | 538 | `GOOGLE_CLOUD_LOCATION` | Selects Vertex AI geographic region. |
| `crates/op-llm/src/gemini.rs` | 549 | `GOOGLE_CLOUD_PROJECT` | Vertex AI destination billing project ID. |
| `crates/op-llm/src/gemini.rs` | 564 | `GEMINI_API_KEY` | Studio primary API token key. |
| `crates/op-llm/src/gemini.rs` | 565 | `GOOGLE_API_KEY` | Studio backup API token key. |
| `crates/op-llm/src/gemini_cli.rs` | 50 | `GOOGLE_APPLICATION_CREDENTIALS` | Sets credentials path context during external process execution. |
| `crates/op-llm/src/gemini_cli.rs` | 74 | `GOOGLE_CLOUD_PROJECT` | Passes project ID down to process environment. |
| `crates/op-llm/src/headless_oauth.rs` | 113 | `GOOGLE_AUTH_TOKEN_FILE` | Determines path to the captured headless token. |
| `crates/op-llm/src/headless_oauth.rs` | 131 | `GOOGLE_CLIENT_ID` | Direct OAuth Client ID override. |
| `crates/op-llm/src/headless_oauth.rs` | 132 | `GOOGLE_CLIENT_SECRET` | Direct OAuth Client Secret override. |
| `crates/op-llm/src/huggingface.rs` | 51 | `HF_TOKEN` | Retrieves target HuggingFace bearer authentication token. |
| `crates/op-llm/src/huggingface.rs` | 52 | `HUGGINGFACE_TOKEN` | Backup HuggingFace key lookup. |
| `crates/op-llm/src/mcp_proxy.rs` | 19 | `OP_MCP_PROXY_BIN` | Configures targeted path of MCP proxy execution agent. |
| `crates/op-llm/src/perplexity.rs` | 118 | `PERPLEXITY_API_KEY` | Retrieves Bearer token for Perplexity search API. |
| `crates/op-llm/src/pty_bridge.rs` | 207 | `GOOGLE_APPLICATION_CREDENTIALS` | Forwards key credentials through generated pseudo-terminals. |
| `crates/op-llm/src/pty_bridge.rs` | 227 | `GOOGLE_CLOUD_PROJECT` | Forwards workspace billing scope inside the PTY shell. |
| `crates/op-llm/src/pty_bridge.rs` | 229 | `GCLOUD_PROJECT` | Alternate key for billing project forwarding in PTY. |
| `crates/op-llm/src/chat.rs` | 47 | `OPENCLAW_DEFAULT_MODEL` | Sets standard model default. |
| `crates/op-llm/src/chat.rs` | 51 | `LLM_PROVIDER` | Decides active platform selection within the ChatManager factory. |
| `crates/op-llm/src/chat.rs` | 52 | `LLM_MODEL` | Explicit global override for target LLM model. |
| `crates/op-llm/src/chat.rs` | 68 | `ENABLE_MCP_PROXY_PROVIDER` | Globally registers or isolates the MCP Proxy provider. |
| `crates/op-llm/src/chat.rs` | 92 | `ENABLE_GEMINI_CLI_PROVIDER` | Globally registers or isolates the local Gemini CLI wrapper. |
| `crates/op-llm/src/chat.rs` | 124 | `GEMINI_API_KEY` | Activates direct Gemini provider logic dynamically. |
| `crates/op-llm/src/chat.rs` | 142 | `OPENCLAW_BASE_URL` | Activates direct internal network OpenClaw provider logic. |
| `crates/op-llm/src/chat.rs` | 143 | `OPENCLAW_DEFAULT_MODEL` | Standard OpenClaw routing model configuration. |
| `crates/op-llm/src/chat.rs` | 161 | `ANTHROPIC_API_KEY` | Standardizes dynamic runtime registration for Anthropic. |
| `crates/op-llm/src/openclaw.rs` | 49 | `OPENCLAW_BASE_URL` | Overrides base location of internal agent system. |
| `crates/op-llm/src/openclaw.rs` | 50 | `OPENCLAW_DEFAULT_MODEL` | Explicit default routing target on OpenClaw clusters. |

---

## 2. Environment Variables with No Default and No Error Handling

The codebase does not contain unsafe `std::env::var().unwrap()` panic-prone calls. It utilizes defensive patterns, wrapping environmental lookups in `if let Ok()`, `.ok()`, or returning propagated `Result` objects wrapped with `.context()` from the `anyhow` crate.

However, several variables are **flagged for weak fallback handling**. If they are missing, initialization functions will instantly propagate errors causing severe, unhandled application aborts or setup failures for that specific provider module:

### Flagged: Core Environmental Failures

*   **`HOME`**
    *   **Citations**: `crates/op-llm/src/gemini.rs:123`, `crates/op-llm/src/gemini.rs:150`
    *   **Risk**: The system attempts to read `std::env::var("HOME")` to construct critical fallback credential path locations (`~/.config/gcloud/...`). If missing, it executes `context("HOME not set")?` which throws a hard error. No default location is defined (such as checking `/etc/gcloud` or a fallback variable).
*   **`ANTHROPIC_API_KEY`**
    *   **Citations**: `crates/op-llm/src/anthropic.rs:150`
    *   **Risk**: Initialization fails directly with a terminal error. The client relies entirely on this key and does not fall back to headless key management systems like Keyring.
*   **`PERPLEXITY_API_KEY`**
    *   **Citations**: `crates/op-llm/src/perplexity.rs:118`
    *   **Risk**: Directly terminates the initialization stream for this provider if called from environment constructor. No local secret extraction is provided.
*   **`HF_TOKEN` / `HUGGINGFACE_TOKEN`**
    *   **Citations**: `crates/op-llm/src/huggingface.rs:51-52`
    *   **Risk**: Direct failure via `.context()` chain. If neither variable is populated, loading the HuggingFace engine aborts immediately.
*   **`GOOGLE_CLOUD_PROJECT`**
    *   **Citations**: `crates/op-llm/src/gemini.rs:549`
    *   **Risk**: If using OAuth Vertex AI mode, missing this variable prevents compilation of billing target metadata, raising a hard error without querying defaults via gcloud CLI directly in this logic path.

---

## 3. Cargo Features and Additive Check

According to `Cargo.toml`:

```toml
[package]
name = "op-dbus"

[features]
default = ["grpc"]
grpc = []
```

### Analysis of Additiveness
The workspace-level and package-level features defined are **strictly additive**. 
*   **Default Feature**: `default = ["grpc"]` binds package-level behavior to activating the `grpc` dependency tree.
*   **Non-Additive Risk Assessment**: The feature flags do not redefine structural behavior or override crate types in a mutually exclusive manner. 
*   **`op-llm` Features**: No features are explicitly declared inside `crates/op-llm/Cargo.toml`. This isolates LLM providers from conditional compilation flags, ensuring uniform availability of all endpoints at runtime.

---

## 4. Hardcoded Paths, Ports, and Addresses

Several critical components utilize hardcoded targets, local addresses, ports, and configuration directories, posing architectural risk when deploying in strictly standardized or sandboxed environments:

### Localhost / Port Mapping
*   **`crates/op-llm/src/openclaw.rs:25`**
    *   `const DEFAULT_BASE_URL: &str = "http://127.0.0.1:18789";`
    *   **Risk**: Points by default to a specific local loopback port. If the cluster runs OpenClaw on an alternate port or containerized network boundary, requests will fail silently without active base URL variable specification.
*   **`crates/op-llm/src/antigravity.rs:104`**
    *   `"1. Connect to Antigravity Bridge: export ANTIGRAVITY_BRIDGE_URL=http://127.0.0.1:7788"`
    *   **Risk**: Recommends port `7788` for developer-facing bridge emulation, cementing specific routing ranges in documentation and runtime error messages.

### Hardcoded GCP Project Identifiers
*   **`crates/op-llm/src/gcloud_adc.rs:30`**
    *   `"geminidev-479406"`
    *   **Risk**: **High Security Concern.** If `GCP_PROJECT` and `GOOGLE_CLOUD_PROJECT` are unpopulated, GCloud ADC falls back silently to billing requests against this hardcoded private sandbox project. In production environments, this can cause billing issues or trace leakages if authorization policies resolve successfully against standard credentials.

### Hardcoded Configuration Files & Local Paths
*   **`crates/op-llm/src/headless_oauth.rs:354`**
    *   `"/tmp/antigravity-token.json"`
    *   **Risk**: If standard home directory pathways fail, the headless system falls back to using public shared memory `/tmp` directory, introducing possibilities for symbolic-link hijacking or local token read exploits by malicious local actors.
*   **`crates/op-llm/src/gemini.rs:151`** & **`crates/op-llm/src/gemini_cli.rs:63`** & **`crates/op-llm/src/pty_bridge.rs:219`**
    *   `"/.config/gcloud/application_default_credentials.json"`
    *   **Risk**: Enforces strict reliance on standard Google Cloud SDK installation layouts.
*   **`crates/op-llm/src/gemini_cli.rs:54`** & **`crates/op-llm/src/pty_bridge.rs:212`**
    *   `"/.config/gcloud/gemini-cli.json"`
    *   **Risk**: Hardcoded specialized service account configuration files.
*   **`crates/op-llm/src/headless_oauth.rs:119`**
    *   `"antigravity/token.json"`
    *   **Risk**: Hardcoded configuration directory lookup relative to standard directory structures.
*   **`crates/op-llm/src/pty_bridge.rs:117`**
    *   `"pty-auth-bridge/sessions"`
    *   **Risk**: Dictates session capture output files path directly.

### External Endpoints & Auth URIs
*   **`crates/op-llm/src/anthropic.rs:20`**: `https://api.anthropic.com/v1`
*   **`crates/op-llm/src/antigravity.rs:38`**: `https://generativelanguage.googleapis.com/v1beta`
*   **`crates/op-llm/src/antigravity_replay.rs:37`**: `https://generativelanguage.googleapis.com/v1beta`
*   **`crates/op-llm/src/gcloud_adc.rs:25`**: `https://cloudaicompanion.googleapis.com/v1`
*   **`crates/op-llm/src/gemini.rs:53`**: `https://generativelanguage.googleapis.com/v1beta`
*   **`crates/op-llm/src/gemini.rs:56`**: `https://oauth2.googleapis.com/token`
*   **`crates/op-llm/src/headless_oauth.rs:25`**: `https://oauth2.googleapis.com/token`
*   **`crates/op-llm/src/headless_oauth.rs:27`**: `https://www.googleapis.com/oauth2/v2/userinfo`
*   **`crates/op-llm/src/huggingface.rs:31`**: `https://api-inference.huggingface.co`
*   **`crates/op-llm/src/perplexity.rs:53`**: `https://api.perplexity.ai`
*   **`crates/op-llm/src/pty_bridge.rs:24-27`**: 
    *   `https://accounts.google.com`
    *   `https://github.com/login/device`
    *   `https://login.microsoftonline.com`
    *   `https://oauth.example.com`

---

## 5. Schema-as-Code Violations

The codebase demonstrates significant deviation from the strict **Schema-as-Code** discipline. Instead of defining request/response structures, authentication data, and chat states using versioned, uniform schema files (e.g., Protocol Buffers via `Prost` or standardized OSCAL models), **data contracts are expressed as ad-hoc, handwritten Serde Rust structs**.

While the workspace integrates Protobuf builders (`prost-build` / `tonic-build`) inside companion crates (e.g., `op-cache`, `op-grpc-bridge`), `op-llm` manually manages serialization boundaries.

### List of Ad-Hoc Data Contracts

#### 1. Chat Core Abstraction Models
*   **File**: `crates/op-llm/src/provider.rs`
*   **Structs**:
    *   `ChatMessage` (Lines 46-53): Handcrafted model storing role, content, tool_calls, and execution states.
    *   `ToolCallInfo` (Lines 94-98): Ad-hoc format using raw `simd_json::OwnedValue` mapping.
    *   `ToolDefinition` (Lines 101-111): Manages ad-hoc schema conversions (`to_anthropic_format`, `to_openai_format`) dynamically at runtime using unstructured JSON structures.
    *   `TokenUsage` (Lines 188-192): Standard metrics defined inside the source.
    *   `ChatResponse` (Lines 202-209): Hand-written wrapper for returned assistant payloads.
    *   `ModelInfo` (Lines 212-222): Dynamic discovery details mapped using primitive typing.

#### 2. Anthropic API Wire Models
*   **File**: `crates/op-llm/src/anthropic.rs`
*   **Structs**: `AnthropicRequest`, `AnthropicMessage`, `AnthropicContent`, `ContentBlock`, `AnthropicResponse`, `ResponseContentBlock`, `AnthropicUsage` (Lines 66-130).
*   **Deviation**: Manages proprietary tag mapping, skipping, and inline text vectorization using custom untagged Serde representations instead of structured API specifications.

#### 3. Google Gemini Wire Models
*   **File**: `crates/op-llm/src/gemini.rs`
*   **Structs**: `GeminiRequest`, `GeminiTool`, `GeminiFunctionDeclaration`, `GeminiToolConfig`, `FunctionCallingConfig`, `GeminiContent`, `GeminiPart`, `GenerationConfig`, `RoutingConfig`, `AutoRoutingMode`, `GeminiResponse`, `GeminiCandidate`, `GeminiContentResponse`, `GeminiPartResponse`, `GeminiFunctionCall`, `UsageMetadata` (Lines 290-385).
*   **Deviation**: Re-creates extensive hierarchical object trees for Google GenAI compatibility. Updates are brittle and must be manually tracked directly within the Rust implementation code.

#### 4. Headless Replay and Credential Storage Models
*   **File**: `crates/op-llm/src/antigravity_replay.rs`
*   **Structs**: `CapturedSession`, `CapturedToken`, `CapturedEndpoint` (Lines 44-71).
*   **Deviation**: Custom JSON representation mapping recorded IDE requests, endpoint metadata, and authorization payloads. Lack of structured version control makes stored capture files prone to parsing breakage upon minor IDE wire updates.

#### 5. Headless OAuth Token Definitions
*   **File**: `crates/op-llm/src/headless_oauth.rs`
*   **Structs**: `OAuthToken` (Lines 34-55).
*   **Deviation**: Expresses the underlying authentication object using ad-hoc floats and options, lacking uniform structural validation schemas.

#### 6. Perplexity Wire Models
*   **File**: `crates/op-llm/src/perplexity.rs`
*   **Structs**: `PerplexityRequest`, `PerplexityMessage`, `PerplexityResponse`, `PerplexityChoice`, `PerplexityUsage` (Lines 66-96).
*   **Deviation**: Standardizes on custom ad-hoc representation structures for Perplexity REST operations.

#### 7. Interactive PTY Auth Models
*   **File**: `crates/op-llm/src/pty_bridge.rs`
*   **Structs**: `AuthRequirement`, `AuthType`, `PtyExecutionResult` (Lines 60-101).
*   **Deviation**: Internal domain objects for orchestrating PTY session capture data and interactive flow parameters mapped natively inside source files.

---

## 6. Architectural and Security Recommendations

1.  **Consolidate Wire Models to Protocol Buffers**: Convert internal wire-boundary schemas (especially `CapturedSession`, `OAuthToken`, and standard `ChatMessage` structs) to `.proto` definitions. Use workspace standard `prost` generators to compilation-bind schemas, ensuring uniform API evolution tracking.
2.  **Mitigate GCloud PATH Hijacking Risk**:
    *   **Citation**: `crates/op-llm/src/gcloud_adc.rs:67`
    *   **Vulnerability**: Spawning processes using naked command binaries (like `Command::new("gcloud")`) searches the active `$PATH` environment dynamically. If deployed on a compromised system or shared executor, this invites PATH hijacking exploits.
    *   **Remediation**: Resolve the absolute path of executable binaries through secure lookups, or enforce configuring full path definitions at runtime.
3.  **Secure Project Hardcoding**:
    *   **Citation**: `crates/op-llm/src/gcloud_adc.rs:30`
    *   **Remediation**: Remove the hardcoded backup identifier `"geminidev-479406"` entirely. Force initialization to fail with a clear configuration error if active project IDs cannot be resolved, preventing silent structural redirection to unintended cloud sandboxes.

---
## ⚠ Citation Warnings
- `crates/op-llm/src/headless_oauth.rs:354`: file has 335 lines
