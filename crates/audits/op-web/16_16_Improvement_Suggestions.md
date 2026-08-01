# Production Security and Quality Audit Report

## 1. Security & Quality Audit Findings

### [CRITICAL] Missing IP-Security and AccessZone Enforcement in Security Middleware
* **File:Line**: `crates/op-web/src/middleware/security.rs:136`
* **Description**: The `ip_security_middleware` identifies the IP security zone and inserts an `AccessZone` extension into the request, but it never enforces authorization boundaries or returns an error. Instead, it blindly calls `next.run(request).await`. Furthermore, none of the API handlers (such as `execute_tool_handler` or `chat_handler`) check for or extract the `AccessZone` extension. Since the server binds to `0.0.0.0` (all interfaces) by default in `main.rs`, any unauthenticated attacker on the network can access the API, execute arbitrary system tools (including shell execution and arbitrary file writes), and completely bypass the intended security zone controls.
* **Exploitability**: Directly exploitable via unauthenticated POST requests to `/api/tool` or `/api/chat` from any network location.

### [CRITICAL] Arbitrary File Write via Path Traversal in Chat Transcript Saving
* **File:Line**: `crates/op-web/src/handlers/chat.rs:881`
* **Description**: In the `save_transcript_to_file` function, the `filename` parameter supplied by the client in `save_transcript_handler` is directly formatted into the file path (`/tmp/{filename}`) without any sanitization. An attacker can supply a path traversal sequence like `../../etc/cron.d/exploit_cron` or `../../home/user/.bashrc` to write user-controlled transcript text to arbitrary locations on the system filesystem, leading to local file overwrites and remote code execution (RCE).
* **Exploitability**: Directly exploitable by an authenticated or unauthenticated user (due to missing middleware enforcement) calling the `/api/chat/transcript` endpoint.

### [HIGH] Authentication Denial of Service via OAuth CSRF Map Purging
* **File:Line**: `crates/op-web/src/handlers/privacy.rs:770`
* **Description**: The `google_auth` endpoint attempts a primitive cleanup of CSRF tokens: when `tokens.len() > 1000`, it clears the entire map (`tokens.clear()`). This immediately invalidates the active login states of all legitimate concurrent users currently going through the Google OAuth flow. An attacker can easily spam the `/api/privacy/google/auth` endpoint 1001 times, wiping out the map and systematically denying authentication access to legitimate users.
* **Exploitability**: Directly exploitable to cause a persistent Denial of Service (DoS) on authentication.

### [HIGH] Plaintext Storage of WireGuard Private Keys in JSON Store
* **File:Line**: `crates/op-web/src/handlers/privacy.rs:563`
* **Description**: In the `signup` handler, a plaintext WireGuard private key generated via `generate_keypair()` is directly saved into the `wg_private_key_encrypted` field of `PrivacyUser` without any encryption. This struct is subsequently written to `/var/lib/op-dbus/privacy-users.json` in plaintext JSON, exposing sensitive private keys to any user or process with read permissions on the state directory.
* **Exploitability**: Exploitable if an attacker achieves local read privileges or if the database backup is compromised.

### [MEDIUM] Memory Out-of-Bounds Read via Unsafe simd_json on Unpadded Strings
* **File:Line**: `crates/op-web/src/users.rs:114` (and `crates/op-web/src/groups_admin.rs:43`)
* **Description**: `simd_json::from_str` is invoked inside `unsafe` blocks on string buffers loaded directly via `tokio::fs::read_to_string` and `std::fs::read_to_string`. `simd_json` relies on trailing padding buffers (typically 32 or 64 bytes) to safely perform vectorized SIMD reads. Parsing standard Rust strings without explicit padding can result in out-of-bounds memory reads and segmentation faults.
* **Exploitability**: High risk of server instability or crash if configuration files are modified or contain unexpected payloads.

---

## 2. Schema-As-Code Violations

### [VIOLATION] Ad-Hoc JSON Payloads in Tool Group Domain API
* **File:Line**: `crates/op-web/src/groups_admin.rs:136`
* **Description**: The data contracts for domain groups (`domains`), available presets, and user profiles are constructed as ad-hoc JSON values using the `simd_json::json!` macro and serialized/deserialized dynamically. These payloads are not defined as versioned, compile-time verified schemas (such as Protocol Buffers or structured OpenAPI definitions), making integration with front-end components fragile.

### [VIOLATION] Ad-Hoc Data Contracts for Model Context Protocol Payloads
* **File:Line**: `crates/op-web/src/mcp.rs:60`
* **Description**: JSON-RPC requests and responses (`McpRequest`, `McpResponse`, and `McpError`) are implemented as local, hand-rolled Rust structs. They lack versioning and schema-driven definition, diverging from standard protocol engineering where such payloads are auto-generated from structured specifications.

