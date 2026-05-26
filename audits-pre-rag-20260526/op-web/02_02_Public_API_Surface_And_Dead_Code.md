### 1. Public API Surface & Glob Re-exports

#### Public Items Overview
A total of **434** public items (`pub` structs, fields, functions, enums, modules, re-exports, type aliases, and constants) were identified across the provided crate files. 

#### Top 10 Most Impactful Public Items
The most critical public entry points and interfaces that define the crate's surface are:
1. **`UnifiedOrchestrator`** (`crates/op-web/src/orchestrator/mod.rs:21`): Core orchestrator managing Multi-turn LLM agent execution and tool-dispatch loops.
2. **`AppState`** (`crates/op-web/src/state.rs:74`): Monolithic shared state structure containing state stores, credentials, D-Bus gRPC clients, and OAuth configurations.
3. **`create_router`** (`crates/op-web/src/routes/mod.rs:36`): Main router initialization assembling the HTTP endpoints, static files, SSE channels, and WebSockets.
4. **`UserStore`** (`crates/op-web/src/users.rs:59`): User account database mapping IP assignments, encrypted keys, API keys, and magic link states.
5. **`PrivacyUser`** (`crates/op-web/src/users.rs:17`): User representation holding critical parameters such as active WireGuard IP allocations, container names, and API keys.
6. **`ip_security_middleware`** (`crates/op-web/src/middleware/security.rs:90`): Global security layer categorizing requests into Access Zones (`TrustedMesh`, `LocalHost`, `PrivateNetwork`, `Public`).
7. **`check_bypass_api_key`** (`crates/op-web/src/middleware/security.rs:19`): Hook verifying if a request possesses master bypass API tokens.
8. **`mcp_discovery_handler`** (`crates/op-web/src/mcp_discovery.rs:14`): Well-known discovery endpoint (`/.well-known/mcp.json`) for automatic MCP server provisioning.
9. **`publish_user_privacy_route`** (`crates/op-web/src/privacy_routes.rs:39`): Key orchestration method publishing desired routing state directly to the D-Bus StateManager.
10. **`derive_route_id`** (`crates/op-web/src/privacy_routes.rs:69`): Cryptographic routine generating deterministic route identifiers using HKDF-SHA256.

#### Glob Re-exports (pub use *)
*   **`crates/op-web/src/orchestrator/mod.rs:6`**: `pub use types::*;` 
    *   *Risk:* Glob re-exports pollute the public API namespace and make it easy to accidentally leak internal-only types when changes are made to the sub-module.

#### Public Fields on Structs that Should Be Private
Exposing fields on configuration or credential-bearing structs as public allows caller mutation, bypassing validation rules, and increasing exposure:
*   **`EmailConfig`** (`crates/op-web/src/email.rs:13`): `smtp_pass`, `smtp_user`, `smtp_host`, `smtp_port` are public.
*   **`UserApiCredentials`** (`crates/op-web/src/state.rs:51` / `crates/op-web/src/users.rs:49`): `token`, `gemini_api_key`, `anthropic_api_key`, `openai_api_key` are public.
*   **`AppState`** (`crates/op-web/src/state.rs:74`): Struct exposes raw, mutable shared lock structures (`csrf_tokens`, `conversations`) and gRPC clients publicly.
*   **`PendingAuth`** (`crates/op-web/src/handlers/auth_bridge.rs:24`): Exposes raw credential tokens, authorization state, and active URLs publicly.

---

### 2. Dead Code & Unused Items

The following items are defined but are never referenced, suppress compiler warnings artificially, or represent entirely empty/stub components:

