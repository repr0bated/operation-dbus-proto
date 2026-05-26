### Workspace & Integration Topology

#### 1. Workspace Dependencies on `op-web`
Based on `Cargo.toml` at the root of the workspace, the following package depends directly on `op-web`:
*   **`op-dbus`** (Root package defined at `Cargo.toml:574`):
    ```toml
    [dependencies]
    ...
    op-web.workspace = true
    ```

---

#### 2. Registered D-Bus Service Names and Object Paths
The `op-web` crate does not register its own D-Bus services. Instead, it acts as a D-Bus client via `zbus`, calling out to the following system-bus state-manager service:
*   **Service Name:** `org.opdbus.v1` (`crates/op-web/src/state_manager_client.rs:13`)
*   **Object Path:** `/org/opdbus/v1/state` (`crates/op-web/src/state_manager_client.rs:14`)
*   **Interface:** `org.opdbus.StateManager` (`crates/op-web/src/state_manager_client.rs:15`)
*   **Methods Invoked:** `QueryState` (`crates/op-web/src/state_manager_client.rs:26`), `ApplyContractMutation` (`crates/op-web/src/state_manager_client.rs:52`)

---

#### 3. Exposed HTTP, WebSocket, and gRPC Endpoints
The unified `op-web` server exposes the following endpoints (via Axum routing):