### [VIOLATION] Ad-Hoc Serialization of Cognitive Agent Configuration
* **File:Line**: `crates/op-web/src/mcp_agents.rs:100`
* **Description**: The `AgentSelectionConfig` used to persist agent mapping to `/var/lib/op-dbus/cognitive-mcp-agents.json` is a basic, ad-hoc JSON struct. It lacks versioning metadata, schemas, or security validation, which violates the schema-as-code discipline for persistent control-plane state.

### [VIOLATION] Ad-Hoc Payload Structs for Chat and Tool Execution Endpoints
* **File:Line**: `crates/op-web/src/handlers/chat.rs:15` (and `crates/op-web/src/handlers/tools.rs:105`)
* **Description**: Structs like `ChatRequest`, `ChatResponse`, `DirectToolRequest`, and `DirectToolResponse` are implemented as ad-hoc data contracts. They do not conform to shared, versioned protobuf schemas, leading to potential drift between front-end expectations and backend system execution.

---

## 3. Proactive Improvement Suggestions

### 1. Enforce IP-Security Zone Authorization in Route Handlers
* **Suggestion**: Extract the `AccessZone` extension inside a custom Axum extractor or a dedicated middleware layer, and reject requests with `StatusCode::FORBIDDEN` if the zone is `Public` and the requested tool requires a higher privilege level (e.g., `Restricted`).
* **Rationale**: Currently, `ip_security_middleware` only tags requests but does not block them, leaving all administrative endpoints fully exposed.
* **Example**: `crates/op-web/src/routes/mod.rs:305`

### 2. Sanitize and Restrict Chat Transcript Output Paths
* **Suggestion**: Strip any path-traversal sequences (`..`, `/`) from the `filename` parameter and enforce that the final output file must reside strictly inside a secure target directory.
* **Rationale**: The lack of sanitization allows users to write files to arbitrary paths on the system, which can be exploited for remote code execution.
* **Example**: `crates/op-web/src/handlers/chat.rs:881`

### 3. Replace Global CSRF Clearing with Key-Value TTL Eviction
* **Suggestion**: Use a cache with individual TTL eviction (such as `lru::LruCache` or a timed map) for CSRF state verification, rather than wiping the entire map when the length exceeds 1000.
* **Rationale**: Wiping the entire map exposes the application to a trivial Denial of Service (DoS) vulnerability.
* **Example**: `crates/op-web/src/handlers/privacy.rs:770`

### 4. Implement Typestate Pattern for PrivacyUser Lifecycle
* **Suggestion**: Split `PrivacyUser` into distinct compile-time types (e.g., `UnverifiedUser`, `VerifiedUser`, and `ProvisionedUser`) to prevent the misuse of fields that are only conditionally populated.
* **Rationale**: Storing mixed optional fields in a single struct leads to unmaintainable runtime checks and potential logic errors.
* **Example**: `crates/op-web/src/users.rs:15`

### 5. Migrate User Storage from Ad-Hoc JSON Files to SQLite State Store
* **Suggestion**: Move the `UserStore` database logic out of plaintext JSON files on disk and use the `SqliteStore` (`state_store`) with proper transactions.
* **Rationale**: Overwriting JSON files on disk is highly inefficient, prone to race conditions, and susceptible to database corruption upon sudden power loss.
* **Example**: `crates/op-web/src/users.rs:52`

### 6. Replace Vectorized String Slicing with Padded simd_json Parsing
* **Suggestion**: Use `simd_json::to_owned_value` on a padded vector of bytes (`Vec<u8>`) instead of calling `simd_json::from_str` on unpadded file contents inside unsafe blocks.
* **Rationale**: Vectorized SIMD parsing can read past the end of standard string allocations, potentially causing undefined behavior or crashes.
* **Example**: `crates/op-web/src/users.rs:114`

### 7. Avoid String Duplication and Allocation in Embedded Static File Serving
* **Suggestion**: Use `bytes::Bytes` or `Arc<str>` to handle SPA assets in `serve_embedded_ui` to avoid copying and cloning file buffers on every HTTP request.
* **Rationale**: Static resources are frequently fetched; copying raw byte vectors repeatedly degrades performance and increases garbage collection pressure.
* **Example**: `crates/op-web/src/embedded_ui.rs:32`

### 8. Instrument Critical Tool Execution Paths with Structured Tracing
* **Suggestion**: Add `#[tracing::instrument(skip(state, arguments), fields(tool_name = %tool_name))]` to `execute_tool_internal` to enable structured observability.
* **Rationale**: The current unstructured logging makes it difficult to track and audit specific tool executions when multiple parallel sessions are active.
* **Example**: `crates/op-web/src/handlers/tools.rs:125`

---
## ⚠ Citation Warnings
- `crates/op-web/src/handlers/chat.rs:881`: file has 453 lines
- `crates/op-web/src/routes/mod.rs:305`: file has 248 lines
- `crates/op-web/src/handlers/chat.rs:881`: file has 453 lines
