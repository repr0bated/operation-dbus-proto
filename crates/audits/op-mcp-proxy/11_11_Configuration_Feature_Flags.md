# Configuration & Quality Audit Report: `op-mcp-proxy`

## 1. Environment Variables Audit

Below is a complete, audited list of all environment variable reads using `std::env::var` across the `op-mcp-proxy` crate.

### 1.1 Complete List of Environment Variable Reads

| File Path | Line Number | Environment Variable | Usage Description |
| :--- | :--- | :--- | :--- |
| `crates/op-mcp-proxy/src/session.rs` | 106 | `WG_PUBKEY` | Overrides the local WireGuard public key identity check. |
| `crates/op-mcp-proxy/src/gcloud_auth.rs` | 22 | `OP_ENABLE_ADC_FALLBACK` | Controls whether Application Default Credentials (ADC) fallback is enabled. |
| `crates/op-mcp-proxy/src/gcloud_auth.rs` | 67 | `MCP_PROXY_TOKEN_FILE` | Explicit file path override for a cached token. |
| `crates/op-mcp-proxy/src/gcloud_auth.rs` | 95 | `GCLOUD_TOKEN` | Direct token value override for testing/auth bypass. |
| `crates/op-mcp-proxy/src/gcloud_auth.rs` | 310 | `MCP_PROXY_VSCODE_AUTH_DIR` | Explicit directory path override to locate VSCode/Cloud Code credential files. |
| `crates/op-mcp-proxy/src/direct_llm.rs` | 110 | `MCP_PROXY_PREFER_VSCODE_AUTH` | Inside `env_flag` wrapper; controls preference of VSCode cache vs Gemini fallback. |
| `crates/op-mcp-proxy/src/direct_llm.rs` | 122 | `MCP_PROXY_DISABLE_GEMINI_OAUTH` | Disables fallback to Gemini CLI-specific OAuth flows. |
| `crates/op-mcp-proxy/src/direct_llm.rs` | 123 | `OP_MCP_PROXY_DISABLE_GEMINI_OAUTH` | Alternative flag to disable fallback to Gemini CLI OAuth. |
| `crates/op-mcp-proxy/src/direct_llm.rs` | 173 | `MCP_PROXY_GENERATE_MAX_ATTEMPTS` | Configures the maximum number of code generation attempts. |
| `crates/op-mcp-proxy/src/direct_llm.rs` | 283 | *Variable* | Generic helper `env_flag(name, default)` that reads whatever name is passed. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 41 | `MCP_PROXY_QUOTA_PROJECT` | Hard override for Google Cloud billing/quota project. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 44 | `GOOGLE_CLOUD_QUOTA_PROJECT` | Standard Google Cloud environment variable for quota billing. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 45 | `QUOTA_PROJECT` | Legacy Google Cloud environment variable for quota billing. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 54 | `MCP_PROXY_GCLOUD_PROJECT` | Hard override for target Google Cloud project identifier. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 57 | `GCLOUD_PROJECT` | Standard gcloud CLI target project environment variable. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 58 | `GOOGLE_CLOUD_PROJECT` | Common target project identifier used by Google SDKs. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 59 | `GOOGLE_CLOUD_PROJECT_ID` | Alternative project ID environment variable. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 75 | `MCP_PROXY_USER_AGENT` | Override for the user agent string sent to Google APIs. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 76 | `USER_AGENT` | Standard client user agent override. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 78 | `MCP_PROXY_X_GOOG_API_CLIENT` | Override for the `x-goog-api-client` telemetry header. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 79 | `X_GOOG_API_CLIENT` | Standard `x-goog-api-client` header override. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 81 | `MCP_PROXY_ORIGIN` | Override for the HTTP `Origin` header. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 83 | `MCP_PROXY_REFERER` | Override for the HTTP `Referer` header. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 85 | `MCP_PROXY_X_CLIENT_DATA` | Telemetry client payload override representing IDE features. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 89 | `MCP_PROXY_SEND_X_GOOG_USER_PROJECT`| Controls insertion of the billing project header. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 141 | `MODEL_ID` | Overrides the Gemini LLM model identifier. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 310 | `CODE_ASSIST_ENDPOINT` | Custom base endpoint URL for the Code Assist API. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 314 | `MCP_PROXY_USE_DAILY_ENDPOINT` | Toggles standard vs daily Code Assist endpoints if not overridden. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 333 | `MCP_PROXY_EXTENSION_ROUTING` | Controls whether target projects are resolved via the VSCode bootstrapping flow. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 519 | `GEMINI_OAUTH_CLIENT_ID` | OAuth Client ID for authenticating token refresh calls. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 520 | `GEMINI_OAUTH_CLIENT_SECRET` | OAuth Client Secret for authenticating token refresh calls. |
| `crates/op-mcp-proxy/src/cloudaicompanion.rs`| 524 | *Variable* | Generic helper `env_flag(name, default)` that reads whatever name is passed. |
| `crates/op-mcp-proxy/src/main.rs` | 44 | `XRAY_SOCKS_ADDR` | Specifies the location of the SOCKS5 proxy to route requests. |
| `crates/op-mcp-proxy/src/main.rs` | 49 | `DIRECT_MODE` | Toggles whether LLM commands are processed locally instead of over DBus. |
| `crates/op-mcp-proxy/src/main.rs` | 58, 89 | `HTTP_SERVER_ADDR` | Bind address for the HTTP server. |
| `crates/op-mcp-proxy/src/main.rs` | 59, 88 | `HTTP_ONLY` | Sets execution to ONLY run the HTTP server and skip standard stdin loops. |
| `crates/op-mcp-proxy/src/main.rs` | 74 | `OP_DBUS_ADDR` | Overrides the target gRPC connection string for the `op-dbus` service. |
| `crates/op-mcp-proxy/src/http_server.rs` | 477 | `VERTEX_PROJECT` | Configures the target Google Cloud project for Vertex AI. |
| `crates/op-mcp-proxy/src/http_server.rs` | 478 | `VERTEX_REGION` | Configures the target Google Cloud region for Vertex AI prediction. |
| `crates/op-mcp-proxy/src/http_server.rs` | 495 | `VERTEX_RATE_LIMIT_RPM` | Sets rate limiting capacity (requests per minute) for the Vertex AI router. |