##### **HTTP REST & Event Endpoints**
*   `GET /api/health` — Service health check (`crates/op-web/src/routes/mod.rs:36`)
*   `GET /api/status` — Comprehensive system status (`crates/op-web/src/routes/mod.rs:37`)
*   `GET /api/dashboard/metrics` — Dashboard metrics (`crates/op-web/src/routes/mod.rs:41`)
*   `GET /api/users` — List users (`crates/op-web/src/routes/mod.rs:44`)
*   `GET /api/users/:id` — Get specific user details (`crates/op-web/src/routes/mod.rs:45`)
*   `GET /api/vpn/status` — WireGuard VPN status (`crates/op-web/src/routes/mod.rs:47`)
*   `GET /api/vpn/connections` — Active WireGuard peers (`crates/op-web/src/routes/mod.rs:48`)
*   `GET /api/vpn/config` — Server VPN credentials (`crates/op-web/src/routes/mod.rs:51`)
*   `GET /api/mail/status` — Maddy mail server status (`crates/op-web/src/routes/mod.rs:53`)
*   `GET /api/mail/queue` — Mail server queue (`crates/op-web/src/routes/mod.rs:54`)
*   `GET /api/mail/accounts` — Mail accounts list (`crates/op-web/src/routes/mod.rs:55`)
*   `GET /api/logs` — Retreive recent local logs (`crates/op-web/src/routes/mod.rs:57`)
*   `GET /api/logs/stream` — SSE stream of real-time logs (`crates/op-web/src/routes/mod.rs:58`)
*   `POST /api/chat` — Direct chat completion (`crates/op-web/src/routes/mod.rs:60`)
*   `POST /api/chat/stream` — SSE-based streaming chat (`crates/op-web/src/routes/mod.rs:61`)
*   `GET /api/chat/sessions` — List active chat sessions (`crates/op-web/src/routes/mod.rs:62`)
*   `POST /api/chat/sessions` — Create a chat session (`crates/op-web/src/routes/mod.rs:63`)
*   `DELETE /api/chat/sessions/:id` — Delete a chat session (`crates/op-web/src/routes/mod.rs:67`)
*   `POST /api/chat/message` — Send chat message (`crates/op-web/src/routes/mod.rs:71`)
*   `GET /api/chat/history/:session_id` — Get chat session history (`crates/op-web/src/routes/mod.rs:72`)
*   `POST /api/chat/transcript` — Write session transcript to file (`crates/op-web/src/routes/mod.rs:76`)
*   `GET /api/chat/system-prompt` — View prompt configuration (`crates/op-web/src/routes/mod.rs:80`)
*   `PUT /api/chat/system-prompt` — Update custom system prompt (`crates/op-web/src/routes/mod.rs:84`)
*   `GET /api/tools` — Get list of registered tools (`crates/op-web/src/routes/mod.rs:88`)
*   `GET /api/tools/:name` — Get specific tool schema (`crates/op-web/src/routes/mod.rs:89`)
*   `POST /api/tool` — Execute tool directly (`crates/op-web/src/routes/mod.rs:90`)
*   `POST /api/tools/:name/execute` — Execute tool by path (`crates/op-web/src/routes/mod.rs:91`)
*   `GET /api/agents` — List spawned cognitive agents (`crates/op-web/src/routes/mod.rs:96`)
*   `POST /api/agents` — Spawn a new cognitive agent (`crates/op-web/src/routes/mod.rs:97`)
*   `GET /api/agents/types` — View types of agents (`crates/op-web/src/routes/mod.rs:98`)
*   `GET /api/agents/:id` — Get agent state (`crates/op-web/src/routes/mod.rs:101`)
*   `DELETE /api/agents/:id` — Terminate a running agent (`crates/op-web/src/routes/mod.rs:102`)
*   `GET /api/llm/status` — Get LLM status (`crates/op-web/src/routes/mod.rs:107`)
*   `GET /api/llm/providers` — List configured LLM providers (`crates/op-web/src/routes/mod.rs:108`)
*   `GET /api/llm/models` — List configured LLM models (`crates/op-web/src/routes/mod.rs:109`)
*   `GET /api/llm/models/:provider` — List provider models (`crates/op-web/src/routes/mod.rs:110`)
*   `POST /api/llm/provider` — Switch LLM provider (`crates/op-web/src/routes/mod.rs:114`)
*   `POST /api/llm/model` — Switch active model (`crates/op-web/src/routes/mod.rs:118`)
*   `GET /api/openclaw/status` — OpenClaw gateway status (`crates/op-web/src/routes/mod.rs:121`)
*   `GET /api/openclaw/config` — Get OpenClaw parameters (`crates/op-web/src/routes/mod.rs:125`)
*   `POST /api/openclaw/chat` — Proxy chat completions to OpenClaw (`crates/op-web/src/routes/mod.rs:129`)
*   `GET /api/openclaw/models` — Proxy models list (`crates/op-web/src/routes/mod.rs:133`)
*   `GET /api/mcp/servers` — View cognitive servers (`crates/op-web/src/routes/mod.rs:138`)
*   `GET /api/mcp/servers/:id` — View specific cognitive server details (`crates/op-web/src/routes/mod.rs:139`)
*   `GET /api/mcp/cognitive/agents` — List available cognitive agents (`crates/op-web/src/routes/mod.rs:140`)
*   `POST /api/mcp/cognitive/agents` — Configure cognitive agents (`crates/op-web/src/routes/mod.rs:144`)
*   `POST /api/mcp/cognitive/memory` — Search or insert cognitive memory (`crates/op-web/src/routes/mod.rs:148`)
*   `DELETE /api/mcp/cognitive/memory/:key` — Purge memory key (`crates/op-web/src/routes/mod.rs:152`)
*   `GET /api/mcp/cognitive/memory/stats` — Memory metrics (`crates/op-web/src/routes/mod.rs:156`)
*   `GET /api/mcp/_config` — Retrieve global MCP settings (`crates/op-web/src/routes/mod.rs:161`)
*   `GET /api/events` — Broad SSE bridge for gRPC state updates (`crates/op-web/src/routes/mod.rs:163`)
*   `POST /api/privacy/signup` — VPN email sign-up (`crates/op-web/src/routes/mod.rs:165`)
*   `GET /api/privacy/verify` — Programmatic WireGuard token validation (`crates/op-web/src/routes/mod.rs:166`)
*   `GET /api/privacy/config/:user_id` — Fetch user WireGuard config (`crates/op-web/src/routes/mod.rs:167`)
*   `GET /api/privacy/status` — Get Wireguard server capacity status (`crates/op-web/src/routes/mod.rs:171`)
*   `POST /api/privacy/credentials` — Persist AI service API keys per user (`crates/op-web/src/routes/mod.rs:172`)
*   `GET /api/privacy/google/auth` — Google OAuth authorization redirect (`crates/op-web/src/routes/mod.rs:177`)
*   `GET /api/privacy/google/callback` — Google OAuth callback exchange (`crates/op-web/src/routes/mod.rs:178`)
*   `GET /privacy/verify` — Redirect target for magic links (`crates/op-web/src/routes/mod.rs:197`)
*   `GET /privacy/access` — User Wireguard configuration UI (`crates/op-web/src/routes/mod.rs:198`)
*   `POST /jsonrpc` — JSON-RPC 2.0 endpoint compatibility alias (`crates/op-web/src/routes/mod.rs:202`)
*   `POST /rpc` — JSON-RPC 2.0 compatibility alias (`crates/op-web/src/routes/mod.rs:203`)
*   `GET /.well-known/mcp.json` — Auto-discovery configuration standard (`crates/op-web/src/routes/mod.rs:208`)

