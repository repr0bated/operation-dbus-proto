# Production Security and Quality Audit: Configuration, Features, & Hardcoded Assets

---

## 1. Environment Variable Reads (`std::env::var`)

The following is a comprehensive inventory of all `std::env::var` calls inside the reviewed files:

| File | Line | Environment Variable | Default Value / Fallback |
| :--- | :--- | :--- | :--- |
| `crates/op-web/src/email.rs` | 30 | `SMTP_HOST` | `"localhost"` |
| `crates/op-web/src/email.rs` | 31 | `SMTP_PORT` | `587` |
| `crates/op-web/src/email.rs` | 35 | `SMTP_USER` | `""` (Empty string) |
| `crates/op-web/src/email.rs` | 36 | `SMTP_PASS` | `""` (Empty string) |
| `crates/op-web/src/email.rs` | 37 | `SMTP_FROM_EMAIL` | `"noreply@example.com"` |
| `crates/op-web/src/email.rs` | 39 | `SMTP_FROM_NAME` | `"Privacy Router"` |
| `crates/op-web/src/email.rs` | 41 | `BASE_URL` | `"http://localhost:8080"` |
| `crates/op-web/src/main.rs` | 31 | `PORT` | `"8080"` |
| `crates/op-web/src/mcp_agents.rs` | 551 | `OP_COGNITIVE_MCP_AGENT_CONFIG` | `/var/lib/op-dbus/cognitive-mcp-agents.json` |
| `crates/op-web/src/privacy_container.rs` | 48 | `PRIVACY_CONTAINER_IMAGE` | `"images:alpine/3.19"` |
| `crates/op-web/src/privacy_container.rs` | 50 | `PRIVACY_CONTAINER_PREFIX` | `"privacy-user-"` |
| `crates/op-web/src/privacy_container.rs` | 52 | `PRIVACY_CONTAINER_DEVICE` | `"privacy0"` |
| `crates/op-web/src/privacy_container.rs` | 54 | `PRIVACY_CONTAINER_STORAGE_POOL` | `None` (Safely handled via `Option`) |
| `crates/op-web/src/privacy_container.rs` | 58 | `PRIVACY_CONTAINER_ATTACH_BRIDGED_NIC` | `true` |
| `crates/op-web/src/privacy_container.rs` | 115 | `PRIVACY_CONTAINER_BRIDGE` | `"ovsbr0"` |
| `crates/op-web/src/privacy_routes.rs` | 55 | `PRIVACY_ROUTE_SHARED_SECRET` | **None** (See section 2) |
| `crates/op-web/src/privacy_routes.rs` | 92 | `PRIVACY_ROUTE_INGRESS_PORT` | `"ovsbr0-sock"` |
| `crates/op-web/src/privacy_routes.rs` | 94 | `PRIVACY_ROUTE_NEXT_HOP` | `"priv_wg"` |
| `crates/op-web/src/privacy_network.rs` | 43 | `PRIVACY_BRIDGE_NAME` | `"ovsbr0"` |
| `crates/op-web/src/privacy_network.rs` | 45 | `PRIVACY_WGCF_TUNNEL` | `"wgcf"` |
| `crates/op-web/src/privacy_network.rs` | 47 | `PRIVACY_PORTS` | `["priv_xray", "priv_warp", "priv_wg", "ovsbr0-mgmt", "ovsbr0-sock"]` |
| `crates/op-web/src/privacy_network.rs` | 55 | `PRIVACY_MGMT_CIDR` | `"10.200.0.1/24"` |
| `crates/op-web/src/privacy_network.rs` | 57 | `PRIVACY_OPENFLOW_CONTROLLER` | `"10.200.0.1:6653"` |
| `crates/op-web/src/privacy_network.rs` | 59 | `XRAY_INGRESS_IP` | `"10.200.0.1"` |
| `crates/op-web/src/privacy_network.rs` | 61 | `PRIVACY_DATAPATH_TYPE` | `"system"` |
| `crates/op-web/src/privacy_network.rs` | 63 | `PRIVACY_FAIL_MODE` | `"standalone"` |
| `crates/op-web/src/state.rs` | 47 | `GOOGLE_OAUTH_CLIENT_ID` | **None** (Safely handled via `.ok()?`) |
| `crates/op-web/src/state.rs` | 48 | `GOOGLE_OAUTH_CLIENT_SECRET` | **None** (Safely handled via `.ok()?`) |
| `crates/op-web/src/state.rs` | 49 | `GOOGLE_OAUTH_REDIRECT_URL` | `"http://localhost:8080/api/privacy/google/callback"` |
| `crates/op-web/src/state.rs` | 253 | `OP_DBUS_GRPC_ADDR` | `"http://10.200.0.2:50051"` |
| `crates/op-web/src/state.rs` | 417 | `OP_WEB_TOOL_SOURCE` | `None` (Safely handled via `Option`) |
| `crates/op-web/src/state.rs` | 420 | `OP_WEB_PULL_TOOLS_FROM_OP_DBUS` | `false` |
| `crates/op-web/src/state.rs` | 426 | `OP_WEB_REMOTE_TOOL_URL` | `None` (Safely handled) |
| `crates/op-web/src/state.rs` | 433 | `OP_DBUS_WEB_HOST` | `"127.0.0.1"` |
| `crates/op-web/src/state.rs` | 434 | `OP_DBUS_WEB_PORT` | `"8081"` |
| `crates/op-web/src/wireguard.rs` | 22 | `WG_INTERFACE` | `"wg0"` |
| `crates/op-web/src/wireguard.rs` | 24 | `WG_SERVER_PUBKEY` | `""` (Fallback to system discovery via command) |
| `crates/op-web/src/wireguard.rs` | 25 | `WG_SERVER_PUBLIC_KEY` | `""` (Fallback to system discovery via command) |
| `crates/op-web/src/wireguard.rs` | 26 | `WIREGUARD_PUBLIC_KEY` | `""` (Fallback to system discovery via command) |
| `crates/op-web/src/wireguard.rs` | 31 | `WG_SERVER_ENDPOINT` | `"148.113.204.83:51820"` (Test endpoint) |
| `crates/op-web/src/wireguard.rs` | 32 | `VPN_ENDPOINT` | `"148.113.204.83:51820"` (Test endpoint) |
| `crates/op-web/src/wireguard.rs` | 35 | `WG_ALLOWED_IPS` | `"0.0.0.0/0, ::/0"` |
| `crates/op-web/src/wireguard.rs` | 36 | `VPN_ALLOWED_IPS` | `"0.0.0.0/0, ::/0"` |
| `crates/op-web/src/wireguard.rs` | 39 | `WG_DNS` | `"10.200.0.1"` |
| `crates/op-web/src/wireguard.rs` | 40 | `VPN_DNS` | `"10.200.0.1"` |
| `crates/op-web/src/bin/op-dbus.rs` | 25 | `OP_DBUS_GRPC_LISTEN` | `"10.200.0.2:50051"` |
| `crates/op-web/src/handlers/openclaw.rs` | 19 | `OPENCLAW_BASE_URL` | `"http://127.0.0.1:8090"` |
| `crates/op-web/src/handlers/openclaw.rs` | 25 | `OPENCLAW_DEFAULT_MODEL` | `"openclaw:main"` |
| `crates/op-web/src/handlers/vpn.rs` | 120 | `VPN_ENDPOINT` | `"148.113.204.83:51820"` |
| `crates/op-web/src/routes/admin.rs` | 238 | `OP_SELF_REPO_PATH` | `None` (Safely handled via `is_ok()`/`ok()`) |
| `crates/op-web/src/routes/mod.rs` | 188 | `OP_WEB_STATIC_DIR` | `None` (Safely handled via `ok()`) |
| `crates/op-web/src/routes/mod.rs` | 204 | `OP_WEB_STATIC_DIR` | `"static"` (Fallback) |