---

### 1.2 Flagged Environment Variables (No Defaults / Unhandled Fallbacks)

1. **`HTTP_SERVER_ADDR` (Silent Fallback Block in `main.rs:88-95`)**
   - **Risk/Impact**: When `HTTP_ONLY` is enabled but the user fails to set `HTTP_SERVER_ADDR`, the application enters an unnotified sleep block on line 92 (`tokio::signal::ctrl_c().await?;`) and immediately exits on termination without ever launching the API server, throwing an error, or logging why the proxy failed to start.
   - **Resolution**: Enforce a strict validation check. If `HTTP_ONLY` is enabled, verify that `HTTP_SERVER_ADDR` is provided, and return an explicit `anyhow::bail!` error if it is missing.

2. **`VERTEX_PROJECT` (No Default leading to Runtime HTTP 503 Errors in `http_server.rs:477`)**
   - **Risk/Impact**: If the HTTP server starts without `VERTEX_PROJECT` defined, it silently defaults `vertex` to `None`. No boot-time error is raised. If `DIRECT_MODE` is also disabled, incoming client chat completion requests immediately fail at runtime with a hardcoded `503 Service Unavailable` error (`no LLM backend configured` on line 215).
   - **Resolution**: Require explicit configuration on startup. Validate that at least one backend (`VERTEX_PROJECT` or `DIRECT_MODE`) is properly configured before binding the listener.

---

## 2. Cargo Features & Dependency Additivity

### 2.1 Workspace Features (`Cargo.toml`)
The workspace includes custom control plane features inside the `op-dbus` package (`Cargo.toml` lines 102-104):
* **`default = ["grpc"]`**
* **`grpc = []`**

### 2.2 Additive Analysis
By default, Cargo features are **fully additive**. 
* Because `op-mcp-proxy` is part of a larger workspace, any member package that transitively depends on other workspace libraries will inherit their enabled features.
* The default feature sets (such as `grpc` inside the main control plane) are automatically compiled into the artifact unless explicitly omitted using the `--no-default-features` flag at compile-time or by declaring dependencies with `default-features = false`.

---

## 3. Hardcoded Paths, Ports, and Network Addresses

The following system paths, command executions, and default network IP addresses are hardcoded in the codebase:

### 3.1 Network Endpoint Defaults
* **`http://10.200.0.2:50051` (`crates/op-mcp-proxy/src/main.rs:75`)**
  - Hardcoded default connection address for the downstream `op-dbus` gRPC control plane service.

### 3.2 Linux System Paths & Sled Shared Memory
* **`/dev/shm/plugin_schema.dat` (`crates/op-mcp-proxy/src/sled.rs:21`)**
  - Absolute system path to the shared memory identity sled. This limits execution to platforms supporting virtual memory-backed filesystems under `/dev/shm`.