##### **Model Context Protocol (MCP) Endpoints**
*   `GET /mcp/agents` — Streaming SSE initialization for agents (`crates/op-web/src/routes/mod.rs:188`)
*   `POST /mcp/agents/message` — Message handler for agents (`crates/op-web/src/routes/mod.rs:192`)
*   `POST /mcp` — General MCP entry point (`crates/op-web/src/routes/mod.rs:194` / `crates/op-web/src/mcp.rs:104`)
*   `GET /mcp/sse` — Standard MCP over SSE transport tunnel (`crates/op-web/src/mcp.rs:105`)
*   `POST /mcp/message` — Standard MCP over SSE command processor (`crates/op-web/src/mcp.rs:106`)
*   `GET /mcp/compact` — Compact meta-tool SSE transport channel (`crates/op-web/src/mcp.rs:98`)
*   `POST /mcp/compact/message` — Compact meta-tool message endpoint (`crates/op-web/src/mcp.rs:101`)

##### **Tool Groups Admin Endpoints**
*   `GET /groups-admin` — Tool Groups portal home page (`crates/op-web/src/groups_admin.rs:125`)
*   `GET /groups-admin/api/groups` — List registered groups (`crates/op-web/src/groups_admin.rs:126`)
*   `GET /groups-admin/api/presets` — List curated tool presets (`crates/op-web/src/groups_admin.rs:127`)
*   `GET /groups-admin/api/profiles` — List saved profiles (`crates/op-web/src/groups_admin.rs:128`)
*   `GET /groups-admin/api/profiles/:name` — Get specific group profile details (`crates/op-web/src/groups_admin.rs:129`)
*   `POST /groups-admin/api/profiles/:name` — Save group profile details (`crates/op-web/src/groups_admin.rs:129`)
*   `GET /groups-admin/api/access-zone` — Access classification page (`crates/op-web/src/groups_admin.rs:130`)
*   `GET /groups-admin/api/trusted-networks` — List verified networks (`crates/op-web/src/groups_admin.rs:133`)
*   `POST /groups-admin/api/trusted-networks` — Add trusted prefix network (`crates/op-web/src/groups_admin.rs:133`)

##### **Admin Endpoints**
*   `GET /admin/prompt` — View full generated LLM system prompt (`crates/op-web/src/routes/admin.rs:24`)
*   `GET /admin/prompt/custom` — Get custom prompt instructions (`crates/op-web/src/routes/admin.rs:25`)
*   `POST /admin/prompt/custom` — Edit custom prompt instructions (`crates/op-web/src/routes/admin.rs:26`)
*   `POST /admin/prompt/test` — Test prompt updates with mock arguments (`crates/op-web/src/routes/admin.rs:27`)
*   `POST /admin/prompt/reload` — Clear runtime system prompt cache (`crates/op-web/src/routes/admin.rs:28`)
*   `GET /admin/config` — View server state variables (`crates/op-web/src/routes/admin.rs:29`)

##### **WebSockets**
*   `GET /ws` — bidirectional real-time chat protocol gateway (`crates/op-web/src/routes/mod.rs:195`)

##### **gRPC Service (via binary `op-dbus`)**
The `op-web` crate contains a secondary binary at `crates/op-web/src/bin/op-dbus.rs` which starts a gRPC server exposing:
*   **Binding Address:** `10.200.0.2:50051` (controlled via `OP_DBUS_GRPC_LISTEN`) (`crates/op-web/src/bin/op-dbus.rs:34`)

---