---

## 2. Environment Variables with No Default & No Error Handling

All environment variables read from the environment are handled safely either via:
1. `unwrap_or_else` or `unwrap_or` fallbacks.
2. Proper propagation of errors using `?` or context wrappers.
3. Functional map matches that fail cleanly (e.g., returning `None` instead of panicking).

The most sensitive required variable is:
*   **`PRIVACY_ROUTE_SHARED_SECRET`** (`crates/op-web/src/privacy_routes.rs:55`)
    *   **Default:** None.
    *   **Handling:** Implements strict error handling using `anyhow::Context` (`.context(...)?`) and returns a cleanly propagated `Result::Err` rather than panicking. It fails gracefully during execution.

There are **no unhandled `unwrap()` calls** on `std::env::var` operations across the codebase.

---

## 3. Cargo Features & Additive Behavior

Based on `Cargo.toml` for the target package:

### Packages & Features
*   **Package Name:** `op-dbus` (within workspace)
*   **Default Features:** `["grpc"]`
*   **Defined Features:**
    *   `grpc = []`

### Additive Behavior Explanation
In Rust/Cargo, features are strictly **additive**. 
*   When a dependency is compiled, Cargo merges all enabled features across the dependency graph.
*   Enabling a feature anywhere in the graph activates it globally for that build step.
*   To bypass default features, consumers must specify `default-features = false` in their `Cargo.toml` dependency declarations.