### 3.3 Commands and WireGuard Interface Literals
* **Command name `"wg"` (`crates/op-mcp-proxy/src/session.rs:110, 129`)**
  - The binary invocation assumes the `wg` executable is pre-installed and available in the system execution `PATH`.
* **Interface name `"wg0"` (`crates/op-mcp-proxy/src/session.rs:111, 131`)**
  - The WireGuard interface name is hardcoded to `"wg0"`. If the system utilizes an alternative interface name (e.g., `wg1`, `wg_vpn`), identity checks and allowed-IP parsing will fail.

### 3.4 Hardcoded Local App Config Paths
* **`"mcp-proxy"` & `"sessions.db"` (`crates/op-mcp-proxy/src/session.rs:96-97`)**
  - SQLite session databases are created in `~/.local/share/mcp-proxy/sessions.db` (via `dirs::data_dir()`).
* **`".antigravity-server"` (`crates/op-mcp-proxy/src/gcloud_auth.rs:76`)**
  - Local home-directory folder candidate searched for cached Google Cloud credentials.
* **`".cache/google-vscode-extension/auth"` (`crates/op-mcp-proxy/src/gcloud_auth.rs:313`, `crates/op-mcp-proxy/src/cloudaicompanion.rs:493`)**
  - Directory path used to extract local authentication tokens generated by the VSCode IDE extension.
* **`"credentials.json"` & `"application_default_credentials.json"` (`crates/op-mcp-proxy/src/gcloud_auth.rs:317-318`)**
  - Hardcoded filenames searched inside cached extension paths.
* **`".gemini/oauth_creds.json"` (`crates/op-mcp-proxy/src/cloudaicompanion.rs:477`)**
  - Storage location for cached Gemini CLI OAuth authorization tokens.

---

## 4. Schema-as-Code / Data Contracts Audit

This codebase bypasses formalized, versioned schemas in several key integration boundaries, leading to data serialization risks:

### 4.1 Memory Offset-Based Data Struct Mirror (`crates/op-mcp-proxy/src/sled.rs:17-43`)
* **Finding**: The `SledSnapshot` module uses a byte-offset layout mapped directly from a shared memory buffer to parse binary structures:
  ```rust
  let wg_pubkey     = &bytes[0..32];
  let mutation_index = u64::from_le_bytes(bytes[32..40].try_into().ok()?);
  let is_valid       = bytes[40] != 0;
  let footprint      = &bytes[48..80];
  let nextdns_profile = fixed_str(&bytes[192..208]);
  let subid           = fixed_str(&bytes[96..160]);
  let control_source  = fixed_str(&bytes[160..192]);
  ```
* **Schema-as-Code Violation**: This bypasses any formal serialization protocol. The code comments note that this *"Mirrors the #[repr(C)] layout from op-identity::schema_bridge — must be kept in sync if the sled struct changes."* 
* **Risk**: If the layout in `op-identity` changes (e.g., fields are reordered or padding is modified) and this mirror is not manually updated in lockstep, `op-mcp-proxy` will parse corrupt data fields, resulting in silent logic failures or authorization mismatches.

### 4.2 Ad-Hoc REST Data Contracts (`crates/op-mcp-proxy/src/http_server.rs:56-105`)
* **Finding**: The server constructs OpenAI compatibility endpoints using ad-hoc `serde` structs:
  ```rust
  pub struct ChatCompletionRequest { ... }
  pub struct ChatMessage { ... }
  struct ChatCompletionResponse { ... }
  ```
* **Schema-as-Code Violation**: Instead of utilizing versioned schemas or Protobuf definition files, public HTTP data contracts are defined as ad-hoc Rust-native structs. 
* **Risk**: Modifications to request payloads are not validated against an external API schema contract, potentially leading to breaking changes with client tools during updates.

### 4.3 Unstructured JSON Payload Generation (`crates/op-mcp-proxy/src/cloudaicompanion.rs:179-199`)
* **Finding**: Payloads sent to external API endpoints are constructed using unstructured `serde_json::json!` macros:
  ```rust
  let body = serde_json::json!({
      "model": model,
      "project": request_project,
      "user_prompt_id": uuid::Uuid::new_v4().to_string(),
      "request": {
          "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
          ...
  ```
* **Schema-as-Code Violation**: High-level system integration requests are coded as ad-hoc strings and runtime-generated dictionaries rather than schema-compliant structures. This increases the risk of API breakage if Google modifies the downstream API signature.