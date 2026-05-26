# Production Security & Quality Audit: `op-web`

## Architecture & Module Map

### Overview
`op-web` is a unified control plane web server designed for `op-dbus-v2`. It acts as the central interface for the control plane, exposing REST APIs, real-time WebSockets, Server-Sent Events (SSE), and Model Context Protocol (MCP) handlers to interact with Linux system resources (including Open vSwitch, systemd/dinit services, and kernel routing), cognitive AI agents, and a privacy-preserving WireGuard network mesh.

### Module Tree
*   `email`: lettre SMTP magic-link delivery ([crates/op-web/src/email.rs:1](crates/op-web/src/email.rs:1))
*   `embedded_ui`: SPA asset compiler and server ([crates/op-web/src/embedded_ui.rs:1](crates/op-web/src/embedded_ui.rs:1))
*   `groups_admin`: Tool group administration portal ([crates/op-web/src/groups_admin.rs:1](crates/op-web/src/groups_admin.rs:1))
*   `handlers`: HTTP endpoints mapping ([crates/op-web/src/handlers/mod.rs:1](crates/op-web/src/handlers/mod.rs:1))
    *   `agents`, `chat`, `dashboard`, `health`, `llm`, `logs`, `mail`, `mcp`, `openclaw`, `privacy`, `status`, `tools`, `users`, `vpn`, `websocket`
*   `mcp`: Standard MCP protocol server ([crates/op-web/src/mcp.rs:1](crates/op-web/src/mcp.rs:1))
*   `mcp_agents`: Multi-agent prewarmed runtime MCP server ([crates/op-web/src/mcp_agents.rs:1](crates/op-web/src/mcp_agents.rs:1))
*   `mcp_compact`: Exposes metadata-driven compression tools for LLMs ([crates/op-web/src/mcp_compact.rs:1](crates/op-web/src/mcp_compact.rs:1))
*   `mcp_discovery`: Automated standard `.well-known` configuration handler ([crates/op-web/src/mcp_discovery.rs:1](crates/op-web/src/mcp_discovery.rs:1))
*   `mcp_smart_router`: Intelligent dynamic protocol capabilities detector ([crates/op-web/src/mcp_smart_router.rs:1](crates/op-web/src/mcp_smart_router.rs:1))
*   `middleware`: Security layers ([crates/op-web/src/middleware/mod.rs:1](crates/op-web/src/middleware/mod.rs:1))
    *   `security`
*   `orchestrator`: AI multi-turn tool executive ([crates/op-web/src/orchestrator/mod.rs:1](crates/op-web/src/orchestrator/mod.rs:1))
    *   `anti_hallucination`, `execution`, `formatting`, `parsing`, `process`, `tools`, `types`
*   `privacy_container`: Provisioning Incus containers via StateManager ([crates/op-web/src/privacy_container.rs:1](crates/op-web/src/privacy_container.rs:1))
*   `privacy_network`: Host network topology config ([crates/op-web/src/privacy_network.rs:1](crates/op-web/src/privacy_network.rs:1))
*   `privacy_openflow`: OpenFlow rule updates bridge ([crates/op-web/src/privacy_openflow.rs:1](crates/op-web/src/privacy_openflow.rs:1))
*   `privacy_routes`: Deterministic routing table compiler ([crates/op-web/src/privacy_routes.rs:1](crates/op-web/src/privacy_routes.rs:1))
*   `routes`: Router definition and composition ([crates/op-web/src/routes/mod.rs:1](crates/op-web/src/routes/mod.rs:1))
*   `sse`: System status streaming router ([crates/op-web/src/sse.rs:1](crates/op-web/src/sse.rs:1))
*   `state`: Central application shared state context ([crates/op-web/src/state.rs:1](crates/op-web/src/state.rs:1))
*   `state_manager_client`: D-Bus StateManager client IPC ([crates/op-web/src/state_manager_client.rs:1](crates/op-web/src/state_manager_client.rs:1))
*   `system_prompt_loader`: Prompt policies filesystem watcher ([crates/op-web/src/system_prompt_loader.rs:1](crates/op-web/src/system_prompt_loader.rs:1))
*   `users`: User profile database ([crates/op-web/src/users.rs:1](crates/op-web/src/users.rs:1))
*   `websocket`: Chat transport gateway ([crates/op-web/src/websocket.rs:1](crates/op-web/src/websocket.rs:1))
*   `wireguard`: Configurer and QR-code generator ([crates/op-web/src/wireguard.rs:1](crates/op-web/src/wireguard.rs:1))