---

## 4. Hardcoded Paths, Ports, and Addresses

### Hardcoded File Paths
*   **`/var/lib/op-dbus/tool-groups.json`** — `crates/op-web/src/groups_admin.rs:42` (Persisted tool groups configurations).
*   **`/var/lib/op-dbus/cognitive-mcp-agents.json`** — `crates/op-web/src/mcp_agents.rs:36` (Persisted cognitive agent routing status).
*   **`/var/lib/op-dbus/privacy-users.json`** — `crates/op-web/src/state.rs:163` & `171` (User store configuration database file).
*   **`/var/lib/op-dbus/state.db`** — `crates/op-web/src/state.rs:218` (Sqlite state storage engine database path).
*   **`/etc/op-dbus/llm-model`** — `crates/op-web/src/state.rs:341` & `crates/op-web/src/handlers/llm.rs:177` (Persisted default model configuration file).
*   **`/etc/op-dbus/llm-provider`** — `crates/op-web/src/state.rs:342` & `crates/op-web/src/handlers/llm.rs:178` (Persisted LLM provider selection file).
*   **`/etc/op-dbus/custom-prompt.txt`** — `crates/op-web/src/routes/admin.rs:241` (Custom system prompt override target).
*   **`/var/log/op-web.log`** — `crates/op-web/src/handlers/logs.rs:32` & `149` (Primary application log file location).
*   **`/var/log/op-dbus.log`** — `crates/op-web/src/handlers/logs.rs:33` & `150` (Control plane log target).
*   **`/tmp/op-web.log`** — `crates/op-web/src/handlers/logs.rs:34` & `151` (Fallback staging directory log location).
*   **`/tmp/...`** (via `format!("/tmp/{}", filename)`) — `crates/op-web/src/handlers/chat.rs:434` (Vulnerable path construction - see Critical Findings).

### Hardcoded Ports, IP Addresses, & Local Hostnames
*   **`587`** — `crates/op-web/src/email.rs:34` (SMTP Submission port fallback).
*   **`http://localhost:8080`** — `crates/op-web/src/email.rs:42` (Fallback Base URL configuration).
*   **`8080`** — `crates/op-web/src/main.rs:33` (Server default listen port).
*   **`[0, 0, 0, 0]`** — `crates/op-web/src/main.rs:34` (Global wildcard bind interface).
*   **`127.0.0.1`** — `crates/op-web/src/server.rs:67` (Default localhost bind address).
*   **`3000`** — `crates/op-web/src/server.rs:67` (Alternative local test server port).
*   **`148.113.204.83:51820`** — `crates/op-web/src/wireguard.rs:34` & `crates/op-web/src/handlers/vpn.rs:121` (Hardcoded Wireguard public test server gateway address).
*   **`10.200.0.1`** — `crates/op-web/src/wireguard.rs:41`, `crates/op-web/src/privacy_network.rs:26`, `crates/op-web/src/privacy_network.rs:27`, `crates/op-web/src/privacy_network.rs:60` (Chokepoint gateway DNS and Xray proxy routing IP).
*   **`10.200.0.2:50051`** — `crates/op-web/src/bin/op-dbus.rs:24`, `crates/op-web/src/state.rs:232` (Hardcoded default listener/client endpoint for gRPC bridge interactions).
*   **`10.100.0.2/32`** — `crates/op-web/src/privacy_container.rs:179` & `crates/op-web/src/wireguard.rs:114` (Hardcoded static assignment addresses for test users).
*   **`http://127.0.0.1:8090`** — `crates/op-web/src/handlers/openclaw.rs:14` & `16` (Hardcoded local loopback gateway configuration for OpenClaw components).
*   **`1.1.1.1`** — `crates/op-web/src/handlers/vpn.rs:125` (Default fallback resolver IP).

---

## 5. Schema-As-Code Violations