| Item | Type | file:line | Recommendation |
| :--- | :--- | :--- | :--- |
| `WebServiceRouter` | struct | `crates/op-web/src/router.rs:13` | Remove (deprecated router module) |
| `WebServiceRouter::new` | fn | `crates/op-web/src/router.rs:17` | Remove |
| `create_websocket_router` | fn | `crates/op-web/src/router.rs:49` | Remove |
| `WebServerConfig` | struct | `crates/op-web/src/server.rs:20` | Remove (main.rs uses raw axum loop) |
| `RateLimitConfig` | struct | `crates/op-web/src/server.rs:33` | Remove |
| `WebServer` | struct | `crates/op-web/src/server.rs:60` | Remove |
| `auth_bridge_routes` | fn | `crates/op-web/src/handlers/auth_bridge.rs:42` | Remove (fails compilation/AppState integration) |
| `mail_queue_handler` | fn | `crates/op-web/src/handlers/mail.rs:46` | Implement database backend or remove |
| `SseEventBroadcaster::broadcast` | fn | `crates/op-web/src/sse.rs:27` | Marked `#[allow(dead_code)]`; implement event emitting |
| `routes::chat` | mod | `crates/op-web/src/routes/mod.rs:30` | Marked `#[allow(dead_code)]`; duplicate of `handlers::chat` |
| `routes::llm` | mod | `crates/op-web/src/routes/mod.rs:32` | Marked `#[allow(dead_code)]`; duplicate of `handlers::llm` |

#### Completely Unused/Dead Files
*   **`crates/op-web/src/router.rs`**: Completely bypassed; `main.rs` initializes the application router via `routes::create_router(state)`.
*   **`crates/op-web/src/server.rs`**: The entire server abstraction, configuration, and rate-limiting setup is unused. `main.rs` manually constructs its `TcpListener` and serves `axum` directly.
*   **`crates/op-web/src/handlers/auth_bridge.rs`**: Not registered in `handlers/mod.rs` or `routes/mod.rs`. It does not compile because it expects a non-existent `.auth_bridge` field on `AppState`.

---

### 3. Production Security & Quality Vulnerability Audit

#### Critical Findings (Exploitable)

##### 1. Arbitrary File Write / Path Traversal in Transcript Saver
*   **Location:** `crates/op-web/src/handlers/chat.rs:434`
*   **Mechanism:** The `save_transcript_handler` extracts the `filename` parameter directly from untrusted user JSON input. It attempts to enforce storage in `/tmp` using:
    ```rust
    let filepath = format!("/tmp/{}", filename);
    match tokio::fs::write(&filepath, &transcript).await { ... }
    ```
*   **Exploitability:** Because no sanitization or path verification is performed on `filename`, an attacker can send path traversal sequences like `../../etc/cron.d/malicious` or `../../root/.ssh/authorized_keys`. This allows arbitrary file writes across the host filesystem with the permissions of the `op-web` process, leading to immediate Privilege Escalation or Remote Code Execution (RCE).

##### 2. Remote Memory Safety Violations / RCE via Unpadded Unsafe `simd_json` Parsing
*   **Location:** `crates/op-web/src/websocket.rs:88` & `crates/op-web/src/handlers/websocket.rs:65`
*   **Mechanism:** The WebSocket string deserialization uses `unsafe { simd_json::from_str(&mut raw) }` directly on string payloads cloned from incoming client frames:
    ```rust
    let mut raw = text.clone();
    let ws_msg: Result<WsMessage, _> = unsafe { simd_json::from_str(&mut raw) };
    ```
*   **Exploitability:** `simd_json`'s `unsafe` deserialization requires the input buffer to be mutated in-place and critically, to possess `simd_json::SIMDJSON_PADDING` bytes (typically 64 bytes) of allocated padding at the end of the buffer. A standard cloned `String` has no such padding guarantee. An attacker sending a malicious payload right at the edge of the allocated string page will trigger out-of-bounds reads/writes, causing undefined behavior, memory corruption, and potential RCE.

##### 3. complete Authentication Bypass / Access Control Failure in Groups Admin API
*   **Location:** `crates/op-web/src/groups_admin.rs:113` & `crates/op-web/src/routes/mod.rs:242`
*   **Mechanism:** The router registers endpoints like `POST /api/profiles/:name` and `POST /api/trusted-networks`. While they are configured to run after the `ip_security_middleware`, this middleware merely appends the client's `AccessZone` to the request's extensions:
    ```rust
    request.extensions_mut().insert(zone);
    next.run(request).await
    ```
    However, the actual handlers in `groups_admin.rs` (such as `save_profile` and `add_trusted_network`) never extract or inspect this extension.