#### 4. Cross-Crate Circular Dependency Analysis
*   The parent `op-dbus` package depends directly on `op-web` (`Cargo.toml:574`).
*   The `op-web` crate depends directly on workspace packages `op-core`, `op-chat`, `op-llm`, `op-tools`, `op-agents`, `op-state`, `op-network`, `op-mcp`, `op-mcp-aggregator`, `op-state-store`, `op-identity`, `op-introspection`, and `op-grpc-bridge`.
*   None of these workspace crates list `op-web` as a dependency in their `Cargo.toml`. `op-web` is structured strictly as the consumer/aggregator of control plane utilities. 
*   **Coupling Risk:** There is no circular dependency risk at compilation time. However, deep runtime coupling exists because `op-web` instantiates the unified gRPC client (`RemoteOperationClient`) connecting back to the gRPC service running on `op-dbus` (`crates/op-web/src/state.rs:292`). If `op-dbus` fails or blocks during startup, `op-web` initialization will hang or error out during the `AppState::new()` phase.

---

### Security & Quality Audit Findings

#### [CRITICAL] Finding 1: Direct, Unauthenticated Remote Code Execution on Host Control Plane
*   **File:** `crates/op-web/src/handlers/tools.rs:62-81`
*   **Endpoint:** `POST /api/tool` and `POST /api/tools/:name/execute`
*   **Description:** The HTTP endpoints for executing arbitrary system tools are completely unprotected. The `ip_security_middleware` applied to the router (`crates/op-web/src/routes/mod.rs:219`) parses IP zones and places the metadata into the request extensions, but **never rejects requests** based on unauthorized security zones. The handlers directly execute whatever tool is requested (such as `shell_exec` or file modifiers) as root without checking the parsed zone.
*   **Impact:** Any remote adversary who can reach the HTTP port can execute arbitrary system commands and read/write any file on the host.
*   **Remediation:** Enforce permission checks within the middleware or handlers:
    ```rust
    let zone = request.extensions().get::<AccessZone>().unwrap_or(&AccessZone::Public);
    if !zone.can_access(SecurityLevel::Elevated) {
        return StatusCode::FORBIDDEN.into_response();
    }
    ```

---

#### [CRITICAL] Finding 2: Unauthenticated Leak of WireGuard Client Private Keys
*   **File:** `crates/op-web/src/handlers/privacy.rs:282-385`
*   **Endpoint:** `GET /privacy/access`
*   **Description:** The HTML confirmation page displays full client configurations including plaintext, unencrypted private keys (`user.wg_private_key_encrypted` is stored in plaintext placeholder as noted in `crates/op-web/src/users.rs:225`). Access to this endpoint is guarded only by an unauthenticated UUID query parameter `user_id`. There is no token validation, cryptographic challenge, or session-binding.
*   **Impact:** Anyone who intercepts, guesses, or gains access to a user's UUID can fetch their plaintext WireGuard private key and hijack their VPN profile.
*   **Remediation:** Require a cryptographically signed session token or require the user to provide their Magic Link token to authorize the config download.

---

#### [CRITICAL] Finding 3: Memory Safety Violations (Undefined Behavior) via Unpadded `simd-json` Parsing
*   **File:** `crates/op-web/src/users.rs:84-87`, `crates/op-web/src/groups_admin.rs:40-44`, `crates/op-web/src/websocket.rs:81-83`
*   **Description:** The codebase invokes `simd_json::from_str` within `unsafe` blocks on standard, unpadded string buffers cloned via `content.clone()` or `text.clone()`. The `simd-json` specification strictly mandates that input buffers must be mutable and terminated with at least `simd_json::PADDING` (usually 32 bytes) of writeable padding. Standard Rust allocations do not guarantee this trailing margin.
*   **Impact:** Parsing malformed or specially crafted JSON input from WebSocket messages or config files can trigger OOB SIMD vector reads, causing segmentation faults, memory corruption, or unexpected program flow.
*   **Remediation:** Allocate padded buffers explicitly or use the safe, automatic padding wrappers:
    ```rust
    let mut raw_bytes = content.into_bytes();
    raw_bytes.reserve(simd_json::PADDING); // ensure memory margin
    let data: StoredData = simd_json::from_slice(&mut raw_bytes)?;
    ```

---