The codebase consistently declares critical data structures and REST API contracts as **ad-hoc Rust `struct` items** and untyped **JSON `serde_json::Value` (aliased as `simd_json::OwnedValue`) payloads** instead of compile-time compiled, version-controlled Protocol Buffer schemas or declarative OSCAL documents.

### Ad-hoc API Request & Response Contracts
*   **`EnabledGroups`** (`crates/op-web/src/groups_admin.rs:28`) — Custom JSON persistence contract for profile groups.
*   **`SaveProfileRequest`** (`crates/op-web/src/groups_admin.rs:141`) — Untyped serialization array structure.
*   **`AddNetworkRequest`** (`crates/op-web/src/groups_admin.rs:188`) — Simple raw string field structure.
*   **`McpRequest` / `McpResponse`** (`crates/op-web/src/mcp.rs:46-54`) — Re-implementing protocol contracts with dynamic untyped fields (`params: Option<Value>`, `result: Option<Value>`).
*   **`JsonRpcRequest` / `JsonRpcResponse`** (`crates/op-web/src/mcp_agents.rs:38-48`) — Redundant ad-hoc redefinition of standard JSON-RPC structures using dynamic values.
*   **`AgentSelectionConfig`** (`crates/op-web/src/mcp_agents.rs:77`) — Internal custom orchestration serialization contract.
*   **`ManagedAgentInfo`** (`crates/op-web/src/mcp_agents.rs:83`) — dynamic unstructured arrays of strings.
*   **`CognitiveRuntimeSnapshot`** (`crates/op-web/src/mcp_agents.rs:94`) — Custom unstructured statistics report object.
*   **`JsonRpcRequest` / `JsonRpcResponse`** (`crates/op-web/src/mcp_compact.rs:32-42`) — Third ad-hoc replication of the JSON-RPC interface contract.
*   **`PendingAuth`** (`crates/op-web/src/handlers/auth_bridge.rs:22`) — dynamic authentication bridging data model.
*   **`WebhookPayload`** (`crates/op-web/src/handlers/auth_bridge.rs:90`) — Dynamic webhook schema processing struct.
*   **`ChatRequest` / `ChatResponse`** (`crates/op-web/src/handlers/chat.rs:23-36`) — Redefined REST-level interaction schemas.
*   **`DashboardMetrics`** (`crates/op-web/src/handlers/dashboard.rs:14`) — Dynamic administrative system tracking payload.
*   **`LogEntry`** (`crates/op-web/src/handlers/logs.rs:21`) — Simple text representation of metrics logs.
*   **`McpServer`** (`crates/op-web/src/handlers/mcp.rs:16`) — Ad-hoc administrative representation.
*   **`Agent`** (`crates/op-web/src/handlers/mcp.rs:27`) — Duplicate representation of the dynamic agent schema.
*   **`MemoryQuery`** (`crates/op-web/src/handlers/mcp.rs:45`) — Custom memory payload structure.
*   **`StatusResponse`** (`crates/op-web/src/handlers/status.rs:41`) — Large dynamic nested structure reflecting system diagnostic status.
*   **`DirectToolRequest` / `DirectToolResponse`** (`crates/op-web/src/handlers/tools.rs:58-64`) — Custom internal debugging tool validation structures.
*   **`UserResponse`** (`crates/op-web/src/handlers/users.rs:11`) — Custom ad-hoc output profile for users.
*   **`VpnStatus` / `VpnConnection` / `VpnConfig`** (`crates/op-web/src/handlers/vpn.rs:14-41`) — Custom local struct specifications.
*   **`MailStatus` / `MailQueueItem`** (`crates/op-web/src/handlers/mail.rs:13-20`) — Dynamic mail statistics tracking structures.
*   **`OpenClawStatusResponse` / `OpenClawConfigResponse`** (`crates/op-web/src/handlers/openclaw.rs:38-51`) — Hardcoded integration mapping schemas.
*   **`SignupRequest` / `VerifyResponse` / `StatusResponse`** (`crates/op-web/src/handlers/privacy.rs:25-50`) — Custom user onboarding logic contracts.

---

## 6. Security Audit Findings

### Critical Finding 1: Static API Backdoors Bypassing IP-Based Access Controls
*   **File:** `crates/op-web/src/middleware/security.rs`
*   **Lines:** 16–20
*   **Code Reference:**
    ```rust
    /// API keys that bypass IP restrictions and grant TrustedMesh access
    const BYPASS_API_KEYS: &[&str] = &[
        "4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f", // Primary MCP access key
        "test-key-huggingface-2024",            // Hugging Face test key
    ];
    ```