*   **Exploitability:** Any remote attacker can send a `POST` request to `/groups-admin/api/profiles/default` or `/groups-admin/api/trusted-networks` to overwrite the entire system tool configuration, add trusted networks, or disable active tools on disk (`/var/lib/op-dbus/tool-groups.json`), bypassing all intended IP network zone security barriers.

##### 4. Hardcoded Master API Keys / Backdoors
*   **Location:** `crates/op-web/src/middleware/security.rs:16`
*   **Mechanism:** The security middleware hardcodes administrative access keys:
    ```rust
    const BYPASS_API_KEYS: &[&str] = &[
        "4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f", // Primary MCP access key
        "test-key-huggingface-2024",            // Hugging Face test key
    ];
    ```
*   **Exploitability:** If any request includes these headers, the IP zone check is completely bypassed, immediately elevating the request to the `AccessZone::TrustedMesh` level. This allows any remote entity with knowledge of these static keys to execute privileged commands.

##### 5. Authentication Denial of Service (DoS) via CSRF Map Clear
*   **Location:** `crates/op-web/src/handlers/privacy.rs:509`
*   **Mechanism:** When initiating a Google OAuth flow, the server generates a CSRF token and puts it into a global map. To keep memory bounded, it implements the following cleanup heuristic:
    ```rust
    if tokens.len() > 1000 {
        tokens.clear();
    }
    ```
*   **Exploitability:** An attacker can trigger this heuristic by sending 1001 rapid unauthenticated requests to `/api/privacy/google/auth`. This instantly deletes the active CSRF tokens of all other legitimate users currently in the OAuth flow, preventing them from logging in.

---

#### High & Medium Findings (Non-Exploitable / Quality)

##### 1. Memory Safety Violations on State Manager D-Bus Interop
*   **Location:** `crates/op-web/src/state_manager_client.rs:32` & `crates/op-web/src/groups_admin.rs:49`
*   **Mechanism:** The server uses `unsafe { simd_json::from_str(&mut state_json) }` to parse JSON returned from the D-Bus system bus via `zbus`. 
*   **Impact:** Normal strings allocated by `zbus` during message parsing do not have the mandatory allocation padding required by `simd_json`'s unsafe parser, leading to undefined memory behavior.

##### 2. Transparent Testing Fallback Leaking Tokens to Tracing Logs
*   **Location:** `crates/op-web/src/email.rs:77-84`
*   **Mechanism:** If `smtp_user` or `smtp_pass` is missing or unconfigured, the magic link token is printed directly to `stdout` and logged at the `INFO` level:
    ```rust
    info!("🔗 MAGIC LINK (no SMTP configured):");
    info!("   Token: {}", token);
    ```
*   **Impact:** A misconfiguration or transient startup error in the environment variables can cause the system to leak one-time login tokens directly to standard logs.

##### 3. Fatal Type & Arity Mismatches in MCP Smart Router
*   **Location:** `crates/op-web/src/mcp_smart_router.rs:81-83` & `crates/op-web/src/mcp_smart_router.rs:91-93`
*   **Mechanism:** The `smart_mcp_handler` attempts to invoke `mcp_compact::mcp_compact_message_handler` and `mcp_agents_message_handler_stateless` using `Value::Null` directly inside the handler parameters:
    ```rust
    crate::mcp_compact::mcp_compact_message_handler(
        axum::extract::Json(serde_json::Value::Null)
    ).await.into_response()
    ```
*   **Impact:** This call fails to supply the required `Extension(state)` parameter (which `mcp_compact_message_handler` requires as its first parameter). It also passes a `serde_json::Value::Null` wrapper into handlers expecting `simd_json::OwnedValue`-derived structs. This results in direct compilation failures, rendering this router completely unusable in production.

##### 4. Hardcoded Developer Paths
*   **Location:** `crates/op-web/src/system_prompt_loader.rs:18`
*   **Mechanism:** The loader hardcodes absolute local machine paths to look for the system prompt:
    ```rust
    "/home/jeremy/git/gemini-op-dbus/LLM-SYSTEM-PROMPT-COMPLETE.txt",
    "/home/jeremy/op-dbus-v2/LLM-SYSTEM-PROMPT-COMPLETE.txt",
    ```
*   **Impact:** This leaks development environment directory structures and system user metadata in the production binary.