### Entry Points
*   **Daemon Entry**: `crates/op-web/src/main.rs` (compiles to binary `op-web-server`)
*   **gRPC Control Entry**: `crates/op-web/src/bin/op-dbus.rs` (compiles to binary `op-dbus`)
*   **Library Interface**: `crates/op-web/src/lib.rs`

***

## Security Findings

### Critical

#### 1. Arbitrary File Write & Path Traversal in Chat Transcript Saving
*   **File**: `crates/op-web/src/handlers/chat.rs`
*   **Line**: [252-258](crates/op-web/src/handlers/chat.rs#L252-L258), [399-402](crates/op-web/src/handlers/chat.rs#L399-L402)
*   **Mechanism**:
    The `save_transcript_handler` parses a user-provided JSON object and extracts a `filename` directly as an unvalidated string:
    ```rust
    let filename = params
        .get("filename")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format!("chat-transcript-{}.txt", chrono::Utc::now().timestamp()));
    ```
    This string is passed straight to `save_transcript_to_file`, which constructs the destination filepath using a simple format string:
    ```rust
    let filepath = format!("/tmp/{}", filename);
    match tokio::fs::write(&filepath, &transcript).await { ... }
    ```
*   **Exploitability**:
    An attacker can make a `POST /api/chat/transcript` request with a payload such as:
    ```json
    {
      "messages": [{"role": "user", "content": "* * * * * root reboot"}],
      "filename": "../etc/cron.d/exploit"
    }
    ```
    Because no path sanitization is performed, `/tmp/../etc/cron.d/exploit` resolves to `/etc/cron.d/exploit`. The server writes the formatted chat transcript containing the malicious cron directive directly to that location, resulting in immediate remote code execution (RCE) with the process's current privileges.

#### 2. Infinite File Descriptor & Task Leak leading to Denial of Service (DoS) in Logs SSE Handler
*   **File**: `crates/op-web/src/handlers/logs.rs`
*   **Line**: [136-150](crates/op-web/src/handlers/logs.rs#L136-L150), [204-206](crates/op-web/src/handlers/logs.rs#L204-L206)
*   **Mechanism**:
    When a client initiates a connection to `GET /api/logs/stream`, the server invokes `logs_stream_handler` and spawns a new background file-tailing task:
    ```rust
    pub async fn logs_stream_handler(
        Extension(state): Extension<Arc<AppState>>,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
        let rx = state.sse_broadcaster.subscribe();
        ...
        let broadcaster = state.sse_broadcaster.clone();
        tokio::spawn(async move {
            use linemux::MuxedLines;
            let mut lines = MuxedLines::new().expect("Failed to create MuxedLines");
            ...
            while let Ok(Some(line)) = lines.next_line().await {
                ...
                broadcaster.broadcast("log", &payload);
            }
        });
    ```
    This background task is fully detached. It contains no mechanism to check if the specific connection stream has terminated or if the subscriber has disconnected. It indefinitely polls `lines.next_line().await` and broadcasts events globally. 
*   **Exploitability**:
    An attacker can repeatedly connect and disconnect to `/api/logs/stream`. Every single connection spawns a perpetual, detached `tokio` thread/task and registers multiple system inotify instances on `/var/log/op-web.log` and `/var/log/op-dbus.log`. 
    Within a few hundred connections, the system runs out of file descriptors or kernel inotify watch limits, causing a full system-wide crash. Furthermore, since *each* spawned background task broadcasts globally, log entry events are amplified exponentially ($O(N^2)$) to active subscribers.

#### 3. Administrative Routes Bypass & Lack of Access Zone Enforcement
*   **File**: `crates/op-web/src/routes/mod.rs`, `crates/op-web/src/routes/admin.rs`, `crates/op-web/src/groups_admin.rs`, `crates/op-web/src/middleware/security.rs`
*   **Line**: [105-115](crates/op-web/src/routes/mod.rs#L105-L115), [43-52](crates/op-web/src/routes/admin.rs#L43-L52), [113-138](crates/op-web/src/middleware/security.rs#L113-L138)
*   **Mechanism**:
    The middleware `ip_security_middleware` resolves the client IP to a cryptographic security `AccessZone` and places it inside the request extensions:
    ```rust
    request.extensions_mut().insert(zone);
    next.run(request).await
    ```
    However, the middleware *never* halts unauthorized requests or returns an error. It passes execution to the router.
    The administrative endpoints registered under `/admin/*` (such as `/admin/prompt/custom` for editing LLM prompts) and `/groups-admin/*` (which controls which D-Bus system administration tools are enabled) *never* extract this `AccessZone` extension nor perform any verification checks.
*   **Exploitability**:
    An attacker on the public internet can call `POST /admin/prompt/custom` to rewrite the system prompt, overriding critical safety boundaries (e.g., instructing the LLM to execute destructive commands). They can also call `POST /groups-admin/api/profiles/default` to modify the system tool configuration, enabling privileged tool subsets.

---

### High

#### 4. Cryptographically Unbound OAuth CSRF State Checking
*   **File**: `crates/op-web/src/handlers/privacy.rs`
*   **Line**: [629](crates/op-web/src/handlers/privacy.rs#L629), [641-654](crates/op-web/src/handlers/privacy.rs#L641-L654)
*   **Mechanism**:
    During Google OAuth initiation, the CSRF token is inserted into a global, shared token collection:
    ```rust
    let mut tokens = state.csrf_tokens.write().await;
    tokens.insert(csrf_token.secret().clone(), csrf_token.secret().clone());
    ```
    In `google_callback`, the incoming state is validated simply by looking it up and removing it from this global list:
    ```rust
    let mut tokens = state.csrf_tokens.write().await;
    if tokens.remove(&query.state).is_none() { ... error ... }
    ```
*   **Impact**:
    Because the token lookup is global and has no cryptographic binding to the browser session of the client that requested it (e.g., via a hashed session cookie), an attacker can easily initiate an OAuth flow to populate `state.csrf_tokens` with a valid token, intercept it, and force a victim's browser to complete the callback with the attacker's authorization code and that token. This successfully links the victim's local routing configuration to the attacker's Google Identity.

#### 5. Unlocked Filesystem Configuration Writes leading to Corruption
*   **File**: `crates/op-web/src/groups_admin.rs`
*   **Line**: [114-120](crates/op-web/src/groups_admin.rs#L114-L120)
*   **Mechanism**:
    The database files are written directly using `tokio::fs::write` on the target path:
    ```rust
    if let Ok(json) = simd_json::to_string_pretty(&*self.profiles.read().await) {
        if let Err(e) = tokio::fs::write(GROUPS_CONFIG_PATH, json).await {
            tracing::error!("Failed to save groups config: {}", e);
        }
    }
    ```
*   **Impact**:
    If two concurrent requests modify tool groups (or save profiles) at the same time, the writes will conflict, leading to corrupted, partial, or empty JSON documents on disk. On restart, the JSON parser will fail, destroying configuration data.

---

### Medium

#### 6. Missing Quota Enforcement for Privacy Network Traffic
*   **File**: `crates/op-web/src/users.rs`, `crates/op-web/src/privacy_container.rs`
*   **Line**: [25-29](crates/op-web/src/users.rs#L25-L29), [118-142](crates/op-web/src/privacy_container.rs#L118-L142)
*   **Mechanism**:
    The database schema defines `privacy_quota_bytes` (defaults to 1 GiB) and `privacy_quota_used_bytes`. However, a systematic search of the codebase reveals that `privacy_quota_used_bytes` is never updated, read, or checked inside any network routing or container initialization logic.
*   **Impact**:
    Users can consume infinite network capacity through the routing mesh, causing denial of service on host bandwidth.

---

### Quality & Operational Oversights

#### 7. Console Disclosure of Secret Magic Link Tokens
*   **File**: `crates/op-web/src/email.rs`
*   **Line**: [77-85](crates/op-web/src/email.rs#L77-L85)
*   **Mechanism**:
    ```rust
    println!("\n=== MAGIC LINK FOR TESTING ===");
    println!("Email: {}", to_email);
    println!("Link: {}", magic_url);
    println!("==============================\n");
    ```
    If SMTP is not configured, the raw login link (which allows instant verification and access to VPN configurations) is dumped directly to `stdout`.
*   **Impact**:
    These tokens can be intercepted by unprivileged local users or log aggregation tooling.

#### 8. Plaintext Control Plane Servicing (No native TLS)
*   **File**: `crates/op-web/src/main.rs`
*   **Line**: [46-51](crates/op-web/src/main.rs#L46-L51)
*   **Mechanism**:
    The HTTP engine binds using a standard, unencrypted TCP listener:
    ```rust
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;
    ```
*   **Impact**:
    Extremely sensitive admin parameters, system tokens, LLM API keys, and configurations are transmitted over the local mesh in cleartext.

***

## Schema-as-Code Compliance Review

As a strict engineering discipline, all data contracts, configurations, state profiles, and message structures must be defined as versioned schemas (e.g., Protocol Buffers or structured OSCAL schemas) to guarantee deterministic system state and prevent parsing regressions. 

The audit identified multiple occurrences where critical system state is expressed as ad-hoc Rust structs and raw JSON string blobs instead of formalized schemas:

### 1. Ad-Hoc Tool Group Config Profiles
*   **Location**: `crates/op-web/src/groups_admin.rs`
*   **Struct**: `EnabledGroups` (Lines [36-40](crates/op-web/src/groups_admin.rs#L36-L40))
*   **Violation**: Handled as an ad-hoc JSON structure saved to `/var/lib/op-dbus/tool-groups.json`. Any divergence in field names during software upgrades will trigger un-versioned parsing failures on startup.

### 2. Cognitive Agent State Definition
*   **Location**: `crates/op-web/src/mcp_agents.rs`
*   **Struct**: `AgentSelectionConfig` (Lines [80-84](crates/op-web/src/mcp_agents.rs#L80-L84))
*   **Violation**: Bypasses official control plane state definitions by writing raw JSON serialization structures directly to disk at `/var/lib/op-dbus/cognitive-mcp-agents.json`.

### 3. Incus State Mapping
*   **Location**: `crates/op-web/src/privacy_container.rs`
*   **Struct**: `IncusInstance` (Lines [33-47](crates/op-web/src/privacy_container.rs#L33-L47))
*   **Violation**: Container lifecycle parameters are mapped to unstructured key-value text maps:
    ```rust
    config: Option<HashMap<String, String>>,
    devices: Option<HashMap<String, HashMap<String, String>>>,
    ```
    This completely prevents compiling compile-time validation rules for memory limits, network routing hooks, or profile assignments.

### 4. OpenFlow Integration Definitions
*   **Location**: `crates/op-web/src/privacy_openflow.rs`
*   **Structs**: `FlowEntry` (Lines [29-39](crates/op-web/src/privacy_openflow.rs#L29-L39)), `FlowAction` (Lines [41-51](crates/op-web/src/privacy_openflow.rs#L41-L51))
*   **Violation**: Cryptographic OpenFlow forwarding constraints are configured using untyped maps:
    ```rust
    match_fields: HashMap<String, String>,
    ```
    This risks producing malformed flow strings that fail at runtime within the Open vSwitch virtual switch.

### 5. Deterministic WireGuard Privacy Routes
*   **Location**: `crates/op-web/src/privacy_routes.rs`
*   **Struct**: `PrivacyRoute` (Lines [18-32](crates/op-web/src/privacy_routes.rs#L18-L32))
*   **Violation**: Exposes a direct structural state contract mapped out of band from `org.opdbus.StateManager` using simple JSON serializations.

### 6. VPN User Profiles Store
*   **Location**: `crates/op-web/src/users.rs`
*   **Struct**: `PrivacyUser` (Lines [17-42](crates/op-web/src/users.rs#L17-L42))
*   **Violation**: Handled as an ad-hoc JSON database structure saved to `/var/lib/op-dbus/privacy-users.json`. This database holds highly sensitive credentials and Wireguard keys, yet contains no format versioning, migrators, or schema evolution mechanisms.