*   **Impact:** **CRITICAL**. These hardcoded API keys are compiled into the binary. Any request supplying one of these keys via `x-api-key`, `Authorization: Bearer`, or `x-op-mcp-token` headers bypasses all IP-based security checks and is mapped directly to `AccessZone::TrustedMesh`. This grants full unauthorized access to elevated, restricted system configurations and tools to anyone who reverse-engineers the binary or extracts these keys.

---

### Critical Finding 2: Arbitrary File Write & Potential Remote Code Execution (RCE) via Path Traversal
*   **File:** `crates/op-web/src/handlers/chat.rs`
*   **Lines:** 379–384, 434–436
*   **Code Reference:**
    ```rust
    // line 379:
    pub async fn save_transcript_handler( ... ) {
        let filename = params.get("filename").and_then(|v| v.as_str()).map(str::to_string)
            .unwrap_or_else(|| format!("chat-transcript-{}.txt", ...));
        ...
        return save_transcript_to_file(&history, filename.as_str(), None).await;
    }
    
    // line 434:
    async fn save_transcript_to_file( ... ) {
        let filepath = format!("/tmp/{}", filename);
        match tokio::fs::write(&filepath, &transcript).await { ... }
    }
    ```
*   **Impact:** **CRITICAL**. The `filename` parameter is received directly from the HTTP client payload without any sanitization or directory traversal prevention. An attacker can supply a value such as `../../etc/cron.d/malicious_job` in the request body. This resolves to `/tmp/../../etc/cron.d/malicious_job` (which is `/etc/cron.d/malicious_job`). When written, this allows an attacker to write arbitrary files onto the filesystem. Since the service likely runs with elevated privileges to execute control plane operations, this leads to immediate Remote Code Execution (RCE).

---

### High Finding 3: Insecure WireGuard Private Key Retrieval via API Key Backdoor
*   **File:** `crates/op-web/src/handlers/privacy.rs`
*   **Lines:** 481–513
*   **Code Reference:**
    ```rust
    pub async fn get_config(
        headers: axum::http::HeaderMap,
        axum::extract::Extension(state): axum::extract::Extension<std::sync::Arc<crate::AppState>>,
        axum::extract::Path(user_id): axum::extract::Path<String>,
    ) -> (axum::http::StatusCode, axum::Json<VerifyResponse>) {
        let auth_token = crate::middleware::security::extract_auth_token(&headers);
        let mut is_authorized = false;

        if let Some(token) = auth_token {
            if crate::middleware::security::check_bypass_api_key(&headers).is_some() {
                is_authorized = true;
            ...
            let config = generate_client_config(
                &user.wg_private_key_encrypted,
                &user.assigned_ip,
                &state.server_config,
            );
    ```
*   **Impact:** **HIGH**. If a request includes the static bypass API key, authorization is granted to retrieve any user's profile configuration using their `user_id`. The returned payload includes the user's plain-text WireGuard private key (stored unencrypted inside the `wg_private_key_encrypted` field as noted in the developer comment on line 140 of `privacy.rs`). This allows any party possessing the bypass keys to compromise the VPN tunnel credentials of any user.

---

### Low Finding 4: Unsanitized File Writes in Switch Model Handler
*   **File:** `crates/op-web/src/handlers/llm.rs`
*   **Lines:** 137–146, 180–190
*   **Code Reference:**
    ```rust
    pub async fn switch_model_handler(
        Extension(state): Extension<Arc<AppState>>,
        Json(request): Json<SwitchModelRequest>,
    ) -> Json<Value> {
        match state.chat_manager.switch_model(request.model.clone()).await {
            Ok(_) => {
                let _ = persist_model(&request.model).await;
    ...
    async fn persist_model(model: &str) -> Result<(), String> {
        ...
        tokio::fs::write(PERSISTED_MODEL_PATH, format!("{model}\n")).await ...
    }
    ```
*   **Impact:** **LOW**. The `model` string provided in the request payload is written directly to the configuration file `/etc/op-dbus/llm-model` without string validation or character restriction. While the path itself is static (preventing traversal), an attacker could inject excessively large payloads, binary characters, or control blocks into the configuration file, causing parsing errors or resource exhaustion when the system reloads.