#### [HIGH] Finding 4: Hardcoded Authorization Bypass Keys
*   **File:** `crates/op-web/src/middleware/security.rs:14-17`
*   **Description:** The primary API keys that bypass all IP restrictions and grant full system access to the control plane are hardcoded in plain sight in the source code:
    ```rust
    const BYPASS_API_KEYS: &[&str] = &[
        "4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f", // Primary MCP access key
        "test-key-huggingface-2024",            // Hugging Face test key
    ];
    ```
*   **Impact:** If an attacker extracts these keys from the binary or reads the source repository, they gain immediate full administrative access.
*   **Remediation:** Load authorized bypass keys from environment variables or a configuration database at runtime.

---

#### [HIGH] Finding 5: Parallel Tempfile Collision & State Corruption Risk
*   **File:** `crates/op-web/src/users.rs:108-112`
*   **Description:** The user registration engine attempts to persist data atomically using a static temporary path:
    ```rust
    let temp_path = format!("{}.tmp", self.storage_path);
    tokio::fs::write(&temp_path, content).await?;
    tokio::fs::rename(&temp_path, &self.storage_path).await?;
    ```
*   **Impact:** When multiple users register or update credentials concurrently, their concurrent saves write to the exact same `{storage_path}.tmp` file. This race condition leads to partial/mixed file writes, corrupting the JSON structure and wiping the database on subsequent restarts.
*   **Remediation:** Generate randomized temporary file names using a library like `tempfile`:
    ```rust
    let temp_file = tempfile::NamedTempFile::new_in(parent_dir)?;
    ```

---

#### [MEDIUM] Finding 6: Permissive Wildcard CORS Configuration on Sensitive Control Plane
*   **File:** `crates/op-web/src/server.rs:163-166`
*   **Description:** The web server enables Cross-Origin Resource Sharing (CORS) with unrestricted wildcards:
    ```rust
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    ```
*   **Impact:** Malicious web pages visited by an authenticated user or administrator can make cross-origin requests to execute administrative tools or extract system data.
*   **Remediation:** Restrict allowed origins to local network interfaces or trusted domain networks.

---

### Schema-as-Code Violations
The system frequently defines communication data contracts as ad-hoc, raw Rust structures, JSON strings, or raw maps. To follow strict schema-as-code discipline, these schemas must be declared in versioned Protocol Buffers or Open-Source Cybersecurity Assessment Language (OSCAL) schemas.

1.  **JSON-RPC and MCP Structs:**
    *   `McpRequest`, `McpResponse` (`crates/op-web/src/mcp.rs:46-64`)
    *   `JsonRpcRequest`, `JsonRpcResponse` (`crates/op-web/src/mcp_agents.rs:26-44`)
    *   `JsonRpcRequest`, `JsonRpcResponse` (`crates/op-web/src/mcp_compact.rs:34-52`)
2.  **Container & OpenFlow Network Contracts:**
    *   `IncusState`, `IncusInstance` (`crates/op-web/src/privacy_container.rs:31-48`)
    *   `OpenFlowConfig`, `BridgeFlowConfig`, `FlowEntry` (`crates/op-web/src/privacy_openflow.rs:11-37`)
    *   `PrivacyRoutesState`, `PrivacyRoute` (`crates/op-web/src/privacy_routes.rs:14-32`)
3.  **Core State Storage Schema:**
    *   `PrivacyUser`, `UserApiCredentials` (`crates/op-web/src/users.rs:18-50`)
    *   `StoredData` (`crates/op-web/src/users.rs:417-420`)
4.  **Admin and Tool Pickers:**
    *   `SaveProfileRequest`, `AddNetworkRequest` (`crates/op-web/src/groups_admin.rs:201-204`, `255-257`)
    *   `PendingAuth` (`crates/op-web/src/handlers/auth_bridge.rs:23-32`)
5.  **Telemetry, Metrics, and Log Types:**
    *   `DashboardMetrics` (`crates/op-web/src/handlers/dashboard.rs:12-22`)
    *   `LogEntry` (`crates/op-web/src/handlers/logs.rs:23-29`)
    *   `MailStatus`, `MailQueueItem` (`crates/op-web/src/handlers/mail.rs:12-28`)
    *   `VpnStatus`, `Bandwidth`, `VpnConnection` (`crates/op-web/src/handlers/vpn.rs:12-39`)